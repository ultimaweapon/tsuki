use super::Thread;
use crate::Value;
use alloc::vec::Vec;

/// Outputs of a call.
pub unsafe trait Outputs {
    const N: i32;

    unsafe fn new(th: &Thread, n: usize) -> Self;
}

unsafe impl Outputs for () {
    const N: i32 = 0;

    #[inline(always)]
    unsafe fn new(_: &Thread, _: usize) -> Self {
        ()
    }
}

unsafe impl Outputs for Value {
    const N: i32 = 1;

    unsafe fn new(th: &Thread, _: usize) -> Self {
        unsafe { Self::from_unsafe(&th.top.read(0)) }
    }
}

unsafe impl Outputs for Vec<Value> {
    const N: i32 = -1;

    unsafe fn new(th: &Thread, n: usize) -> Self {
        let mut r = Vec::with_capacity(n);

        for i in 0..n {
            let v = unsafe { th.top.get().add(i) };

            r.push(unsafe { Value::from_unsafe(&raw const (*v).val) });
        }

        r
    }
}
