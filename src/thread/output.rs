use super::Thread;
use crate::Value;
use alloc::vec::Vec;

/// Outputs of a call.
pub unsafe trait Outputs<D> {
    const N: i32;

    unsafe fn new(th: &Thread<D>, n: usize) -> Self;
}

unsafe impl<D> Outputs<D> for () {
    const N: i32 = 0;

    #[inline(always)]
    unsafe fn new(_: &Thread<D>, _: usize) -> Self {
        ()
    }
}

unsafe impl<D> Outputs<D> for Value<D> {
    const N: i32 = 1;

    unsafe fn new(th: &Thread<D>, _: usize) -> Self {
        unsafe { Self::from_unsafe(&th.top.read(0)) }
    }
}

unsafe impl<D> Outputs<D> for Vec<Value<D>> {
    const N: i32 = -1;

    unsafe fn new(th: &Thread<D>, n: usize) -> Self {
        let mut r = Vec::with_capacity(n);

        for i in 0..n {
            let v = unsafe { th.top.get().add(i) };

            r.push(unsafe { Value::from_unsafe(&raw const (*v).val) });
        }

        r
    }
}
