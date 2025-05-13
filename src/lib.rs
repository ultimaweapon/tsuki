pub use self::error::*;
pub use self::gc::*;
pub use self::lapi::*;
pub use self::lauxlib::*;
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
mod lmem;
mod lobject;
mod lopcodes;
mod lparser;
mod lstate;
mod lstring;
mod lstrlib;
mod ltable;
mod ltm;
mod lundump;
mod lvm;
mod lzio;

pub unsafe fn lua_pop(td: *mut lua_State, n: c_int) {
    unsafe { lua_settop(td, -(n) - 1) };
}

#[inline(always)]
unsafe extern "C" fn api_incr_top(td: *mut lua_State) {
    unsafe { (*td).top.p = ((*td).top.p).offset(1) };

    if unsafe { (*td).top.p > (*(*td).ci).top.p } {
        panic!("stack overflow");
    }
}
