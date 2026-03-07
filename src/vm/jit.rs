use crate::lstate::CallInfo;
use crate::{LuaFn, Thread};
use alloc::boxed::Box;
use core::mem::transmute;
use core::pin::Pin;
use core::task::{Context, Poll};

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
    let jitted = &*(*p).jitted;

    if jitted.is_empty() {
        todo!()
    }

    // Invoke jitted function.
    let jitted = transmute(jitted.as_ptr());
    let state = State {};

    Invoker { state, jitted }.await
}

/// Implementation of [Future] to invoke jitted function.
struct Invoker {
    state: State,
    jitted: unsafe extern "C-unwind" fn(*mut State, *mut Context) -> Error,
}

impl Future for Invoker {
    type Output = Result<(), Box<dyn core::error::Error>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let r = unsafe { (self.jitted)(&mut self.state, cx) };

        if !r.ptr.is_null() {
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
    ptr: *mut (),
    vtb: *const (),
}
