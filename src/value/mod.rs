use crate::{Arg, Args, Context, Fp, LuaFn, Nil, Object, Ref, Ret, Str, Table, Thread, Value};
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

impl From<i64> for UnsafeValue {
    #[inline(always)]
    fn from(value: i64) -> Self {
        Self {
            value_: UntaggedValue { i: value },
            tt_: 3 | 0 << 4,
        }
    }
}

impl From<f64> for UnsafeValue {
    #[inline(always)]
    fn from(value: f64) -> Self {
        Self {
            value_: UntaggedValue { n: value },
            tt_: 3 | 1 << 4,
        }
    }
}

impl From<&Str> for UnsafeValue {
    #[inline(always)]
    fn from(value: &Str) -> Self {
        Self {
            value_: UntaggedValue { gc: &value.hdr },
            tt_: value.hdr.tt | 1 << 6,
        }
    }
}

impl From<Ref<Str>> for UnsafeValue {
    fn from(value: Ref<Str>) -> Self {
        Self {
            value_: UntaggedValue { gc: &value.hdr },
            tt_: value.hdr.tt | 1 << 6,
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

impl From<&LuaFn> for UnsafeValue {
    #[inline(always)]
    fn from(value: &LuaFn) -> Self {
        Self {
            value_: UntaggedValue { gc: &value.hdr },
            tt_: value.hdr.tt | 1 << 6,
        }
    }
}

impl From<Ref<LuaFn>> for UnsafeValue {
    fn from(value: Ref<LuaFn>) -> Self {
        Self {
            value_: UntaggedValue { gc: &value.hdr },
            tt_: value.hdr.tt | 1 << 6,
        }
    }
}

impl From<&Thread> for UnsafeValue {
    #[inline(always)]
    fn from(value: &Thread) -> Self {
        Self {
            value_: UntaggedValue { gc: &value.hdr },
            tt_: 8 | 0 << 4 | 1 << 6,
        }
    }
}

impl From<Ref<Thread>> for UnsafeValue {
    fn from(value: Ref<Thread>) -> Self {
        Self {
            value_: UntaggedValue { gc: &value.hdr },
            tt_: 8 | 0 << 4 | 1 << 6,
        }
    }
}

impl From<Value> for UnsafeValue {
    fn from(value: Value) -> Self {
        match value {
            Value::Nil => Self::from(Nil),
            Value::Bool(v) => Self::from(v),
            Value::Fp(v) => Self::from(Fp(v)),
            Value::Int(v) => Self::from(v),
            Value::Num(v) => Self::from(v),
            Value::Str(v) => Self::from(v),
            Value::Table(v) => Self::from(v),
            Value::LuaFn(v) => Self::from(v),
            Value::Thread(v) => Self::from(v),
        }
    }
}

impl<'a, 'b> From<&Arg<'a, 'b>> for UnsafeValue {
    #[inline(always)]
    fn from(value: &Arg<'a, 'b>) -> Self {
        let v = value.get_raw_or_null();

        if v.is_null() {
            Self::from(Nil)
        } else {
            unsafe { v.read() }
        }
    }
}

impl<'a, 'b> From<Arg<'a, 'b>> for UnsafeValue {
    #[inline(always)]
    fn from(value: Arg<'a, 'b>) -> Self {
        Self::from(&value)
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
