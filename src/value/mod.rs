use crate::{
    Arg, Args, Context, Fp, LuaFn, Nil, Object, Ref, Ret, Str, Table, Thread, UserData, Value,
};
use alloc::boxed::Box;

/// The outside **must** never be able to construct or have the value of this type.
#[repr(C)]
pub struct UnsafeValue<D> {
    pub value_: UntaggedValue<D>,
    pub tt_: u8,
}

impl<D> UnsafeValue<D> {
    #[inline(always)]
    pub(crate) unsafe fn from_obj(s: *const Object<D>) -> Self {
        Self {
            value_: UntaggedValue { gc: s },
            tt_: unsafe { (*s).tt | 1 << 6 },
        }
    }
}

impl<D> Clone for UnsafeValue<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for UnsafeValue<D> {}

impl<D> From<Nil> for UnsafeValue<D> {
    #[inline(always)]
    fn from(_: Nil) -> Self {
        Self {
            value_: UntaggedValue { i: 0 },
            tt_: 0 | 0 << 4,
        }
    }
}

impl<D> From<bool> for UnsafeValue<D> {
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

impl<D> From<Fp<D>> for UnsafeValue<D> {
    #[inline(always)]
    fn from(value: Fp<D>) -> Self {
        Self {
            value_: UntaggedValue { f: value.0 },
            tt_: 2 | 0 << 4,
        }
    }
}

impl<D> From<i32> for UnsafeValue<D> {
    #[inline(always)]
    fn from(value: i32) -> Self {
        Self {
            value_: UntaggedValue { i: value.into() },
            tt_: 3 | 0 << 4,
        }
    }
}

impl<D> From<i64> for UnsafeValue<D> {
    #[inline(always)]
    fn from(value: i64) -> Self {
        Self {
            value_: UntaggedValue { i: value },
            tt_: 3 | 0 << 4,
        }
    }
}

impl<D> From<u8> for UnsafeValue<D> {
    #[inline(always)]
    fn from(value: u8) -> Self {
        Self {
            value_: UntaggedValue { i: value.into() },
            tt_: 3 | 0 << 4,
        }
    }
}

impl<D> From<u32> for UnsafeValue<D> {
    #[inline(always)]
    fn from(value: u32) -> Self {
        Self {
            value_: UntaggedValue { i: value.into() },
            tt_: 3 | 0 << 4,
        }
    }
}

impl<D> From<f64> for UnsafeValue<D> {
    #[inline(always)]
    fn from(value: f64) -> Self {
        Self {
            value_: UntaggedValue { n: value },
            tt_: 3 | 1 << 4,
        }
    }
}

impl<D> From<&Str<D>> for UnsafeValue<D> {
    #[inline(always)]
    fn from(value: &Str<D>) -> Self {
        Self {
            value_: UntaggedValue { gc: &value.hdr },
            tt_: value.hdr.tt | 1 << 6,
        }
    }
}

impl<D> From<Ref<Str<D>, D>> for UnsafeValue<D> {
    fn from(value: Ref<Str<D>, D>) -> Self {
        Self {
            value_: UntaggedValue { gc: &value.hdr },
            tt_: value.hdr.tt | 1 << 6,
        }
    }
}

impl<D> From<&Table<D>> for UnsafeValue<D> {
    #[inline(always)]
    fn from(value: &Table<D>) -> Self {
        Self {
            value_: UntaggedValue { gc: &value.hdr },
            tt_: 5 | 0 << 4 | 1 << 6,
        }
    }
}

impl<D> From<Ref<Table<D>, D>> for UnsafeValue<D> {
    fn from(value: Ref<Table<D>, D>) -> Self {
        Self {
            value_: UntaggedValue { gc: &value.hdr },
            tt_: 5 | 0 << 4 | 1 << 6,
        }
    }
}

impl<D> From<&LuaFn<D>> for UnsafeValue<D> {
    #[inline(always)]
    fn from(value: &LuaFn<D>) -> Self {
        Self {
            value_: UntaggedValue { gc: &value.hdr },
            tt_: value.hdr.tt | 1 << 6,
        }
    }
}

impl<D> From<Ref<LuaFn<D>, D>> for UnsafeValue<D> {
    fn from(value: Ref<LuaFn<D>, D>) -> Self {
        Self {
            value_: UntaggedValue { gc: &value.hdr },
            tt_: value.hdr.tt | 1 << 6,
        }
    }
}

impl<D, T> From<&UserData<D, T>> for UnsafeValue<D> {
    #[inline(always)]
    fn from(value: &UserData<D, T>) -> Self {
        Self {
            value_: UntaggedValue { gc: &value.hdr },
            tt_: value.hdr.tt | 1 << 6,
        }
    }
}

impl<D, T> From<Ref<UserData<D, T>, D>> for UnsafeValue<D> {
    #[inline(always)]
    fn from(value: Ref<UserData<D, T>, D>) -> Self {
        Self {
            value_: UntaggedValue { gc: &value.hdr },
            tt_: value.hdr.tt | 1 << 6,
        }
    }
}

impl<D> From<&Thread<D>> for UnsafeValue<D> {
    #[inline(always)]
    fn from(value: &Thread<D>) -> Self {
        Self {
            value_: UntaggedValue { gc: &value.hdr },
            tt_: 8 | 0 << 4 | 1 << 6,
        }
    }
}

impl<D> From<Ref<Thread<D>, D>> for UnsafeValue<D> {
    fn from(value: Ref<Thread<D>, D>) -> Self {
        Self {
            value_: UntaggedValue { gc: &value.hdr },
            tt_: 8 | 0 << 4 | 1 << 6,
        }
    }
}

impl<D> From<Value<D>> for UnsafeValue<D> {
    fn from(value: Value<D>) -> Self {
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

impl<'a, 'b, D> From<&Arg<'a, 'b, D>> for UnsafeValue<D> {
    #[inline(always)]
    fn from(value: &Arg<'a, 'b, D>) -> Self {
        let v = value.get_raw_or_null();

        if v.is_null() {
            Self::from(Nil)
        } else {
            unsafe { v.read() }
        }
    }
}

impl<'a, 'b, D> From<Arg<'a, 'b, D>> for UnsafeValue<D> {
    #[inline(always)]
    fn from(value: Arg<'a, 'b, D>) -> Self {
        Self::from(&value)
    }
}

#[repr(C)]
pub union UntaggedValue<D> {
    pub gc: *const Object<D>,
    pub f: for<'a> fn(
        Context<'a, D, Args>,
    ) -> Result<Context<'a, D, Ret>, Box<dyn core::error::Error>>,
    pub i: i64,
    pub n: f64,
}

impl<D> Clone for UntaggedValue<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for UntaggedValue<D> {}
