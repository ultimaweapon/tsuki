use crate::value::{UnsafeValue, UntaggedValue};
use crate::{Nil, Table};
use core::cell::Cell;

/// Each item in the stack.
///
/// The value of this type must be able to copy directly to [UnsafeValue].
#[repr(C)]
pub(crate) struct StackValue<A> {
    pub tt_: u8,
    pub tbcdelta: u16,
    pub value_: UntaggedValue<A>,
}

impl<A> Clone for StackValue<A> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A> Copy for StackValue<A> {}

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
    pub unsafe fn copy(&self, from: isize, to: isize) {
        let s = self.0.get();
        let from = unsafe { s.offset(from) };
        let to = unsafe { s.offset(to) };

        unsafe { (*to).tt_ = (*from).tt_ };
        unsafe { (*to).value_ = (*from).value_ };
    }

    #[inline(always)]
    pub unsafe fn read(&self, i: isize) -> UnsafeValue<D> {
        unsafe { self.0.get().offset(i).read().into() }
    }

    #[inline(always)]
    pub unsafe fn write(&self, val: UnsafeValue<D>) {
        let ptr = self.0.get();

        unsafe { (*ptr).tt_ = val.tt_ };
        unsafe { (*ptr).value_ = val.value_ };
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
