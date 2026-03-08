use crate::lobject::Proto;
use crate::lstate::CallInfo;
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
    let state = State {};

    Invoker { state, jitted }.await
}

#[inline(never)]
unsafe fn compile<A>(g: &Lua<A>, p: *mut Proto<A>) {
    // https://users.rust-lang.org/t/calling-a-rust-function-from-cranelift/103948/5.
    let host = Triple::host();
    let mut sig = Signature::new(CallConv::triple_default(&host));
    let ptr = Type::triple_pointer_type(&host);

    sig.params.push(AbiParam::new(ptr)); // *mut State
    sig.params.push(AbiParam::new(ptr)); // *mut Context
    sig.params.push(AbiParam::new(ptr)); // *mut Error

    // Setup builder.
    let mut ctx = g.jit.builder_context.borrow_mut();
    let mut fun = Function::with_name_signature(Default::default(), sig);
    let mut fb = FunctionBuilder::new(&mut fun, &mut ctx);

    // Compile.
    let code = unsafe { core::slice::from_raw_parts((*p).code, (*p).sizecode as usize) };
    let entry = fb.create_block();
    let mut pc = 0;

    fb.append_block_params_for_function_params(entry);
    fb.switch_to_block(entry);

    loop {
        let i = match code.get(pc).copied() {
            Some(v) => v,
            None => break,
        };

        pc += 1;

        match i & 0x7F {
            v => todo!("{v}"),
        }
    }

    fb.seal_all_blocks();
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

/// Implementation of [Future] to invoke jitted function.
struct Invoker {
    state: State,
    jitted: unsafe extern "C-unwind" fn(*mut State, *mut Context, *mut Error),
}

impl Future for Invoker {
    type Output = Result<(), Box<dyn core::error::Error>>;

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
struct State {}

/// Contains error from jitted function.
///
/// This struct must have the same layout as a pointer to trait object.
#[repr(C)]
#[derive(Clone, Copy)]
struct Error {
    obj: *mut (),
    vtb: *const (),
}
