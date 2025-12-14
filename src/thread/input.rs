use super::Thread;
use crate::context::Arg;
use crate::value::UnsafeValue;
use crate::{Fp, Nil, Object, Str, Table, Value};
use alloc::vec::Vec;
use core::marker::PhantomData;

/// Implementation of [Inputs] which size does not known at compile time.
///
/// This struct is more efficient than a [Vec] of [Value] since it store [UnsafeValue] directly.
///
/// The order of arguments is the same as push order (e.g. first argument pushed first).
pub struct DynamicInputs<'a, A> {
    list: Vec<UnsafeValue<A>>,
    phantom: PhantomData<&'a Object<A>>,
}

impl<'a, D> DynamicInputs<'a, D> {
    /// Constructs a new, empty [DynamicInputs] with at least the specified capacity.
    #[inline]
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            list: Vec::with_capacity(cap),
            phantom: PhantomData,
        }
    }

    /// Push a `nil` value.
    #[inline]
    pub fn push_nil(&mut self) {
        self.list.push(UnsafeValue::from(Nil));
    }

    /// Push a `boolean` value.
    #[inline]
    pub fn push_bool(&mut self, v: bool) {
        self.list.push(UnsafeValue::from(v));
    }

    /// Push an `integer` value.
    #[inline]
    pub fn push_int(&mut self, v: i64) {
        self.list.push(UnsafeValue::from(v));
    }

    /// Push a `float` value.
    #[inline]
    pub fn push_float(&mut self, v: f64) {
        self.list.push(UnsafeValue::from(v));
    }

    /// Push a `string` value.
    #[inline]
    pub fn push_str(&mut self, v: &'a Str<D>) {
        self.list.push(UnsafeValue::from(v));
    }

    /// Push a `table` value.
    #[inline]
    pub fn push_table(&mut self, v: &'a Table<D>) {
        self.list.push(UnsafeValue::from(v));
    }

    /// Push a Rust function.
    #[inline]
    pub fn push_fp(&mut self, v: Fp<D>) {
        self.list.push(UnsafeValue::from(v));
    }

    /// Push argument passed to Rust function.
    #[inline]
    pub fn push_arg<'b>(&mut self, v: Arg<'a, 'b, D>)
    where
        'b: 'a,
    {
        self.list.push(v.into());
    }
}

impl<'a, D> Default for DynamicInputs<'a, D> {
    fn default() -> Self {
        Self {
            list: Vec::new(),
            phantom: PhantomData,
        }
    }
}

unsafe impl<'a, D> Inputs<D> for DynamicInputs<'a, D> {
    #[inline(always)]
    fn len(&self) -> usize {
        self.list.len()
    }

    unsafe fn push_to(self, th: &Thread<D>) {
        for (i, v) in self.list.into_iter().enumerate() {
            if unsafe { (v.tt_ & 1 << 6 != 0) && (*v.value_.gc).global != th.hdr.global } {
                panic!("argument #{i} come from a different Lua");
            }

            unsafe { th.top.write(v) };
            unsafe { th.top.add(1) };
        }
    }
}

/// Arguments to invoke a callable value.
///
/// # Safety
/// The value returned from [`Inputs::len()`] must be exactly the same as the values pushed to the
/// thread by [`Inputs::push_to()`].
pub unsafe trait Inputs<D> {
    fn len(&self) -> usize;

    /// # Panics
    /// If any argument does not come from the same [Lua](crate::Lua) as `th`.
    ///
    /// # Safety
    /// The stack of `th` must be able to push more [`Inputs::len()`] items.
    unsafe fn push_to(self, th: &Thread<D>);
}

unsafe impl<D> Inputs<D> for () {
    #[inline(always)]
    fn len(&self) -> usize {
        0
    }

    #[inline(always)]
    unsafe fn push_to(self, _: &Thread<D>) {}
}

unsafe impl<T: Into<UnsafeValue<D>>, D> Inputs<D> for T {
    #[inline(always)]
    fn len(&self) -> usize {
        1
    }

    unsafe fn push_to(self, th: &Thread<D>) {
        let v = self.into();

        if unsafe { (v.tt_ & 1 << 6 != 0) && (*v.value_.gc).global != th.hdr.global } {
            panic!("argument #0 come from a different Lua");
        }

        unsafe { th.top.write(v) };
        unsafe { th.top.add(1) };
    }
}

unsafe impl<'a, A> Inputs<A> for Vec<Value<'a, A>> {
    #[inline(always)]
    fn len(&self) -> usize {
        self.len()
    }

    unsafe fn push_to(self, th: &Thread<A>) {
        for (i, v) in self.into_iter().enumerate() {
            let v = UnsafeValue::from(v);

            if unsafe { (v.tt_ & 1 << 6 != 0) && (*v.value_.gc).global != th.hdr.global } {
                panic!("argument #{i} was created from different Lua");
            }

            unsafe { th.top.write(v) };
            unsafe { th.top.add(1) };
        }
    }
}

macro_rules! count {
    () => (0usize);
    ($x:tt,$($xs:tt)*) => (1usize + count!($($xs)*));
}

macro_rules! impl_tuple {
    ($($idx:tt $name:tt),+) => {
        unsafe impl<Z, $($name,)+> Inputs<Z> for ($($name,)+)
        where
            $($name: Into<UnsafeValue<Z>>,)+
        {
            #[inline(always)]
            fn len(&self) -> usize {
                count!($($name,)+)
            }

            unsafe fn push_to(self, th: &Thread<Z>) {
                $({
                    let v = self.$idx.into();

                    if unsafe { (v.tt_ & 1 << 6 != 0) && (*v.value_.gc).global != th.hdr.global } {
                        panic!("argument #{} come from a different Lua", $idx);
                    }

                    unsafe { th.top.write(v) };
                    unsafe { th.top.add(1) };
                })+
            }
        }
    };
}

impl_tuple!(0 A, 1 B);
impl_tuple!(0 A, 1 B, 2 C);
impl_tuple!(0 A, 1 B, 2 C, 3 D);
impl_tuple!(0 A, 1 B, 2 C, 3 D, 4 E);
impl_tuple!(0 A, 1 B, 2 C, 3 D, 4 E, 5 F);
impl_tuple!(0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G);
impl_tuple!(0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G, 7 H);
impl_tuple!(0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G, 7 H, 8 I);
impl_tuple!(0 A, 1 B, 2 C, 3 D, 4 E, 5 F, 6 G, 7 H, 8 I, 9 J);
