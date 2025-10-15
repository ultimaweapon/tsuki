use crate::lapi::{
    lua_call, lua_copy, lua_geti, lua_getmetatable, lua_gettop, lua_isstring, lua_next, lua_pcall,
    lua_pushboolean, lua_pushcclosure, lua_pushinteger, lua_pushlstring, lua_pushnil,
    lua_pushstring, lua_pushvalue, lua_rawequal, lua_rawget, lua_rawgeti, lua_rawlen, lua_rawset,
    lua_rotate, lua_setfield, lua_setmetatable, lua_settop, lua_setupvalue, lua_stringtonumber,
    lua_toboolean, lua_tolstring, lua_type, lua_typename,
};
use crate::lauxlib::{
    luaL_Reg, luaL_argerror, luaL_checkany, luaL_checkinteger, luaL_checklstring, luaL_checkstack,
    luaL_checktype, luaL_error, luaL_getmetafield, luaL_optinteger, luaL_optlstring, luaL_setfuncs,
    luaL_tolstring, luaL_typeerror, luaL_where,
};
use crate::ldebug::luaG_runerror;
use crate::{ChunkInfo, Thread, api_incr_top};
use libc::{isalnum, isdigit, strspn, toupper};
use std::boxed::Box;
use std::ffi::{CStr, c_char, c_int, c_void};
use std::io::Write;
use std::ptr::{null, null_mut};
use std::string::{String, ToString};
use std::{format, print, println};

unsafe fn ipairsaux(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut i: i64 = luaL_checkinteger(L, 2 as libc::c_int)?;
    i = (i as u64).wrapping_add(1 as libc::c_int as u64) as i64;
    lua_pushinteger(L, i);

    if lua_geti(L, 1 as libc::c_int, i)? == 0 as libc::c_int {
        Ok(1)
    } else {
        Ok(2)
    }
}

unsafe fn luaB_ipairs(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    luaL_checkany(L, 1 as libc::c_int)?;
    lua_pushcclosure(L, ipairsaux, 0 as libc::c_int);
    lua_pushvalue(L, 1 as libc::c_int);
    lua_pushinteger(L, 0 as libc::c_int as i64);
    return Ok(3 as libc::c_int);
}

unsafe fn generic_reader(
    ud: *mut c_void,
    mut size: *mut usize,
) -> Result<*const c_char, Box<dyn std::error::Error>> {
    let L = ud.cast();

    luaL_checkstack(
        L,
        2,
        b"too many nested functions\0" as *const u8 as *const libc::c_char,
    )?;
    lua_pushvalue(L, 1 as libc::c_int);
    lua_call(L, 0, 1)?;
    if lua_type(L, -(1 as libc::c_int)) == 0 as libc::c_int {
        lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
        *size = 0 as libc::c_int as usize;
        return Ok(0 as *const libc::c_char);
    } else if ((lua_isstring(L, -(1 as libc::c_int)) == 0) as libc::c_int != 0 as libc::c_int)
        as libc::c_int as libc::c_long
        != 0
    {
        luaL_error(L, "reader function must return a string")?;
    }
    lua_copy(L, -(1 as libc::c_int), 5 as libc::c_int);
    lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;

    Ok(lua_tolstring(L, 5 as libc::c_int, size))
}

unsafe fn dofilecont(mut L: *mut Thread, mut d1: libc::c_int) -> libc::c_int {
    return lua_gettop(L) - 1 as libc::c_int;
}

static mut base_funcs: [luaL_Reg; 21] = [{
    let mut init = luaL_Reg {
        name: b"ipairs\0" as *const u8 as *const libc::c_char,
        func: Some(luaB_ipairs),
    };
    init
}];
