use super::Thread;
use crate::Object;
use crate::value::UnsafeValue;
use alloc::vec::Vec;
use core::marker::PhantomData;

/// Implementation of [`Args`] which size does not known at compile time.
pub struct DynamicArgs<'a> {
    list: Vec<UnsafeValue>,
    phantom: PhantomData<&'a Object>,
}

unsafe impl<'a> Args for DynamicArgs<'a> {
    #[inline(always)]
    fn len(&self) -> usize {
        self.list.len()
    }

    unsafe fn push_to(self, th: &Thread) {
        for (i, v) in self.list.into_iter().enumerate() {
            if unsafe { (v.tt_ & 1 << 6 != 0) && (*v.value_.gc).global != th.hdr.global } {
                panic!("argument #{i} come from a different Lua");
            }

            unsafe { th.top.write(v) };
            unsafe { th.top.add(1) };
        }
    }
}

/// Arguments to invoke Lua function.
///
/// # Safety
/// The value returned from [`Args::len()`] must be exactly the same as the values pushed to the
/// thread in [`Args::push_to()`].
pub unsafe trait Args {
    fn len(&self) -> usize;

    /// # Panics
    /// If any argument does not come from the same [Lua](crate::Lua) as `th`.
    ///
    /// # Safety
    /// The stack of `th` must be able to push more [`Args::len()`] items.
    unsafe fn push_to(self, th: &Thread);
}

unsafe impl Args for () {
    #[inline(always)]
    fn len(&self) -> usize {
        0
    }

    #[inline(always)]
    unsafe fn push_to(self, _: &Thread) {}
}

unsafe impl<T: Into<UnsafeValue>> Args for T {
    #[inline(always)]
    fn len(&self) -> usize {
        1
    }

    unsafe fn push_to(self, th: &Thread) {
        let v = self.into();

        if unsafe { (v.tt_ & 1 << 6 != 0) && (*v.value_.gc).global != th.hdr.global } {
            panic!("argument #0 come from a different Lua");
        }

        unsafe { th.top.write(v) };
        unsafe { th.top.add(1) };
    }
}
