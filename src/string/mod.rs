pub(crate) use self::table::*;

use crate::lstring::luaS_hash;
use crate::{Lua, Object};
use alloc::vec::Vec;
use core::alloc::Layout;
use core::cell::{Cell, UnsafeCell};
use core::ffi::c_char;
use core::mem::offset_of;
use core::ptr::addr_of_mut;

mod table;

/// Lua string.
#[repr(C)]
pub struct Str<D> {
    pub(crate) hdr: Object<D>,
    utf8: bool,
    pub(crate) extra: Cell<u8>,
    pub(crate) shrlen: Cell<u8>,
    pub(crate) hash: Cell<u32>,
    pub(crate) u: UnsafeCell<C2RustUnnamed_8<D>>,
    pub(crate) contents: [c_char; 1],
}

impl<D> Str<D> {
    #[inline(always)]
    pub(crate) unsafe fn from_str<T>(g: *const Lua<D>, str: T) -> *const Self
    where
        T: AsRef<str> + AsRef<[u8]> + Into<Vec<u8>>,
    {
        unsafe { Self::new(g, str, true) }
    }

    #[inline(always)]
    pub(crate) unsafe fn from_bytes<T>(g: *const Lua<D>, str: T) -> *const Self
    where
        T: AsRef<[u8]> + Into<Vec<u8>>,
    {
        unsafe { Self::new(g, str, false) }
    }

    #[inline(never)]
    unsafe fn new<T>(g: *const Lua<D>, str: T, utf8: bool) -> *const Self
    where
        T: AsRef<[u8]> + Into<Vec<u8>>,
    {
        // Check if long string.
        let s = str.as_ref();

        if s.len() > 40 {
            let str = str.into();
            let s = unsafe { Self::alloc(g, str.len(), 4 | 1 << 4, (*g).seed) };

            unsafe { addr_of_mut!((*s).utf8).write(utf8) };
            unsafe { addr_of_mut!((*s).shrlen).write(Cell::new(0xff)) };
            unsafe { (*(*s).u.get()).lnglen = str.len() };
            unsafe {
                (*s).contents
                    .as_mut_ptr()
                    .copy_from_nonoverlapping(str.as_ptr().cast(), str.len())
            };

            return s;
        }

        // Add to string table.
        let h = unsafe { luaS_hash(s.as_ptr().cast(), s.len(), (*g).seed) };

        match unsafe { (*g).strt.insert(h, s) } {
            Ok(v) => unsafe {
                (*g).gc.resurrect(v.cast());
                v
            },
            Err(e) => unsafe {
                let str = str.into();
                let v = Self::alloc(g, str.len(), 4 | 0 << 4, h);

                addr_of_mut!((*v).utf8).write(utf8);
                addr_of_mut!((*v).shrlen).write(Cell::new(str.len().try_into().unwrap()));
                (*v).contents
                    .as_mut_ptr()
                    .copy_from_nonoverlapping(str.as_ptr().cast(), str.len());

                (*(*v).u.get()).hnext = *e;
                *e = v;

                v
            },
        }
    }

    /// Returns `true` if this string is UTF-8.
    ///
    /// Use [`Self::as_str()`] instead if you want [`str`] from this string.
    #[inline(always)]
    pub fn is_utf8(&self) -> bool {
        self.utf8
    }

    /// Returns the length of this string, in bytes.
    #[inline(always)]
    pub fn len(&self) -> usize {
        match self.shrlen.get() {
            0xFF => unsafe { (*self.u.get()).lnglen },
            v => v.into(),
        }
    }

    /// Returns [`str`] if this string is UTF-8.
    #[inline(always)]
    pub fn as_str(&self) -> Option<&str> {
        self.utf8
            .then(|| unsafe { core::str::from_utf8_unchecked(self.as_bytes()) })
    }

    /// Returns byte slice of this string.
    #[inline(always)]
    pub fn as_bytes(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.contents.as_ptr().cast(), self.len()) }
    }

    #[inline(always)]
    pub(crate) fn as_ptr(&self) -> *const c_char {
        self.contents.as_ptr()
    }

    unsafe fn alloc(g: *const Lua<D>, l: usize, tag: u8, h: u32) -> *mut Self {
        let size = offset_of!(Self, contents) + l + 1;
        let align = align_of::<Self>();
        let layout = Layout::from_size_align(size, align).unwrap().pad_to_align();
        let o = unsafe { (*g).gc.alloc(tag, layout).cast::<Self>() };

        unsafe { addr_of_mut!((*o).hash).write(Cell::new(h)) };
        unsafe { addr_of_mut!((*o).extra).write(Cell::new(0)) };
        unsafe { (*o).contents.as_mut_ptr().add(l).write(0) };

        o
    }
}

impl<D> Drop for Str<D> {
    fn drop(&mut self) {
        if self.shrlen.get() != 0xff {
            unsafe { (*self.hdr.global).strt.remove(self) };
        }
    }
}

impl<D> PartialEq<str> for Str<D> {
    #[inline(always)]
    fn eq(&self, other: &str) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
pub(crate) union C2RustUnnamed_8<D> {
    pub lnglen: usize,
    pub hnext: *const Str<D>,
}
