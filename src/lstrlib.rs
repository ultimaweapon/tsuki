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

#[derive(Copy, Clone)]
#[repr(C)]
pub union C2RustUnnamed_0 {
    pub dummy: libc::c_int,
    pub little: libc::c_char,
}

pub type KOption = libc::c_uint;

#[derive(Copy, Clone)]
#[repr(C)]
pub struct GMatchState {
    pub src: *const libc::c_char,
    pub p: *const libc::c_char,
    pub lastmatch: *const libc::c_char,
    pub ms: MatchState,
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

unsafe fn arith_idiv(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    return arith(
        L,
        6 as libc::c_int,
        b"__idiv\0" as *const u8 as *const libc::c_char,
    );
}

static mut stringmetamethods: [luaL_Reg; 10] = [{
    let mut init = luaL_Reg {
        name: b"__idiv\0" as *const u8 as *const libc::c_char,
        func: Some(arith_idiv),
    };
    init
}];

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

static mut nativeendian: C2RustUnnamed_0 = C2RustUnnamed_0 {
    dummy: 1 as libc::c_int,
};

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
            name: b"gmatch\0" as *const u8 as *const libc::c_char,
            func: Some(gmatch),
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
