#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments
)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::lobject::{UValue, Udata};
use crate::{Lua, Object, Str};
use core::alloc::Layout;
use core::cell::Cell;
use core::mem::offset_of;
use core::ptr::{addr_of_mut, null};
use libc::{memcmp, strlen};

pub unsafe fn luaS_eqlngstr(a: *mut Str, b: *mut Str) -> libc::c_int {
    let len: usize = (*(*a).u.get()).lnglen;
    return (a == b
        || len == (*(*b).u.get()).lnglen
            && memcmp(
                ((*a).contents).as_mut_ptr() as *const libc::c_void,
                ((*b).contents).as_mut_ptr() as *const libc::c_void,
                len as _,
            ) == 0 as libc::c_int) as libc::c_int;
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

pub unsafe fn luaS_hashlongstr(ts: *mut Str) -> libc::c_uint {
    if (*ts).extra.get() as libc::c_int == 0 as libc::c_int {
        let len: usize = (*(*ts).u.get()).lnglen;
        (*ts).hash.set(luaS_hash(
            ((*ts).contents).as_mut_ptr(),
            len,
            (*ts).hash.get(),
        ));
        (*ts).extra.set(1 as libc::c_int as u8);
    }
    return (*ts).hash.get();
}

pub unsafe fn luaS_createlngstrobj(g: *const Lua, l: usize) -> *mut Str {
    let ts = Str::new(g, l, 4 | 1 << 4, (*g).seed);

    (*(*ts).u.get()).lnglen = l;
    addr_of_mut!((*ts).shrlen).write(Cell::new(0xff));

    return ts;
}

pub unsafe fn luaS_newlstr(g: *const Lua, str: *const libc::c_char, l: usize) -> *const Str {
    if l <= 40 {
        let h = unsafe { luaS_hash(str, l, (*g).seed) };
        let str = core::slice::from_raw_parts(str.cast(), l);

        match (*g).strt.insert(h, str) {
            Ok(v) => {
                if (*v).hdr.marked.is_dead((*g).currentwhite.get()) {
                    (*v).hdr
                        .marked
                        .set((*v).hdr.marked.get() ^ (1 << 3 | 1 << 4));
                }

                v
            }
            Err(e) => {
                let v = Str::new(g, l, 4 | 0 << 4, h);

                addr_of_mut!((*v).shrlen).write(Cell::new(l.try_into().unwrap()));
                (*v).contents
                    .as_mut_ptr()
                    .copy_from_nonoverlapping(str.as_ptr().cast(), str.len());

                (*(*v).u.get()).hnext = *e;
                *e = v;

                v
            }
        }
    } else {
        let s = luaS_createlngstrobj(g, l);

        (*s).contents.as_mut_ptr().copy_from_nonoverlapping(str, l);

        s
    }
}

pub unsafe fn luaS_new(g: *const Lua, str: *const libc::c_char) -> *const Str {
    luaS_newlstr(g, str, strlen(str))
}

pub unsafe fn luaS_newudata(g: *const Lua, s: usize, nuvalue: libc::c_int) -> *mut Udata {
    let mut i: libc::c_int = 0;
    let min = offset_of!(Udata, uv) + size_of::<UValue>() * nuvalue as usize;
    let size = min + s;
    let align = align_of::<Udata>();
    let layout = Layout::from_size_align(size, align).unwrap().pad_to_align();
    let o = Object::new(g, 7 | 0 << 4, layout).cast::<Udata>();

    (*o).len = s;
    (*o).nuvalue = nuvalue as libc::c_ushort;
    (*o).metatable = null();
    i = 0 as libc::c_int;

    while i < nuvalue {
        (*((*o).uv).as_mut_ptr().offset(i as isize)).uv.tt_ = 0 | 0 << 4;
        i += 1;
    }

    o
}
