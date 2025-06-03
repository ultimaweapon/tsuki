use crate::LuaClosure;
use crate::lobject::{StackValue, TValue, Value};
use std::cell::Cell;
use std::mem::zeroed;

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
    pub unsafe fn write(&self, val: TValue) {
        unsafe { self.0.get().write(StackValue { val }) };
    }

    #[inline(always)]
    pub fn write_nil(&self) {
        let v = TValue {
            value_: unsafe { zeroed() },
            tt_: 0 | 0 << 4,
        };

        unsafe { self.write(v) };
    }

    #[inline(always)]
    pub fn write_lua(&self, f: &LuaClosure) {
        let v = TValue {
            value_: Value { gc: &f.hdr },
            tt_: 6 | 0 << 4 | 1 << 6,
        };

        unsafe { self.write(v) };
    }
}
