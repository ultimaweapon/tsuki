#![allow(
    dead_code,
    mutable_transmutes,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments,
    unused_mut
)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::lmem::{luaM_malloc_, luaM_realloc_, luaM_toobig};
use crate::lobject::{TString, Table, UValue, Udata};
use crate::{Lua, StringTable, Thread};
use libc::{memcmp, memcpy, strlen};
use std::alloc::Layout;
use std::mem::offset_of;

pub unsafe fn luaS_eqlngstr(mut a: *mut TString, mut b: *mut TString) -> libc::c_int {
    let mut len: usize = (*(*a).u.get()).lnglen;
    return (a == b
        || len == (*(*b).u.get()).lnglen
            && memcmp(
                ((*a).contents).as_mut_ptr() as *const libc::c_void,
                ((*b).contents).as_mut_ptr() as *const libc::c_void,
                len as _,
            ) == 0 as libc::c_int) as libc::c_int;
}

pub unsafe fn luaS_hash(
    mut str: *const libc::c_char,
    mut l: usize,
    mut seed: libc::c_uint,
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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaS_hashlongstr(mut ts: *mut TString) -> libc::c_uint {
    if (*ts).extra as libc::c_int == 0 as libc::c_int {
        let mut len: usize = (*(*ts).u.get()).lnglen;
        (*ts).hash = luaS_hash(((*ts).contents).as_mut_ptr(), len, (*ts).hash);
        (*ts).extra = 1 as libc::c_int as u8;
    }
    return (*ts).hash;
}

unsafe fn tablerehash(mut vect: *mut *mut TString, mut osize: libc::c_int, mut nsize: libc::c_int) {
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
            let mut hnext: *mut TString = (*(*p).u.get()).hnext;
            let mut h: libc::c_uint = ((*p).hash & (nsize - 1 as libc::c_int) as libc::c_uint)
                as libc::c_int as libc::c_uint;
            (*(*p).u.get()).hnext = *vect.offset(h as isize);
            let ref mut fresh2 = *vect.offset(h as isize);
            *fresh2 = p;
            p = hnext;
        }
        i += 1;
    }
}

pub unsafe fn luaS_resize(mut L: *mut Thread, mut nsize: libc::c_int) {
    let mut tb = (*(*L).global).strt.get();
    let mut osize: libc::c_int = (*tb).size;
    let mut newvect: *mut *mut TString = 0 as *mut *mut TString;
    if nsize < osize {
        tablerehash((*tb).hash, osize, nsize);
    }
    newvect = luaM_realloc_(
        L,
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

pub unsafe fn luaS_init(mut L: *mut Thread) {
    let g = (*L).global;
    let mut tb = (*g).strt.get();

    (*tb).hash = luaM_malloc_(
        g,
        128usize.wrapping_mul(::core::mem::size_of::<*mut TString>()),
    ) as *mut *mut TString;

    tablerehash((*tb).hash, 0 as libc::c_int, 128 as libc::c_int);

    (*tb).size = 128 as libc::c_int;
}

unsafe fn createstrobj(L: *mut Thread, l: usize, tag: u8, h: libc::c_uint) -> *mut TString {
    let size = offset_of!(TString, contents) + l + 1;
    let align = align_of::<TString>();
    let layout = Layout::from_size_align(size, align).unwrap().pad_to_align();
    let o = (*(*L).global).gc.alloc(tag, layout);
    let ts = o as *mut TString;

    (*ts).hash = h;
    (*ts).extra = 0 as libc::c_int as u8;
    *((*ts).contents).as_mut_ptr().offset(l as isize) = '\0' as i32 as libc::c_char;

    return ts;
}

pub unsafe fn luaS_createlngstrobj(mut L: *mut Thread, mut l: usize) -> *mut TString {
    let ts: *mut TString = createstrobj(L, l, 4 | 1 << 4, (*(*L).global).seed);

    (*(*ts).u.get()).lnglen = l;
    (*ts).shrlen = 0xff as libc::c_int as u8;

    return ts;
}

pub unsafe fn luaS_remove(g: *const Lua, mut ts: *mut TString) {
    let mut tb = (*g).strt.get();
    let mut p: *mut *mut TString = &mut *((*tb).hash).offset(
        ((*ts).hash & ((*tb).size - 1 as libc::c_int) as libc::c_uint) as libc::c_int as isize,
    ) as *mut *mut TString;
    while *p != ts {
        p = &raw mut (*(**p).u.get()).hnext;
    }
    *p = (*(**p).u.get()).hnext;
    (*tb).nuse -= 1;
    (*tb).nuse;
}

unsafe fn growstrtab(mut L: *mut Thread, mut tb: *mut StringTable) {
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
        luaS_resize(L, (*tb).size * 2 as libc::c_int);
    }
}

unsafe fn internshrstr(
    mut L: *mut Thread,
    mut str: *const libc::c_char,
    mut l: usize,
) -> *mut TString {
    let mut ts: *mut TString = 0 as *mut TString;
    let g = (*L).global;
    let mut tb = (*g).strt.get();
    let mut h: libc::c_uint = luaS_hash(str, l, (*g).seed);
    let mut list: *mut *mut TString = &mut *((*tb).hash)
        .offset((h & ((*tb).size - 1 as libc::c_int) as libc::c_uint) as libc::c_int as isize)
        as *mut *mut TString;
    ts = *list;

    while !ts.is_null() {
        if l == (*ts).shrlen as usize
            && memcmp(
                str as *const libc::c_void,
                ((*ts).contents).as_mut_ptr() as *const libc::c_void,
                l.wrapping_mul(::core::mem::size_of::<libc::c_char>()) as _,
            ) == 0 as libc::c_int
        {
            if (*ts).hdr.marked.is_dead((*g).gc.currentwhite()) {
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
        growstrtab(L, tb);
        list = &mut *((*tb).hash)
            .offset((h & ((*tb).size - 1 as libc::c_int) as libc::c_uint) as libc::c_int as isize)
            as *mut *mut TString;
    }

    ts = createstrobj(L, l, 4 | 0 << 4, h);

    (*ts).shrlen = l as u8;
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

pub unsafe fn luaS_newlstr(
    mut L: *mut Thread,
    mut str: *const libc::c_char,
    mut l: usize,
) -> Result<*mut TString, Box<dyn std::error::Error>> {
    if l <= 40 as libc::c_int as usize {
        return Ok(internshrstr(L, str, l));
    } else {
        let mut ts: *mut TString = 0 as *mut TString;
        if ((l.wrapping_mul(::core::mem::size_of::<libc::c_char>())
            >= (if (::core::mem::size_of::<usize>() as libc::c_ulong)
                < ::core::mem::size_of::<i64>() as libc::c_ulong
            {
                !(0 as libc::c_int as usize)
            } else {
                0x7fffffffffffffff as libc::c_longlong as usize
            })
            .wrapping_sub(::core::mem::size_of::<TString>())) as libc::c_int
            != 0 as libc::c_int) as libc::c_int as libc::c_long
            != 0
        {
            luaM_toobig(L)?;
        }
        ts = luaS_createlngstrobj(L, l);
        memcpy(
            ((*ts).contents).as_mut_ptr() as *mut libc::c_void,
            str as *const libc::c_void,
            l.wrapping_mul(::core::mem::size_of::<libc::c_char>()),
        );
        return Ok(ts);
    };
}

pub unsafe fn luaS_new(
    mut L: *mut Thread,
    mut str: *const libc::c_char,
) -> Result<*mut TString, Box<dyn std::error::Error>> {
    luaS_newlstr(L, str, strlen(str))
}

pub unsafe fn luaS_newudata(
    mut L: *mut Thread,
    mut s: usize,
    mut nuvalue: libc::c_int,
) -> Result<*mut Udata, Box<dyn std::error::Error>> {
    let mut i: libc::c_int = 0;
    let min = offset_of!(Udata, uv) + size_of::<UValue>().wrapping_mul(nuvalue as usize);

    if ((s
        > (if (::core::mem::size_of::<usize>() as libc::c_ulong)
            < ::core::mem::size_of::<i64>() as libc::c_ulong
        {
            !(0 as libc::c_int as usize)
        } else {
            0x7fffffffffffffff as libc::c_longlong as usize
        })
        .wrapping_sub(min)) as libc::c_int
        != 0) as libc::c_int as libc::c_long
        != 0
    {
        luaM_toobig(L)?;
    }

    let size = min + s;
    let align = align_of::<Udata>();
    let layout = Layout::from_size_align(size, align).unwrap().pad_to_align();
    let o = (*(*L).global).gc.alloc(7 | 0 << 4, layout);
    let u = o as *mut Udata;

    (*u).len = s;
    (*u).nuvalue = nuvalue as libc::c_ushort;
    (*u).metatable = 0 as *mut Table;
    i = 0 as libc::c_int;

    while i < nuvalue {
        (*((*u).uv).as_mut_ptr().offset(i as isize)).uv.tt_ = 0 | 0 << 4;
        i += 1;
    }

    return Ok(u);
}
