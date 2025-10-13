use crate::collections::{BTreeMap, CollectionValue};
use crate::gc::Object;
use crate::value::{UnsafeValue, UntaggedValue};
use crate::{LuaFn, Ref, Str, Table, Thread, UserData};
use core::any::Any;

/// This type **MUST** never exposed to outside.
///
/// # Safety
/// - [RegValue::into_unsafe()] must produce a valid value.
/// - [RegValue::from_unsafe()] must returns [Ref] for object.
pub unsafe trait RegValue<A> {
    type In<'a>
    where
        A: 'a;
    type Out<'a>
    where
        A: 'a;

    fn into_unsafe<'a>(v: Self::In<'a>) -> UnsafeValue<A>
    where
        A: 'a;
    unsafe fn from_unsafe<'a>(v: *const UnsafeValue<A>) -> Self::Out<'a>
    where
        A: 'a;
}

unsafe impl<A> RegValue<A> for bool {
    type In<'a>
        = bool
    where
        A: 'a;
    type Out<'a>
        = bool
    where
        A: 'a;

    #[inline(always)]
    fn into_unsafe<'a>(v: Self::In<'a>) -> UnsafeValue<A>
    where
        A: 'a,
    {
        v.into()
    }

    #[inline(always)]
    unsafe fn from_unsafe<'a>(v: *const UnsafeValue<A>) -> Self::Out<'a>
    where
        A: 'a,
    {
        unsafe { (*v).tt_ & 0x3f == 1 | 1 << 4 }
    }
}

unsafe impl<A> RegValue<A> for i8 {
    type In<'a>
        = i8
    where
        A: 'a;
    type Out<'a>
        = i8
    where
        A: 'a;

    #[inline(always)]
    fn into_unsafe<'a>(v: Self::In<'a>) -> UnsafeValue<A>
    where
        A: 'a,
    {
        v.into()
    }

    #[inline(always)]
    unsafe fn from_unsafe<'a>(v: *const UnsafeValue<A>) -> Self::Out<'a>
    where
        A: 'a,
    {
        unsafe { (*v).value_.i as i8 }
    }
}

unsafe impl<A> RegValue<A> for i16 {
    type In<'a>
        = i16
    where
        A: 'a;
    type Out<'a>
        = i16
    where
        A: 'a;

    #[inline(always)]
    fn into_unsafe<'a>(v: Self::In<'a>) -> UnsafeValue<A>
    where
        A: 'a,
    {
        v.into()
    }

    unsafe fn from_unsafe<'a>(v: *const UnsafeValue<A>) -> Self::Out<'a>
    where
        A: 'a,
    {
        unsafe { (*v).value_.i as i16 }
    }
}

unsafe impl<A> RegValue<A> for i32 {
    type In<'a>
        = i32
    where
        A: 'a;
    type Out<'a>
        = i32
    where
        A: 'a;

    #[inline(always)]
    fn into_unsafe<'a>(v: Self::In<'a>) -> UnsafeValue<A>
    where
        A: 'a,
    {
        v.into()
    }

    #[inline(always)]
    unsafe fn from_unsafe<'a>(v: *const UnsafeValue<A>) -> Self::Out<'a>
    where
        A: 'a,
    {
        unsafe { (*v).value_.i as i32 }
    }
}

unsafe impl<A> RegValue<A> for i64 {
    type In<'a>
        = i64
    where
        A: 'a;
    type Out<'a>
        = i64
    where
        A: 'a;

    #[inline(always)]
    fn into_unsafe<'a>(v: Self::In<'a>) -> UnsafeValue<A>
    where
        A: 'a,
    {
        v.into()
    }

    #[inline(always)]
    unsafe fn from_unsafe<'a>(v: *const UnsafeValue<A>) -> Self::Out<'a>
    where
        A: 'a,
    {
        unsafe { (*v).value_.i }
    }
}

unsafe impl<A> RegValue<A> for u8 {
    type In<'a>
        = u8
    where
        A: 'a;
    type Out<'a>
        = u8
    where
        A: 'a;

    #[inline(always)]
    fn into_unsafe<'a>(v: Self::In<'a>) -> UnsafeValue<A>
    where
        A: 'a,
    {
        v.into()
    }

    #[inline(always)]
    unsafe fn from_unsafe<'a>(v: *const UnsafeValue<A>) -> Self::Out<'a>
    where
        A: 'a,
    {
        unsafe { (*v).value_.i as u8 }
    }
}

unsafe impl<A> RegValue<A> for u16 {
    type In<'a>
        = u16
    where
        A: 'a;
    type Out<'a>
        = u16
    where
        A: 'a;

    #[inline(always)]
    fn into_unsafe<'a>(v: Self::In<'a>) -> UnsafeValue<A>
    where
        A: 'a,
    {
        v.into()
    }

    #[inline(always)]
    unsafe fn from_unsafe<'a>(v: *const UnsafeValue<A>) -> Self::Out<'a>
    where
        A: 'a,
    {
        unsafe { (*v).value_.i as u16 }
    }
}

unsafe impl<A> RegValue<A> for u32 {
    type In<'a>
        = u32
    where
        A: 'a;
    type Out<'a>
        = u32
    where
        A: 'a;

    #[inline(always)]
    fn into_unsafe<'a>(v: Self::In<'a>) -> UnsafeValue<A>
    where
        A: 'a,
    {
        v.into()
    }

    #[inline(always)]
    unsafe fn from_unsafe<'a>(v: *const UnsafeValue<A>) -> Self::Out<'a>
    where
        A: 'a,
    {
        unsafe { (*v).value_.i as u32 }
    }
}

unsafe impl<A> RegValue<A> for f32 {
    type In<'a>
        = f32
    where
        A: 'a;
    type Out<'a>
        = f32
    where
        A: 'a;

    #[inline(always)]
    fn into_unsafe<'a>(v: Self::In<'a>) -> UnsafeValue<A>
    where
        A: 'a,
    {
        v.into()
    }

    #[inline(always)]
    unsafe fn from_unsafe<'a>(v: *const UnsafeValue<A>) -> Self::Out<'a>
    where
        A: 'a,
    {
        unsafe { (*v).value_.n as f32 }
    }
}

unsafe impl<A> RegValue<A> for f64 {
    type In<'a>
        = f64
    where
        A: 'a;
    type Out<'a>
        = f64
    where
        A: 'a;

    #[inline(always)]
    fn into_unsafe<'a>(v: Self::In<'a>) -> UnsafeValue<A>
    where
        A: 'a,
    {
        v.into()
    }

    #[inline(always)]
    unsafe fn from_unsafe<'a>(v: *const UnsafeValue<A>) -> Self::Out<'a>
    where
        A: 'a,
    {
        unsafe { (*v).value_.n }
    }
}

unsafe impl<A> RegValue<A> for Str<A> {
    type In<'a>
        = &'a Str<A>
    where
        A: 'a;
    type Out<'a>
        = Ref<'a, Str<A>>
    where
        A: 'a;

    #[inline(always)]
    fn into_unsafe<'a>(v: Self::In<'a>) -> UnsafeValue<A>
    where
        A: 'a,
    {
        v.into()
    }

    #[inline(always)]
    unsafe fn from_unsafe<'a>(v: *const UnsafeValue<A>) -> Self::Out<'a>
    where
        A: 'a,
    {
        unsafe { Ref::new((*v).value_.gc.cast()) }
    }
}

unsafe impl<A> RegValue<A> for Table<A> {
    type In<'a>
        = &'a Table<A>
    where
        A: 'a;
    type Out<'a>
        = Ref<'a, Table<A>>
    where
        A: 'a;

    #[inline(always)]
    fn into_unsafe<'a>(v: Self::In<'a>) -> UnsafeValue<A>
    where
        A: 'a,
    {
        v.into()
    }

    #[inline(always)]
    unsafe fn from_unsafe<'a>(v: *const UnsafeValue<A>) -> Self::Out<'a>
    where
        A: 'a,
    {
        unsafe { Ref::new((*v).value_.gc.cast()) }
    }
}

unsafe impl<A> RegValue<A> for LuaFn<A> {
    type In<'a>
        = &'a LuaFn<A>
    where
        A: 'a;
    type Out<'a>
        = Ref<'a, LuaFn<A>>
    where
        A: 'a;

    #[inline(always)]
    fn into_unsafe<'a>(v: Self::In<'a>) -> UnsafeValue<A>
    where
        A: 'a,
    {
        v.into()
    }

    #[inline(always)]
    unsafe fn from_unsafe<'a>(v: *const UnsafeValue<A>) -> Self::Out<'a>
    where
        A: 'a,
    {
        unsafe { Ref::new((*v).value_.gc.cast()) }
    }
}

unsafe impl<A, T: Any> RegValue<A> for UserData<A, T> {
    type In<'a>
        = &'a UserData<A, T>
    where
        A: 'a;
    type Out<'a>
        = Ref<'a, UserData<A, T>>
    where
        A: 'a;

    #[inline(always)]
    fn into_unsafe<'a>(v: Self::In<'a>) -> UnsafeValue<A>
    where
        A: 'a,
    {
        v.into()
    }

    #[inline(always)]
    unsafe fn from_unsafe<'a>(v: *const UnsafeValue<A>) -> Self::Out<'a>
    where
        A: 'a,
    {
        unsafe { Ref::new((*v).value_.gc.cast()) }
    }
}

unsafe impl<A> RegValue<A> for Thread<A> {
    type In<'a>
        = &'a Thread<A>
    where
        A: 'a;
    type Out<'a>
        = Ref<'a, Thread<A>>
    where
        A: 'a;

    #[inline(always)]
    fn into_unsafe<'a>(v: Self::In<'a>) -> UnsafeValue<A>
    where
        A: 'a,
    {
        v.into()
    }

    #[inline(always)]
    unsafe fn from_unsafe<'a>(v: *const UnsafeValue<A>) -> Self::Out<'a>
    where
        A: 'a,
    {
        unsafe { Ref::new((*v).value_.gc.cast()) }
    }
}

unsafe impl<A, K, V> RegValue<A> for BTreeMap<A, K, V>
where
    K: 'static,
    V: CollectionValue<A> + 'static,
{
    type In<'a>
        = &'a Self
    where
        A: 'a;

    type Out<'a>
        = Ref<'a, Self>
    where
        A: 'a;

    #[inline(always)]
    fn into_unsafe<'a>(v: Self::In<'a>) -> UnsafeValue<A>
    where
        A: 'a,
    {
        UnsafeValue {
            tt_: 14 | 1 << 4 | 1 << 6,
            value_: UntaggedValue {
                gc: v as *const Self as *const Object<A>,
            },
        }
    }

    #[inline(always)]
    unsafe fn from_unsafe<'a>(v: *const UnsafeValue<A>) -> Self::Out<'a>
    where
        A: 'a,
    {
        unsafe { Ref::new((*v).value_.gc.cast()) }
    }
}
