use crate::{Args, Context, Fp, Nil, Object, Ref, Ret, Table};
use alloc::boxed::Box;

/// The outside **must** never be able to construct or have the value of this type.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct UnsafeValue {
    pub value_: UntaggedValue,
    pub tt_: u8,
}

impl UnsafeValue {
    #[inline(always)]
    pub(crate) unsafe fn from_obj(s: *const Object) -> Self {
        Self {
            value_: UntaggedValue { gc: s },
            tt_: unsafe { (*s).tt | 1 << 6 },
        }
    }
}

impl From<Nil> for UnsafeValue {
    #[inline(always)]
    fn from(_: Nil) -> Self {
        Self {
            value_: UntaggedValue { i: 0 },
            tt_: 0 | 0 << 4,
        }
    }
}

impl From<bool> for UnsafeValue {
    #[inline(always)]
    fn from(value: bool) -> Self {
        Self {
            value_: UntaggedValue { i: 0 },
            tt_: match value {
                true => 1 | 1 << 4,
                false => 1 | 0 << 4,
            },
        }
    }
}

impl From<Fp> for UnsafeValue {
    #[inline(always)]
    fn from(value: Fp) -> Self {
        Self {
            value_: UntaggedValue { f: value.0 },
            tt_: 2 | 0 << 4,
        }
    }
}

impl From<&Table> for UnsafeValue {
    #[inline(always)]
    fn from(value: &Table) -> Self {
        Self {
            value_: UntaggedValue { gc: &value.hdr },
            tt_: 5 | 0 << 4 | 1 << 6,
        }
    }
}

impl From<Ref<Table>> for UnsafeValue {
    fn from(value: Ref<Table>) -> Self {
        Self {
            value_: UntaggedValue { gc: &value.hdr },
            tt_: 5 | 0 << 4 | 1 << 6,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union UntaggedValue {
    pub gc: *const Object,
    pub f: for<'a> fn(Context<'a, Args>) -> Result<Context<'a, Ret>, Box<dyn core::error::Error>>,
    pub i: i64,
    pub n: f64,
}
