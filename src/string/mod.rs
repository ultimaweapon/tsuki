use crate::{Lua, Object};
use core::alloc::Layout;
use core::cell::{Cell, UnsafeCell};
use core::ffi::c_char;
use core::mem::offset_of;
use core::ptr::addr_of_mut;

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

impl Str {
    pub(crate) unsafe fn new(g: *const Lua, l: usize, tag: u8, h: u32) -> *mut Str {
        let size = offset_of!(Str, contents) + l + 1;
        let align = align_of::<Str>();
        let layout = Layout::from_size_align(size, align).unwrap().pad_to_align();
        let o = unsafe { Object::new(g, tag, layout).cast::<Str>() };

        unsafe { addr_of_mut!((*o).hash).write(Cell::new(h)) };
        unsafe { addr_of_mut!((*o).extra).write(Cell::new(0)) };
        unsafe { *((*o).contents).as_mut_ptr().offset(l as isize) = '\0' as i32 as libc::c_char };

        o
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
pub(crate) union C2RustUnnamed_8 {
    pub lnglen: usize,
    pub hnext: *const Str,
}
