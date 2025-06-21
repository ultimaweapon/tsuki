pub(crate) use self::legacy::*;
pub(crate) use self::node::*;

use crate::gc::{luaC_barrier_, luaC_barrierback_};
use crate::ltm::{TM_EQ, TM_GC, luaT_gettm};
use crate::{Lua, Object, Ref, Str, UnsafeValue};
use core::alloc::Layout;
use core::cell::Cell;
use core::ptr::{addr_of_mut, null, null_mut};
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
    pub(crate) metatable: Cell<*const Table>,
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

    /// Returns metatable for this table.
    pub fn metatable(&self) -> Option<Ref<Table>> {
        let v = self.metatable.get();

        match v.is_null() {
            true => None,
            false => Some(unsafe { Ref::new(self.hdr.global().to_rc(), v) }),
        }
    }

    /// Set metatable for this table.
    ///
    /// Use [`Self::remove_metatable()`] if you want to remove the metatable.
    ///
    /// # Panics
    /// If `v` come from different [Lua](crate::Lua) instance.
    pub fn set_metatable(&self, v: &Table) -> Result<(), MetatableError> {
        // Check if metatable come from the same Lua.
        if v.hdr.global != self.hdr.global {
            panic!("attempt to set metatable created from a different Lua");
        }

        // Prevent __gc metamethod.
        if v.flags.get() & 1 << TM_GC == 0 {
            let name = self.hdr.global().tmname[TM_GC as usize].get();

            if unsafe { !luaT_gettm(v, TM_GC, name).is_null() } {
                return Err(MetatableError::HasGc);
            }
        }

        // Set metatable.
        self.metatable.set(v);

        if self.hdr.marked.get() & 1 << 5 != 0 && v.hdr.marked.get() & (1 << 3 | 1 << 4) != 0 {
            unsafe { luaC_barrier_(self.hdr.global, &self.hdr, &v.hdr) };
        }

        Ok(())
    }

    /// Removes metatable from this table.
    #[inline(always)]
    pub fn remove_metatable(&self) {
        self.metatable.set(null());
    }

    /// Returns `true` if the table contains a value for the specified key.
    ///
    /// # Panics
    /// If `k` come from different [Lua](crate::Lua) instance.
    pub fn contains_key(&self, k: impl Into<UnsafeValue>) -> bool {
        // Check if key come from the same Lua.
        let k = k.into();

        if unsafe { (k.tt_ & 1 << 6 != 0) && (*k.value_.gc).global != self.hdr.global } {
            panic!("attempt to check the table with key from a different Lua");
        }

        // Get value.
        let v = unsafe { luaH_get(self, &k) };

        unsafe { (*v).tt_ & 0xf != 0 }
    }

    /// Returns `true` if the table contains a value for the specified key.
    pub fn contains_str_key(&self, k: impl AsRef<[u8]>) -> bool {
        let k = unsafe { Str::from_bytes(self.hdr.global, k) };
        let k = unsafe { UnsafeValue::from_obj(k.cast()) };
        let v = unsafe { luaH_get(self, &k) };

        unsafe { (*v).tt_ & 0xf != 0 }
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
        let k = unsafe { Str::from_str(self.hdr.global, k) };
        let k = unsafe { UnsafeValue::from_obj(k.cast()) };

        unsafe { self.set(k, v).unwrap_unchecked() };
    }

    /// Inserts a value with string key into this table.
    ///
    /// # Safety
    /// `v` must come from the same [Lua](crate::Lua) instance.
    pub unsafe fn set_str_key_unchecked(&self, k: impl AsRef<str>, v: impl Into<UnsafeValue>) {
        let k = unsafe { Str::from_str(self.hdr.global, k) };
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

/// Error when attempt to set an invalid metatable.
#[derive(Debug, Error)]
pub enum MetatableError {
    #[error("the metatable contains __gc metamethod, which Tsuki does not support")]
    HasGc,
}
