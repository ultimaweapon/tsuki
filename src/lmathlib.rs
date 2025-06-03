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

use crate::lapi::{lua_compare, lua_pushboolean};
use crate::lauxlib::{luaL_checkany, luaL_optnumber};
use crate::{
    Thread, lua_createtable, lua_gettop, lua_isinteger, lua_newuserdatauv, lua_pushinteger,
    lua_pushnil, lua_pushnumber, lua_pushstring, lua_pushvalue, lua_setfield, lua_settop,
    lua_tointegerx, lua_touserdata, lua_type, luaL_Reg, luaL_argerror, luaL_checkinteger,
    luaL_checknumber, luaL_error, luaL_optinteger, luaL_setfuncs,
};
use libc::time;
use libm::{
    acos, asin, atan2, ceil, cos, exp, fabs, floor, fmod, log, log2, log10, sin, sqrt, tan,
};
use std::ffi::c_int;
use std::ptr::null_mut;

#[derive(Copy, Clone)]
#[repr(C)]
struct RanState {
    s: [libc::c_ulong; 4],
}

unsafe fn math_abs(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    if lua_isinteger(L, 1 as libc::c_int) != 0 {
        let mut n: i64 = lua_tointegerx(L, 1 as libc::c_int, 0 as *mut libc::c_int);
        if n < 0 as libc::c_int as i64 {
            n = (0 as libc::c_uint as u64).wrapping_sub(n as u64) as i64;
        }
        lua_pushinteger(L, n);
    } else {
        lua_pushnumber(L, fabs(luaL_checknumber(L, 1 as libc::c_int)?));
    }
    return Ok(1 as libc::c_int);
}

unsafe fn math_sin(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    lua_pushnumber(L, sin(luaL_checknumber(L, 1 as libc::c_int)?));
    return Ok(1 as libc::c_int);
}

unsafe fn math_cos(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    lua_pushnumber(L, cos(luaL_checknumber(L, 1 as libc::c_int)?));
    return Ok(1 as libc::c_int);
}

unsafe fn math_tan(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    lua_pushnumber(L, tan(luaL_checknumber(L, 1 as libc::c_int)?));
    return Ok(1 as libc::c_int);
}

unsafe fn math_asin(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    lua_pushnumber(L, asin(luaL_checknumber(L, 1 as libc::c_int)?));
    return Ok(1 as libc::c_int);
}

unsafe fn math_acos(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    lua_pushnumber(L, acos(luaL_checknumber(L, 1 as libc::c_int)?));
    return Ok(1 as libc::c_int);
}

unsafe fn math_atan(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut y: f64 = luaL_checknumber(L, 1 as libc::c_int)?;
    let mut x: f64 = luaL_optnumber(L, 2 as libc::c_int, 1 as libc::c_int as f64)?;
    lua_pushnumber(L, atan2(y, x));
    return Ok(1 as libc::c_int);
}

unsafe fn math_toint(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut valid: libc::c_int = 0;
    let mut n: i64 = lua_tointegerx(L, 1 as libc::c_int, &mut valid);
    if (valid != 0 as libc::c_int) as libc::c_int as libc::c_long != 0 {
        lua_pushinteger(L, n);
    } else {
        luaL_checkany(L, 1 as libc::c_int)?;
        lua_pushnil(L);
    }
    return Ok(1 as libc::c_int);
}

unsafe fn pushnumint(mut L: *const Thread, mut d: f64) {
    let mut n: i64 = 0;
    if d >= (-(0x7fffffffffffffff as libc::c_longlong) - 1 as libc::c_int as libc::c_longlong)
        as libc::c_double
        && d < -((-(0x7fffffffffffffff as libc::c_longlong) - 1 as libc::c_int as libc::c_longlong)
            as libc::c_double)
        && {
            n = d as libc::c_longlong;
            1 as libc::c_int != 0
        }
    {
        lua_pushinteger(L, n);
    } else {
        lua_pushnumber(L, d);
    };
}

unsafe fn math_floor(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    if lua_isinteger(L, 1 as libc::c_int) != 0 {
        lua_settop(L, 1 as libc::c_int)?;
    } else {
        let mut d: f64 = floor(luaL_checknumber(L, 1 as libc::c_int)?);
        pushnumint(L, d);
    }
    return Ok(1 as libc::c_int);
}

unsafe fn math_ceil(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    if lua_isinteger(L, 1 as libc::c_int) != 0 {
        lua_settop(L, 1 as libc::c_int)?;
    } else {
        let mut d: f64 = ceil(luaL_checknumber(L, 1 as libc::c_int)?);
        pushnumint(L, d);
    }
    return Ok(1 as libc::c_int);
}

unsafe fn math_fmod(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    if lua_isinteger(L, 1 as libc::c_int) != 0 && lua_isinteger(L, 2 as libc::c_int) != 0 {
        let mut d: i64 = lua_tointegerx(L, 2 as libc::c_int, 0 as *mut libc::c_int);
        if (d as u64).wrapping_add(1 as libc::c_uint as u64) <= 1 as libc::c_uint as u64 {
            (((d != 0 as libc::c_int as i64) as libc::c_int != 0 as libc::c_int) as libc::c_int
                as libc::c_long
                != 0
                || luaL_argerror(L, 2 as libc::c_int, "zero")? != 0) as libc::c_int;
            lua_pushinteger(L, 0 as libc::c_int as i64);
        } else {
            lua_pushinteger(
                L,
                lua_tointegerx(L, 1 as libc::c_int, 0 as *mut libc::c_int) % d,
            );
        }
    } else {
        lua_pushnumber(
            L,
            fmod(
                luaL_checknumber(L, 1 as libc::c_int)?,
                luaL_checknumber(L, 2 as libc::c_int)?,
            ),
        );
    }
    return Ok(1 as libc::c_int);
}

unsafe fn math_modf(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    if lua_isinteger(L, 1 as libc::c_int) != 0 {
        lua_settop(L, 1 as libc::c_int)?;
        lua_pushnumber(L, 0 as libc::c_int as f64);
    } else {
        let mut n: f64 = luaL_checknumber(L, 1 as libc::c_int)?;
        let mut ip: f64 = if n < 0 as libc::c_int as f64 {
            ceil(n)
        } else {
            floor(n)
        };
        pushnumint(L, ip);
        lua_pushnumber(L, if n == ip { 0.0f64 } else { n - ip });
    }
    return Ok(2 as libc::c_int);
}

unsafe fn math_sqrt(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    lua_pushnumber(L, sqrt(luaL_checknumber(L, 1 as libc::c_int)?));
    return Ok(1 as libc::c_int);
}

unsafe fn math_ult(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut a: i64 = luaL_checkinteger(L, 1 as libc::c_int)?;
    let mut b: i64 = luaL_checkinteger(L, 2 as libc::c_int)?;
    lua_pushboolean(L, ((a as u64) < b as u64) as libc::c_int);
    return Ok(1 as libc::c_int);
}

unsafe fn math_log(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut x: f64 = luaL_checknumber(L, 1 as libc::c_int)?;
    let mut res: f64 = 0.;
    if lua_type(L, 2 as libc::c_int) <= 0 as libc::c_int {
        res = log(x);
    } else {
        let mut base: f64 = luaL_checknumber(L, 2 as libc::c_int)?;
        if base == 2.0f64 {
            res = log2(x);
        } else if base == 10.0f64 {
            res = log10(x);
        } else {
            res = log(x) / log(base);
        }
    }
    lua_pushnumber(L, res);
    return Ok(1 as libc::c_int);
}

unsafe fn math_exp(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    lua_pushnumber(L, exp(luaL_checknumber(L, 1 as libc::c_int)?));
    return Ok(1 as libc::c_int);
}

unsafe fn math_deg(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    lua_pushnumber(
        L,
        luaL_checknumber(L, 1 as libc::c_int)?
            * (180.0f64 / 3.141592653589793238462643383279502884f64),
    );
    return Ok(1 as libc::c_int);
}

unsafe fn math_rad(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    lua_pushnumber(
        L,
        luaL_checknumber(L, 1 as libc::c_int)?
            * (3.141592653589793238462643383279502884f64 / 180.0f64),
    );
    return Ok(1 as libc::c_int);
}

unsafe fn math_min(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut n: libc::c_int = lua_gettop(L);
    let mut imin: libc::c_int = 1 as libc::c_int;
    let mut i: libc::c_int = 0;
    (((n >= 1 as libc::c_int) as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
        || luaL_argerror(L, 1 as libc::c_int, "value expected")? != 0) as libc::c_int;
    i = 2 as libc::c_int;
    while i <= n {
        if lua_compare(L, i, imin, 1 as libc::c_int)? != 0 {
            imin = i;
        }
        i += 1;
    }
    lua_pushvalue(L, imin);
    return Ok(1 as libc::c_int);
}

unsafe fn math_max(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut n: libc::c_int = lua_gettop(L);
    let mut imax: libc::c_int = 1 as libc::c_int;
    let mut i: libc::c_int = 0;
    (((n >= 1 as libc::c_int) as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
        || luaL_argerror(L, 1 as libc::c_int, "value expected")? != 0) as libc::c_int;
    i = 2 as libc::c_int;
    while i <= n {
        if lua_compare(L, imax, i, 1 as libc::c_int)? != 0 {
            imax = i;
        }
        i += 1;
    }
    lua_pushvalue(L, imax);
    return Ok(1 as libc::c_int);
}

unsafe fn math_type(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    if lua_type(L, 1 as libc::c_int) == 3 as libc::c_int {
        lua_pushstring(
            L,
            if lua_isinteger(L, 1 as libc::c_int) != 0 {
                b"integer\0" as *const u8 as *const libc::c_char
            } else {
                b"float\0" as *const u8 as *const libc::c_char
            },
        );
    } else {
        luaL_checkany(L, 1 as libc::c_int)?;
        lua_pushnil(L);
    }
    return Ok(1 as libc::c_int);
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
    {
        let mut init = luaL_Reg {
            name: 0 as *const libc::c_char,
            func: None,
        };
        init
    },
];

unsafe fn setrandfunc(mut L: *const Thread) -> Result<(), Box<dyn std::error::Error>> {
    let state = lua_newuserdatauv(L, ::core::mem::size_of::<RanState>(), 0)? as *mut RanState;
    randseed(L, state);
    lua_settop(L, -(2 as libc::c_int) - 1 as libc::c_int)?;
    luaL_setfuncs(L, &raw const randfuncs as *const luaL_Reg, 1 as libc::c_int)?;
    Ok(())
}

static mut mathlib: [luaL_Reg; 28] = [
    {
        let mut init = luaL_Reg {
            name: b"abs\0" as *const u8 as *const libc::c_char,
            func: Some(math_abs),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"acos\0" as *const u8 as *const libc::c_char,
            func: Some(math_acos),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"asin\0" as *const u8 as *const libc::c_char,
            func: Some(math_asin),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"atan\0" as *const u8 as *const libc::c_char,
            func: Some(math_atan),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"ceil\0" as *const u8 as *const libc::c_char,
            func: Some(math_ceil),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"cos\0" as *const u8 as *const libc::c_char,
            func: Some(math_cos),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"deg\0" as *const u8 as *const libc::c_char,
            func: Some(math_deg),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"exp\0" as *const u8 as *const libc::c_char,
            func: Some(math_exp),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"tointeger\0" as *const u8 as *const libc::c_char,
            func: Some(math_toint),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"floor\0" as *const u8 as *const libc::c_char,
            func: Some(math_floor),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"fmod\0" as *const u8 as *const libc::c_char,
            func: Some(math_fmod),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"ult\0" as *const u8 as *const libc::c_char,
            func: Some(math_ult),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"log\0" as *const u8 as *const libc::c_char,
            func: Some(math_log),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"max\0" as *const u8 as *const libc::c_char,
            func: Some(math_max),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"min\0" as *const u8 as *const libc::c_char,
            func: Some(math_min),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"modf\0" as *const u8 as *const libc::c_char,
            func: Some(math_modf),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"rad\0" as *const u8 as *const libc::c_char,
            func: Some(math_rad),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"sin\0" as *const u8 as *const libc::c_char,
            func: Some(math_sin),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"sqrt\0" as *const u8 as *const libc::c_char,
            func: Some(math_sqrt),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"tan\0" as *const u8 as *const libc::c_char,
            func: Some(math_tan),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"type\0" as *const u8 as *const libc::c_char,
            func: Some(math_type),
        };
        init
    },
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
    {
        let mut init = luaL_Reg {
            name: b"pi\0" as *const u8 as *const libc::c_char,
            func: None,
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"huge\0" as *const u8 as *const libc::c_char,
            func: None,
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"maxinteger\0" as *const u8 as *const libc::c_char,
            func: None,
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"mininteger\0" as *const u8 as *const libc::c_char,
            func: None,
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

pub unsafe fn luaopen_math(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    lua_createtable(
        L,
        0 as libc::c_int,
        (::core::mem::size_of::<[luaL_Reg; 28]>() as libc::c_ulong)
            .wrapping_div(::core::mem::size_of::<luaL_Reg>() as libc::c_ulong)
            .wrapping_sub(1 as libc::c_int as libc::c_ulong) as libc::c_int,
    )?;
    luaL_setfuncs(L, &raw const mathlib as *const luaL_Reg, 0 as libc::c_int)?;
    lua_pushnumber(L, 3.141592653589793238462643383279502884f64);
    lua_setfield(
        L,
        -(2 as libc::c_int),
        b"pi\0" as *const u8 as *const libc::c_char,
    )?;
    lua_pushnumber(L, ::core::f64::INFINITY);
    lua_setfield(
        L,
        -(2 as libc::c_int),
        b"huge\0" as *const u8 as *const libc::c_char,
    )?;
    lua_pushinteger(L, 0x7fffffffffffffff as libc::c_longlong);
    lua_setfield(
        L,
        -(2 as libc::c_int),
        b"maxinteger\0" as *const u8 as *const libc::c_char,
    )?;
    lua_pushinteger(
        L,
        -(0x7fffffffffffffff as libc::c_longlong) - 1 as libc::c_int as libc::c_longlong,
    );
    lua_setfield(
        L,
        -(2 as libc::c_int),
        b"mininteger\0" as *const u8 as *const libc::c_char,
    )?;
    setrandfunc(L)?;
    return Ok(1 as libc::c_int);
}
