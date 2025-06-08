pub(crate) use self::legacy::*;
pub(crate) use self::node::*;

use crate::gc::luaC_barrierback_;
use crate::ltm::TM_EQ;
use crate::{Object, UnsafeValue};
use core::cell::Cell;
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

        // Set.
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
}

/// Represents an error when the operation on a table fails.
#[derive(Debug, Error)]
pub enum TableError {
    #[error("key is nil")]
    NilKey,

    #[error("key is NaN")]
    NanKey,
}
