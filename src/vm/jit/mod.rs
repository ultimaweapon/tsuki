pub use self::allocator::*;
pub use self::future::*;

use self::emitter::Emitter;
use self::funcs::RustFuncs;
use super::{
    OP_CALL, OP_CLOSURE, OP_GETTABUP, OP_LOADK, OP_NEWTABLE, OP_RETURN, OP_VARARG, OP_VARARGPREP,
    luaV_finishget,
};
use crate::ldo::luaD_poscall;
use crate::lfunc::luaF_close;
use crate::lobject::Proto;
use crate::lstate::CallInfo;
use crate::ltm::{luaT_adjustvarargs, luaT_getvarargs};
use crate::value::UnsafeValue;
use crate::{Lua, LuaFn, StackValue, Str, Table, Thread, luaH_getshortstr};
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::mem::{offset_of, transmute};
use core::ops::Deref;
use core::pin::Pin;
use core::ptr::null_mut;
use core::task::{Context, Poll};
use cranelift_codegen::FinalizedRelocTarget;
use cranelift_codegen::binemit::Reloc;
use cranelift_codegen::ir::types::I32;
use cranelift_codegen::ir::{
    AbiParam, ExternalName, Function, InstBuilder, JumpTableData, MemFlags, Signature, TrapCode,
    Type,
};
use cranelift_codegen::isa::CallConv;
use cranelift_frontend::FunctionBuilder;
use target_lexicon::Triple;

mod allocator;
mod emitter;
mod funcs;
mod future;

pub async unsafe fn run<A>(
    td: *const Thread<A>,
    ci: *mut CallInfo,
) -> Result<(), Box<dyn core::error::Error>> {
    // Check if already jitted.
    let f = (*(*td).stack.get().add((*ci).func))
        .value_
        .gc
        .cast::<LuaFn<A>>();
    let p = (*f).p.get();

    if (*p).jitted.is_empty() {
        compile((*td).hdr.global(), p)?;
    }

    // Invoke jitted function.
    let jitted = transmute((*p).jitted as *const u8);
    let state = State {
        td,
        ci,
        next_block: 0,
    };

    Invoker { state, jitted }.await
}

#[inline(never)]
unsafe fn compile<A>(g: &Lua<A>, p: *mut Proto<A>) -> Result<(), std::io::Error> {
    // https://users.rust-lang.org/t/calling-a-rust-function-from-cranelift/103948/5.
    let mut sig = Signature::new(CallConv::triple_default(&HOST));
    let ptr = Type::triple_pointer_type(&HOST);

    sig.params.push(AbiParam::new(ptr)); // *mut State
    sig.params.push(AbiParam::new(ptr)); // *mut Context
    sig.params.push(AbiParam::new(ptr)); // *mut Error

    // Setup builder.
    let mut ctx = g.jit.builder_context.borrow_mut();
    let mut fun = Function::with_name_signature(Default::default(), sig);
    let mut fb = FunctionBuilder::new(&mut fun, &mut ctx);

    // Create entry block.
    let code = unsafe { core::slice::from_raw_parts((*p).code, (*p).sizecode as usize) };
    let entry = fb.create_block();

    fb.append_block_params_for_function_params(entry);
    fb.switch_to_block(entry);
    fb.seal_block(entry);

    // Load arguments.
    let st = fb.declare_var(ptr);
    let cx = fb.declare_var(ptr);
    let ret = fb.declare_var(ptr);

    fb.def_var(st, fb.block_params(entry)[0]);
    fb.def_var(cx, fb.block_params(entry)[1]);
    fb.def_var(ret, fb.block_params(entry)[2]);

    // Compile instructions.
    let mut pc = 0;
    let mut funcs = RustFuncs::<A>::default();
    let mut resumes = Vec::new();
    let jump = fb.create_block();
    let mut emit = Emitter::new(&mut fb, code, st, cx, ret, &mut funcs, &mut resumes, jump);

    loop {
        // Get instruction.
        let i = code.get(pc).copied().unwrap();

        pc += 1;

        // Emit IR.
        let r = match i & 0x7F {
            OP_LOADK => emit.loadk(i, pc),
            OP_GETTABUP => emit.gettabup(i, pc),
            OP_NEWTABLE => emit.newtable(i, pc),
            OP_CALL => emit.call(i, pc),
            OP_RETURN => emit.r#return(i, pc),
            OP_CLOSURE => emit.closure(i, pc),
            OP_VARARG => emit.vararg(i, pc),
            OP_VARARGPREP => emit.varargprep(i, pc),
            v => todo!("OP {v}"),
        };

        match r {
            Some(v) => pc = v,
            None => break,
        }
    }

    drop(emit);

    // Create root jump table.
    let def = fb.create_block();
    let bc = fb.func.dfg.block_call(def, []);
    let jt = fb.create_jump_table(JumpTableData::new(bc, &resumes));

    fb.switch_to_block(def);
    fb.ins().trap(TrapCode::unwrap_user(1));
    fb.switch_to_block(jump);
    fb.seal_block(jump);

    // Jump to resume block.
    let st = fb.use_var(st);
    let v = fb.ins().load(
        I32,
        MemFlags::trusted(),
        st,
        offset_of!(State<A>, next_block) as i32,
    );

    fb.ins().br_table(v, jt);

    // Seal all resume blocks.
    fb.seal_block(def);

    for b in resumes {
        fb.seal_block(b.block(&fb.func.dfg.value_lists));
    }

    fb.finalize();

    drop(ctx);

    // Prepare to generate machine code.
    let mut ctx = g.jit.codegen_context.borrow_mut();

    ctx.func = fun;

    // Generate machine code.
    let code = ctx
        .compile(g.jit.isa.deref(), &mut Default::default())
        .unwrap();
    let data = code.buffer.data();
    let align = usize::try_from(code.buffer.alignment).unwrap();
    let mut allocator = g.jit.allocator.borrow_mut();
    let mut buf = allocator.allocate(data.len().try_into().unwrap(), align.try_into().unwrap())?;

    buf.copy_from_slice(data);

    // Apply relocations.
    for r in code.buffer.relocs() {
        let off = usize::try_from(r.offset).unwrap();

        match r.kind {
            Reloc::Abs4 => todo!(),
            Reloc::Abs8 => match &r.target {
                FinalizedRelocTarget::ExternalName(ExternalName::User(v)) => {
                    let f = funcs.get(*v) as usize;

                    buf[off..(off + 8)].copy_from_slice(&f.to_ne_bytes());
                }
                FinalizedRelocTarget::ExternalName(ExternalName::TestCase(_)) => todo!(),
                FinalizedRelocTarget::ExternalName(ExternalName::LibCall(_)) => todo!(),
                FinalizedRelocTarget::ExternalName(ExternalName::KnownSymbol(_)) => todo!(),
                FinalizedRelocTarget::Func(_) => todo!(),
            },
            Reloc::X86PCRel4 => todo!(),
            Reloc::X86CallPCRel4 => todo!(),
            Reloc::X86CallPLTRel4 => todo!(),
            Reloc::X86GOTPCRel4 => todo!(),
            Reloc::X86SecRel => todo!(),
            Reloc::Arm32Call => todo!(),
            Reloc::Arm64Call => todo!(),
            Reloc::S390xPCRel32Dbl => todo!(),
            Reloc::S390xPLTRel32Dbl => todo!(),
            Reloc::ElfX86_64TlsGd => todo!(),
            Reloc::MachOX86_64Tlv => todo!(),
            Reloc::MachOAarch64TlsAdrPage21 => todo!(),
            Reloc::MachOAarch64TlsAdrPageOff12 => todo!(),
            Reloc::Aarch64TlsDescAdrPage21 => todo!(),
            Reloc::Aarch64TlsDescLd64Lo12 => todo!(),
            Reloc::Aarch64TlsDescAddLo12 => todo!(),
            Reloc::Aarch64TlsDescCall => todo!(),
            Reloc::Aarch64AdrGotPage21 => todo!(),
            Reloc::Aarch64AdrPrelPgHi21 => todo!(),
            Reloc::Aarch64AddAbsLo12Nc => todo!(),
            Reloc::Aarch64Ld64GotLo12Nc => todo!(),
            Reloc::RiscvCallPlt => todo!(),
            Reloc::RiscvTlsGdHi20 => todo!(),
            Reloc::RiscvPCRelLo12I => todo!(),
            Reloc::RiscvGotHi20 => todo!(),
            Reloc::RiscvPCRelHi20 => todo!(),
            Reloc::S390xTlsGd64 => todo!(),
            Reloc::S390xTlsGdCall => todo!(),
            Reloc::PulleyPcRel => todo!(),
            Reloc::PulleyCallIndirectHost => todo!(),
        }
    }

    ctx.clear();

    (*p).jitted = buf.seal();

    Ok(())
}

unsafe extern "C-unwind" fn adjustvarargs<A>(
    td: *const Thread<A>,
    nfixparams: i32,
    ci: *mut CallInfo,
    p: *const Proto<A>,
    ret: *mut Error,
) {
    if let Err(e) = luaT_adjustvarargs(td, nfixparams, ci, p) {
        (*ret).set_error(e);
    };
}

unsafe extern "C-unwind" fn getvarargs<A>(
    td: *const Thread<A>,
    ci: *mut CallInfo,
    r#where: *mut StackValue<A>,
    wanted: i32,
    ret: *mut Error,
) {
    if let Err(e) = luaT_getvarargs(td, ci, r#where, wanted) {
        (*ret).set_error(e);
    }
}

unsafe extern "C-unwind" fn finishget<A>(
    td: *const Thread<A>,
    tab: *const UnsafeValue<A>,
    key: *const UnsafeValue<A>,
    props_tried: bool,
    out: *mut UnsafeValue<A>,
    ret: *mut Error,
) {
    match luaV_finishget(&*td, tab, key, props_tried) {
        Ok(v) => out.write(v),
        Err(e) => (*ret).set_error(e),
    }
}

unsafe extern "C-unwind" fn getshortstr<A>(
    t: *const Table<A>,
    key: *const Str<A>,
) -> *const UnsafeValue<A> {
    luaH_getshortstr(t, key)
}

unsafe extern "C-unwind" fn precall<A>(
    td: *const Thread<A>,
    ci: *mut CallInfo,
    f: *mut StackValue<A>,
    nresults: i32,
    cx: *mut Context,
    ret: *mut Error,
) -> *mut CallInfo {
    let f = Pin::new_unchecked((*ci).pending_future.precall.init(td, f, nresults));
    let r = match f.poll(&mut *cx) {
        Poll::Ready(v) => v,
        Poll::Pending => {
            (*ret).vtb = 1usize as *const ();

            return null_mut();
        }
    };

    (*ci).pending_future.precall.drop::<A>();

    match r {
        Ok(v) => v,
        Err(e) => {
            (*ret).set_error(e);

            null_mut()
        }
    }
}

unsafe extern "C-unwind" fn resume_precall<A>(
    ci: *mut CallInfo,
    cx: *mut Context,
    ret: *mut Error,
) -> *mut CallInfo {
    let r = match (*ci).pending_future.precall.poll::<A>(&mut *cx) {
        Poll::Ready(v) => v,
        Poll::Pending => {
            (*ret).vtb = 1usize as *const ();

            return null_mut();
        }
    };

    (*ci).pending_future.precall.drop::<A>();

    match r {
        Ok(v) => v,
        Err(e) => {
            (*ret).set_error(e);

            null_mut()
        }
    }
}

unsafe extern "C-unwind" fn run_lua<A>(
    td: *const Thread<A>,
    ci: *mut CallInfo,
    cx: *mut Context,
    ret: *mut Error,
) {
    let f = Pin::new_unchecked((*ci).pending_future.run.init(td, ci));
    let r = match f.poll(&mut *cx) {
        Poll::Ready(v) => v,
        Poll::Pending => {
            (*ret).vtb = 1usize as *const ();
            return;
        }
    };

    (*ci).pending_future.run.drop::<A>();

    if let Err(e) = r {
        (*ret).set_error(e);
    }
}

unsafe extern "C-unwind" fn resume_run_lua<A>(
    ci: *mut CallInfo,
    cx: *mut Context,
    ret: *mut Error,
) {
    let r = match (*ci).pending_future.run.poll::<A>(&mut *cx) {
        Poll::Ready(v) => v,
        Poll::Pending => {
            (*ret).vtb = 1usize as *const ();
            return;
        }
    };

    (*ci).pending_future.run.drop::<A>();

    if let Err(e) = r {
        (*ret).set_error(e);
    }
}

unsafe extern "C-unwind" fn close<A>(
    td: *const Thread<A>,
    lv: *mut StackValue<A>,
    ret: *mut Error,
) {
    if let Err(e) = luaF_close(&*td, lv) {
        (*ret).set_error(e);
    }
}

unsafe extern "C-unwind" fn poscall<A>(
    td: *const Thread<A>,
    ci: *mut CallInfo,
    nres: i32,
    ret: *mut Error,
) {
    if let Err(e) = luaD_poscall(&*td, ci, nres) {
        (*ret).set_error(e);
    }
}

unsafe extern "C-unwind" fn pushclosure<A>(
    td: *const Thread<A>,
    p: *mut Proto<A>,
    f: *const LuaFn<A>,
    base: *mut StackValue<A>,
    ra: *mut StackValue<A>,
) {
    super::pushclosure(td, p, &(*f).upvals, base, ra);
}

unsafe extern "C-unwind" fn step_gc<A>(td: *const Thread<A>) {
    (*td).hdr.global().gc.step();
}

unsafe extern "C-unwind" fn create_table<A>(td: *const Thread<A>) -> *const Table<A> {
    Table::new((*td).hdr.global)
}

/// Implementation of [Future] to invoke jitted function.
struct Invoker<A> {
    state: State<A>,
    jitted: unsafe extern "C-unwind" fn(*mut State<A>, *mut Context, *mut Error),
}

impl<A> Future for Invoker<A> {
    type Output = Result<(), Box<dyn core::error::Error>>;

    #[inline]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut r = Error {
            obj: null_mut(),
            vtb: null_mut(),
        };

        unsafe { (self.jitted)(&mut self.state, cx, &mut r) };

        if !r.obj.is_null() {
            Poll::Ready(Err(unsafe { Box::from_raw(transmute(r)) }))
        } else if r.vtb.is_null() {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    }
}

/// State of a call to jitted function.
#[repr(C)]
struct State<A> {
    td: *const Thread<A>,
    ci: *mut CallInfo,
    next_block: u32,
}

/// Contains error from jitted function.
///
/// This struct must have the same layout as a pointer to trait object.
#[repr(C)]
#[derive(Clone, Copy)]
struct Error {
    obj: *mut (),
    vtb: *const (),
}

impl Error {
    fn set_error(&mut self, e: Box<dyn core::error::Error>) {
        *self = unsafe { transmute(Box::into_raw(e)) };
    }
}

const HOST: Triple = Triple::host();
