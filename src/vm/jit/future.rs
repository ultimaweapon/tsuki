use super::run;
use crate::ldo::{luaD_precall, luaD_pretailcall};
use crate::lstate::CallInfo;
use crate::{StackValue, Thread};
use alloc::boxed::Box;
use core::pin::Pin;
use core::task::{Context, Poll};

/// Encapsulates a future returned from [luaD_precall].
#[repr(C, align(8))]
#[derive(Clone, Copy)]
pub struct PrecallFuture([u8; Self::size(luaD_precall::<()>)]);

const _: () = assert!(align_of::<PrecallFuture>() >= PrecallFuture::align(luaD_precall::<()>));

impl PrecallFuture {
    #[inline]
    pub unsafe fn init<A>(
        &mut self,
        td: *const Thread<A>,
        f: *mut StackValue<A>,
        nresults: i32,
    ) -> &mut impl Future<Output = Result<*mut CallInfo, Box<dyn core::error::Error>>> {
        let f = luaD_precall(td, f, nresults);
        let p = self.0.as_mut_ptr().cast();

        core::ptr::write(p, f);

        &mut *p
    }

    pub unsafe fn poll<A>(
        &mut self,
        cx: &mut Context,
    ) -> Poll<Result<*mut CallInfo, Box<dyn core::error::Error>>> {
        let f = Pin::new_unchecked(self.get(luaD_precall::<A>));

        f.poll(cx)
    }

    pub unsafe fn drop<A>(&mut self) {
        let f = self.get(luaD_precall::<A>);

        core::ptr::drop_in_place(f);
    }

    #[inline(always)]
    unsafe fn get<A, R>(
        &mut self,
        _: unsafe fn(*const Thread<A>, *mut StackValue<A>, nresults: i32) -> R,
    ) -> &mut R {
        let p = self.0.as_mut_ptr().cast();

        &mut *p
    }

    const fn align<A, R>(
        _: unsafe fn(*const Thread<A>, *mut StackValue<A>, nresults: i32) -> R,
    ) -> usize {
        align_of::<R>()
    }

    const fn size<A, R>(
        _: unsafe fn(*const Thread<A>, *mut StackValue<A>, nresults: i32) -> R,
    ) -> usize {
        size_of::<R>()
    }
}

/// Encapsulates a future returned from [luaD_pretailcall].
#[repr(C, align(8))]
#[derive(Clone, Copy)]
pub struct TailcallFuture([u8; Self::size(luaD_pretailcall::<()>)]);

const _: () =
    assert!(align_of::<TailcallFuture>() >= TailcallFuture::align(luaD_pretailcall::<()>));

impl TailcallFuture {
    #[inline]
    pub unsafe fn init<A>(
        &mut self,
        td: *const Thread<A>,
        ci: *mut CallInfo,
        func: *mut StackValue<A>,
        narg1: i32,
        delta: i32,
    ) -> &mut impl Future<Output = Result<i32, Box<dyn core::error::Error>>> {
        let f = luaD_pretailcall(td, ci, func, narg1, delta);
        let p = self.0.as_mut_ptr().cast();

        core::ptr::write(p, f);

        &mut *p
    }

    pub unsafe fn poll<A>(
        &mut self,
        cx: &mut Context,
    ) -> Poll<Result<i32, Box<dyn core::error::Error>>> {
        let f = Pin::new_unchecked(self.get(luaD_pretailcall::<A>));

        f.poll(cx)
    }

    pub unsafe fn drop<A>(&mut self) {
        let f = self.get(luaD_pretailcall::<A>);

        core::ptr::drop_in_place(f);
    }

    #[inline(always)]
    unsafe fn get<A, R>(
        &mut self,
        _: unsafe fn(*const Thread<A>, *mut CallInfo, *mut StackValue<A>, i32, i32) -> R,
    ) -> &mut R {
        let p = self.0.as_mut_ptr().cast();

        &mut *p
    }

    const fn align<A, R>(
        _: unsafe fn(*const Thread<A>, *mut CallInfo, *mut StackValue<A>, i32, i32) -> R,
    ) -> usize {
        align_of::<R>()
    }

    const fn size<A, R>(
        _: unsafe fn(*const Thread<A>, *mut CallInfo, *mut StackValue<A>, i32, i32) -> R,
    ) -> usize {
        size_of::<R>()
    }
}

/// Encapsulates a future returned from [run].
#[repr(C, align(8))]
#[derive(Clone, Copy)]
pub struct RunFuture([u8; Self::size(run::<()>)]);

const _: () = assert!(align_of::<RunFuture>() >= RunFuture::align(run::<()>));

impl RunFuture {
    #[inline]
    pub unsafe fn init<A>(
        &mut self,
        td: *const Thread<A>,
        ci: *mut CallInfo,
    ) -> &mut impl Future<Output = Result<(), Box<dyn core::error::Error>>> {
        let f = run(td, ci);
        let p = self.0.as_mut_ptr().cast();

        core::ptr::write(p, f);

        &mut *p
    }

    pub unsafe fn poll<A>(
        &mut self,
        cx: &mut Context,
    ) -> Poll<Result<(), Box<dyn core::error::Error>>> {
        let f = Pin::new_unchecked(self.get(run::<A>));

        f.poll(cx)
    }

    pub unsafe fn drop<A>(&mut self) {
        let f = self.get(run::<A>);

        core::ptr::drop_in_place(f);
    }

    #[inline(always)]
    unsafe fn get<A, R>(&mut self, _: unsafe fn(*const Thread<A>, *mut CallInfo) -> R) -> &mut R {
        let p = self.0.as_mut_ptr().cast();

        &mut *p
    }

    const fn align<A, R>(_: unsafe fn(*const Thread<A>, *mut CallInfo) -> R) -> usize {
        align_of::<R>()
    }

    const fn size<A, R>(_: unsafe fn(*const Thread<A>, *mut CallInfo) -> R) -> usize {
        size_of::<R>()
    }
}
