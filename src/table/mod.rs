pub(crate) use self::node::*;

use crate::{Object, TValue};
use core::cell::Cell;
use thiserror::Error;

mod node;

/// Lua table.
#[repr(C)]
pub struct Table {
    pub(crate) hdr: Object,
    pub(crate) flags: Cell<u8>,
    pub(crate) lsizenode: Cell<u8>,
    pub(crate) alimit: Cell<libc::c_uint>,
    pub(crate) array: Cell<*mut TValue>,
    pub(crate) node: Cell<*mut Node>,
    pub(crate) lastfree: Cell<*mut Node>,
    pub(crate) metatable: Cell<*mut Table>,
}

/// Represents an error when the operation on a table fails.
#[derive(Debug, Error)]
pub enum TableError {
    #[error("key is nil")]
    NilKey,

    #[error("key is NaN")]
    NanKey,
}
