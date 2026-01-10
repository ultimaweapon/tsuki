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

static mut strlib: [luaL_Reg; 17] = [{
    let mut init = luaL_Reg {
        name: b"gmatch\0" as *const u8 as *const libc::c_char,
        func: Some(gmatch),
    };
    init
}];
