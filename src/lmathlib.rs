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

use crate::lapi::{
    lua_compare, lua_createtable, lua_gettop, lua_isinteger, lua_newuserdatauv, lua_pushboolean,
    lua_pushinteger, lua_pushnil, lua_pushnumber, lua_pushstring, lua_pushvalue, lua_setfield,
    lua_tointegerx, lua_touserdata, lua_type,
};
use crate::lauxlib::{
    luaL_Reg, luaL_argerror, luaL_checkany, luaL_checkinteger, luaL_checknumber, luaL_error,
    luaL_optinteger, luaL_optnumber, luaL_setfuncs,
};
use crate::{Thread, lua_settop};
use libc::time;
use libm::{
    acos, asin, atan2, ceil, cos, exp, fabs, floor, fmod, log, log2, log10, sin, sqrt, tan,
};
use std::boxed::Box;
use std::ffi::c_int;
use std::ptr::null_mut;

#[derive(Copy, Clone)]
#[repr(C)]
struct RanState {
    s: [libc::c_ulong; 4],
}

unsafe extern "C" fn rotl(mut x: libc::c_ulong, mut n: libc::c_int) -> libc::c_ulong {
    return x << n | (x & 0xffffffffffffffff as libc::c_ulong) >> 64 as libc::c_int - n;
}

unsafe extern "C" fn nextrand(mut state: *mut libc::c_ulong) -> libc::c_ulong {
    let mut state0: libc::c_ulong = *state.offset(0 as libc::c_int as isize);
    let mut state1: libc::c_ulong = *state.offset(1 as libc::c_int as isize);
    let mut state2: libc::c_ulong = *state.offset(2 as libc::c_int as isize) ^ state0;
    let mut state3: libc::c_ulong = *state.offset(3 as libc::c_int as isize) ^ state1;
    let mut res: libc::c_ulong = (rotl(
        state1.wrapping_mul(5 as libc::c_int as libc::c_ulong),
        7 as libc::c_int,
    ))
    .wrapping_mul(9 as libc::c_int as libc::c_ulong);
    *state.offset(0 as libc::c_int as isize) = state0 ^ state3;
    *state.offset(1 as libc::c_int as isize) = state1 ^ state2;
    *state.offset(2 as libc::c_int as isize) = state2 ^ state1 << 17 as libc::c_int;
    *state.offset(3 as libc::c_int as isize) = rotl(state3, 45 as libc::c_int);
    return res;
}

unsafe extern "C" fn I2d(mut x: libc::c_ulong) -> f64 {
    let mut sx: libc::c_long = ((x & 0xffffffffffffffff as libc::c_ulong)
        >> 64 as libc::c_int - 53 as libc::c_int) as libc::c_long;
    let mut res: f64 = sx as f64
        * (0.5f64
            / ((1 as libc::c_int as libc::c_ulong) << 53 as libc::c_int - 1 as libc::c_int)
                as libc::c_double);
    if sx < 0 as libc::c_int as libc::c_long {
        res += 1.0f64;
    }
    return res;
}

unsafe extern "C" fn project(mut ran: u64, mut n: u64, mut state: *mut RanState) -> u64 {
    if n & n.wrapping_add(1 as libc::c_int as u64) == 0 as libc::c_int as u64 {
        return ran & n;
    } else {
        let mut lim: u64 = n;
        lim |= lim >> 1 as libc::c_int;
        lim |= lim >> 2 as libc::c_int;
        lim |= lim >> 4 as libc::c_int;
        lim |= lim >> 8 as libc::c_int;
        lim |= lim >> 16 as libc::c_int;
        lim |= lim >> 32 as libc::c_int;
        loop {
            ran &= lim;
            if !(ran > n) {
                break;
            }
            ran =
                (nextrand(((*state).s).as_mut_ptr()) & 0xffffffffffffffff as libc::c_ulong) as u64;
        }
        return ran;
    };
}

unsafe fn math_random(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut low: i64 = 0;
    let mut up: i64 = 0;
    let mut p: u64 = 0;
    let mut state: *mut RanState = lua_touserdata(
        L,
        -(1000000 as libc::c_int) - 1000 as libc::c_int - 1 as libc::c_int,
    ) as *mut RanState;
    let mut rv: libc::c_ulong = nextrand(((*state).s).as_mut_ptr());
    match lua_gettop(L) {
        0 => {
            lua_pushnumber(L, I2d(rv));
            return Ok(1 as libc::c_int);
        }
        1 => {
            low = 1 as libc::c_int as i64;
            up = luaL_checkinteger(L, 1 as libc::c_int)?;
            if up == 0 as libc::c_int as i64 {
                lua_pushinteger(L, (rv & 0xffffffffffffffff as libc::c_ulong) as u64 as i64);
                return Ok(1 as libc::c_int);
            }
        }
        2 => {
            low = luaL_checkinteger(L, 1 as libc::c_int)?;
            up = luaL_checkinteger(L, 2 as libc::c_int)?;
        }
        _ => {
            return luaL_error(L, "wrong number of arguments");
        }
    }
    (((low <= up) as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long != 0
        || luaL_argerror(L, 1 as libc::c_int, "interval is empty")? != 0) as libc::c_int;
    p = project(
        (rv & 0xffffffffffffffff as libc::c_ulong) as u64,
        (up as u64).wrapping_sub(low as u64),
        state,
    );
    lua_pushinteger(L, p.wrapping_add(low as u64) as i64);
    return Ok(1 as libc::c_int);
}

unsafe fn setseed(mut L: *const Thread, mut state: *mut libc::c_ulong, mut n1: u64, mut n2: u64) {
    let mut i: libc::c_int = 0;
    *state.offset(0 as libc::c_int as isize) = n1 as libc::c_ulong;
    *state.offset(1 as libc::c_int as isize) = 0xff as libc::c_int as libc::c_ulong;
    *state.offset(2 as libc::c_int as isize) = n2 as libc::c_ulong;
    *state.offset(3 as libc::c_int as isize) = 0 as libc::c_int as libc::c_ulong;
    i = 0 as libc::c_int;
    while i < 16 as libc::c_int {
        nextrand(state);
        i += 1;
    }
    lua_pushinteger(L, n1 as i64);
    lua_pushinteger(L, n2 as i64);
}

unsafe fn randseed(mut L: *const Thread, mut state: *mut RanState) {
    let mut seed1: u64 = time(null_mut()) as u64;
    let mut seed2: u64 = L as usize as u64;
    setseed(L, ((*state).s).as_mut_ptr(), seed1, seed2);
}

unsafe fn math_randomseed(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut state: *mut RanState = lua_touserdata(
        L,
        -(1000000 as libc::c_int) - 1000 as libc::c_int - 1 as libc::c_int,
    ) as *mut RanState;
    if lua_type(L, 1 as libc::c_int) == -(1 as libc::c_int) {
        randseed(L, state);
    } else {
        let mut n1: i64 = luaL_checkinteger(L, 1 as libc::c_int)?;
        let mut n2: i64 = luaL_optinteger(L, 2 as libc::c_int, 0 as libc::c_int as i64)?;
        setseed(L, ((*state).s).as_mut_ptr(), n1 as u64, n2 as u64);
    }
    return Ok(2 as libc::c_int);
}

static mut randfuncs: [luaL_Reg; 3] = [
    {
        let mut init = luaL_Reg {
            name: b"random\0" as *const u8 as *const libc::c_char,
            func: Some(math_random),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"randomseed\0" as *const u8 as *const libc::c_char,
            func: Some(math_randomseed),
        };
        init
    },
];

unsafe fn setrandfunc(mut L: *const Thread) -> Result<(), Box<dyn std::error::Error>> {
    let state = lua_newuserdatauv(L, ::core::mem::size_of::<RanState>(), 0) as *mut RanState;
    randseed(L, state);
    lua_settop(L, -(2 as libc::c_int) - 1 as libc::c_int)?;
    luaL_setfuncs(L, &raw const randfuncs as *const luaL_Reg, 1 as libc::c_int)?;
    Ok(())
}

static mut mathlib: [luaL_Reg; 28] = [
    {
        let mut init = luaL_Reg {
            name: b"random\0" as *const u8 as *const libc::c_char,
            func: None,
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"randomseed\0" as *const u8 as *const libc::c_char,
            func: None,
        };
        init
    },
];

pub unsafe fn luaopen_math(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    lua_createtable(
        L,
        0 as libc::c_int,
        (::core::mem::size_of::<[luaL_Reg; 28]>() as libc::c_ulong)
            .wrapping_div(::core::mem::size_of::<luaL_Reg>() as libc::c_ulong)
            .wrapping_sub(1 as libc::c_int as libc::c_ulong) as libc::c_int,
    );
    luaL_setfuncs(L, &raw const mathlib as *const luaL_Reg, 0 as libc::c_int)?;
    setrandfunc(L)?;
    return Ok(1 as libc::c_int);
}
