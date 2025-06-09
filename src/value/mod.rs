use crate::{AsyncContext, Context, Fp, Object, YieldContext};
use std::boxed::Box;

/// The outside **must** never be able to construct or have the value of this type.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct UnsafeValue {
    pub value_: UntaggedValue,
    pub tt_: u8,
}

impl From<fn(&mut Context) -> Result<(), Box<dyn core::error::Error>>> for UnsafeValue {
    fn from(value: fn(&mut Context) -> Result<(), Box<dyn core::error::Error>>) -> Self {
        todo!()
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
