use crate::Object;
use core::cell::{Cell, UnsafeCell};
use core::ffi::c_char;

/// Lua string.
#[repr(C)]
pub struct Str {
    pub(crate) hdr: Object,
    pub(crate) extra: Cell<u8>,
    pub(crate) shrlen: Cell<u8>,
    pub(crate) hash: Cell<u32>,
    pub(crate) u: UnsafeCell<C2RustUnnamed_8>,
    pub(crate) contents: [c_char; 1],
}

#[derive(Copy, Clone)]
#[repr(C)]
pub(crate) union C2RustUnnamed_8 {
    pub lnglen: usize,
    pub hnext: *const Str,
}
