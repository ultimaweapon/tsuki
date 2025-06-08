use super::Thread;
use crate::Object;
use crate::lobject::UnsafeValue;
use alloc::vec::Vec;
use core::marker::PhantomData;

/// Implementation of [`Args`] which size does not known at compile time.
pub struct DynamicArgs<'a> {
    list: Vec<UnsafeValue>,
    phantom: PhantomData<&'a Object>,
}

impl<'a> Args for DynamicArgs<'a> {
    #[inline(always)]
    fn len(&self) -> usize {
        self.list.len()
    }

    #[inline(always)]
    unsafe fn push_to(self, th: &Thread) {
        for v in self.list {
            unsafe { th.top.write(v) };
            unsafe { th.top.add(1) };
        }
    }
}

/// Arguments to invoke Lua function.
pub trait Args {
    fn len(&self) -> usize;
    unsafe fn push_to(self, th: &Thread);
}

impl Args for () {
    #[inline(always)]
    fn len(&self) -> usize {
        0
    }

    #[inline(always)]
    unsafe fn push_to(self, _: &Thread) {}
}

impl<T: Into<UnsafeValue>> Args for T {
    #[inline(always)]
    fn len(&self) -> usize {
        1
    }

    #[inline(always)]
    unsafe fn push_to(self, th: &Thread) {
        unsafe { th.top.write(self.into()) };
        unsafe { th.top.add(1) };
    }
}
