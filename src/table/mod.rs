pub(crate) use self::legacy::*;
pub(crate) use self::node::*;

use crate::gc::luaC_barrierback_;
use crate::ltm::TM_EQ;
use crate::{Lua, Object, Str, UnsafeValue};
use core::alloc::Layout;
use core::cell::Cell;
use core::ptr::{addr_of_mut, null_mut};
use thiserror::Error;

mod legacy;
mod node;

/// Lua table.
#[repr(C)]
pub struct Table {
    pub(crate) hdr: Object,
    pub(crate) flags: Cell<u8>,
    pub(crate) lsizenode: Cell<u8>,
    pub(crate) alimit: Cell<libc::c_uint>,
    pub(crate) array: Cell<*mut UnsafeValue>,
    pub(crate) node: Cell<*mut Node>,
    pub(crate) lastfree: Cell<*mut Node>,
    pub(crate) metatable: Cell<*mut Table>,
}

impl Table {
    pub(crate) unsafe fn new(g: *const Lua) -> *const Self {
        let layout = Layout::new::<Self>();
        let o = unsafe { Object::new(g, 5 | 0 << 4, layout).cast::<Self>() };

        unsafe {
            addr_of_mut!((*o).flags).write(Cell::new(!(!(0 as libc::c_uint) << TM_EQ + 1) as u8))
        };
        unsafe { addr_of_mut!((*o).lsizenode).write(Cell::new(0)) };
        unsafe { addr_of_mut!((*o).alimit).write(Cell::new(0)) };
        unsafe { addr_of_mut!((*o).array).write(Cell::new(null_mut())) };
        unsafe { addr_of_mut!((*o).node).write(Cell::new(&raw mut dummynode_)) };
        unsafe { addr_of_mut!((*o).lastfree).write(Cell::new(null_mut())) };
        unsafe { addr_of_mut!((*o).metatable).write(Cell::new(null_mut())) };

        o
    }

    /// Inserts a key-value pair into this table.
    ///
    /// # Panics
    /// If `k` or `v` come from different [Lua](crate::Lua) instance.
    pub fn set(
        &self,
        k: impl Into<UnsafeValue>,
        v: impl Into<UnsafeValue>,
    ) -> Result<(), TableError> {
        // Check if key come from the same Lua.
        let k = k.into();

        if unsafe { (k.tt_ & 1 << 6 != 0) && (*k.value_.gc).global != self.hdr.global } {
            panic!("attempt to set the table with key from a different Lua");
        }

        // Check if value come from the same Lua.
        let v = v.into();

        if unsafe { (v.tt_ & 1 << 6 != 0) && (*v.value_.gc).global != self.hdr.global } {
            panic!("attempt to set the table with value from a different Lua");
        }

        unsafe { self.set_unchecked(k, v) }
    }

    /// # Safety
    /// `k` and `v` must come from the same [Lua](crate::Lua) instance.
    pub unsafe fn set_unchecked(
        &self,
        k: impl Into<UnsafeValue>,
        v: impl Into<UnsafeValue>,
    ) -> Result<(), TableError> {
        let k = k.into();
        let v = v.into();

        unsafe { luaH_set(self, &k, &v)? };

        self.flags
            .set((self.flags.get() as libc::c_uint & !!(!(0 as libc::c_uint) << TM_EQ + 1)) as u8);

        if (v.tt_ & 1 << 6 != 0) && (self.hdr.marked.get() & 1 << 5 != 0) {
            if unsafe { (*v.value_.gc).marked.get() & (1 << 3 | 1 << 4) != 0 } {
                unsafe { luaC_barrierback_(&self.hdr) };
            }
        }

        Ok(())
    }

    /// Inserts a value with string key into this table.
    ///
    /// # Panics
    /// If `v` come from different [Lua](crate::Lua) instance.
    pub fn set_str_key(&self, k: impl AsRef<str>, v: impl Into<UnsafeValue>) {
        let k = unsafe { Str::new(self.hdr.global, k.as_ref()) };
        let k = unsafe { UnsafeValue::from_obj(k.cast()) };

        unsafe { self.set(k, v).unwrap_unchecked() };
    }

    /// Inserts a value with string key into this table.
    ///
    /// # Safety
    /// `v` must come from the same [Lua](crate::Lua) instance.
    pub unsafe fn set_str_key_unchecked(&self, k: impl AsRef<str>, v: impl Into<UnsafeValue>) {
        let k = unsafe { Str::new(self.hdr.global, k.as_ref()) };
        let k = unsafe { UnsafeValue::from_obj(k.cast()) };

        unsafe { self.set_unchecked(k, v).unwrap_unchecked() };
    }
}

/// Represents an error when the operation on a table fails.
#[derive(Debug, Error)]
pub enum TableError {
    #[error("key is nil")]
    NilKey,

    #[error("key is NaN")]
    NanKey,
}
