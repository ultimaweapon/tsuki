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
#![allow(unused_parens)]
#![allow(path_statements)]

use crate::lgc::{luaC_fix, luaC_fullgc, luaC_newobj};
use crate::lmem::{luaM_malloc_, luaM_realloc_, luaM_toobig};
use crate::lobject::{GCObject, TString, Table, UValue, Udata};
use crate::lstate::lua_State;
use crate::{Lua, stringtable};
use libc::{memcmp, memcpy, strcmp, strlen};

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaS_eqlngstr(mut a: *mut TString, mut b: *mut TString) -> libc::c_int {
    let mut len: usize = (*a).u.lnglen;
    return (a == b
        || len == (*b).u.lnglen
            && memcmp(
                ((*a).contents).as_mut_ptr() as *const libc::c_void,
                ((*b).contents).as_mut_ptr() as *const libc::c_void,
                len as _,
            ) == 0 as libc::c_int) as libc::c_int;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaS_hash(
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
        l;
    }
    return h;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaS_hashlongstr(mut ts: *mut TString) -> libc::c_uint {
    if (*ts).extra as libc::c_int == 0 as libc::c_int {
        let mut len: usize = (*ts).u.lnglen;
        (*ts).hash = luaS_hash(((*ts).contents).as_mut_ptr(), len, (*ts).hash);
        (*ts).extra = 1 as libc::c_int as u8;
    }
    return (*ts).hash;
}

unsafe extern "C" fn tablerehash(
    mut vect: *mut *mut TString,
    mut osize: libc::c_int,
    mut nsize: libc::c_int,
) {
    let mut i: libc::c_int = 0;
    i = osize;
    while i < nsize {
        let ref mut fresh0 = *vect.offset(i as isize);
        *fresh0 = 0 as *mut TString;
        i += 1;
        i;
    }
    i = 0 as libc::c_int;
    while i < osize {
        let mut p: *mut TString = *vect.offset(i as isize);
        let ref mut fresh1 = *vect.offset(i as isize);
        *fresh1 = 0 as *mut TString;
        while !p.is_null() {
            let mut hnext: *mut TString = (*p).u.hnext;
            let mut h: libc::c_uint = ((*p).hash & (nsize - 1 as libc::c_int) as libc::c_uint)
                as libc::c_int as libc::c_uint;
            (*p).u.hnext = *vect.offset(h as isize);
            let ref mut fresh2 = *vect.offset(h as isize);
            *fresh2 = p;
            p = hnext;
        }
        i += 1;
        i;
    }
}

pub unsafe fn luaS_resize(mut L: *mut lua_State, mut nsize: libc::c_int) {
    let mut tb = (*(*L).l_G).strt.get();
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

pub unsafe fn luaS_clearcache(g: *const Lua) {
    let mut i: libc::c_int = 0;
    let mut j: libc::c_int = 0;
    i = 0 as libc::c_int;

    while i < 53 as libc::c_int {
        j = 0 as libc::c_int;

        while j < 2 as libc::c_int {
            if (*(*g).strcache[i as usize][j as usize].get()).marked & (1 << 3 | 1 << 4) != 0 {
                (*g).strcache[i as usize][j as usize].set((*g).memerrmsg.get());
            }
            j += 1;
            j;
        }

        i += 1;
        i;
    }
}

pub unsafe fn luaS_init(mut L: *mut lua_State) -> Result<(), Box<dyn std::error::Error>> {
    let g = (*L).l_G;
    let mut i: libc::c_int = 0;
    let mut j: libc::c_int = 0;
    let mut tb = (*g).strt.get();

    (*tb).hash = luaM_malloc_(
        g,
        128usize.wrapping_mul(::core::mem::size_of::<*mut TString>()),
    ) as *mut *mut TString;

    tablerehash((*tb).hash, 0 as libc::c_int, 128 as libc::c_int);

    (*tb).size = 128 as libc::c_int;
    (*g).memerrmsg.set(luaS_newlstr(
        L,
        b"not enough memory\0" as *const u8 as *const libc::c_char,
        ::core::mem::size_of::<[libc::c_char; 18]>()
            .wrapping_div(::core::mem::size_of::<libc::c_char>())
            .wrapping_sub(1),
    )?);

    luaC_fix(L, ((*g).memerrmsg.get() as *mut GCObject));

    i = 0 as libc::c_int;

    while i < 53 as libc::c_int {
        j = 0 as libc::c_int;
        while j < 2 as libc::c_int {
            (*g).strcache[i as usize][j as usize].set((*g).memerrmsg.get());
            j += 1;
            j;
        }
        i += 1;
        i;
    }

    Ok(())
}

unsafe extern "C" fn createstrobj(
    mut L: *mut lua_State,
    mut l: usize,
    mut tag: libc::c_int,
    mut h: libc::c_uint,
) -> *mut TString {
    let mut ts: *mut TString = 0 as *mut TString;
    let mut o: *mut GCObject = 0 as *mut GCObject;
    let mut totalsize: usize = 0;
    totalsize = 24usize.wrapping_add(
        l.wrapping_add(1 as libc::c_int as usize)
            .wrapping_mul(::core::mem::size_of::<libc::c_char>()),
    );
    o = luaC_newobj((*L).l_G, tag, totalsize);
    ts = (o as *mut TString);
    (*ts).hash = h;
    (*ts).extra = 0 as libc::c_int as u8;
    *((*ts).contents).as_mut_ptr().offset(l as isize) = '\0' as i32 as libc::c_char;
    return ts;
}

pub unsafe fn luaS_createlngstrobj(mut L: *mut lua_State, mut l: usize) -> *mut TString {
    let mut ts: *mut TString = createstrobj(
        L,
        l,
        4 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int,
        (*(*L).l_G).seed,
    );
    (*ts).u.lnglen = l;
    (*ts).shrlen = 0xff as libc::c_int as u8;
    return ts;
}

pub unsafe fn luaS_remove(g: *const Lua, mut ts: *mut TString) {
    let mut tb = (*g).strt.get();
    let mut p: *mut *mut TString = &mut *((*tb).hash).offset(
        ((*ts).hash & ((*tb).size - 1 as libc::c_int) as libc::c_uint) as libc::c_int as isize,
    ) as *mut *mut TString;
    while *p != ts {
        p = &mut (**p).u.hnext;
    }
    *p = (**p).u.hnext;
    (*tb).nuse -= 1;
    (*tb).nuse;
}

unsafe extern "C" fn growstrtab(mut L: *mut lua_State, mut tb: *mut stringtable) {
    if (((*tb).nuse == 2147483647 as libc::c_int) as libc::c_int != 0 as libc::c_int) as libc::c_int
        as libc::c_long
        != 0
    {
        luaC_fullgc(L, 1 as libc::c_int);
        if (*tb).nuse == 2147483647 as libc::c_int {
            todo!("invoke handle_alloc_error");
        }
    }
    if (*tb).size
        <= (if 2147483647 as libc::c_int as usize
            <= (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<*mut TString>())
        {
            2147483647 as libc::c_int as libc::c_uint
        } else {
            (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<*mut TString>())
                as libc::c_uint
        }) as libc::c_int
            / 2 as libc::c_int
    {
        luaS_resize(L, (*tb).size * 2 as libc::c_int);
    }
}
unsafe extern "C" fn internshrstr(
    mut L: *mut lua_State,
    mut str: *const libc::c_char,
    mut l: usize,
) -> *mut TString {
    let mut ts: *mut TString = 0 as *mut TString;
    let g = (*L).l_G;
    let mut tb: *mut stringtable = (*g).strt.get();
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
            if (*ts).marked as libc::c_int
                & ((*g).currentwhite.get() as libc::c_int
                    ^ ((1 as libc::c_int) << 3 as libc::c_int
                        | (1 as libc::c_int) << 4 as libc::c_int))
                != 0
            {
                (*ts).marked = ((*ts).marked as libc::c_int
                    ^ ((1 as libc::c_int) << 3 as libc::c_int
                        | (1 as libc::c_int) << 4 as libc::c_int))
                    as u8;
            }
            return ts;
        }
        ts = (*ts).u.hnext;
    }
    if (*tb).nuse >= (*tb).size {
        growstrtab(L, tb);
        list = &mut *((*tb).hash)
            .offset((h & ((*tb).size - 1 as libc::c_int) as libc::c_uint) as libc::c_int as isize)
            as *mut *mut TString;
    }
    ts = createstrobj(
        L,
        l,
        4 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int,
        h,
    );
    (*ts).shrlen = l as u8;
    memcpy(
        ((*ts).contents).as_mut_ptr() as *mut libc::c_void,
        str as *const libc::c_void,
        l.wrapping_mul(::core::mem::size_of::<libc::c_char>()),
    );
    (*ts).u.hnext = *list;
    *list = ts;
    (*tb).nuse += 1;
    (*tb).nuse;
    return ts;
}

pub unsafe fn luaS_newlstr(
    mut L: *mut lua_State,
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
    mut L: *mut lua_State,
    mut str: *const libc::c_char,
) -> Result<*mut TString, Box<dyn std::error::Error>> {
    let mut i = ((str as usize & 0xffffffff) as libc::c_uint).wrapping_rem(53);
    let mut j: libc::c_int = 0;
    let p = &((*(*L).l_G).strcache[i as usize]);

    for v in p {
        if strcmp(str, ((*v.get()).contents).as_mut_ptr()) == 0 {
            return Ok(v.get());
        }
    }

    j = 2 as libc::c_int - 1 as libc::c_int;

    while j > 0 as libc::c_int {
        let ref fresh3 = p[j as usize];
        fresh3.set(p[(j - 1) as usize].get());
        j -= 1;
    }

    let ref fresh4 = p[0];

    fresh4.set(luaS_newlstr(L, str, strlen(str) as _)?);

    return Ok(p[0].get());
}

pub unsafe fn luaS_newudata(
    mut L: *mut lua_State,
    mut s: usize,
    mut nuvalue: libc::c_int,
) -> Result<*mut Udata, Box<dyn std::error::Error>> {
    let mut u: *mut Udata = 0 as *mut Udata;
    let mut i: libc::c_int = 0;
    let mut o: *mut GCObject = 0 as *mut GCObject;
    if ((s
        > (if (::core::mem::size_of::<usize>() as libc::c_ulong)
            < ::core::mem::size_of::<i64>() as libc::c_ulong
        {
            !(0 as libc::c_int as usize)
        } else {
            0x7fffffffffffffff as libc::c_longlong as usize
        })
        .wrapping_sub(
            (if nuvalue == 0 as libc::c_int {
                32
            } else {
                40usize
                    .wrapping_add((::core::mem::size_of::<UValue>()).wrapping_mul(nuvalue as usize))
            }),
        )) as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        luaM_toobig(L)?;
    }
    o = luaC_newobj(
        (*L).l_G,
        7 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int,
        (if nuvalue == 0 as libc::c_int {
            32
        } else {
            40usize.wrapping_add((::core::mem::size_of::<UValue>()).wrapping_mul(nuvalue as usize))
        })
        .wrapping_add(s),
    );
    u = (o as *mut Udata);
    (*u).len = s;
    (*u).nuvalue = nuvalue as libc::c_ushort;
    (*u).metatable = 0 as *mut Table;
    i = 0 as libc::c_int;
    while i < nuvalue {
        (*((*u).uv).as_mut_ptr().offset(i as isize)).uv.tt_ =
            (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
        i += 1;
        i;
    }
    return Ok(u);
}
