use crate::value::UnsafeValue;
use crate::{Dynamic, LuaFn, Ref, Str, Table, Thread, UserData, Value};

/// This type **MUST** never exposed to outside.
pub trait CollectionValue<A> {
    type In<'a>: Into<UnsafeValue<A>>
    where
        A: 'a;
    type Out<'a>
    where
        A: 'a;

    unsafe fn from_collection<'a>(v: *const UnsafeValue<A>) -> Self::Out<'a>
    where
        A: 'a;
}

impl<A> CollectionValue<A> for Str<A> {
    type In<'a>
        = &'a Self
    where
        A: 'a;
    type Out<'a>
        = Ref<'a, Self>
    where
        A: 'a;

    #[inline(always)]
    unsafe fn from_collection<'a>(v: *const UnsafeValue<A>) -> Self::Out<'a>
    where
        A: 'a,
    {
        unsafe { Ref::new((*v).value_.gc.cast()) }
    }
}

impl<A> CollectionValue<A> for Table<A> {
    type In<'a>
        = &'a Self
    where
        A: 'a;
    type Out<'a>
        = Ref<'a, Self>
    where
        A: 'a;

    #[inline(always)]
    unsafe fn from_collection<'a>(v: *const UnsafeValue<A>) -> Self::Out<'a>
    where
        A: 'a,
    {
        unsafe { Ref::new((*v).value_.gc.cast()) }
    }
}

impl<A> CollectionValue<A> for LuaFn<A> {
    type In<'a>
        = &'a Self
    where
        A: 'a;
    type Out<'a>
        = Ref<'a, Self>
    where
        A: 'a;

    #[inline(always)]
    unsafe fn from_collection<'a>(v: *const UnsafeValue<A>) -> Self::Out<'a>
    where
        A: 'a,
    {
        unsafe { Ref::new((*v).value_.gc.cast()) }
    }
}

impl<A, T: ?Sized + 'static> CollectionValue<A> for UserData<A, T> {
    type In<'a>
        = &'a Self
    where
        A: 'a;
    type Out<'a>
        = Ref<'a, Self>
    where
        A: 'a;

    #[inline(always)]
    unsafe fn from_collection<'a>(v: *const UnsafeValue<A>) -> Self::Out<'a>
    where
        A: 'a,
    {
        unsafe { Ref::new((*v).value_.gc.cast()) }
    }
}

impl<A> CollectionValue<A> for Thread<A> {
    type In<'a>
        = &'a Self
    where
        A: 'a;
    type Out<'a>
        = Ref<'a, Self>
    where
        A: 'a;

    #[inline(always)]
    unsafe fn from_collection<'a>(v: *const UnsafeValue<A>) -> Self::Out<'a>
    where
        A: 'a,
    {
        unsafe { Ref::new((*v).value_.gc.cast()) }
    }
}

impl<A> CollectionValue<A> for Dynamic {
    type In<'a>
        = UnsafeValue<A>
    where
        A: 'a;
    type Out<'a>
        = Value<'a, A>
    where
        A: 'a;

    #[inline(always)]
    unsafe fn from_collection<'a>(v: *const UnsafeValue<A>) -> Self::Out<'a>
    where
        A: 'a,
    {
        unsafe { Value::from_unsafe(v) }
    }
}
