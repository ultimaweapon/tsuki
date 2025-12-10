use crate::context::{Arg, Args, Context, Ret};
use crate::{
    AsyncFp, Float, Fp, LuaFn, Nil, Number, Object, Ref, StackValue, Str, Table, Thread, UserData,
    Value, Yield, YieldFp,
};
use alloc::boxed::Box;
use core::error::Error;
use core::mem::{ManuallyDrop, transmute, zeroed};
use core::pin::Pin;
use core::ptr::addr_of;

/// The outside **must** never be able to construct or have the value of this type.
///
/// Do not change layout of this type or add a new field to it since there are other structs that
/// assume the layout of this type and make use of padded space.
#[repr(C)]
pub struct UnsafeValue<A> {
    pub tt_: u8,
    pub value_: UntaggedValue<A>,
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

impl<D> Default for UnsafeValue<D> {
    #[inline(always)]
    fn default() -> Self {
        Self {
            value_: unsafe { zeroed() },
            tt_: 0,
        }
    }
}

impl<D> Clone for UnsafeValue<D> {
    #[inline(always)]
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

impl<A> From<Fp<A>> for UnsafeValue<A> {
    #[inline(always)]
    fn from(value: Fp<A>) -> Self {
        Self {
            value_: UntaggedValue { f: value.0 },
            tt_: 2 | 0 << 4,
        }
    }
}

impl<A> From<YieldFp<A>> for UnsafeValue<A> {
    #[inline(always)]
    fn from(value: YieldFp<A>) -> Self {
        Self {
            value_: UntaggedValue { y: value.0 },
            tt_: 2 | 1 << 4,
        }
    }
}

impl<A> From<AsyncFp<A>> for UnsafeValue<A> {
    #[inline(always)]
    fn from(value: AsyncFp<A>) -> Self {
        Self {
            value_: UntaggedValue { a: value.0 },
            tt_: 2 | 2 << 4,
        }
    }
}

impl<D> From<i8> for UnsafeValue<D> {
    #[inline(always)]
    fn from(value: i8) -> Self {
        Self {
            value_: UntaggedValue { i: value.into() },
            tt_: 3 | 0 << 4,
        }
    }
}

impl<D> From<i16> for UnsafeValue<D> {
    #[inline(always)]
    fn from(value: i16) -> Self {
        Self {
            value_: UntaggedValue { i: value.into() },
            tt_: 3 | 0 << 4,
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

impl<D> From<u16> for UnsafeValue<D> {
    #[inline(always)]
    fn from(value: u16) -> Self {
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

impl<A> From<f32> for UnsafeValue<A> {
    #[inline(always)]
    fn from(value: f32) -> Self {
        Float::from(value).into()
    }
}

impl<A> From<f64> for UnsafeValue<A> {
    #[inline(always)]
    fn from(value: f64) -> Self {
        Float::from(value).into()
    }
}

impl<A> From<Float> for UnsafeValue<A> {
    #[inline(always)]
    fn from(value: Float) -> Self {
        Self {
            value_: UntaggedValue { n: value },
            tt_: 3 | 1 << 4,
        }
    }
}

impl<A> From<Number> for UnsafeValue<A> {
    #[inline(always)]
    fn from(value: Number) -> Self {
        match value {
            Number::Int(v) => v.into(),
            Number::Float(v) => v.into(),
        }
    }
}

impl<D> From<&Str<D>> for UnsafeValue<D> {
    #[inline(always)]
    fn from(value: &Str<D>) -> Self {
        Self {
            value_: UntaggedValue { gc: &value.hdr },
            tt_: 4 | 0 << 4 | 1 << 6,
        }
    }
}

impl<'a, D> From<Ref<'a, Str<D>>> for UnsafeValue<D> {
    #[inline(always)]
    fn from(value: Ref<'a, Str<D>>) -> Self {
        Self {
            value_: UntaggedValue { gc: &value.hdr },
            tt_: 4 | 0 << 4 | 1 << 6,
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

impl<'a, D> From<Ref<'a, Table<D>>> for UnsafeValue<D> {
    #[inline(always)]
    fn from(value: Ref<'a, Table<D>>) -> Self {
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
            tt_: 6 | 0 << 4 | 1 << 6,
        }
    }
}

impl<'a, D> From<Ref<'a, LuaFn<D>>> for UnsafeValue<D> {
    #[inline(always)]
    fn from(value: Ref<'a, LuaFn<D>>) -> Self {
        Self {
            value_: UntaggedValue { gc: &value.hdr },
            tt_: value.hdr.tt | 1 << 6,
        }
    }
}

impl<D, T: ?Sized> From<&UserData<D, T>> for UnsafeValue<D> {
    #[inline(always)]
    fn from(value: &UserData<D, T>) -> Self {
        Self {
            value_: UntaggedValue { gc: &value.hdr },
            tt_: 7 | 0 << 4 | 1 << 6,
        }
    }
}

impl<'a, D, T: ?Sized> From<Ref<'a, UserData<D, T>>> for UnsafeValue<D> {
    #[inline(always)]
    fn from(value: Ref<'a, UserData<D, T>>) -> Self {
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

impl<'a, D> From<Ref<'a, Thread<D>>> for UnsafeValue<D> {
    #[inline(always)]
    fn from(value: Ref<'a, Thread<D>>) -> Self {
        Self {
            value_: UntaggedValue { gc: &value.hdr },
            tt_: 8 | 0 << 4 | 1 << 6,
        }
    }
}

impl<'a, A> From<Value<'a, A>> for UnsafeValue<A> {
    #[inline(never)]
    fn from(value: Value<'a, A>) -> Self {
        let value = ManuallyDrop::new(value);
        let v = addr_of!(value).cast::<u64>();
        let t = unsafe { v.read() as u8 };
        let v = unsafe { v.add(1).cast::<UntaggedValue<A>>() };

        if t & 1 << 6 != 0 {
            unsafe { (*(*v).gc).unref() };
        }

        Self {
            tt_: t,
            value_: unsafe { v.read() },
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

impl<A> From<StackValue<A>> for UnsafeValue<A> {
    #[inline(always)]
    fn from(value: StackValue<A>) -> Self {
        unsafe { transmute(value) }
    }
}

#[repr(C, align(8))]
pub union UntaggedValue<A> {
    pub gc: *const Object<A>,
    pub f: fn(Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn Error>>,
    pub y: fn(Yield<A>) -> Result<Context<A, Ret>, Box<dyn Error>>,
    pub a: fn(
        Context<A, Args>,
    ) -> Pin<Box<dyn Future<Output = Result<Context<A, Ret>, Box<dyn Error>>> + '_>>,
    pub i: i64,
    pub n: Float,
}

impl<A> Clone for UntaggedValue<A> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A> Copy for UntaggedValue<A> {}
