pub use self::error::*;
pub use self::gc::*;
pub use self::lapi::{
    lua_arith, lua_call, lua_createtable, lua_dump, lua_gettable, lua_gettop, lua_isinteger,
    lua_isstring, lua_newuserdatauv, lua_pcall, lua_pushcclosure, lua_pushinteger, lua_pushlstring,
    lua_pushnil, lua_pushnumber, lua_pushstring, lua_pushvalue, lua_rotate, lua_setfield,
    lua_setmetatable, lua_settop, lua_stringtonumber, lua_toboolean, lua_tointegerx, lua_tolstring,
    lua_tonumberx, lua_topointer, lua_touserdata, lua_type, lua_typename,
};
pub use self::lauxlib::{
    C2RustUnnamed, luaL_Buffer, luaL_Reg, luaL_addlstring, luaL_addstring, luaL_addvalue,
    luaL_argerror, luaL_buffinit, luaL_buffinitsize, luaL_checkinteger, luaL_checklstring,
    luaL_checknumber, luaL_checkstack, luaL_checktype, luaL_error, luaL_getmetafield,
    luaL_loadbufferx, luaL_loadfilex, luaL_newstate, luaL_optinteger, luaL_optlstring,
    luaL_prepbuffsize, luaL_pushresult, luaL_pushresultsize, luaL_requiref, luaL_setfuncs,
    luaL_tolstring, luaL_typeerror,
};
pub use self::lbaselib::luaopen_base;
pub use self::lstate::{lua_State, lua_close, lua_newthread};

use std::ffi::c_int;

mod error;
mod gc;
mod lapi;
mod lauxlib;
mod lbaselib;
mod lcode;
mod lctype;
mod ldebug;
mod ldo;
mod ldump;
mod lfunc;
mod lgc;
mod llex;
mod lmathlib;
mod lmem;
mod lobject;
mod lopcodes;
mod lparser;
mod lstate;
mod lstring;
mod lstrlib;
mod ltable;
mod ltablib;
mod ltm;
mod lundump;
mod lvm;
mod lzio;

#[inline(always)]
pub unsafe fn lua_pop(td: *mut lua_State, n: c_int) -> Result<(), Box<dyn std::error::Error>> {
    unsafe { lua_settop(td, -(n) - 1) }
}

#[inline(always)]
unsafe extern "C" fn api_incr_top(td: *mut lua_State) {
    unsafe { (*td).top.p = ((*td).top.p).offset(1) };

    if unsafe { (*td).top.p > (*(*td).ci).top.p } {
        panic!("stack overflow");
    }
}
