use super::Thread;
use crate::value::UnsafeValue;
use crate::{Args, Context, Fp, Nil, Object, Ret, Str, Table};
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::marker::PhantomData;

/// Implementation of [`Inputs`] which size does not known at compile time.
///
/// The order of arguments is the same as push order (e.g. first argument pushed first).
#[derive(Default)]
pub struct DynamicInputs<'a> {
    list: Vec<UnsafeValue>,
    phantom: PhantomData<&'a Object>,
}

impl<'a> DynamicInputs<'a> {
    /// Constructs a new, empty [`DynamicInputs`] with at least the specified capacity.
    #[inline(always)]
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            list: Vec::with_capacity(cap),
            phantom: PhantomData,
        }
    }

    /// Push a `nil` value.
    #[inline(always)]
    pub fn push_nil(&mut self) {
        self.list.push(UnsafeValue::from(Nil));
    }

    /// Push a `boolean` value.
    #[inline(always)]
    pub fn push_bool(&mut self, v: bool) {
        self.list.push(UnsafeValue::from(v));
    }

    /// Push an `integer` value.
    #[inline(always)]
    pub fn push_int(&mut self, v: i64) {
        self.list.push(UnsafeValue::from(v));
    }

    /// Push a `float` value.
    #[inline(always)]
    pub fn push_num(&mut self, v: f64) {
        self.list.push(UnsafeValue::from(v));
    }

    /// Push a `string` value.
    #[inline(always)]
    pub fn push_str(&mut self, v: &'a Str) {
        self.list.push(UnsafeValue::from(v));
    }

    /// Push a `table` value.
    #[inline(always)]
    pub fn push_table(&mut self, v: &'a Table) {
        self.list.push(UnsafeValue::from(v));
    }

    /// Push a Rust function.
    #[inline(always)]
    pub fn push_fp(
        &mut self,
        v: fn(Context<Args>) -> Result<Context<Ret>, Box<dyn core::error::Error>>,
    ) {
        self.list.push(UnsafeValue::from(Fp(v)));
    }
}

unsafe impl<'a> Inputs for DynamicInputs<'a> {
    #[inline(always)]
    fn len(&self) -> usize {
        self.list.len()
    }

    unsafe fn push_to(self, th: &Thread) {
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
pub unsafe trait Inputs {
    fn len(&self) -> usize;

    /// # Panics
    /// If any argument does not come from the same [Lua](crate::Lua) as `th`.
    ///
    /// # Safety
    /// The stack of `th` must be able to push more [`Inputs::len()`] items.
    unsafe fn push_to(self, th: &Thread);
}

unsafe impl Inputs for () {
    #[inline(always)]
    fn len(&self) -> usize {
        0
    }

    #[inline(always)]
    unsafe fn push_to(self, _: &Thread) {}
}

unsafe impl<T: Into<UnsafeValue>> Inputs for T {
    #[inline(always)]
    fn len(&self) -> usize {
        1
    }

    unsafe fn push_to(self, th: &Thread) {
        let v = self.into();

        if unsafe { (v.tt_ & 1 << 6 != 0) && (*v.value_.gc).global != th.hdr.global } {
            panic!("argument #0 come from a different Lua");
        }

        unsafe { th.top.write(v) };
        unsafe { th.top.add(1) };
    }
}
