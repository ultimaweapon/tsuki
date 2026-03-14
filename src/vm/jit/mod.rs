use self::compiler::Compiler;
use self::funcs::RustFuncs;
use super::OP_VARARGPREP;
use crate::lobject::Proto;
use crate::lstate::CallInfo;
use crate::ltm::luaT_adjustvarargs;
use crate::{Lua, LuaFn, Thread};
use alloc::boxed::Box;
use core::mem::transmute;
use core::ops::Deref;
use core::pin::Pin;
use core::ptr::null_mut;
use core::task::{Context, Poll};
use cranelift_codegen::ir::{AbiParam, Function, Signature, Type};
use cranelift_codegen::isa::CallConv;
use cranelift_frontend::FunctionBuilder;
use target_lexicon::Triple;

mod compiler;
mod funcs;

pub async unsafe fn run<A>(
    td: &Thread<A>,
    ci: *mut CallInfo,
) -> Result<(), Box<dyn core::error::Error>> {
    // Check if already jitted.
    let f = (*td.stack.get().add((*ci).func))
        .value_
        .gc
        .cast::<LuaFn<A>>();
    let p = (*f).p.get();

    if (*p).jitted.is_null() {
        compile(td.hdr.global(), p);
    }

    // Invoke jitted function.
    let jitted = transmute((*p).jitted);
    let state = State { td, ci };

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
    let mut funcs = RustFuncs::default();
    let mut com = Compiler::new::<A>(fb, st, cx, ret, &mut funcs);
    let mut pc = 0;

    loop {
        let i = match code.get(pc).copied() {
            Some(v) => v,
            None => break,
        };

        pc += 1;

        pc = match i & 0x7F {
            OP_VARARGPREP => com.emit_varargprep::<A>(i, pc),
            v => todo!("{v}"),
        };
    }

    drop(com);
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
