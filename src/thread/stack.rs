use crate::Table;
use crate::lobject::StackValue;
use crate::value::{UnsafeValue, UntaggedValue};
use core::cell::Cell;
use core::mem::zeroed;

/// Pointer to an item in the stack.
pub(crate) struct StackPtr(Cell<*mut StackValue>);

impl StackPtr {
    #[inline(always)]
    pub unsafe fn new(v: *mut StackValue) -> Self {
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
    pub fn get(&self) -> *mut StackValue {
        self.0.get()
    }

    #[inline(always)]
    pub unsafe fn set(&self, v: *mut StackValue) {
        self.0.set(v);
    }

    #[inline(always)]
    pub unsafe fn read(&self, i: isize) -> UnsafeValue {
        unsafe { self.0.get().offset(i).read().val }
    }

    #[inline(always)]
    pub unsafe fn write(&self, val: UnsafeValue) {
        unsafe { self.0.get().write(StackValue { val }) };
    }

    #[inline(always)]
    pub fn write_nil(&self) {
        let v = UnsafeValue {
            value_: unsafe { zeroed() },
            tt_: 0 | 0 << 4,
        };

        unsafe { self.write(v) };
    }

    #[inline(always)]
    pub fn write_table(&self, t: &Table) {
        let v = UnsafeValue {
            value_: UntaggedValue { gc: &t.hdr },
            tt_: 5 | 0 << 4 | 1 << 6,
        };

        unsafe { self.write(v) };
    }
}
