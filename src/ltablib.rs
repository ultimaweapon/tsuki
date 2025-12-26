#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments
)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::lapi::{
    lua_call, lua_checkstack, lua_compare, lua_createtable, lua_geti, lua_getmetatable, lua_gettop,
    lua_isstring, lua_pushinteger, lua_pushlstring, lua_pushnil, lua_pushstring, lua_pushvalue,
    lua_rawget, lua_rotate, lua_setfield, lua_seti, lua_toboolean, lua_tolstring, lua_type,
    lua_typename,
};
use crate::lauxlib::{
    luaL_Reg, luaL_argerror, luaL_checkinteger, luaL_checktype, luaL_error, luaL_len,
    luaL_optinteger, luaL_optlstring, luaL_setfuncs,
};
use crate::{Thread, lua_pop, lua_settop};
use std::boxed::Box;
use std::ffi::c_int;
use std::vec::Vec;

type IdxT = libc::c_uint;

unsafe fn checkfield(L: *const Thread, key: *const libc::c_char, n: libc::c_int) -> c_int {
    lua_pushstring(L, key);
    (lua_rawget(L, -n) != 0 as libc::c_int) as libc::c_int
}

unsafe fn tinsert(L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut pos: i64 = 0;
    checktab(
        L,
        1 as libc::c_int,
        1 as libc::c_int | 2 as libc::c_int | 4 as libc::c_int,
    )?;
    let mut e: i64 = luaL_len(L, 1 as libc::c_int)?;
    e = (e as u64).wrapping_add(1 as libc::c_int as u64) as i64;
    match lua_gettop(L) {
        2 => {
            pos = e;
        }
        3 => {
            let mut i: i64 = 0;
            pos = luaL_checkinteger(L, 2 as libc::c_int)?;
            ((((pos as u64).wrapping_sub(1 as libc::c_uint as u64) < e as u64) as libc::c_int
                != 0 as libc::c_int) as libc::c_int as libc::c_long
                != 0
                || luaL_argerror(L, 2 as libc::c_int, "position out of bounds")? != 0)
                as libc::c_int;
            i = e;
            while i > pos {
                lua_geti(L, 1 as libc::c_int, i - 1 as libc::c_int as i64)?;
                lua_seti(L, 1 as libc::c_int, i)?;
                i -= 1;
            }
        }
        _ => {
            return luaL_error(L, "wrong number of arguments to 'insert'");
        }
    }
    lua_seti(L, 1 as libc::c_int, pos)?;
    return Ok(0 as libc::c_int);
}

unsafe fn tremove(L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    checktab(
        L,
        1 as libc::c_int,
        1 as libc::c_int | 2 as libc::c_int | 4 as libc::c_int,
    )?;
    let size: i64 = luaL_len(L, 1 as libc::c_int)?;
    let mut pos: i64 = luaL_optinteger(L, 2 as libc::c_int, size)?;
    if pos != size {
        ((((pos as u64).wrapping_sub(1 as libc::c_uint as u64) <= size as u64) as libc::c_int
            != 0 as libc::c_int) as libc::c_int as libc::c_long
            != 0
            || luaL_argerror(L, 2 as libc::c_int, "position out of bounds")? != 0)
            as libc::c_int;
    }
    lua_geti(L, 1 as libc::c_int, pos)?;
    while pos < size {
        lua_geti(L, 1 as libc::c_int, pos + 1 as libc::c_int as i64)?;
        lua_seti(L, 1 as libc::c_int, pos)?;
        pos += 1;
    }
    lua_pushnil(L);
    lua_seti(L, 1 as libc::c_int, pos)?;
    return Ok(1 as libc::c_int);
}

unsafe fn tmove(L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let f: i64 = luaL_checkinteger(L, 2 as libc::c_int)?;
    let e: i64 = luaL_checkinteger(L, 3 as libc::c_int)?;
    let t: i64 = luaL_checkinteger(L, 4 as libc::c_int)?;
    let tt: libc::c_int = if !(lua_type(L, 5 as libc::c_int) <= 0 as libc::c_int) {
        5 as libc::c_int
    } else {
        1 as libc::c_int
    };
    checktab(L, 1 as libc::c_int, 1 as libc::c_int)?;
    checktab(L, tt, 2 as libc::c_int)?;
    if e >= f {
        let mut n: i64 = 0;
        let mut i: i64 = 0;
        (((f > 0 as libc::c_int as i64 || e < 0x7fffffffffffffff as libc::c_longlong + f)
            as libc::c_int
            != 0 as libc::c_int) as libc::c_int as libc::c_long
            != 0
            || luaL_argerror(L, 3 as libc::c_int, "too many elements to move")? != 0)
            as libc::c_int;
        n = e - f + 1 as libc::c_int as i64;
        (((t <= 0x7fffffffffffffff as libc::c_longlong - n + 1 as libc::c_int as libc::c_longlong)
            as libc::c_int
            != 0 as libc::c_int) as libc::c_int as libc::c_long
            != 0
            || luaL_argerror(L, 4 as libc::c_int, "destination wrap around")? != 0)
            as libc::c_int;
        if t > e || t <= f || tt != 1 && lua_compare(L, 1, tt, 0)? == 0 {
            i = 0 as libc::c_int as i64;
            while i < n {
                lua_geti(L, 1 as libc::c_int, f + i)?;
                lua_seti(L, tt, t + i)?;
                i += 1;
            }
        } else {
            i = n - 1 as libc::c_int as i64;
            while i >= 0 as libc::c_int as i64 {
                lua_geti(L, 1 as libc::c_int, f + i)?;
                lua_seti(L, tt, t + i)?;
                i -= 1;
            }
        }
    }
    lua_pushvalue(L, tt);
    return Ok(1 as libc::c_int);
}

unsafe fn tpack(L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut i: libc::c_int = 0;
    let n: libc::c_int = lua_gettop(L);
    lua_createtable(L, n, 1 as libc::c_int);
    lua_rotate(L, 1 as libc::c_int, 1 as libc::c_int);
    i = n;
    while i >= 1 as libc::c_int {
        lua_seti(L, 1 as libc::c_int, i as i64)?;
        i -= 1;
    }
    lua_pushinteger(L, n as i64);
    lua_setfield(
        L,
        1 as libc::c_int,
        b"n\0" as *const u8 as *const libc::c_char,
    )?;
    return Ok(1 as libc::c_int);
}

static mut tab_funcs: [luaL_Reg; 8] = [
    {
        let init = luaL_Reg {
            name: b"insert\0" as *const u8 as *const libc::c_char,
            func: Some(tinsert),
        };
        init
    },
    {
        let init = luaL_Reg {
            name: b"pack\0" as *const u8 as *const libc::c_char,
            func: Some(tpack),
        };
        init
    },
    {
        let init = luaL_Reg {
            name: b"remove\0" as *const u8 as *const libc::c_char,
            func: Some(tremove),
        };
        init
    },
    {
        let init = luaL_Reg {
            name: b"move\0" as *const u8 as *const libc::c_char,
            func: Some(tmove),
        };
        init
    },
];
