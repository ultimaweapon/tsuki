pub use self::allocator::*;
pub use self::future::*;

pub(self) use self::rust::*;

use self::emitter::Emitter;
use super::{
    OP_ADD, OP_ADDI, OP_CALL, OP_CLOSE, OP_CLOSURE, OP_DIVK, OP_EQ, OP_EQI, OP_EQK, OP_FORLOOP,
    OP_FORPREP, OP_GETFIELD, OP_GETI, OP_GETTABLE, OP_GETTABUP, OP_GETUPVAL, OP_GTI, OP_JMP,
    OP_LABEL, OP_LEN, OP_LFALSESKIP, OP_LOADFALSE, OP_LOADI, OP_LOADK, OP_LOADNIL, OP_LOADTRUE,
    OP_MMBIN, OP_MMBINI, OP_MMBINK, OP_MODK, OP_MOVE, OP_MUL, OP_NEWTABLE, OP_NOT, OP_RETURN,
    OP_RETURN0, OP_SELF, OP_SETFIELD, OP_SETLIST, OP_SETTABLE, OP_SETTABUP, OP_SETUPVAL,
    OP_TAILCALL, OP_TBC, OP_TEST, OP_VARARG, OP_VARARGPREP, luaV_equalobj, luaV_finishget,
    luaV_finishset, luaV_objlen,
};
use crate::gc::Object;
use crate::ldo::luaD_poscall;
use crate::lfunc::{luaF_close, luaF_newtbcupval};
use crate::lobject::Proto;
use crate::lstate::CallInfo;
use crate::ltm::{
    luaT_adjustvarargs, luaT_callorderiTM, luaT_getvarargs, luaT_trybinTM, luaT_trybinassocTM,
    luaT_trybiniTM,
};
use crate::value::UnsafeValue;
use crate::{ArithError, Lua, LuaFn, StackValue, Table, Thread};
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::mem::{offset_of, transmute};
use core::ops::Deref;
use core::pin::Pin;
use core::ptr::null_mut;
use core::task::{Context, Poll};
use cranelift_codegen::FinalizedRelocTarget;
use cranelift_codegen::binemit::Reloc;
use cranelift_codegen::ir::types::{I8, I32};
use cranelift_codegen::ir::{
    AbiParam, ExternalName, Function, InstBuilder, JumpTableData, MemFlags, Signature, TrapCode,
    Type,
};
use cranelift_codegen::isa::CallConv;
use cranelift_frontend::FunctionBuilder;
use target_lexicon::Triple;

mod allocator;
mod emitter;
mod future;
mod rust;

pub async unsafe fn run<A>(
    td: *const Thread<A>,
    ci: *mut CallInfo,
) -> Result<(), Box<dyn core::error::Error>> {
    loop {
        // Check if already jitted.
        let f = (*(*td).stack.get().add((*ci).func))
            .value_
            .gc
            .cast::<LuaFn<A>>();
        let p = (*f).p.get();

        if (*p).jitted.is_empty() {
            compile((*td).hdr.global(), p)?;
        }

        // Set up state.
        let jitted = transmute((*p).jitted as *const u8);
        let state = State {
            td,
            ci,
            next_block: 0,
        };

        // Invoke jitted function.
        let f = Invoker { state, jitted };

        match f.await {
            Ok(Status::Finished) => break Ok(()),
            Ok(Status::Replaced) => (),
            Err(e) => break Err(e),
        }
    }
}

#[inline(never)]
unsafe fn compile<A>(g: &Lua<A>, p: *mut Proto<A>) -> Result<(), std::io::Error> {
    // https://users.rust-lang.org/t/calling-a-rust-function-from-cranelift/103948/5.
    let mut sig = Signature::new(CallConv::triple_default(&HOST));
    let ptr = Type::triple_pointer_type(&HOST);

    sig.params.push(AbiParam::new(ptr)); // *mut State
    sig.params.push(AbiParam::new(ptr)); // *mut Context
    sig.params.push(AbiParam::new(ptr)); // *mut Error

    sig.returns.push(AbiParam::new(I8));

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
    let mut rust = RustFuncs::<A>::default();
    let mut resumes = Vec::new();
    let jump = fb.create_block();
    let mut emit = Emitter::new(&mut fb, code, st, cx, ret, &mut rust, &mut resumes, jump);

    loop {
        emit.prepare(pc);

        // Get instruction.
        let i = code.get(pc).copied().unwrap();

        pc += 1;

        // Emit IR.
        let r = match i & 0x7F {
            OP_MOVE => emit.move_(i, pc),
            OP_LOADI => emit.loadi(i, pc),
            OP_LOADK => emit.loadk(i, pc),
            OP_LOADFALSE => emit.loadfalse(i, pc),
            OP_LFALSESKIP => emit.lfalseskip(i, pc),
            OP_LOADTRUE => emit.loadtrue(i, pc),
            OP_LOADNIL => emit.loadnil(i, pc),
            OP_GETUPVAL => emit.getupval(i, pc),
            OP_SETUPVAL => emit.setupval(i, pc),
            OP_GETTABUP => emit.gettabup(i, pc),
            OP_GETTABLE => emit.gettable(i, pc),
            OP_GETI => emit.geti(i, pc),
            OP_GETFIELD => emit.getfield(i, pc),
            OP_SETTABUP => emit.settabup(i, pc),
            OP_SETTABLE => emit.settable(i, pc),
            OP_SETFIELD => emit.setfield(i, pc),
            OP_NEWTABLE => emit.newtable(i, pc),
            OP_SELF => emit.self_(i, pc),
            OP_ADDI => emit.addi(i, pc),
            OP_MODK => emit.modk(i, pc),
            OP_DIVK => emit.divk(i, pc),
            OP_ADD => emit.add(i, pc),
            OP_MUL => emit.mul(i, pc),
            OP_MMBIN => emit.mmbin(i, pc),
            OP_MMBINI => emit.mmbini(i, pc),
            OP_MMBINK => emit.mmbink(i, pc),
            OP_NOT => emit.not(i, pc),
            OP_LEN => emit.len(i, pc),
            OP_CLOSE => emit.close(i, pc),
            OP_TBC => emit.tbc(i, pc),
            OP_JMP => emit.jmp(i, pc),
            OP_EQ => emit.eq(i, pc),
            OP_EQK => emit.eqk(i, pc),
            OP_EQI => emit.eqi(i, pc),
            OP_GTI => emit.gti(i, pc),
            OP_TEST => emit.test(i, pc),
            OP_CALL => emit.call(i, pc),
            OP_TAILCALL => emit.tailcall(i, pc),
            OP_RETURN => emit.return_(i, pc),
            OP_RETURN0 => emit.return0(i, pc),
            OP_FORLOOP => emit.forloop(i, pc),
            OP_FORPREP => emit.forprep(i, pc),
            OP_SETLIST => emit.setlist(i, pc),
            OP_CLOSURE => emit.closure(i, pc),
            OP_VARARG => emit.vararg(i, pc),
            OP_VARARGPREP => emit.varargprep(i, pc),
            OP_LABEL => emit.label(i, pc),
            v => todo!("OP {v}"),
        };

        match r {
            Some(v) => pc = v,
            None => break,
        }
    }

    drop(emit);

    if resumes.len() == 1 {
        let b = resumes[0].block(&fb.func.dfg.value_lists);

        fb.switch_to_block(jump);
        fb.seal_block(jump);
        fb.ins().jump(b, []);
    } else {
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
        fb.seal_block(def);
    }

    // Seal all resume blocks.
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
            Reloc::Abs8 => match &r.target {
                FinalizedRelocTarget::ExternalName(ExternalName::User(v)) => {
                    let f = rust.get(*v) as usize;

                    buf[off..(off + 8)].copy_from_slice(&f.to_ne_bytes());
                }
                v => todo!("{v:?}"),
            },
            v => todo!("{v:?}"),
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

unsafe extern "C-unwind" fn finishset<A>(
    td: *const Thread<A>,
    tab: *const UnsafeValue<A>,
    key: *const UnsafeValue<A>,
    val: *const UnsafeValue<A>,
    slot: *const UnsafeValue<A>,
    ret: *mut Error,
) {
    if let Err(e) = luaV_finishset(&*td, tab, key, val, slot) {
        (*ret).set_error(e);
    }
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

unsafe extern "C-unwind" fn pretailcall<A>(
    td: *const Thread<A>,
    ci: *mut CallInfo,
    f: *mut StackValue<A>,
    narg1: i32,
    delta: i32,
    cx: *mut Context,
    ret: *mut Error,
) -> i32 {
    let f = Pin::new_unchecked((*ci).pending_future.tailcall.init(td, ci, f, narg1, delta));
    let r = match f.poll(&mut *cx) {
        Poll::Ready(v) => v,
        Poll::Pending => {
            (*ret).vtb = 1usize as *const ();

            return 0;
        }
    };

    (*ci).pending_future.tailcall.drop::<A>();

    match r {
        Ok(v) => v,
        Err(e) => {
            (*ret).set_error(e);

            0
        }
    }
}

unsafe extern "C-unwind" fn resume_pretailcall<A>(
    ci: *mut CallInfo,
    cx: *mut Context,
    ret: *mut Error,
) -> i32 {
    let r = match (*ci).pending_future.tailcall.poll::<A>(&mut *cx) {
        Poll::Ready(v) => v,
        Poll::Pending => {
            (*ret).vtb = 1usize as *const ();

            return 0;
        }
    };

    (*ci).pending_future.tailcall.drop::<A>();

    match r {
        Ok(v) => v,
        Err(e) => {
            (*ret).set_error(e);

            0
        }
    }
}

unsafe extern "C-unwind" fn run_lua<A>(
    td: *const Thread<A>,
    ci: *mut CallInfo,
    new: *mut CallInfo,
    cx: *mut Context,
    ret: *mut Error,
) {
    let f = Pin::new_unchecked((*ci).pending_future.run.init(td, new));
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

unsafe extern "C-unwind" fn barrier<A>(p: *const Object<A>, v: *const UnsafeValue<A>) {
    if (*v).tt_ & 1 << 6 != 0 {
        let v = (*v).value_.gc;

        if (*p).marked.get() & 1 << 5 != 0 && (*v).marked.is_white() {
            (*p).global().gc.barrier(p, v);
        }
    }
}

unsafe extern "C-unwind" fn barrier_back<A>(p: *const UnsafeValue<A>, v: *const UnsafeValue<A>) {
    if (*v).tt_ & 1 << 6 != 0 {
        let p = (*p).value_.gc;

        if (*p).marked.get() & 1 << 5 != 0 && (*(*v).value_.gc).marked.is_white() {
            (*p).global().gc.barrier_back(p);
        }
    }
}

unsafe extern "C-unwind" fn create_table<A>(td: *const Thread<A>) -> *const Table<A> {
    Table::new((*td).hdr.global)
}

unsafe extern "C-unwind" fn equalobj<A>(
    td: *const Thread<A>,
    t1: *const UnsafeValue<A>,
    t2: *const UnsafeValue<A>,
    ret: *mut Error,
) -> i32 {
    match luaV_equalobj(td.as_ref(), t1, t2) {
        Ok(v) => v.into(),
        Err(e) => {
            (*ret).set_error(e);
            0
        }
    }
}

unsafe extern "C-unwind" fn objlen<A>(
    td: *const Thread<A>,
    v: *const UnsafeValue<A>,
    out: *mut UnsafeValue<A>,
    ret: *mut Error,
) {
    match luaV_objlen(&*td, v) {
        Ok(v) => out.write(v),
        Err(e) => (*ret).set_error(e),
    }
}

unsafe extern "C-unwind" fn trybinTM<A>(
    td: *const Thread<A>,
    p1: *const UnsafeValue<A>,
    p2: *const UnsafeValue<A>,
    event: u32,
    out: *mut UnsafeValue<A>,
    ret: *mut Error,
) {
    match luaT_trybinTM(&*td, p1, p2, event) {
        Ok(v) => out.write(v),
        Err(e) => (*ret).set_error(e),
    }
}

unsafe extern "C-unwind" fn trybiniTM<A>(
    td: *const Thread<A>,
    p1: *const UnsafeValue<A>,
    i2: i64,
    flip: i32,
    event: u32,
    out: *mut UnsafeValue<A>,
    ret: *mut Error,
) {
    match luaT_trybiniTM(&*td, p1, i2, flip, event) {
        Ok(v) => out.write(v),
        Err(e) => (*ret).set_error(e),
    }
}

unsafe extern "C-unwind" fn trybinassocTM<A>(
    td: *const Thread<A>,
    p1: *const UnsafeValue<A>,
    p2: *const UnsafeValue<A>,
    flip: i32,
    event: u32,
    out: *mut UnsafeValue<A>,
    ret: *mut Error,
) {
    match luaT_trybinassocTM(&*td, p1, p2, flip, event) {
        Ok(v) => out.write(v),
        Err(e) => (*ret).set_error(e),
    }
}

unsafe extern "C-unwind" fn callorderiTM<A>(
    td: *const Thread<A>,
    p1: *const UnsafeValue<A>,
    v2: i32,
    flip: i32,
    float: i32,
    event: u32,
    ret: *mut Error,
) -> bool {
    match luaT_callorderiTM(&*td, p1, v2, flip, float, event) {
        Ok(v) => v,
        Err(e) => {
            (*ret).set_error(e);
            false
        }
    }
}

unsafe extern "C-unwind" fn newtbcupval<A>(
    td: *const Thread<A>,
    level: *mut StackValue<A>,
    ret: *mut Error,
) {
    if let Err(e) = luaF_newtbcupval(td, level) {
        (*ret).set_error(e);
    }
}

unsafe extern "C-unwind" fn forprep<A>(
    td: *const Thread<A>,
    ra: *mut StackValue<A>,
    ret: *mut Error,
) -> u8 {
    match super::forprep(td, ra) {
        Ok(v) => v.into(),
        Err(e) => {
            (*ret).set_error(e);
            0
        }
    }
}

unsafe extern "C-unwind" fn mod_zero(ret: *mut Error) {
    (*ret).set_error(Box::new(ArithError::ModZero));
}

/// Implementation of [Future] to invoke jitted function.
struct Invoker<A> {
    state: State<A>,
    jitted: unsafe extern "C-unwind" fn(*mut State<A>, *mut Context, *mut Error) -> Status,
}

impl<A> Future for Invoker<A> {
    type Output = Result<Status, Box<dyn core::error::Error>>;

    #[inline]
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut r = Error::default();
        let s = unsafe { (self.jitted)(&mut self.state, cx, &mut r) };

        if !r.obj.is_null() {
            Poll::Ready(Err(unsafe { Box::from_raw(transmute(r)) }))
        } else if r.vtb.is_null() {
            Poll::Ready(Ok(s))
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
#[derive(Default, Clone, Copy)]
struct Error {
    obj: *mut (),
    vtb: *const (),
}

impl Error {
    fn set_error(&mut self, e: Box<dyn core::error::Error>) {
        *self = unsafe { transmute(Box::into_raw(e)) };
    }
}

/// Status of a call to jitted function.
#[repr(u8)]
#[derive(Clone, Copy)]
enum Status {
    Finished,
    Replaced,
}

impl From<Status> for i64 {
    #[inline(always)]
    fn from(value: Status) -> Self {
        value as i64
    }
}

const HOST: Triple = Triple::host();
