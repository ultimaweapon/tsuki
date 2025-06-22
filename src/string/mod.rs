pub(crate) use self::table::*;

use crate::lstring::luaS_hash;
use crate::{Lua, Object};
use core::alloc::Layout;
use core::cell::{Cell, UnsafeCell};
use core::ffi::c_char;
use core::mem::offset_of;
use core::ptr::addr_of_mut;

mod table;

/// Lua string.
#[repr(C)]
pub struct Str {
    pub(crate) hdr: Object,
    unicode: bool,
    pub(crate) extra: Cell<u8>,
    pub(crate) shrlen: Cell<u8>,
    pub(crate) hash: Cell<u32>,
    pub(crate) u: UnsafeCell<C2RustUnnamed_8>,
    pub(crate) contents: [c_char; 1],
}

impl Str {
    #[inline(always)]
    pub(crate) unsafe fn from_str(g: *const Lua, str: impl AsRef<str>) -> *const Str {
        unsafe { Self::new(g, str.as_ref(), true) }
    }

    #[inline(always)]
    pub(crate) unsafe fn from_bytes(g: *const Lua, str: impl AsRef<[u8]>) -> *const Str {
        unsafe { Self::new(g, str, false) }
    }

    #[inline(never)]
    unsafe fn new(g: *const Lua, str: impl AsRef<[u8]>, unicode: bool) -> *const Str {
        // Check if long string.
        let str = str.as_ref();

        if str.len() > 40 {
            let s = unsafe { Self::alloc(g, str.len(), 4 | 1 << 4, (*g).seed) };

            unsafe { addr_of_mut!((*s).unicode).write(unicode) };
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
        let h = unsafe { luaS_hash(str.as_ptr().cast(), str.len(), (*g).seed) };

        match unsafe { (*g).strt.insert(h, str) } {
            Ok(v) => unsafe {
                if (*v).hdr.marked.is_dead((*g).currentwhite.get()) {
                    (*v).hdr
                        .marked
                        .set((*v).hdr.marked.get() ^ (1 << 3 | 1 << 4));
                }

                v
            },
            Err(e) => unsafe {
                let v = Self::alloc(g, str.len(), 4 | 0 << 4, h);

                addr_of_mut!((*v).unicode).write(unicode);
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
        self.unicode
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
        self.unicode
            .then(|| unsafe { core::str::from_utf8_unchecked(self.as_bytes()) })
    }

    /// Returns byte slice of this string.
    #[inline(always)]
    pub fn as_bytes(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.contents.as_ptr().cast(), self.len()) }
    }

    unsafe fn alloc(g: *const Lua, l: usize, tag: u8, h: u32) -> *mut Str {
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

impl Drop for Str {
    fn drop(&mut self) {
        if self.shrlen.get() != 0xff {
            unsafe { (*self.hdr.global).strt.remove(self) };
        }
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
pub(crate) union C2RustUnnamed_8 {
    pub lnglen: usize,
    pub hnext: *const Str,
}
