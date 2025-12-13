pub(crate) use self::table::*;

use crate::lobject::luaO_str2num;
use crate::lstring::luaS_hash;
use crate::{Lua, Number, Object};
use alloc::vec::Vec;
use core::alloc::Layout;
use core::cell::Cell;
use core::ffi::c_char;
use core::mem::offset_of;
use core::ptr::{addr_of_mut, null};

mod table;

/// Lua string.
///
/// Use [Lua::create_str()] or [Context::create_str()](crate::Context::create_str()) to create the
/// value of this type.
///
/// Although the string is currently null-terminated but there is a plan to remove this so
/// **do not** rely on this.
#[repr(C)]
pub struct Str<A> {
    pub(crate) hdr: Object<A>,
    pub(crate) ty: Cell<Option<ContentType>>,
    pub(crate) len: usize,
    pub(crate) extra: Cell<u8>,
    pub(crate) hash: Cell<u32>,
    pub(crate) hnext: Cell<*const Self>,
    pub(crate) contents: [c_char; 1],
}

impl<A> Str<A> {
    const SHORT_LEN: usize = 40;

    /// Returns [Ok] if new string was allocated or [Err] for interned string.
    #[inline(always)]
    pub(crate) unsafe fn from_str<T>(g: *const Lua<A>, str: T) -> Result<*const Self, *const Self>
    where
        T: AsRef<str> + AsRef<[u8]> + Into<Vec<u8>>,
    {
        unsafe { Self::new(g, str, Some(ContentType::Utf8)) }
    }

    /// Returns [Ok] if new string was allocated or [Err] for interned string.
    #[inline(always)]
    pub(crate) unsafe fn from_bytes<T>(g: *const Lua<A>, str: T) -> Result<*const Self, *const Self>
    where
        T: AsRef<[u8]> + Into<Vec<u8>>,
    {
        unsafe { Self::new(g, str, None) }
    }

    #[inline(never)]
    unsafe fn new<T>(
        g: *const Lua<A>,
        str: T,
        ty: Option<ContentType>,
    ) -> Result<*const Self, *const Self>
    where
        T: AsRef<[u8]> + Into<Vec<u8>>,
    {
        // Check if long string.
        let s = str.as_ref();

        if s.len() > Self::SHORT_LEN {
            let str = str.into();
            let s = unsafe { Self::alloc(g, str.len(), 4 | 0 << 4, (*g).seed) };

            unsafe { addr_of_mut!((*s).ty).write(Cell::new(ty)) };
            unsafe { addr_of_mut!((*s).hnext).write(Cell::new(null())) };
            unsafe {
                (*s).contents
                    .as_mut_ptr()
                    .copy_from_nonoverlapping(str.as_ptr().cast(), str.len())
            };

            return Ok(s);
        }

        // Add to string table.
        let h = unsafe { luaS_hash(s.as_ptr().cast(), s.len(), (*g).seed) };

        match unsafe { (*g).strt.insert(h, s) } {
            Ok(v) => unsafe {
                (*g).gc.resurrect(v.cast());
                Err(v)
            },
            Err(e) => unsafe {
                let str = str.into();
                let v = Self::alloc(g, str.len(), 4 | 0 << 4, h);

                addr_of_mut!((*v).ty).write(Cell::new(ty));
                addr_of_mut!((*v).hnext).write(Cell::new(*e));
                (*v).contents
                    .as_mut_ptr()
                    .copy_from_nonoverlapping(str.as_ptr().cast(), str.len());

                *e = v;
                Ok(v)
            },
        }
    }

    /// Returns `true` if this string is UTF-8.
    ///
    /// Use [Self::as_utf8()] instead if you want [str] from this string.
    #[inline(always)]
    pub fn is_utf8(&self) -> bool {
        match self.ty.get() {
            Some(v) => v == ContentType::Utf8,
            None => self.load_type() == ContentType::Utf8,
        }
    }

    /// Returns the length of this string, in bytes.
    #[inline(always)]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns [str] if this string is UTF-8.
    #[inline(always)]
    pub fn as_utf8(&self) -> Option<&str> {
        match self.is_utf8() {
            true => Some(unsafe { core::str::from_utf8_unchecked(self.as_bytes()) }),
            false => None,
        }
    }

    /// Returns byte slice of this string.
    #[inline(always)]
    pub const fn as_bytes(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.contents.as_ptr().cast(), self.len()) }
    }

    /// Parses this string as a Lua number.
    ///
    /// return [None] if the content is not valid number literal.
    ///
    /// This has the same semantic as `lua_stringtonumber`, **except** it does not accept
    /// hexadecimal floating-point. It also does not treat U+000B VERTICAL TAB as a whitespace.
    pub fn to_num(&self) -> Option<Number> {
        luaO_str2num(self.as_bytes())
    }

    #[inline(always)]
    pub(crate) fn is_short(&self) -> bool {
        self.len <= Self::SHORT_LEN
    }

    unsafe fn alloc(g: *const Lua<A>, l: usize, tag: u8, h: u32) -> *mut Self {
        let size = offset_of!(Self, contents) + l + 1;
        let align = align_of::<Self>();
        let layout = Layout::from_size_align(size, align).unwrap().pad_to_align();
        let o = unsafe { (*g).gc.alloc(tag, layout).cast::<Self>() };

        unsafe { addr_of_mut!((*o).len).write(l) };
        unsafe { addr_of_mut!((*o).hash).write(Cell::new(h)) };
        unsafe { addr_of_mut!((*o).extra).write(Cell::new(0)) };
        unsafe { (*o).contents.as_mut_ptr().add(l).write(0) };

        o
    }

    #[inline(never)]
    fn load_type(&self) -> ContentType {
        let ty = match core::str::from_utf8(self.as_bytes()) {
            Ok(_) => ContentType::Utf8,
            Err(_) => ContentType::Binary,
        };

        self.ty.set(Some(ty));

        ty
    }
}

impl<D> Drop for Str<D> {
    #[inline(always)]
    fn drop(&mut self) {
        if self.is_short() {
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

/// Type of [Str::contents].
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContentType {
    Binary,
    Utf8,
}
