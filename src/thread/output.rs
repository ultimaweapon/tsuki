use super::Thread;
use crate::Value;
use alloc::vec::Vec;

/// Outputs of a call.
pub unsafe trait Outputs<'a, D> {
    const N: i32;

    unsafe fn new(th: &'a Thread<D>, n: usize) -> Self;
}

unsafe impl<'a, D> Outputs<'a, D> for () {
    const N: i32 = 0;

    #[inline(always)]
    unsafe fn new(_: &'a Thread<D>, _: usize) -> Self {
        ()
    }
}

unsafe impl<'a, D> Outputs<'a, D> for Value<'a, D> {
    const N: i32 = 1;

    unsafe fn new(th: &'a Thread<D>, _: usize) -> Self {
        unsafe { Self::from_unsafe(&th.top.read(0)) }
    }
}

unsafe impl<'a, D> Outputs<'a, D> for Vec<Value<'a, D>> {
    const N: i32 = -1;

    unsafe fn new(th: &'a Thread<D>, n: usize) -> Self {
        let mut r = Vec::with_capacity(n);

        for i in 0..n {
            let v = unsafe { th.top.get().add(i) };

            r.push(unsafe { Value::from_unsafe(&raw const (*v).val) });
        }

        r
    }
}
