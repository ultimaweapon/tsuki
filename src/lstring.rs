#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments
)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::lmem::{luaM_malloc_, luaM_realloc_};
use crate::lobject::{TString, UValue, Udata};
use crate::{Lua, Object, StringTable};
use core::alloc::Layout;
use core::cell::Cell;
use core::mem::offset_of;
use core::ptr::{addr_of_mut, null};
use libc::{memcmp, memcpy, strlen};

pub unsafe fn luaS_eqlngstr(a: *mut TString, b: *mut TString) -> libc::c_int {
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

pub unsafe fn luaS_hashlongstr(ts: *mut TString) -> libc::c_uint {
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

unsafe fn tablerehash(vect: *mut *mut TString, osize: libc::c_int, nsize: libc::c_int) {
    let mut i: libc::c_int = 0;
    i = osize;
    while i < nsize {
        let ref mut fresh0 = *vect.offset(i as isize);
        *fresh0 = 0 as *mut TString;
        i += 1;
    }
    i = 0 as libc::c_int;
    while i < osize {
        let mut p: *mut TString = *vect.offset(i as isize);
        let ref mut fresh1 = *vect.offset(i as isize);
        *fresh1 = 0 as *mut TString;
        while !p.is_null() {
            let hnext: *mut TString = (*(*p).u.get()).hnext;
            let h: libc::c_uint = ((*p).hash.get() & (nsize - 1 as libc::c_int) as libc::c_uint)
                as libc::c_int as libc::c_uint;
            (*(*p).u.get()).hnext = *vect.offset(h as isize);
            let ref mut fresh2 = *vect.offset(h as isize);
            *fresh2 = p;
            p = hnext;
        }
        i += 1;
    }
}

pub unsafe fn luaS_resize(g: *const Lua, nsize: libc::c_int) {
    let tb = (*g).strt.get();
    let osize: libc::c_int = (*tb).size;
    let mut newvect: *mut *mut TString = 0 as *mut *mut TString;
    if nsize < osize {
        tablerehash((*tb).hash, osize, nsize);
    }
    newvect = luaM_realloc_(
        g,
        (*tb).hash as *mut libc::c_void,
        (osize as usize).wrapping_mul(::core::mem::size_of::<*mut TString>()),
        (nsize as usize).wrapping_mul(::core::mem::size_of::<*mut TString>()),
    ) as *mut *mut TString;
    if ((newvect == 0 as *mut libc::c_void as *mut *mut TString) as libc::c_int != 0 as libc::c_int)
        as libc::c_int as libc::c_long
        != 0
    {
        if nsize < osize {
            tablerehash((*tb).hash, nsize, osize);
        }
    } else {
        (*tb).hash = newvect;
        (*tb).size = nsize;
        if nsize > osize {
            tablerehash(newvect, osize, nsize);
        }
    };
}

pub unsafe fn luaS_init(g: *const Lua) {
    let tb = (*g).strt.get();

    (*tb).hash = luaM_malloc_(
        g,
        128usize.wrapping_mul(::core::mem::size_of::<*mut TString>()),
    ) as *mut *mut TString;

    tablerehash((*tb).hash, 0 as libc::c_int, 128 as libc::c_int);

    (*tb).size = 128 as libc::c_int;
}

unsafe fn createstrobj(g: *const Lua, l: usize, tag: u8, h: u32) -> *mut TString {
    let size = offset_of!(TString, contents) + l + 1;
    let align = align_of::<TString>();
    let layout = Layout::from_size_align(size, align).unwrap().pad_to_align();
    let o = Object::new(g, tag, layout).cast::<TString>();

    addr_of_mut!((*o).hash).write(Cell::new(h));
    addr_of_mut!((*o).extra).write(Cell::new(0));
    *((*o).contents).as_mut_ptr().offset(l as isize) = '\0' as i32 as libc::c_char;

    o
}

pub unsafe fn luaS_createlngstrobj(g: *const Lua, l: usize) -> *mut TString {
    let ts: *mut TString = createstrobj(g, l, 4 | 1 << 4, (*g).seed);

    (*(*ts).u.get()).lnglen = l;
    addr_of_mut!((*ts).shrlen).write(Cell::new(0xff));

    return ts;
}

pub unsafe fn luaS_remove(g: *const Lua, ts: *mut TString) {
    let tb = (*g).strt.get();
    let mut p: *mut *mut TString = &mut *((*tb).hash).offset(
        ((*ts).hash.get() & ((*tb).size - 1 as libc::c_int) as libc::c_uint) as libc::c_int
            as isize,
    ) as *mut *mut TString;
    while *p != ts {
        p = &raw mut (*(**p).u.get()).hnext;
    }
    *p = (*(**p).u.get()).hnext;
    (*tb).nuse -= 1;
    (*tb).nuse;
}

unsafe fn growstrtab(g: *const Lua, tb: *mut StringTable) {
    if (*tb).size
        <= (if 2147483647 as libc::c_int as usize
            <= (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<*mut TString>())
        {
            2147483647 as libc::c_int as libc::c_uint
        } else {
            (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<*mut TString>())
                as libc::c_uint
        }) as libc::c_int
            / 2
    {
        luaS_resize(g, (*tb).size * 2 as libc::c_int);
    }
}

unsafe fn internshrstr(g: *const Lua, str: *const libc::c_char, l: usize) -> *mut TString {
    let mut ts: *mut TString = 0 as *mut TString;
    let tb = (*g).strt.get();
    let h: libc::c_uint = luaS_hash(str, l, (*g).seed);
    let mut list: *mut *mut TString = &mut *((*tb).hash)
        .offset((h & ((*tb).size - 1 as libc::c_int) as libc::c_uint) as libc::c_int as isize)
        as *mut *mut TString;
    ts = *list;

    while !ts.is_null() {
        if l == (*ts).shrlen.get() as usize
            && memcmp(
                str as *const libc::c_void,
                ((*ts).contents).as_mut_ptr() as *const libc::c_void,
                l.wrapping_mul(::core::mem::size_of::<libc::c_char>()) as _,
            ) == 0 as libc::c_int
        {
            if (*ts).hdr.marked.is_dead((*g).currentwhite.get()) {
                (*ts)
                    .hdr
                    .marked
                    .set((*ts).hdr.marked.get() ^ (1 << 3 | 1 << 4));
            }

            return ts;
        }
        ts = (*(*ts).u.get()).hnext;
    }

    if (*tb).nuse >= (*tb).size {
        growstrtab(g, tb);

        list = (*tb)
            .hash
            .offset((h & ((*tb).size - 1) as libc::c_uint) as libc::c_int as isize)
            as *mut *mut TString;
    }

    ts = createstrobj(g, l, 4 | 0 << 4, h);

    addr_of_mut!((*ts).shrlen).write(Cell::new(l.try_into().unwrap()));
    memcpy(
        ((*ts).contents).as_mut_ptr() as *mut libc::c_void,
        str as *const libc::c_void,
        l,
    );
    (*(*ts).u.get()).hnext = *list;
    *list = ts;
    (*tb).nuse += 1;
    (*tb).nuse;

    return ts;
}

pub unsafe fn luaS_newlstr(g: *const Lua, str: *const libc::c_char, l: usize) -> *mut TString {
    if l <= 40 {
        internshrstr(g, str, l)
    } else {
        let ts = luaS_createlngstrobj(g, l);

        memcpy(((*ts).contents).as_mut_ptr().cast(), str.cast(), l);

        ts
    }
}

pub unsafe fn luaS_new(g: *const Lua, str: *const libc::c_char) -> *mut TString {
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
