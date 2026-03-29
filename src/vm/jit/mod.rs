pub use self::future::*;

use self::emitter::Emitter;
use self::funcs::RustFuncs;
use super::{OP_CALL, OP_GETTABUP, OP_LOADK, OP_RETURN, OP_VARARGPREP, luaV_finishget};
use crate::ldo::luaD_poscall;
use crate::lfunc::luaF_close;
use crate::lobject::Proto;
use crate::lstate::CallInfo;
use crate::ltm::luaT_adjustvarargs;
use crate::value::UnsafeValue;
use crate::{Lua, LuaFn, StackValue, Str, Table, Thread, luaH_getshortstr};
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::mem::{offset_of, transmute};
use core::ops::Deref;
use core::pin::Pin;
use core::ptr::null_mut;
use core::task::{Context, Poll};
use cranelift_codegen::ir::types::I32;
use cranelift_codegen::ir::{
    AbiParam, BlockCall, Function, InstBuilder, JumpTableData, MemFlags, Signature, TrapCode, Type,
    ValueListPool,
};
use cranelift_codegen::isa::CallConv;
use cranelift_frontend::FunctionBuilder;
use target_lexicon::Triple;

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

    if (*p).jitted.is_null() {
        compile((*td).hdr.global(), p);
    }

    // Invoke jitted function.
    let jitted = transmute((*p).jitted);
    let state = State {
        td,
        ci,
        next_block: 0,
    };

    Invoker { state, jitted }.await
}

#[inline(never)]
unsafe fn compile<A>(g: &Lua<A>, p: *mut Proto<A>) {
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
    let mut vlp = ValueListPool::new();
    let mut funcs = RustFuncs::<A>::default();
    let mut resumes = Vec::new();
    let jump = fb.create_block();
    let mut emit = Emitter::new(
        &mut fb,
        &mut vlp,
        st,
        cx,
        ret,
        &mut funcs,
        &mut resumes,
        jump,
    );

    loop {
        // Get instruction.
        let i = code.get(pc).copied().unwrap();

        pc += 1;

        // Emit IR.
        let r = match i & 0x7F {
            OP_LOADK => emit.loadk(i, pc),
            OP_GETTABUP => emit.gettabup(i, pc),
            OP_CALL => emit.call(i, pc),
            OP_RETURN => emit.r#return(i, pc),
            OP_VARARGPREP => emit.varargprep(i, pc),
            v => todo!("{v}"),
        };

        match r {
            Some(v) => pc = v,
            None => break,
        }
    }

    drop(emit);

    // Create root jump table.
    let def = fb.create_block();
    let jt = fb.create_jump_table(JumpTableData::new(
        BlockCall::new(def, [], &mut vlp),
        &resumes,
    ));

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
        fb.seal_block(b.block(&vlp));
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

    ctx.clear();

    todo!()
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
