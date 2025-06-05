pub use self::builder::*;
pub use self::error::*;
pub use self::function::*;
pub use self::gc::*;
pub use self::lapi::{
    lua_arith, lua_call, lua_closeslot, lua_createtable, lua_getglobal, lua_getiuservalue,
    lua_gettable, lua_gettop, lua_getupvalue, lua_iscfunction, lua_isinteger, lua_isstring,
    lua_isuserdata, lua_load, lua_newuserdatauv, lua_pcall, lua_pushcclosure, lua_pushinteger,
    lua_pushlstring, lua_pushnil, lua_pushnumber, lua_pushstring, lua_pushthread, lua_pushvalue,
    lua_rotate, lua_setfield, lua_setiuservalue, lua_setmetatable, lua_settable, lua_settop,
    lua_stringtonumber, lua_toboolean, lua_tocfunction, lua_tointegerx, lua_tolstring,
    lua_tonumberx, lua_tothread, lua_touserdata, lua_type, lua_typename, lua_upvalueid,
    lua_upvaluejoin, lua_xmove,
};
pub use self::lauxlib::{
    luaL_Reg, luaL_argerror, luaL_checkinteger, luaL_checklstring, luaL_checknumber,
    luaL_checkstack, luaL_checktype, luaL_error, luaL_getmetafield, luaL_optinteger,
    luaL_optlstring, luaL_setfuncs, luaL_tolstring, luaL_typeerror,
};
pub use self::lstate::lua_closethread;
pub use self::parser::*;
pub use self::table::*;
pub use self::thread::*;

use self::lmem::luaM_free_;
use self::lobject::{TString, TValue, Table};
use std::cell::{Cell, UnsafeCell};
use std::ffi::c_int;
use std::marker::PhantomPinned;
use std::pin::Pin;
use std::ptr::null_mut;
use std::rc::Rc;

mod builder;
mod error;
mod function;
mod gc;
mod lapi;
mod lauxlib;
mod lbaselib;
mod lcode;
mod lctype;
mod ldebug;
mod ldo;
mod lfunc;
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
mod lvm;
mod lzio;
mod parser;
mod table;
mod thread;

#[inline(always)]
pub unsafe fn lua_pop(th: *const Thread, n: c_int) -> Result<(), Box<dyn std::error::Error>> {
    unsafe { lua_settop(th, -(n) - 1) }
}

#[inline(always)]
unsafe fn api_incr_top(th: *const Thread) {
    unsafe { (*th).top.add(1) };

    if unsafe { (*th).top.get() > (*(*th).ci.get()).top } {
        panic!("stack overflow");
    }
}

/// Global states shared with all Lua threads.
///
/// Use [`Builder`] to get the instance of this type.
pub struct Lua {
    currentwhite: Cell<u8>,
    all: Cell<*const Object>,
    refs: Cell<*const Object>,
    gc: Gc,
    GCestimate: Cell<usize>,
    lastatomic: Cell<usize>,
    strt: UnsafeCell<StringTable>,
    l_registry: UnsafeCell<TValue>,
    nilvalue: UnsafeCell<TValue>,
    seed: libc::c_uint,
    gcstate: Cell<u8>,
    gcstopem: Cell<u8>,
    gcstp: Cell<u8>,
    gcpause: Cell<u8>,
    gcstepmul: Cell<u8>,
    gcstepsize: Cell<u8>,
    sweepgc: Cell<*mut *const Object>,
    gray: Cell<*const Object>,
    grayagain: Cell<*const Object>,
    weak: Cell<*const Object>,
    ephemeron: Cell<*const Object>,
    allweak: Cell<*const Object>,
    fixedgc: Cell<*const Object>,
    twups: Cell<*const Thread>,
    tmname: [Cell<*mut TString>; 25],
    mt: [Cell<*mut Table>; 9],
    _phantom: PhantomPinned,
}

impl Lua {
    /// Create a new Lua thread (AKA coroutine).
    #[inline(always)]
    pub fn spawn(self: &Pin<Rc<Self>>) -> Ref<Thread> {
        Thread::new(self)
    }

    fn reset_gray(&self) {
        self.grayagain.set(null_mut());
        self.gray.set(null_mut());
        self.ephemeron.set(null_mut());
        self.allweak.set(null_mut());
        self.weak.set(null_mut());
    }
}

impl Drop for Lua {
    fn drop(&mut self) {
        unsafe { luaC_freeallobjects(self) };
        unsafe {
            luaM_free_(
                self,
                (*self.strt.get()).hash as *mut libc::c_void,
                ((*self.strt.get()).size as usize).wrapping_mul(size_of::<*mut TString>()),
            )
        };
    }
}

#[repr(C)]
struct StringTable {
    hash: *mut *mut TString,
    nuse: libc::c_int,
    size: libc::c_int,
}
