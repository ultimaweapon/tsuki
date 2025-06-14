use crate::{AsyncContext, Fp, Object, Str, YieldContext};
use std::boxed::Box;

/// The outside **must** never be able to construct or have the value of this type.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct UnsafeValue {
    pub value_: UntaggedValue,
    pub tt_: u8,
}

impl UnsafeValue {
    #[inline(always)]
    pub(crate) unsafe fn from_str(s: *const Str) -> Self {
        Self {
            value_: UntaggedValue { gc: s.cast() },
            tt_: unsafe { (*s).hdr.tt | 1 << 6 },
        }
    }
}

impl From<Fp> for UnsafeValue {
    #[inline(always)]
    fn from(value: Fp) -> Self {
        Self {
            value_: UntaggedValue { f: value },
            tt_: 2 | 0 << 4,
        }
    }
}

impl From<fn(YieldContext) -> Result<(), Box<dyn core::error::Error>>> for UnsafeValue {
    fn from(value: fn(YieldContext) -> Result<(), Box<dyn core::error::Error>>) -> Self {
        todo!()
    }
}

impl<'a, F> From<fn(AsyncContext<'a>) -> F> for UnsafeValue
where
    F: Future<Output = Result<(), Box<dyn core::error::Error>>> + 'a,
{
    fn from(value: fn(AsyncContext<'a>) -> F) -> Self {
        todo!()
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union UntaggedValue {
    pub gc: *const Object,
    pub f: Fp,
    pub i: i64,
    pub n: f64,
}
