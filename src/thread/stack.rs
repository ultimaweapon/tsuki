use crate::lobject::StackValue;
use crate::value::UnsafeValue;
use crate::{Nil, Table};
use core::cell::Cell;

/// Pointer to an item in the stack.
pub(crate) struct StackPtr<D>(Cell<*mut StackValue<D>>);

impl<D> StackPtr<D> {
    #[inline(always)]
    pub unsafe fn new(v: *mut StackValue<D>) -> Self {
        Self(Cell::new(v))
    }

    #[inline(always)]
    pub unsafe fn add(&self, n: usize) {
        unsafe { self.0.set(self.0.get().add(n)) };
    }

    #[inline(always)]
    pub unsafe fn sub(&self, n: usize) {
        unsafe { self.0.set(self.0.get().sub(n)) };
    }

    #[inline(always)]
    pub fn get(&self) -> *mut StackValue<D> {
        self.0.get()
    }

    #[inline(always)]
    pub unsafe fn set(&self, v: *mut StackValue<D>) {
        self.0.set(v);
    }

    #[inline(always)]
    pub unsafe fn read(&self, i: isize) -> UnsafeValue<D> {
        unsafe { self.0.get().offset(i).read().val }
    }

    #[inline(always)]
    pub unsafe fn write(&self, val: UnsafeValue<D>) {
        unsafe { self.0.get().write(StackValue { val }) };
    }

    #[inline(always)]
    pub fn write_nil(&self) {
        unsafe { self.write(Nil.into()) };
    }

    #[inline(always)]
    pub fn write_table(&self, t: &Table<D>) {
        unsafe { self.write(t.into()) };
    }
}
