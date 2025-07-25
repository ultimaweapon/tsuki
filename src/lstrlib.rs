#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments,
    unused_mut
)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::lapi::{
    lua_arith, lua_call, lua_createtable, lua_gettable, lua_gettop, lua_isinteger, lua_isstring,
    lua_newuserdatauv, lua_pushcclosure, lua_pushinteger, lua_pushlstring, lua_pushnil,
    lua_pushnumber, lua_pushstring, lua_pushvalue, lua_rotate, lua_setfield, lua_setmetatable,
    lua_stringtonumber, lua_toboolean, lua_tointegerx, lua_tolstring, lua_tonumberx, lua_topointer,
    lua_touserdata, lua_type, lua_typename,
};
use crate::lauxlib::{
    luaL_Reg, luaL_argerror, luaL_checkinteger, luaL_checklstring, luaL_checknumber,
    luaL_checkstack, luaL_error, luaL_getmetafield, luaL_optinteger, luaL_optlstring,
    luaL_setfuncs, luaL_tolstring, luaL_typeerror,
};
use crate::{Thread, lua_pop, lua_settop};
use libc::{
    isalnum, isalpha, iscntrl, isdigit, isgraph, islower, ispunct, isspace, isupper, isxdigit,
    memchr, memcmp, memcpy, snprintf, strchr, strcpy, strlen, strpbrk, strspn, tolower,
};
use std::boxed::Box;
use std::ffi::{CStr, c_int};
use std::format;
use std::vec::Vec;

pub const Knop: KOption = 10;
pub const Kpadding: KOption = 8;
pub const Kpaddalign: KOption = 9;
pub const Kzstr: KOption = 7;

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Header {
    pub L: *const Thread,
    pub islittle: libc::c_int,
    pub maxalign: libc::c_int,
}

pub const Kstring: KOption = 6;
pub const Kchar: KOption = 5;

#[derive(Copy, Clone)]
#[repr(C)]
pub union C2RustUnnamed_0 {
    pub dummy: libc::c_int,
    pub little: libc::c_char,
}

pub const Kdouble: KOption = 4;
pub const Knumber: KOption = 3;
pub const Kfloat: KOption = 2;
pub const Kint: KOption = 0;
pub type KOption = libc::c_uint;
pub const Kuint: KOption = 1;

#[derive(Copy, Clone)]
#[repr(C)]
pub struct MatchState {
    pub src_init: *const libc::c_char,
    pub src_end: *const libc::c_char,
    pub p_end: *const libc::c_char,
    pub L: *const Thread,
    pub matchdepth: libc::c_int,
    pub level: libc::c_uchar,
    pub capture: [C2RustUnnamed_2; 32],
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct C2RustUnnamed_2 {
    pub init: *const libc::c_char,
    pub len: isize,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct GMatchState {
    pub src: *const libc::c_char,
    pub p: *const libc::c_char,
    pub lastmatch: *const libc::c_char,
    pub ms: MatchState,
}

unsafe fn str_len(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut l: usize = 0;
    luaL_checklstring(L, 1 as libc::c_int, &mut l)?;
    lua_pushinteger(L, l as i64);
    return Ok(1 as libc::c_int);
}

unsafe fn posrelatI(mut pos: i64, mut len: usize) -> usize {
    if pos > 0 as libc::c_int as i64 {
        return pos as usize;
    } else if pos == 0 as libc::c_int as i64 {
        return 1 as libc::c_int as usize;
    } else if pos < -(len as i64) {
        return 1 as libc::c_int as usize;
    } else {
        return len
            .wrapping_add(pos as usize)
            .wrapping_add(1 as libc::c_int as usize);
    };
}

unsafe fn str_reverse(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut l: usize = 0;
    let mut s: *const libc::c_char = luaL_checklstring(L, 1 as libc::c_int, &mut l)?;
    let mut s = std::slice::from_raw_parts(s.cast(), l).to_vec();

    s.reverse();
    lua_pushlstring(L, s);

    return Ok(1 as libc::c_int);
}

unsafe fn str_lower(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut l: usize = 0;
    let mut s: *const libc::c_char = luaL_checklstring(L, 1 as libc::c_int, &mut l)?;
    let mut s = std::slice::from_raw_parts(s.cast::<u8>(), l).to_vec();

    for b in &mut s {
        b.make_ascii_lowercase();
    }

    lua_pushlstring(L, s);

    return Ok(1 as libc::c_int);
}

unsafe fn str_upper(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut l: usize = 0;
    let mut s: *const libc::c_char = luaL_checklstring(L, 1 as libc::c_int, &mut l)?;
    let mut s = std::slice::from_raw_parts(s.cast::<u8>(), l).to_vec();

    for b in &mut s {
        b.make_ascii_uppercase();
    }

    lua_pushlstring(L, s);

    return Ok(1 as libc::c_int);
}

unsafe fn str_rep(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut l: usize = 0;
    let mut lsep: usize = 0;
    let mut s: *const libc::c_char = luaL_checklstring(L, 1 as libc::c_int, &mut l)?;
    let mut n: i64 = luaL_checkinteger(L, 2 as libc::c_int)?;
    let mut sep: *const libc::c_char = luaL_optlstring(
        L,
        3 as libc::c_int,
        b"\0" as *const u8 as *const libc::c_char,
        &mut lsep,
    )?;
    if n <= 0 as libc::c_int as i64 {
        lua_pushstring(L, b"\0" as *const u8 as *const libc::c_char);
    } else if ((l.wrapping_add(lsep) < l
        || l.wrapping_add(lsep) as libc::c_ulonglong
            > ((if (::core::mem::size_of::<usize>() as libc::c_ulong)
                < ::core::mem::size_of::<libc::c_int>() as libc::c_ulong
            {
                !(0 as libc::c_int as usize)
            } else {
                2147483647 as libc::c_int as usize
            }) as libc::c_ulonglong)
                .wrapping_div(n as libc::c_ulonglong)) as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        return luaL_error(L, "resulting string too large");
    } else {
        let s = std::slice::from_raw_parts(s.cast(), l);
        let sep = std::slice::from_raw_parts(sep.cast(), lsep);
        let mut totallen: usize = (n as usize * l).wrapping_add((n - 1) as usize * lsep);
        let mut b = Vec::with_capacity(totallen);

        loop {
            let fresh0 = n;
            n = n - 1;
            if !(fresh0 > 1 as libc::c_int as i64) {
                break;
            }

            b.extend_from_slice(s);

            if lsep > 0 as libc::c_int as usize {
                b.extend_from_slice(sep);
            }
        }

        b.extend_from_slice(s);
        lua_pushlstring(L, b);
    }
    return Ok(1 as libc::c_int);
}

unsafe fn str_byte(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut l: usize = 0;
    let mut s: *const libc::c_char = luaL_checklstring(L, 1 as libc::c_int, &mut l)?;
    let mut pi: i64 = luaL_optinteger(L, 2 as libc::c_int, 1 as libc::c_int as i64)?;
    let mut posi: usize = posrelatI(pi, l);
    let mut pose: usize = getendpos(L, 3 as libc::c_int, pi, l)?;
    let mut n: libc::c_int = 0;
    let mut i: libc::c_int = 0;
    if posi > pose {
        return Ok(0 as libc::c_int);
    }
    if ((pose.wrapping_sub(posi) >= 2147483647 as libc::c_int as usize) as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        return luaL_error(L, "string slice too long");
    }
    n = pose.wrapping_sub(posi) as libc::c_int + 1 as libc::c_int;
    luaL_checkstack(
        L,
        n.try_into().unwrap(),
        b"string slice too long\0" as *const u8 as *const libc::c_char,
    )?;
    i = 0 as libc::c_int;
    while i < n {
        lua_pushinteger(
            L,
            *s.offset(
                posi.wrapping_add(i as usize)
                    .wrapping_sub(1 as libc::c_int as usize) as isize,
            ) as libc::c_uchar as i64,
        );
        i += 1;
    }
    return Ok(n);
}

unsafe fn str_char(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut n: libc::c_int = lua_gettop(L);
    let mut b = Vec::with_capacity(n.try_into().unwrap());
    let mut i = 1 as libc::c_int;

    while i <= n {
        let mut c: u64 = luaL_checkinteger(L, i)? as u64;
        ((c <= 255 as libc::c_int as u64) || luaL_argerror(L, i, "value out of range")? != 0)
            as libc::c_int;
        b.push(c as u8);
        i += 1;
    }

    lua_pushlstring(L, b);

    return Ok(1 as libc::c_int);
}

unsafe fn tonum(
    mut L: *const Thread,
    mut arg: libc::c_int,
) -> Result<libc::c_int, Box<dyn std::error::Error>> {
    if lua_type(L, arg) == 3 as libc::c_int {
        lua_pushvalue(L, arg);
        return Ok(1 as libc::c_int);
    } else {
        let mut len: usize = 0;
        let mut s: *const libc::c_char = lua_tolstring(L, arg, &mut len);
        return Ok((!s.is_null()
            && lua_stringtonumber(L, s) == len.wrapping_add(1 as libc::c_int as usize))
            as libc::c_int);
    };
}

unsafe fn trymt(
    mut L: *const Thread,
    mut mtname: *const libc::c_char,
) -> Result<(), Box<dyn std::error::Error>> {
    lua_settop(L, 2 as libc::c_int)?;
    if ((lua_type(L, 2 as libc::c_int) == 4 as libc::c_int
        || luaL_getmetafield(L, 2 as libc::c_int, mtname)? == 0) as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        luaL_error(
            L,
            format!(
                "attempt to {} a '{}' with a '{}'",
                CStr::from_ptr(mtname.offset(2)).to_string_lossy(),
                lua_typename(lua_type(L, -2)),
                lua_typename(lua_type(L, -1))
            ),
        )?;
    }
    lua_rotate(L, -(3 as libc::c_int), 1 as libc::c_int);
    lua_call(L, 2 as libc::c_int, 1 as libc::c_int)
}

unsafe fn arith(
    mut L: *const Thread,
    mut op: libc::c_int,
    mut mtname: *const libc::c_char,
) -> Result<c_int, Box<dyn std::error::Error>> {
    if tonum(L, 1 as libc::c_int)? != 0 && tonum(L, 2 as libc::c_int)? != 0 {
        lua_arith(L, op)?;
    } else {
        trymt(L, mtname)?;
    }
    return Ok(1 as libc::c_int);
}

unsafe fn arith_add(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    return arith(
        L,
        0 as libc::c_int,
        b"__add\0" as *const u8 as *const libc::c_char,
    );
}

unsafe fn arith_sub(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    return arith(
        L,
        1 as libc::c_int,
        b"__sub\0" as *const u8 as *const libc::c_char,
    );
}

unsafe fn arith_mul(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    return arith(
        L,
        2 as libc::c_int,
        b"__mul\0" as *const u8 as *const libc::c_char,
    );
}

unsafe fn arith_mod(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    return arith(
        L,
        3 as libc::c_int,
        b"__mod\0" as *const u8 as *const libc::c_char,
    );
}

unsafe fn arith_pow(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    return arith(
        L,
        4 as libc::c_int,
        b"__pow\0" as *const u8 as *const libc::c_char,
    );
}

unsafe fn arith_div(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    return arith(
        L,
        5 as libc::c_int,
        b"__div\0" as *const u8 as *const libc::c_char,
    );
}

unsafe fn arith_idiv(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    return arith(
        L,
        6 as libc::c_int,
        b"__idiv\0" as *const u8 as *const libc::c_char,
    );
}

unsafe fn arith_unm(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    return arith(
        L,
        12 as libc::c_int,
        b"__unm\0" as *const u8 as *const libc::c_char,
    );
}

static mut stringmetamethods: [luaL_Reg; 10] = [
    {
        let mut init = luaL_Reg {
            name: b"__add\0" as *const u8 as *const libc::c_char,
            func: Some(arith_add),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"__sub\0" as *const u8 as *const libc::c_char,
            func: Some(arith_sub),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"__mul\0" as *const u8 as *const libc::c_char,
            func: Some(arith_mul),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"__mod\0" as *const u8 as *const libc::c_char,
            func: Some(arith_mod),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"__pow\0" as *const u8 as *const libc::c_char,
            func: Some(arith_pow),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"__div\0" as *const u8 as *const libc::c_char,
            func: Some(arith_div),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"__idiv\0" as *const u8 as *const libc::c_char,
            func: Some(arith_idiv),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"__unm\0" as *const u8 as *const libc::c_char,
            func: Some(arith_unm),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: 0 as *const libc::c_char,
            func: None,
        };
        init
    },
];

unsafe fn check_capture(
    mut ms: *mut MatchState,
    mut l: libc::c_int,
) -> Result<c_int, Box<dyn std::error::Error>> {
    l -= '1' as i32;
    if ((l < 0 as libc::c_int
        || l >= (*ms).level as libc::c_int
        || (*ms).capture[l as usize].len == -(1 as libc::c_int) as isize) as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        return luaL_error((*ms).L, format!("invalid capture index %{}", l + 1));
    }
    return Ok(l);
}

unsafe fn capture_to_close(mut ms: *mut MatchState) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut level: libc::c_int = (*ms).level as libc::c_int;
    level -= 1;
    while level >= 0 as libc::c_int {
        if (*ms).capture[level as usize].len == -(1 as libc::c_int) as isize {
            return Ok(level);
        }
        level -= 1;
    }
    return luaL_error((*ms).L, "invalid pattern capture");
}

unsafe fn classend(
    mut ms: *mut MatchState,
    mut p: *const libc::c_char,
) -> Result<*const libc::c_char, Box<dyn std::error::Error>> {
    let fresh1 = p;
    p = p.offset(1);
    match *fresh1 as libc::c_int {
        37 => {
            if ((p == (*ms).p_end) as libc::c_int != 0 as libc::c_int) as libc::c_int
                as libc::c_long
                != 0
            {
                luaL_error((*ms).L, "malformed pattern (ends with '%')")?;
            }
            return Ok(p.offset(1 as libc::c_int as isize));
        }
        91 => {
            if *p as libc::c_int == '^' as i32 {
                p = p.offset(1);
            }
            loop {
                if ((p == (*ms).p_end) as libc::c_int != 0 as libc::c_int) as libc::c_int
                    as libc::c_long
                    != 0
                {
                    luaL_error((*ms).L, "malformed pattern (missing ']')")?;
                }
                let fresh2 = p;
                p = p.offset(1);
                if *fresh2 as libc::c_int == '%' as i32 && p < (*ms).p_end {
                    p = p.offset(1);
                }
                if !(*p as libc::c_int != ']' as i32) {
                    break;
                }
            }
            return Ok(p.offset(1 as libc::c_int as isize));
        }
        _ => return Ok(p),
    };
}

unsafe extern "C" fn match_class(mut c: libc::c_int, mut cl: libc::c_int) -> libc::c_int {
    let mut res: libc::c_int = 0;
    match tolower(cl) {
        97 => {
            res = isalpha(c);
        }
        99 => {
            res = iscntrl(c);
        }
        100 => {
            res = isdigit(c);
        }
        103 => {
            res = isgraph(c);
        }
        108 => {
            res = islower(c);
        }
        112 => {
            res = ispunct(c);
        }
        115 => {
            res = isspace(c);
        }
        117 => {
            res = isupper(c);
        }
        119 => {
            res = isalnum(c);
        }
        120 => {
            res = isxdigit(c);
        }
        122 => {
            res = (c == 0 as libc::c_int) as libc::c_int;
        }
        _ => return (cl == c) as libc::c_int,
    }
    return if islower(cl) != 0 {
        res
    } else {
        (res == 0) as libc::c_int
    };
}

unsafe extern "C" fn matchbracketclass(
    mut c: libc::c_int,
    mut p: *const libc::c_char,
    mut ec: *const libc::c_char,
) -> libc::c_int {
    let mut sig: libc::c_int = 1 as libc::c_int;
    if *p.offset(1 as libc::c_int as isize) as libc::c_int == '^' as i32 {
        sig = 0 as libc::c_int;
        p = p.offset(1);
    }
    loop {
        p = p.offset(1);
        if !(p < ec) {
            break;
        }
        if *p as libc::c_int == '%' as i32 {
            p = p.offset(1);
            if match_class(c, *p as libc::c_uchar as libc::c_int) != 0 {
                return sig;
            }
        } else if *p.offset(1 as libc::c_int as isize) as libc::c_int == '-' as i32
            && p.offset(2 as libc::c_int as isize) < ec
        {
            p = p.offset(2 as libc::c_int as isize);
            if *p.offset(-(2 as libc::c_int as isize)) as libc::c_uchar as libc::c_int <= c
                && c <= *p as libc::c_uchar as libc::c_int
            {
                return sig;
            }
        } else if *p as libc::c_uchar as libc::c_int == c {
            return sig;
        }
    }
    return (sig == 0) as libc::c_int;
}

unsafe extern "C" fn singlematch(
    mut ms: *mut MatchState,
    mut s: *const libc::c_char,
    mut p: *const libc::c_char,
    mut ep: *const libc::c_char,
) -> libc::c_int {
    if s >= (*ms).src_end {
        return 0 as libc::c_int;
    } else {
        let mut c: libc::c_int = *s as libc::c_uchar as libc::c_int;
        match *p as libc::c_int {
            46 => return 1 as libc::c_int,
            37 => {
                return match_class(
                    c,
                    *p.offset(1 as libc::c_int as isize) as libc::c_uchar as libc::c_int,
                );
            }
            91 => return matchbracketclass(c, p, ep.offset(-(1 as libc::c_int as isize))),
            _ => return (*p as libc::c_uchar as libc::c_int == c) as libc::c_int,
        }
    };
}

unsafe fn matchbalance(
    mut ms: *mut MatchState,
    mut s: *const libc::c_char,
    mut p: *const libc::c_char,
) -> Result<*const libc::c_char, Box<dyn std::error::Error>> {
    if ((p >= ((*ms).p_end).offset(-(1 as libc::c_int as isize))) as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        luaL_error((*ms).L, "malformed pattern (missing arguments to '%b')")?;
    }
    if *s as libc::c_int != *p as libc::c_int {
        return Ok(0 as *const libc::c_char);
    } else {
        let mut b: libc::c_int = *p as libc::c_int;
        let mut e: libc::c_int = *p.offset(1 as libc::c_int as isize) as libc::c_int;
        let mut cont: libc::c_int = 1 as libc::c_int;
        loop {
            s = s.offset(1);
            if !(s < (*ms).src_end) {
                break;
            }
            if *s as libc::c_int == e {
                cont -= 1;
                if cont == 0 as libc::c_int {
                    return Ok(s.offset(1 as libc::c_int as isize));
                }
            } else if *s as libc::c_int == b {
                cont += 1;
            }
        }
    }
    return Ok(0 as *const libc::c_char);
}

unsafe fn max_expand(
    mut ms: *mut MatchState,
    mut s: *const libc::c_char,
    mut p: *const libc::c_char,
    mut ep: *const libc::c_char,
) -> Result<*const libc::c_char, Box<dyn std::error::Error>> {
    let mut i: isize = 0 as libc::c_int as isize;
    while singlematch(ms, s.offset(i as isize), p, ep) != 0 {
        i += 1;
    }
    while i >= 0 as libc::c_int as isize {
        let mut res: *const libc::c_char = match_0(
            ms,
            s.offset(i as isize),
            ep.offset(1 as libc::c_int as isize),
        )?;
        if !res.is_null() {
            return Ok(res);
        }
        i -= 1;
    }
    return Ok(0 as *const libc::c_char);
}

unsafe fn min_expand(
    mut ms: *mut MatchState,
    mut s: *const libc::c_char,
    mut p: *const libc::c_char,
    mut ep: *const libc::c_char,
) -> Result<*const libc::c_char, Box<dyn std::error::Error>> {
    loop {
        let mut res: *const libc::c_char = match_0(ms, s, ep.offset(1 as libc::c_int as isize))?;
        if !res.is_null() {
            return Ok(res);
        } else if singlematch(ms, s, p, ep) != 0 {
            s = s.offset(1);
        } else {
            return Ok(0 as *const libc::c_char);
        }
    }
}

unsafe fn start_capture(
    mut ms: *mut MatchState,
    mut s: *const libc::c_char,
    mut p: *const libc::c_char,
    mut what: libc::c_int,
) -> Result<*const libc::c_char, Box<dyn std::error::Error>> {
    let mut res: *const libc::c_char = 0 as *const libc::c_char;
    let mut level: libc::c_int = (*ms).level as libc::c_int;
    if level >= 32 as libc::c_int {
        luaL_error((*ms).L, "too many captures")?;
    }
    (*ms).capture[level as usize].init = s;
    (*ms).capture[level as usize].len = what as isize;
    (*ms).level = (level + 1 as libc::c_int) as libc::c_uchar;
    res = match_0(ms, s, p)?;
    if res.is_null() {
        (*ms).level = ((*ms).level).wrapping_sub(1);
        (*ms).level;
    }
    return Ok(res);
}

unsafe fn end_capture(
    mut ms: *mut MatchState,
    mut s: *const libc::c_char,
    mut p: *const libc::c_char,
) -> Result<*const libc::c_char, Box<dyn std::error::Error>> {
    let mut l: libc::c_int = capture_to_close(ms)?;
    let mut res: *const libc::c_char = 0 as *const libc::c_char;
    (*ms).capture[l as usize].len = s.offset_from((*ms).capture[l as usize].init);
    res = match_0(ms, s, p)?;
    if res.is_null() {
        (*ms).capture[l as usize].len = -(1 as libc::c_int) as isize;
    }
    return Ok(res);
}

unsafe fn match_capture(
    mut ms: *mut MatchState,
    mut s: *const libc::c_char,
    mut l: libc::c_int,
) -> Result<*const libc::c_char, Box<dyn std::error::Error>> {
    let mut len: usize = 0;
    l = check_capture(ms, l)?;
    len = (*ms).capture[l as usize].len as usize;
    if ((*ms).src_end).offset_from(s) as libc::c_long as usize >= len
        && memcmp(
            (*ms).capture[l as usize].init as *const libc::c_void,
            s as *const libc::c_void,
            len,
        ) == 0 as libc::c_int
    {
        return Ok(s.offset(len as isize));
    } else {
        return Ok(0 as *const libc::c_char);
    };
}

unsafe fn match_0(
    mut ms: *mut MatchState,
    mut s: *const libc::c_char,
    mut p: *const libc::c_char,
) -> Result<*const libc::c_char, Box<dyn std::error::Error>> {
    let mut ep_0: *const libc::c_char = 0 as *const libc::c_char;
    let mut current_block: u64;
    let fresh3 = (*ms).matchdepth;
    (*ms).matchdepth = (*ms).matchdepth - 1;
    if ((fresh3 == 0 as libc::c_int) as libc::c_int != 0 as libc::c_int) as libc::c_int
        as libc::c_long
        != 0
    {
        luaL_error((*ms).L, "pattern too complex")?;
    }
    loop {
        if !(p != (*ms).p_end) {
            current_block = 6476622998065200121;
            break;
        }
        match *p as libc::c_int {
            40 => {
                if *p.offset(1 as libc::c_int as isize) as libc::c_int == ')' as i32 {
                    s = start_capture(
                        ms,
                        s,
                        p.offset(2 as libc::c_int as isize),
                        -(2 as libc::c_int),
                    )?;
                } else {
                    s = start_capture(
                        ms,
                        s,
                        p.offset(1 as libc::c_int as isize),
                        -(1 as libc::c_int),
                    )?;
                }
                current_block = 6476622998065200121;
                break;
            }
            41 => {
                s = end_capture(ms, s, p.offset(1 as libc::c_int as isize))?;
                current_block = 6476622998065200121;
                break;
            }
            36 => {
                if !(p.offset(1 as libc::c_int as isize) != (*ms).p_end) {
                    s = if s == (*ms).src_end {
                        s
                    } else {
                        0 as *const libc::c_char
                    };
                    current_block = 6476622998065200121;
                    break;
                }
            }
            37 => match *p.offset(1 as libc::c_int as isize) as libc::c_int {
                98 => {
                    current_block = 17965632435239708295;
                    match current_block {
                        17965632435239708295 => {
                            s = matchbalance(ms, s, p.offset(2 as libc::c_int as isize))?;
                            if s.is_null() {
                                current_block = 6476622998065200121;
                                break;
                            }
                            p = p.offset(4 as libc::c_int as isize);
                            continue;
                        }
                        8236137900636309791 => {
                            let mut ep: *const libc::c_char = 0 as *const libc::c_char;
                            let mut previous: libc::c_char = 0;
                            p = p.offset(2 as libc::c_int as isize);
                            if ((*p as libc::c_int != '[' as i32) as libc::c_int
                                != 0 as libc::c_int) as libc::c_int
                                as libc::c_long
                                != 0
                            {
                                luaL_error((*ms).L, "missing '[' after '%f' in pattern")?;
                            }
                            ep = classend(ms, p)?;
                            previous = (if s == (*ms).src_init {
                                '\0' as i32
                            } else {
                                *s.offset(-(1 as libc::c_int as isize)) as libc::c_int
                            }) as libc::c_char;
                            if matchbracketclass(
                                previous as libc::c_uchar as libc::c_int,
                                p,
                                ep.offset(-(1 as libc::c_int as isize)),
                            ) == 0
                                && matchbracketclass(
                                    *s as libc::c_uchar as libc::c_int,
                                    p,
                                    ep.offset(-(1 as libc::c_int as isize)),
                                ) != 0
                            {
                                p = ep;
                                continue;
                            } else {
                                s = 0 as *const libc::c_char;
                                current_block = 6476622998065200121;
                                break;
                            }
                        }
                        _ => {
                            s = match_capture(
                                ms,
                                s,
                                *p.offset(1 as libc::c_int as isize) as libc::c_uchar
                                    as libc::c_int,
                            )?;
                            if s.is_null() {
                                current_block = 6476622998065200121;
                                break;
                            }
                            p = p.offset(2 as libc::c_int as isize);
                            continue;
                        }
                    }
                }
                102 => {
                    current_block = 8236137900636309791;
                    match current_block {
                        17965632435239708295 => {
                            s = matchbalance(ms, s, p.offset(2 as libc::c_int as isize))?;
                            if s.is_null() {
                                current_block = 6476622998065200121;
                                break;
                            }
                            p = p.offset(4 as libc::c_int as isize);
                            continue;
                        }
                        8236137900636309791 => {
                            let mut ep: *const libc::c_char = 0 as *const libc::c_char;
                            let mut previous: libc::c_char = 0;
                            p = p.offset(2 as libc::c_int as isize);
                            if ((*p as libc::c_int != '[' as i32) as libc::c_int
                                != 0 as libc::c_int) as libc::c_int
                                as libc::c_long
                                != 0
                            {
                                luaL_error((*ms).L, "missing '[' after '%f' in pattern")?;
                            }
                            ep = classend(ms, p)?;
                            previous = (if s == (*ms).src_init {
                                '\0' as i32
                            } else {
                                *s.offset(-(1 as libc::c_int as isize)) as libc::c_int
                            }) as libc::c_char;
                            if matchbracketclass(
                                previous as libc::c_uchar as libc::c_int,
                                p,
                                ep.offset(-(1 as libc::c_int as isize)),
                            ) == 0
                                && matchbracketclass(
                                    *s as libc::c_uchar as libc::c_int,
                                    p,
                                    ep.offset(-(1 as libc::c_int as isize)),
                                ) != 0
                            {
                                p = ep;
                                continue;
                            } else {
                                s = 0 as *const libc::c_char;
                                current_block = 6476622998065200121;
                                break;
                            }
                        }
                        _ => {
                            s = match_capture(
                                ms,
                                s,
                                *p.offset(1 as libc::c_int as isize) as libc::c_uchar
                                    as libc::c_int,
                            )?;
                            if s.is_null() {
                                current_block = 6476622998065200121;
                                break;
                            }
                            p = p.offset(2 as libc::c_int as isize);
                            continue;
                        }
                    }
                }
                48 | 49 | 50 | 51 | 52 | 53 | 54 | 55 | 56 | 57 => {
                    current_block = 14576567515993809846;
                    match current_block {
                        17965632435239708295 => {
                            s = matchbalance(ms, s, p.offset(2 as libc::c_int as isize))?;
                            if s.is_null() {
                                current_block = 6476622998065200121;
                                break;
                            }
                            p = p.offset(4 as libc::c_int as isize);
                            continue;
                        }
                        8236137900636309791 => {
                            let mut ep: *const libc::c_char = 0 as *const libc::c_char;
                            let mut previous: libc::c_char = 0;
                            p = p.offset(2 as libc::c_int as isize);
                            if ((*p as libc::c_int != '[' as i32) as libc::c_int
                                != 0 as libc::c_int) as libc::c_int
                                as libc::c_long
                                != 0
                            {
                                luaL_error((*ms).L, "missing '[' after '%f' in pattern")?;
                            }
                            ep = classend(ms, p)?;
                            previous = (if s == (*ms).src_init {
                                '\0' as i32
                            } else {
                                *s.offset(-(1 as libc::c_int as isize)) as libc::c_int
                            }) as libc::c_char;
                            if matchbracketclass(
                                previous as libc::c_uchar as libc::c_int,
                                p,
                                ep.offset(-(1 as libc::c_int as isize)),
                            ) == 0
                                && matchbracketclass(
                                    *s as libc::c_uchar as libc::c_int,
                                    p,
                                    ep.offset(-(1 as libc::c_int as isize)),
                                ) != 0
                            {
                                p = ep;
                                continue;
                            } else {
                                s = 0 as *const libc::c_char;
                                current_block = 6476622998065200121;
                                break;
                            }
                        }
                        _ => {
                            s = match_capture(
                                ms,
                                s,
                                *p.offset(1 as libc::c_int as isize) as libc::c_uchar
                                    as libc::c_int,
                            )?;
                            if s.is_null() {
                                current_block = 6476622998065200121;
                                break;
                            }
                            p = p.offset(2 as libc::c_int as isize);
                            continue;
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        }
        ep_0 = classend(ms, p)?;
        if singlematch(ms, s, p, ep_0) == 0 {
            if *ep_0 as libc::c_int == '*' as i32
                || *ep_0 as libc::c_int == '?' as i32
                || *ep_0 as libc::c_int == '-' as i32
            {
                p = ep_0.offset(1 as libc::c_int as isize);
            } else {
                s = 0 as *const libc::c_char;
                current_block = 6476622998065200121;
                break;
            }
        } else {
            match *ep_0 as libc::c_int {
                63 => {
                    let mut res: *const libc::c_char = 0 as *const libc::c_char;
                    res = match_0(
                        ms,
                        s.offset(1 as libc::c_int as isize),
                        ep_0.offset(1 as libc::c_int as isize),
                    )?;
                    if !res.is_null() {
                        s = res;
                        current_block = 6476622998065200121;
                        break;
                    } else {
                        p = ep_0.offset(1 as libc::c_int as isize);
                    }
                }
                43 => {
                    s = s.offset(1);
                    current_block = 5161946086944071447;
                    break;
                }
                42 => {
                    current_block = 5161946086944071447;
                    break;
                }
                45 => {
                    s = min_expand(ms, s, p, ep_0)?;
                    current_block = 6476622998065200121;
                    break;
                }
                _ => {
                    s = s.offset(1);
                    p = ep_0;
                }
            }
        }
    }
    match current_block {
        5161946086944071447 => {
            s = max_expand(ms, s, p, ep_0)?;
        }
        _ => {}
    }
    (*ms).matchdepth += 1;
    (*ms).matchdepth;
    return Ok(s);
}

unsafe extern "C" fn lmemfind(
    mut s1: *const libc::c_char,
    mut l1: usize,
    mut s2: *const libc::c_char,
    mut l2: usize,
) -> *const libc::c_char {
    if l2 == 0 as libc::c_int as usize {
        return s1;
    } else if l2 > l1 {
        return 0 as *const libc::c_char;
    } else {
        let mut init: *const libc::c_char = 0 as *const libc::c_char;
        l2 = l2.wrapping_sub(1);
        l1 = l1.wrapping_sub(l2);
        while l1 > 0 as libc::c_int as usize && {
            init = memchr(s1 as *const libc::c_void, *s2 as libc::c_int, l1) as *const libc::c_char;
            !init.is_null()
        } {
            init = init.offset(1);
            if memcmp(
                init as *const libc::c_void,
                s2.offset(1 as libc::c_int as isize) as *const libc::c_void,
                l2,
            ) == 0 as libc::c_int
            {
                return init.offset(-(1 as libc::c_int as isize));
            } else {
                l1 = l1.wrapping_sub(init.offset_from(s1) as libc::c_long as usize);
                s1 = init;
            }
        }
        return 0 as *const libc::c_char;
    };
}

unsafe fn get_onecapture(
    mut ms: *mut MatchState,
    mut i: libc::c_int,
    mut s: *const libc::c_char,
    mut e: *const libc::c_char,
    mut cap: *mut *const libc::c_char,
) -> Result<usize, Box<dyn std::error::Error>> {
    if i >= (*ms).level as libc::c_int {
        if ((i != 0 as libc::c_int) as libc::c_int != 0 as libc::c_int) as libc::c_int
            as libc::c_long
            != 0
        {
            luaL_error((*ms).L, format!("invalid capture index %{}", i + 1))?;
        }
        *cap = s;
        return Ok(e.offset_from(s) as libc::c_long as usize);
    } else {
        let mut capl: isize = (*ms).capture[i as usize].len;
        *cap = (*ms).capture[i as usize].init;
        if ((capl == -(1 as libc::c_int) as isize) as libc::c_int != 0 as libc::c_int)
            as libc::c_int as libc::c_long
            != 0
        {
            luaL_error((*ms).L, "unfinished capture")?;
        } else if capl == -(2 as libc::c_int) as isize {
            lua_pushinteger(
                (*ms).L,
                (((*ms).capture[i as usize].init).offset_from((*ms).src_init) as libc::c_long
                    + 1 as libc::c_int as libc::c_long) as i64,
            );
        }
        return Ok(capl as usize);
    };
}

unsafe fn push_onecapture(
    mut ms: *mut MatchState,
    mut i: libc::c_int,
    mut s: *const libc::c_char,
    mut e: *const libc::c_char,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut cap: *const libc::c_char = 0 as *const libc::c_char;
    let mut l: isize = get_onecapture(ms, i, s, e, &mut cap)? as isize;

    if l != -(2 as libc::c_int) as isize {
        lua_pushlstring((*ms).L, std::slice::from_raw_parts(cap.cast(), l as usize));
    }

    Ok(())
}

unsafe fn push_captures(
    mut ms: *mut MatchState,
    mut s: *const libc::c_char,
    mut e: *const libc::c_char,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut nlevels = if (*ms).level as libc::c_int == 0 as libc::c_int && !s.is_null() {
        1
    } else {
        usize::from((*ms).level)
    };

    luaL_checkstack(
        (*ms).L,
        nlevels,
        b"too many captures\0" as *const u8 as *const libc::c_char,
    )?;

    for i in 0..nlevels {
        push_onecapture(ms, i.try_into().unwrap(), s, e)?;
    }

    Ok(nlevels.try_into().unwrap())
}

unsafe extern "C" fn nospecials(mut p: *const libc::c_char, mut l: usize) -> libc::c_int {
    let mut upto: usize = 0 as libc::c_int as usize;
    loop {
        if !(strpbrk(
            p.offset(upto as isize),
            b"^$*+?.([%-\0" as *const u8 as *const libc::c_char,
        ))
        .is_null()
        {
            return 0 as libc::c_int;
        }
        upto = upto.wrapping_add((strlen(p.offset(upto as isize))).wrapping_add(1));
        if !(upto <= l) {
            break;
        }
    }
    return 1 as libc::c_int;
}

unsafe fn prepstate(
    mut ms: *mut MatchState,
    mut L: *const Thread,
    mut s: *const libc::c_char,
    mut ls: usize,
    mut p: *const libc::c_char,
    mut lp: usize,
) {
    (*ms).L = L;
    (*ms).matchdepth = 200 as libc::c_int;
    (*ms).src_init = s;
    (*ms).src_end = s.offset(ls as isize);
    (*ms).p_end = p.offset(lp as isize);
}

unsafe extern "C" fn reprepstate(mut ms: *mut MatchState) {
    (*ms).level = 0 as libc::c_int as libc::c_uchar;
}

unsafe fn str_find_aux(
    mut L: *const Thread,
    mut find: libc::c_int,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut ls: usize = 0;
    let mut lp: usize = 0;
    let mut s: *const libc::c_char = luaL_checklstring(L, 1 as libc::c_int, &mut ls)?;
    let mut p: *const libc::c_char = luaL_checklstring(L, 2 as libc::c_int, &mut lp)?;
    let mut init: usize = (posrelatI(
        luaL_optinteger(L, 3 as libc::c_int, 1 as libc::c_int as i64)?,
        ls,
    ))
    .wrapping_sub(1 as libc::c_int as usize);
    if init > ls {
        lua_pushnil(L);
        return Ok(1 as libc::c_int);
    }
    if find != 0 && (lua_toboolean(L, 4 as libc::c_int) != 0 || nospecials(p, lp) != 0) {
        let mut s2: *const libc::c_char =
            lmemfind(s.offset(init as isize), ls.wrapping_sub(init), p, lp);
        if !s2.is_null() {
            lua_pushinteger(
                L,
                (s2.offset_from(s) as libc::c_long + 1 as libc::c_int as libc::c_long) as i64,
            );
            lua_pushinteger(
                L,
                (s2.offset_from(s) as libc::c_long as usize).wrapping_add(lp) as i64,
            );
            return Ok(2 as libc::c_int);
        }
    } else {
        let mut ms: MatchState = MatchState {
            src_init: 0 as *const libc::c_char,
            src_end: 0 as *const libc::c_char,
            p_end: 0 as *const libc::c_char,
            L: 0 as *mut Thread,
            matchdepth: 0,
            level: 0,
            capture: [C2RustUnnamed_2 {
                init: 0 as *const libc::c_char,
                len: 0,
            }; 32],
        };
        let mut s1: *const libc::c_char = s.offset(init as isize);
        let mut anchor: libc::c_int = (*p as libc::c_int == '^' as i32) as libc::c_int;
        if anchor != 0 {
            p = p.offset(1);
            lp = lp.wrapping_sub(1);
        }
        prepstate(&mut ms, L, s, ls, p, lp);
        loop {
            let mut res: *const libc::c_char = 0 as *const libc::c_char;
            reprepstate(&mut ms);
            res = match_0(&mut ms, s1, p)?;
            if !res.is_null() {
                if find != 0 {
                    lua_pushinteger(
                        L,
                        (s1.offset_from(s) as libc::c_long + 1 as libc::c_int as libc::c_long)
                            as i64,
                    );
                    lua_pushinteger(L, res.offset_from(s) as libc::c_long as i64);
                    return push_captures(
                        &mut ms,
                        0 as *const libc::c_char,
                        0 as *const libc::c_char,
                    )
                    .map(|v| v + 2);
                } else {
                    return push_captures(&mut ms, s1, res);
                }
            }
            let fresh4 = s1;
            s1 = s1.offset(1);
            if !(fresh4 < ms.src_end && anchor == 0) {
                break;
            }
        }
    }
    lua_pushnil(L);
    return Ok(1 as libc::c_int);
}

unsafe fn str_find(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    return str_find_aux(L, 1 as libc::c_int);
}

unsafe fn str_match(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    return str_find_aux(L, 0 as libc::c_int);
}

unsafe fn gmatch_aux(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut gm: *mut GMatchState = lua_touserdata(
        L,
        -(1000000 as libc::c_int) - 1000 as libc::c_int - 3 as libc::c_int,
    ) as *mut GMatchState;
    let mut src: *const libc::c_char = 0 as *const libc::c_char;
    (*gm).ms.L = L;
    src = (*gm).src;
    while src <= (*gm).ms.src_end {
        let mut e: *const libc::c_char = 0 as *const libc::c_char;
        reprepstate(&mut (*gm).ms);
        e = match_0(&mut (*gm).ms, src, (*gm).p)?;
        if !e.is_null() && e != (*gm).lastmatch {
            (*gm).lastmatch = e;
            (*gm).src = (*gm).lastmatch;
            return push_captures(&mut (*gm).ms, src, e);
        }
        src = src.offset(1);
    }
    return Ok(0 as libc::c_int);
}

unsafe fn gmatch(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut ls: usize = 0;
    let mut lp: usize = 0;
    let mut s: *const libc::c_char = luaL_checklstring(L, 1 as libc::c_int, &mut ls)?;
    let mut p: *const libc::c_char = luaL_checklstring(L, 2 as libc::c_int, &mut lp)?;
    let mut init: usize = (posrelatI(
        luaL_optinteger(L, 3 as libc::c_int, 1 as libc::c_int as i64)?,
        ls,
    ))
    .wrapping_sub(1 as libc::c_int as usize);
    let mut gm: *mut GMatchState = 0 as *mut GMatchState;
    lua_settop(L, 2 as libc::c_int)?;
    gm = lua_newuserdatauv(L, ::core::mem::size_of::<GMatchState>(), 0) as *mut GMatchState;
    if init > ls {
        init = ls.wrapping_add(1 as libc::c_int as usize);
    }
    prepstate(&mut (*gm).ms, L, s, ls, p, lp);
    (*gm).src = s.offset(init as isize);
    (*gm).p = p;
    (*gm).lastmatch = 0 as *const libc::c_char;
    lua_pushcclosure(L, gmatch_aux, 3 as libc::c_int);
    return Ok(1 as libc::c_int);
}

unsafe fn add_s(
    mut ms: *mut MatchState,
    mut b: &mut Vec<u8>,
    mut s: *const libc::c_char,
    mut e: *const libc::c_char,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut l: usize = 0;
    let mut L = (*ms).L;
    let mut news: *const libc::c_char = lua_tolstring(L, 3 as libc::c_int, &mut l);
    let mut p: *const libc::c_char = 0 as *const libc::c_char;
    loop {
        p = memchr(news as *const libc::c_void, '%' as i32, l) as *mut libc::c_char;
        if p.is_null() {
            break;
        }
        b.extend_from_slice(std::slice::from_raw_parts(
            news.cast(),
            p.offset_from(news) as libc::c_long as usize,
        ));
        p = p.offset(1);
        if *p as libc::c_int == '%' as i32 {
            b.push(*p as _);
        } else if *p as libc::c_int == '0' as i32 {
            b.extend_from_slice(std::slice::from_raw_parts(
                s.cast(),
                e.offset_from(s) as libc::c_long as usize,
            ));
        } else if isdigit(*p as libc::c_uchar as libc::c_int) != 0 {
            let mut cap: *const libc::c_char = 0 as *const libc::c_char;
            let mut resl: isize =
                get_onecapture(ms, *p as libc::c_int - '1' as i32, s, e, &mut cap)? as isize;
            if resl == -(2 as libc::c_int) as isize {
                let mut l = 0;
                let s = lua_tolstring(L, -1, &mut l);
                b.extend_from_slice(std::slice::from_raw_parts(s.cast(), l));
                lua_pop(L, 1)?;
            } else {
                b.extend_from_slice(std::slice::from_raw_parts(cap.cast(), resl as usize));
            }
        } else {
            luaL_error(L, "invalid use of '%' in replacement string")?;
        }
        l = l.wrapping_sub(
            p.offset(1 as libc::c_int as isize).offset_from(news) as libc::c_long as usize,
        );
        news = p.offset(1 as libc::c_int as isize);
    }

    b.extend_from_slice(std::slice::from_raw_parts(news.cast(), l));

    Ok(())
}

unsafe fn add_value(
    mut ms: *mut MatchState,
    mut b: &mut Vec<u8>,
    mut s: *const libc::c_char,
    mut e: *const libc::c_char,
    mut tr: libc::c_int,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut L = (*ms).L;
    match tr {
        6 => {
            let mut n: libc::c_int = 0;
            lua_pushvalue(L, 3 as libc::c_int);
            n = push_captures(ms, s, e)?;
            lua_call(L, n, 1 as libc::c_int)?;
        }
        5 => {
            push_onecapture(ms, 0 as libc::c_int, s, e)?;
            lua_gettable(L, 3 as libc::c_int)?;
        }
        _ => {
            add_s(ms, b, s, e)?;
            return Ok(1 as libc::c_int);
        }
    }
    if lua_toboolean(L, -(1 as libc::c_int)) == 0 {
        lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
        b.extend_from_slice(std::slice::from_raw_parts(
            s.cast(),
            e.offset_from(s) as libc::c_long as usize,
        ));
        return Ok(0 as libc::c_int);
    } else if ((lua_isstring(L, -(1 as libc::c_int)) == 0) as libc::c_int != 0 as libc::c_int)
        as libc::c_int as libc::c_long
        != 0
    {
        return luaL_error(
            L,
            format_args!(
                "invalid replacement value (a {})",
                lua_typename(lua_type(L, -1))
            ),
        );
    } else {
        let mut l = 0;
        let s = lua_tolstring(L, -1, &mut l);
        b.extend_from_slice(std::slice::from_raw_parts(s.cast(), l));
        lua_pop(L, 1)?;
        return Ok(1 as libc::c_int);
    };
}

unsafe fn str_gsub(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut srcl: usize = 0;
    let mut lp: usize = 0;
    let mut src: *const libc::c_char = luaL_checklstring(L, 1 as libc::c_int, &mut srcl)?;
    let mut p: *const libc::c_char = luaL_checklstring(L, 2 as libc::c_int, &mut lp)?;
    let mut lastmatch: *const libc::c_char = 0 as *const libc::c_char;
    let mut tr: libc::c_int = lua_type(L, 3 as libc::c_int);
    let mut max_s: i64 = luaL_optinteger(
        L,
        4 as libc::c_int,
        srcl.wrapping_add(1 as libc::c_int as usize) as i64,
    )?;
    let mut anchor: libc::c_int = (*p as libc::c_int == '^' as i32) as libc::c_int;
    let mut n: i64 = 0 as libc::c_int as i64;
    let mut changed: libc::c_int = 0 as libc::c_int;
    let mut b = Vec::new();
    let mut ms: MatchState = MatchState {
        src_init: 0 as *const libc::c_char,
        src_end: 0 as *const libc::c_char,
        p_end: 0 as *const libc::c_char,
        L: 0 as *mut Thread,
        matchdepth: 0,
        level: 0,
        capture: [C2RustUnnamed_2 {
            init: 0 as *const libc::c_char,
            len: 0,
        }; 32],
    };

    (((tr == 3 as libc::c_int
        || tr == 4 as libc::c_int
        || tr == 6 as libc::c_int
        || tr == 5 as libc::c_int) as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
        || luaL_typeerror(L, 3 as libc::c_int, "string/function/table")? != 0) as libc::c_int;
    if anchor != 0 {
        p = p.offset(1);
        lp = lp.wrapping_sub(1);
    }
    prepstate(&mut ms, L, src, srcl, p, lp);
    while n < max_s {
        let mut e: *const libc::c_char = 0 as *const libc::c_char;
        reprepstate(&mut ms);
        e = match_0(&mut ms, src, p)?;
        if !e.is_null() && e != lastmatch {
            n += 1;
            changed = add_value(&mut ms, &mut b, src, e, tr)? | changed;
            lastmatch = e;
            src = lastmatch;
        } else {
            if !(src < ms.src_end) {
                break;
            }
            let fresh6 = src;
            src = src.offset(1);

            b.push(*fresh6 as u8);
        }
        if anchor != 0 {
            break;
        }
    }
    if changed == 0 {
        lua_pushvalue(L, 1 as libc::c_int);
    } else {
        b.extend_from_slice(std::slice::from_raw_parts(
            src.cast(),
            (ms.src_end).offset_from(src) as libc::c_long as usize,
        ));
        lua_pushlstring(L, b);
    }
    lua_pushinteger(L, n);
    return Ok(2 as libc::c_int);
}

static mut nativeendian: C2RustUnnamed_0 = C2RustUnnamed_0 {
    dummy: 1 as libc::c_int,
};

unsafe extern "C" fn digit(mut c: libc::c_int) -> libc::c_int {
    return ('0' as i32 <= c && c <= '9' as i32) as libc::c_int;
}

unsafe extern "C" fn getnum(mut fmt: *mut *const libc::c_char, mut df: libc::c_int) -> libc::c_int {
    if digit(**fmt as libc::c_int) == 0 {
        return df;
    } else {
        let mut a: libc::c_int = 0 as libc::c_int;
        loop {
            let fresh20 = *fmt;
            *fmt = (*fmt).offset(1);
            a = a * 10 as libc::c_int + (*fresh20 as libc::c_int - '0' as i32);
            if !(digit(**fmt as libc::c_int) != 0
                && a <= ((if (::core::mem::size_of::<usize>() as libc::c_ulong)
                    < ::core::mem::size_of::<libc::c_int>() as libc::c_ulong
                {
                    !(0 as libc::c_int as usize)
                } else {
                    2147483647 as libc::c_int as usize
                }) as libc::c_int
                    - 9 as libc::c_int)
                    / 10 as libc::c_int)
            {
                break;
            }
        }
        return a;
    };
}

unsafe fn getnumlimit(
    mut h: *mut Header,
    mut fmt: *mut *const libc::c_char,
    mut df: libc::c_int,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut sz: libc::c_int = getnum(fmt, df);
    if ((sz > 16 as libc::c_int || sz <= 0 as libc::c_int) as libc::c_int != 0 as libc::c_int)
        as libc::c_int as libc::c_long
        != 0
    {
        return luaL_error(
            (*h).L,
            format!("integral size ({}) out of limits [1,16]", sz),
        );
    }
    return Ok(sz);
}

unsafe fn initheader(mut L: *const Thread, mut h: *mut Header) {
    (*h).L = L;
    (*h).islittle = nativeendian.little as libc::c_int;
    (*h).maxalign = 1 as libc::c_int;
}

unsafe fn getoption(
    mut h: *mut Header,
    mut fmt: *mut *const libc::c_char,
    mut size: *mut libc::c_int,
) -> Result<KOption, Box<dyn std::error::Error>> {
    let fresh21 = *fmt;
    *fmt = (*fmt).offset(1);
    let mut opt: libc::c_int = *fresh21 as libc::c_int;
    *size = 0 as libc::c_int;
    match opt {
        98 => {
            *size = ::core::mem::size_of::<libc::c_char>() as libc::c_ulong as libc::c_int;
            return Ok(Kint);
        }
        66 => {
            *size = ::core::mem::size_of::<libc::c_char>() as libc::c_ulong as libc::c_int;
            return Ok(Kuint);
        }
        104 => {
            *size = ::core::mem::size_of::<libc::c_short>() as libc::c_ulong as libc::c_int;
            return Ok(Kint);
        }
        72 => {
            *size = ::core::mem::size_of::<libc::c_short>() as libc::c_ulong as libc::c_int;
            return Ok(Kuint);
        }
        108 => {
            *size = ::core::mem::size_of::<libc::c_long>() as libc::c_ulong as libc::c_int;
            return Ok(Kint);
        }
        76 => {
            *size = ::core::mem::size_of::<libc::c_long>() as libc::c_ulong as libc::c_int;
            return Ok(Kuint);
        }
        106 => {
            *size = ::core::mem::size_of::<i64>() as libc::c_ulong as libc::c_int;
            return Ok(Kint);
        }
        74 => {
            *size = ::core::mem::size_of::<i64>() as libc::c_ulong as libc::c_int;
            return Ok(Kuint);
        }
        84 => {
            *size = ::core::mem::size_of::<usize>() as libc::c_ulong as libc::c_int;
            return Ok(Kuint);
        }
        102 => {
            *size = ::core::mem::size_of::<libc::c_float>() as libc::c_ulong as libc::c_int;
            return Ok(Kfloat);
        }
        110 => {
            *size = ::core::mem::size_of::<f64>() as libc::c_ulong as libc::c_int;
            return Ok(Knumber);
        }
        100 => {
            *size = ::core::mem::size_of::<libc::c_double>() as libc::c_ulong as libc::c_int;
            return Ok(Kdouble);
        }
        105 => {
            *size = getnumlimit(
                h,
                fmt,
                ::core::mem::size_of::<libc::c_int>() as libc::c_ulong as libc::c_int,
            )?;
            return Ok(Kint);
        }
        73 => {
            *size = getnumlimit(
                h,
                fmt,
                ::core::mem::size_of::<libc::c_int>() as libc::c_ulong as libc::c_int,
            )?;
            return Ok(Kuint);
        }
        115 => {
            *size = getnumlimit(
                h,
                fmt,
                ::core::mem::size_of::<usize>() as libc::c_ulong as libc::c_int,
            )?;
            return Ok(Kstring);
        }
        99 => {
            *size = getnum(fmt, -(1 as libc::c_int));
            if ((*size == -(1 as libc::c_int)) as libc::c_int != 0 as libc::c_int) as libc::c_int
                as libc::c_long
                != 0
            {
                luaL_error((*h).L, "missing size for format option 'c'")?;
            }
            return Ok(Kchar);
        }
        122 => return Ok(Kzstr),
        120 => {
            *size = 1 as libc::c_int;
            return Ok(Kpadding);
        }
        88 => return Ok(Kpaddalign),
        32 => {}
        60 => {
            (*h).islittle = 1 as libc::c_int;
        }
        62 => {
            (*h).islittle = 0 as libc::c_int;
        }
        61 => {
            (*h).islittle = nativeendian.little as libc::c_int;
        }
        33 => {
            let maxalign: libc::c_int = 8 as libc::c_ulong as libc::c_int;
            (*h).maxalign = getnumlimit(h, fmt, maxalign)?;
        }
        _ => {
            luaL_error(
                (*h).L,
                format!(
                    "invalid format option '{}'",
                    char::from_u32(opt as _).unwrap()
                ),
            )?;
        }
    }
    return Ok(Knop);
}

unsafe fn getdetails(
    mut h: *mut Header,
    mut totalsize: usize,
    mut fmt: *mut *const libc::c_char,
    mut psize: *mut libc::c_int,
    mut ntoalign: *mut libc::c_int,
) -> Result<KOption, Box<dyn std::error::Error>> {
    let mut opt: KOption = getoption(h, fmt, psize)?;
    let mut align: libc::c_int = *psize;
    if opt as libc::c_uint == Kpaddalign as libc::c_int as libc::c_uint {
        if **fmt as libc::c_int == '\0' as i32
            || getoption(h, fmt, &mut align)? as libc::c_uint
                == Kchar as libc::c_int as libc::c_uint
            || align == 0 as libc::c_int
        {
            luaL_argerror(
                (*h).L,
                1 as libc::c_int,
                "invalid next option for option 'X'",
            )?;
        }
    }
    if align <= 1 as libc::c_int || opt as libc::c_uint == Kchar as libc::c_int as libc::c_uint {
        *ntoalign = 0 as libc::c_int;
    } else {
        if align > (*h).maxalign {
            align = (*h).maxalign;
        }
        if ((align & align - 1 as libc::c_int != 0 as libc::c_int) as libc::c_int
            != 0 as libc::c_int) as libc::c_int as libc::c_long
            != 0
        {
            luaL_argerror(
                (*h).L,
                1 as libc::c_int,
                "format asks for alignment not power of 2",
            )?;
        }
        *ntoalign = align - (totalsize & (align - 1 as libc::c_int) as usize) as libc::c_int
            & align - 1 as libc::c_int;
    }
    return Ok(opt);
}

fn packint(
    mut b: &mut Vec<u8>,
    mut n: u64,
    mut islittle: libc::c_int,
    mut size: libc::c_int,
    mut neg: libc::c_int,
) {
    let mut i: libc::c_int = 0;
    let o = b.len();

    b.push((n & 0xFF) as u8);

    i = 1 as libc::c_int;

    while i < size {
        n >>= 8;
        b.push((n & 0xFF) as u8);
        i += 1;
    }

    if neg != 0 && size > ::core::mem::size_of::<i64>() as libc::c_ulong as libc::c_int {
        b[size_of::<i64>()..].fill(0xFF);
    }

    if islittle == 0 {
        b[o..].reverse();
    }
}

unsafe fn copywithendian(
    mut dest: *mut libc::c_char,
    mut src: *const libc::c_char,
    mut size: libc::c_int,
    mut islittle: libc::c_int,
) {
    if islittle == nativeendian.little as libc::c_int {
        memcpy(
            dest as *mut libc::c_void,
            src as *const libc::c_void,
            size as usize,
        );
    } else {
        dest = dest.offset((size - 1 as libc::c_int) as isize);
        loop {
            let fresh22 = size;
            size = size - 1;
            if !(fresh22 != 0 as libc::c_int) {
                break;
            }
            let fresh23 = src;
            src = src.offset(1);
            let fresh24 = dest;
            dest = dest.offset(-1);
            *fresh24 = *fresh23;
        }
    };
}

unsafe fn str_pack(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut h: Header = Header {
        L: 0 as *mut Thread,
        islittle: 0,
        maxalign: 0,
    };
    let mut fmt: *const libc::c_char = luaL_checklstring(L, 1 as libc::c_int, 0 as *mut usize)?;
    let mut arg: libc::c_int = 1 as libc::c_int;
    let mut totalsize: usize = 0 as libc::c_int as usize;
    let mut b = Vec::new();

    initheader(L, &mut h);
    lua_pushnil(L);

    while *fmt as libc::c_int != '\0' as i32 {
        let mut size: libc::c_int = 0;
        let mut ntoalign: libc::c_int = 0;
        let mut opt: KOption = getdetails(&mut h, totalsize, &mut fmt, &mut size, &mut ntoalign)?;
        totalsize = totalsize.wrapping_add((ntoalign + size) as usize);
        loop {
            let fresh25 = ntoalign;
            ntoalign = ntoalign - 1;
            if !(fresh25 > 0 as libc::c_int) {
                break;
            }

            b.push(0);
        }
        arg += 1;
        let mut current_block_33: u64;
        match opt as libc::c_uint {
            0 => {
                let mut n: i64 = luaL_checkinteger(L, arg)?;
                if size < ::core::mem::size_of::<i64>() as libc::c_ulong as libc::c_int {
                    let mut lim: i64 =
                        (1 as libc::c_int as i64) << size * 8 as libc::c_int - 1 as libc::c_int;
                    (((-lim <= n && n < lim) as libc::c_int != 0 as libc::c_int) as libc::c_int
                        as libc::c_long
                        != 0
                        || luaL_argerror(L, arg, "integer overflow")? != 0)
                        as libc::c_int;
                }
                packint(
                    &mut b,
                    n as u64,
                    h.islittle,
                    size,
                    (n < 0 as libc::c_int as i64) as libc::c_int,
                );
                current_block_33 = 3222590281903869779;
            }
            1 => {
                let mut n_0: i64 = luaL_checkinteger(L, arg)?;
                if size < ::core::mem::size_of::<i64>() as libc::c_ulong as libc::c_int {
                    ((((n_0 as u64) < (1 as libc::c_int as u64) << size * 8 as libc::c_int)
                        as libc::c_int
                        != 0 as libc::c_int) as libc::c_int as libc::c_long
                        != 0
                        || luaL_argerror(L, arg, "unsigned overflow")? != 0)
                        as libc::c_int;
                }
                packint(&mut b, n_0 as u64, h.islittle, size, 0 as libc::c_int);
                current_block_33 = 3222590281903869779;
            }
            2 => {
                let mut f: libc::c_float = luaL_checknumber(L, arg)? as libc::c_float;
                let mut buff = [0; size_of::<f32>()];

                copywithendian(
                    buff.as_mut_ptr().cast(),
                    &mut f as *mut libc::c_float as *mut libc::c_char,
                    ::core::mem::size_of::<libc::c_float>() as libc::c_ulong as libc::c_int,
                    h.islittle,
                );

                b.extend_from_slice(&buff[..size as usize]);
                current_block_33 = 3222590281903869779;
            }
            3 | 4 => {
                let mut f_0: f64 = luaL_checknumber(L, arg)?;
                let mut buff_0 = [0; size_of::<f64>()];

                copywithendian(
                    buff_0.as_mut_ptr().cast(),
                    &mut f_0 as *mut f64 as *mut libc::c_char,
                    ::core::mem::size_of::<f64>() as libc::c_ulong as libc::c_int,
                    h.islittle,
                );

                b.extend_from_slice(&buff_0[..size as usize]);
                current_block_33 = 3222590281903869779;
            }
            5 => {
                let mut len: usize = 0;
                let mut s: *const libc::c_char = luaL_checklstring(L, arg, &mut len)?;
                (((len <= size as usize) as libc::c_int != 0 as libc::c_int) as libc::c_int
                    as libc::c_long
                    != 0
                    || luaL_argerror(L, arg, "string longer than given size")? != 0)
                    as libc::c_int;
                b.extend_from_slice(std::slice::from_raw_parts(s.cast(), len));
                loop {
                    let fresh27 = len;
                    len = len.wrapping_add(1);
                    if !(fresh27 < size as usize) {
                        break;
                    }

                    b.push(0);
                }
                current_block_33 = 3222590281903869779;
            }
            6 => {
                let mut len_0: usize = 0;
                let mut s_0: *const libc::c_char = luaL_checklstring(L, arg, &mut len_0)?;
                (((size >= ::core::mem::size_of::<usize>() as libc::c_ulong as libc::c_int
                    || len_0 < (1 as libc::c_int as usize) << size * 8 as libc::c_int)
                    as libc::c_int
                    != 0 as libc::c_int) as libc::c_int as libc::c_long
                    != 0
                    || luaL_argerror(L, arg, "string length does not fit in given size")? != 0)
                    as libc::c_int;
                packint(&mut b, len_0 as u64, h.islittle, size, 0 as libc::c_int);
                b.extend_from_slice(std::slice::from_raw_parts(s_0.cast(), len_0));
                totalsize = totalsize.wrapping_add(len_0);
                current_block_33 = 3222590281903869779;
            }
            7 => {
                let mut len_1: usize = 0;
                let mut s_1: *const libc::c_char = luaL_checklstring(L, arg, &mut len_1)?;
                (((strlen(s_1) == len_1) as libc::c_int != 0 as libc::c_int) as libc::c_int
                    as libc::c_long
                    != 0
                    || luaL_argerror(L, arg, "string contains zeros")? != 0)
                    as libc::c_int;
                b.extend_from_slice(std::slice::from_raw_parts(s_1.cast(), len_1));
                b.push(0);
                totalsize = totalsize.wrapping_add(len_1.wrapping_add(1 as libc::c_int as usize));
                current_block_33 = 3222590281903869779;
            }
            8 => {
                b.push(0);
                current_block_33 = 12790994980371924011;
            }
            9 | 10 => {
                current_block_33 = 12790994980371924011;
            }
            _ => {
                current_block_33 = 3222590281903869779;
            }
        }
        match current_block_33 {
            12790994980371924011 => {
                arg -= 1;
            }
            _ => {}
        }
    }

    lua_pushlstring(L, b);

    return Ok(1 as libc::c_int);
}

unsafe fn str_packsize(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut h: Header = Header {
        L: 0 as *mut Thread,
        islittle: 0,
        maxalign: 0,
    };
    let mut fmt: *const libc::c_char = luaL_checklstring(L, 1 as libc::c_int, 0 as *mut usize)?;
    let mut totalsize: usize = 0 as libc::c_int as usize;
    initheader(L, &mut h);
    while *fmt as libc::c_int != '\0' as i32 {
        let mut size: libc::c_int = 0;
        let mut ntoalign: libc::c_int = 0;
        let mut opt: KOption = getdetails(&mut h, totalsize, &mut fmt, &mut size, &mut ntoalign)?;
        (((opt as libc::c_uint != Kstring as libc::c_int as libc::c_uint
            && opt as libc::c_uint != Kzstr as libc::c_int as libc::c_uint)
            as libc::c_int
            != 0 as libc::c_int) as libc::c_int as libc::c_long
            != 0
            || luaL_argerror(L, 1 as libc::c_int, "variable-length format")? != 0)
            as libc::c_int;
        size += ntoalign;
        (((totalsize
            <= (if (::core::mem::size_of::<usize>() as libc::c_ulong)
                < ::core::mem::size_of::<libc::c_int>() as libc::c_ulong
            {
                !(0 as libc::c_int as usize)
            } else {
                2147483647 as libc::c_int as usize
            })
            .wrapping_sub(size as usize)) as libc::c_int
            != 0 as libc::c_int) as libc::c_int as libc::c_long
            != 0
            || luaL_argerror(L, 1 as libc::c_int, "format result too large")? != 0)
            as libc::c_int;
        totalsize = totalsize.wrapping_add(size as usize);
    }
    lua_pushinteger(L, totalsize as i64);
    return Ok(1 as libc::c_int);
}

unsafe fn unpackint(
    mut L: *const Thread,
    mut str: *const libc::c_char,
    mut islittle: libc::c_int,
    mut size: libc::c_int,
    mut issigned: libc::c_int,
) -> Result<i64, Box<dyn std::error::Error>> {
    let mut res: u64 = 0 as libc::c_int as u64;
    let mut i: libc::c_int = 0;
    let mut limit: libc::c_int =
        if size <= ::core::mem::size_of::<i64>() as libc::c_ulong as libc::c_int {
            size
        } else {
            ::core::mem::size_of::<i64>() as libc::c_ulong as libc::c_int
        };
    i = limit - 1 as libc::c_int;
    while i >= 0 as libc::c_int {
        res <<= 8 as libc::c_int;
        res |= *str.offset(
            (if islittle != 0 {
                i
            } else {
                size - 1 as libc::c_int - i
            }) as isize,
        ) as libc::c_uchar as u64;
        i -= 1;
    }
    if size < ::core::mem::size_of::<i64>() as libc::c_ulong as libc::c_int {
        if issigned != 0 {
            let mut mask: u64 =
                (1 as libc::c_int as u64) << size * 8 as libc::c_int - 1 as libc::c_int;
            res = (res ^ mask).wrapping_sub(mask);
        }
    } else if size > ::core::mem::size_of::<i64>() as libc::c_ulong as libc::c_int {
        let mut mask_0: libc::c_int = if issigned == 0 || res as i64 >= 0 as libc::c_int as i64 {
            0 as libc::c_int
        } else {
            ((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int
        };
        i = limit;
        while i < size {
            if ((*str.offset(
                (if islittle != 0 {
                    i
                } else {
                    size - 1 as libc::c_int - i
                }) as isize,
            ) as libc::c_uchar as libc::c_int
                != mask_0) as libc::c_int
                != 0 as libc::c_int) as libc::c_int as libc::c_long
                != 0
            {
                luaL_error(
                    L,
                    format!("{size}-byte integer does not fit into Lua Integer"),
                )?;
            }
            i += 1;
        }
    }
    return Ok(res as i64);
}

unsafe fn str_unpack(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut h: Header = Header {
        L: 0 as *mut Thread,
        islittle: 0,
        maxalign: 0,
    };
    let mut fmt: *const libc::c_char = luaL_checklstring(L, 1 as libc::c_int, 0 as *mut usize)?;
    let mut ld: usize = 0;
    let mut data: *const libc::c_char = luaL_checklstring(L, 2 as libc::c_int, &mut ld)?;
    let mut pos: usize = (posrelatI(
        luaL_optinteger(L, 3 as libc::c_int, 1 as libc::c_int as i64)?,
        ld,
    ))
    .wrapping_sub(1 as libc::c_int as usize);
    let mut n: libc::c_int = 0 as libc::c_int;
    (((pos <= ld) as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long != 0
        || luaL_argerror(L, 3 as libc::c_int, "initial position out of string")? != 0)
        as libc::c_int;
    initheader(L, &mut h);
    while *fmt as libc::c_int != '\0' as i32 {
        let mut size: libc::c_int = 0;
        let mut ntoalign: libc::c_int = 0;
        let mut opt: KOption = getdetails(&mut h, pos, &mut fmt, &mut size, &mut ntoalign)?;
        ((((ntoalign as usize).wrapping_add(size as usize) <= ld.wrapping_sub(pos)) as libc::c_int
            != 0 as libc::c_int) as libc::c_int as libc::c_long
            != 0
            || luaL_argerror(L, 2 as libc::c_int, "data string too short")? != 0)
            as libc::c_int;
        pos = pos.wrapping_add(ntoalign as usize);
        luaL_checkstack(
            L,
            2,
            b"too many results\0" as *const u8 as *const libc::c_char,
        )?;
        n += 1;
        match opt as libc::c_uint {
            0 | 1 => {
                let mut res: i64 = unpackint(
                    L,
                    data.offset(pos as isize),
                    h.islittle,
                    size,
                    (opt as libc::c_uint == Kint as libc::c_int as libc::c_uint) as libc::c_int,
                )?;
                lua_pushinteger(L, res);
            }
            2 => {
                let mut f: libc::c_float = 0.;
                copywithendian(
                    &mut f as *mut libc::c_float as *mut libc::c_char,
                    data.offset(pos as isize),
                    ::core::mem::size_of::<libc::c_float>() as libc::c_ulong as libc::c_int,
                    h.islittle,
                );
                lua_pushnumber(L, f as f64);
            }
            3 => {
                let mut f_0: f64 = 0.;
                copywithendian(
                    &mut f_0 as *mut f64 as *mut libc::c_char,
                    data.offset(pos as isize),
                    ::core::mem::size_of::<f64>() as libc::c_ulong as libc::c_int,
                    h.islittle,
                );
                lua_pushnumber(L, f_0);
            }
            4 => {
                let mut f_1: libc::c_double = 0.;
                copywithendian(
                    &mut f_1 as *mut libc::c_double as *mut libc::c_char,
                    data.offset(pos as isize),
                    ::core::mem::size_of::<libc::c_double>() as libc::c_ulong as libc::c_int,
                    h.islittle,
                );
                lua_pushnumber(L, f_1);
            }
            5 => {
                lua_pushlstring(
                    L,
                    std::slice::from_raw_parts(data.offset(pos as isize).cast(), size as usize),
                );
            }
            6 => {
                let mut len: usize = unpackint(
                    L,
                    data.offset(pos as isize),
                    h.islittle,
                    size,
                    0 as libc::c_int,
                )? as usize;
                (((len <= ld.wrapping_sub(pos).wrapping_sub(size as usize)) as libc::c_int
                    != 0 as libc::c_int) as libc::c_int as libc::c_long
                    != 0
                    || luaL_argerror(L, 2 as libc::c_int, "data string too short")? != 0)
                    as libc::c_int;
                lua_pushlstring(
                    L,
                    std::slice::from_raw_parts(
                        data.offset(pos as isize).offset(size as isize).cast(),
                        len,
                    ),
                );
                pos = pos.wrapping_add(len);
            }
            7 => {
                let mut len_0: usize = strlen(data.offset(pos as isize));
                (((pos.wrapping_add(len_0) < ld) as libc::c_int != 0 as libc::c_int) as libc::c_int
                    as libc::c_long
                    != 0
                    || luaL_argerror(L, 2 as libc::c_int, "unfinished string for format 'z'")? != 0)
                    as libc::c_int;
                lua_pushlstring(
                    L,
                    std::slice::from_raw_parts(data.offset(pos as isize).cast(), len_0),
                );
                pos = pos.wrapping_add(len_0.wrapping_add(1 as libc::c_int as usize));
            }
            9 | 8 | 10 => {
                n -= 1;
            }
            _ => {}
        }
        pos = pos.wrapping_add(size as usize);
    }
    lua_pushinteger(L, pos.wrapping_add(1 as libc::c_int as usize) as i64);
    return Ok(n + 1 as libc::c_int);
}

static mut strlib: [luaL_Reg; 17] = [
    {
        let mut init = luaL_Reg {
            name: b"byte\0" as *const u8 as *const libc::c_char,
            func: Some(str_byte),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"char\0" as *const u8 as *const libc::c_char,
            func: Some(str_char),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"find\0" as *const u8 as *const libc::c_char,
            func: Some(str_find),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"gmatch\0" as *const u8 as *const libc::c_char,
            func: Some(gmatch),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"gsub\0" as *const u8 as *const libc::c_char,
            func: Some(str_gsub),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"len\0" as *const u8 as *const libc::c_char,
            func: Some(str_len),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"lower\0" as *const u8 as *const libc::c_char,
            func: Some(str_lower),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"match\0" as *const u8 as *const libc::c_char,
            func: Some(str_match),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"rep\0" as *const u8 as *const libc::c_char,
            func: Some(str_rep),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"reverse\0" as *const u8 as *const libc::c_char,
            func: Some(str_reverse),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"upper\0" as *const u8 as *const libc::c_char,
            func: Some(str_upper),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"pack\0" as *const u8 as *const libc::c_char,
            func: Some(str_pack),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"packsize\0" as *const u8 as *const libc::c_char,
            func: Some(str_packsize),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"unpack\0" as *const u8 as *const libc::c_char,
            func: Some(str_unpack),
        };
        init
    },
];
