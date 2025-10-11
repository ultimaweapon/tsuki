use crate::value::{UnsafeValue, UntaggedValue};
use crate::{LuaFn, Ref, Str, Table, Thread, UserData};

/// This type **MUST** never exposed to outside.
pub trait RegValue<'a, A> {
    type Out;

    fn into_unsafe(self) -> UnsafeValue<A>;
    unsafe fn from_unsafe(v: *const UnsafeValue<A>) -> Self::Out;
}

impl<'a, A> RegValue<'a, A> for bool {
    type Out = bool;

    #[inline(always)]
    fn into_unsafe(self) -> UnsafeValue<A> {
        self.into()
    }

    #[inline(always)]
    unsafe fn from_unsafe(v: *const UnsafeValue<A>) -> Self::Out {
        unsafe { (*v).tt_ & 0x3f == 1 | 1 << 4 }
    }
}

impl<'a, A> RegValue<'a, A> for i8 {
    type Out = i8;

    #[inline(always)]
    fn into_unsafe(self) -> UnsafeValue<A> {
        self.into()
    }

    #[inline(always)]
    unsafe fn from_unsafe(v: *const UnsafeValue<A>) -> Self::Out {
        unsafe { (*v).value_.i as i8 }
    }
}

impl<'a, A> RegValue<'a, A> for i16 {
    type Out = i16;

    #[inline(always)]
    fn into_unsafe(self) -> UnsafeValue<A> {
        self.into()
    }

    unsafe fn from_unsafe(v: *const UnsafeValue<A>) -> Self::Out {
        unsafe { (*v).value_.i as i16 }
    }
}

impl<'a, A> RegValue<'a, A> for i32 {
    type Out = i32;

    #[inline(always)]
    fn into_unsafe(self) -> UnsafeValue<A> {
        self.into()
    }

    #[inline(always)]
    unsafe fn from_unsafe(v: *const UnsafeValue<A>) -> Self::Out {
        unsafe { (*v).value_.i as i32 }
    }
}

impl<'a, A> RegValue<'a, A> for i64 {
    type Out = i64;

    #[inline(always)]
    fn into_unsafe(self) -> UnsafeValue<A> {
        self.into()
    }

    #[inline(always)]
    unsafe fn from_unsafe(v: *const UnsafeValue<A>) -> Self::Out {
        unsafe { (*v).value_.i }
    }
}

impl<'a, A> RegValue<'a, A> for u8 {
    type Out = u8;

    #[inline(always)]
    fn into_unsafe(self) -> UnsafeValue<A> {
        self.into()
    }

    #[inline(always)]
    unsafe fn from_unsafe(v: *const UnsafeValue<A>) -> Self::Out {
        unsafe { (*v).value_.i as u8 }
    }
}

impl<'a, A> RegValue<'a, A> for u16 {
    type Out = u16;

    #[inline(always)]
    fn into_unsafe(self) -> UnsafeValue<A> {
        self.into()
    }

    #[inline(always)]
    unsafe fn from_unsafe(v: *const UnsafeValue<A>) -> Self::Out {
        unsafe { (*v).value_.i as u16 }
    }
}

impl<'a, A> RegValue<'a, A> for u32 {
    type Out = u32;

    #[inline(always)]
    fn into_unsafe(self) -> UnsafeValue<A> {
        self.into()
    }

    #[inline(always)]
    unsafe fn from_unsafe(v: *const UnsafeValue<A>) -> Self::Out {
        unsafe { (*v).value_.i as u32 }
    }
}

impl<'a, A> RegValue<'a, A> for u64 {
    type Out = u64;

    #[inline(always)]
    fn into_unsafe(self) -> UnsafeValue<A> {
        UnsafeValue {
            tt_: 3 | 0 << 4,
            value_: UntaggedValue { i: self as i64 },
        }
    }

    #[inline(always)]
    unsafe fn from_unsafe(v: *const UnsafeValue<A>) -> Self::Out {
        unsafe { (*v).value_.i as u64 }
    }
}

impl<'a, A> RegValue<'a, A> for f32 {
    type Out = f32;

    #[inline(always)]
    fn into_unsafe(self) -> UnsafeValue<A> {
        self.into()
    }

    #[inline(always)]
    unsafe fn from_unsafe(v: *const UnsafeValue<A>) -> Self::Out {
        unsafe { (*v).value_.n as f32 }
    }
}

impl<'a, A> RegValue<'a, A> for f64 {
    type Out = f64;

    #[inline(always)]
    fn into_unsafe(self) -> UnsafeValue<A> {
        self.into()
    }

    #[inline(always)]
    unsafe fn from_unsafe(v: *const UnsafeValue<A>) -> Self::Out {
        unsafe { (*v).value_.n }
    }
}

impl<'a, A> RegValue<'a, A> for Ref<'a, Str<A>> {
    type Out = Ref<'a, Str<A>>;

    #[inline(always)]
    fn into_unsafe(self) -> UnsafeValue<A> {
        self.into()
    }

    #[inline(always)]
    unsafe fn from_unsafe(v: *const UnsafeValue<A>) -> Self::Out {
        unsafe { Ref::new((*v).value_.gc.cast()) }
    }
}

impl<'a, A> RegValue<'a, A> for Ref<'a, Table<A>> {
    type Out = Ref<'a, Table<A>>;

    #[inline(always)]
    fn into_unsafe(self) -> UnsafeValue<A> {
        self.into()
    }

    #[inline(always)]
    unsafe fn from_unsafe(v: *const UnsafeValue<A>) -> Self::Out {
        unsafe { Ref::new((*v).value_.gc.cast()) }
    }
}

impl<'a, A> RegValue<'a, A> for Ref<'a, LuaFn<A>> {
    type Out = Ref<'a, LuaFn<A>>;

    #[inline(always)]
    fn into_unsafe(self) -> UnsafeValue<A> {
        self.into()
    }

    #[inline(always)]
    unsafe fn from_unsafe(v: *const UnsafeValue<A>) -> Self::Out {
        unsafe { Ref::new((*v).value_.gc.cast()) }
    }
}

impl<'a, A, T> RegValue<'a, A> for Ref<'a, UserData<A, T>> {
    type Out = Ref<'a, UserData<A, T>>;

    #[inline(always)]
    fn into_unsafe(self) -> UnsafeValue<A> {
        self.into()
    }

    #[inline(always)]
    unsafe fn from_unsafe(v: *const UnsafeValue<A>) -> Self::Out {
        unsafe { Ref::new((*v).value_.gc.cast()) }
    }
}

impl<'a, A> RegValue<'a, A> for Ref<'a, Thread<A>> {
    type Out = Ref<'a, Thread<A>>;

    #[inline(always)]
    fn into_unsafe(self) -> UnsafeValue<A> {
        self.into()
    }

    #[inline(always)]
    unsafe fn from_unsafe(v: *const UnsafeValue<A>) -> Self::Out {
        unsafe { Ref::new((*v).value_.gc.cast()) }
    }
}
