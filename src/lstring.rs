#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments
)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::lobject::Udata;
use crate::value::UnsafeValue;
use crate::{Lua, Str};
use core::alloc::Layout;
use core::mem::offset_of;
use core::ptr::null;

pub unsafe fn luaS_eqlngstr<D>(a: *const Str<D>, b: *const Str<D>) -> libc::c_int {
    (a == b || (*a).as_bytes() == (*b).as_bytes()).into()
}

pub unsafe fn luaS_hash(
    str: *const libc::c_char,
    mut l: usize,
    seed: libc::c_uint,
) -> libc::c_uint {
    let mut h: libc::c_uint = seed ^ l as libc::c_uint;
    while l > 0 as libc::c_int as usize {
        h ^= (h << 5 as libc::c_int)
            .wrapping_add(h >> 2 as libc::c_int)
            .wrapping_add(
                *str.offset(l.wrapping_sub(1 as libc::c_int as usize) as isize) as u8
                    as libc::c_uint,
            );
        l = l.wrapping_sub(1);
    }
    return h;
}

pub unsafe fn luaS_hashlongstr<D>(ts: *mut Str<D>) -> libc::c_uint {
    if (*ts).extra.get() as libc::c_int == 0 as libc::c_int {
        let s = (*ts).as_bytes();

        (*ts)
            .hash
            .set(luaS_hash(s.as_ptr().cast(), s.len(), (*ts).hash.get()));
        (*ts).extra.set(1 as libc::c_int as u8);
    }
    return (*ts).hash.get();
}

pub unsafe fn luaS_newudata<D>(g: *const Lua<D>, s: usize, nuvalue: libc::c_int) -> *mut Udata<D> {
    let mut i: libc::c_int = 0;
    let min = offset_of!(Udata<D>, uv) + size_of::<UnsafeValue<D>>() * nuvalue as usize;
    let size = min + s;
    let align = align_of::<Udata<D>>();
    let layout = Layout::from_size_align(size, align).unwrap().pad_to_align();
    let o = (*g).gc.alloc(7 | 0 << 4, layout).cast::<Udata<D>>();

    (*o).len = s;
    (*o).nuvalue = nuvalue as libc::c_ushort;
    (*o).metatable = null();
    i = 0 as libc::c_int;

    while i < nuvalue {
        (*((*o).uv).as_mut_ptr().offset(i as isize)).tt_ = 0 | 0 << 4;
        i += 1;
    }

    o
}
