use crate::Object;
use crate::lobject::{Proto, UpVal};
use std::cell::Cell;

/// Lua function.
#[repr(C)]
pub struct LuaFn {
    pub(crate) hdr: Object,
    pub(crate) p: Cell<*mut Proto>,
    pub(crate) upvals: Box<[Cell<*mut UpVal>]>,
}
