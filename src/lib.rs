pub use self::builder::*;
pub use self::error::*;
pub use self::function::*;
pub use self::gc::*;
pub use self::lapi::{
    lua_arith, lua_call, lua_closeslot, lua_createtable, lua_dump, lua_getglobal,
    lua_getiuservalue, lua_gettable, lua_gettop, lua_getupvalue, lua_iscfunction, lua_isinteger,
    lua_isstring, lua_isuserdata, lua_load, lua_newuserdatauv, lua_pcall, lua_pushcclosure,
    lua_pushinteger, lua_pushlstring, lua_pushnil, lua_pushnumber, lua_pushstring, lua_pushthread,
    lua_pushvalue, lua_rotate, lua_setfield, lua_setiuservalue, lua_setmetatable, lua_settable,
    lua_settop, lua_stringtonumber, lua_toboolean, lua_tocfunction, lua_tointegerx, lua_tolstring,
    lua_tonumberx, lua_tothread, lua_touserdata, lua_type, lua_typename, lua_upvalueid,
    lua_upvaluejoin, lua_xmove,
};
pub use self::lauxlib::{
    luaL_Reg, luaL_argerror, luaL_checkinteger, luaL_checklstring, luaL_checknumber,
    luaL_checkstack, luaL_checktype, luaL_error, luaL_getmetafield, luaL_optinteger,
    luaL_optlstring, luaL_setfuncs, luaL_tolstring, luaL_typeerror,
};
pub use self::lstate::lua_closethread;
pub use self::table::*;
pub use self::thread::*;

use self::lmem::luaM_free_;
use self::lobject::{StackValue, TString, TValue, Table};
use std::alloc::{Layout, handle_alloc_error};
use std::cell::{Cell, UnsafeCell};
use std::ffi::c_int;
use std::marker::PhantomPinned;
use std::ops::Deref;
use std::pin::Pin;
use std::ptr::{addr_of_mut, null, null_mut};
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
mod ldump;
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
    pub fn spawn(self: &Pin<Rc<Self>>) -> *mut Thread {
        // Create new thread.
        let layout = Layout::new::<Thread>();
        let th = unsafe { Object::new(self.deref(), 8, layout).cast::<Thread>() };

        unsafe { addr_of_mut!((*th).global).write(self.deref()) };
        unsafe { addr_of_mut!((*th).stack).write(Cell::new(null_mut())) };
        unsafe { addr_of_mut!((*th).ci).write(Cell::new(null_mut())) };
        unsafe { addr_of_mut!((*th).nci).write(Cell::new(0)) };
        unsafe { addr_of_mut!((*th).twups).write(Cell::new(th)) };
        unsafe { addr_of_mut!((*th).hook).write(Cell::new(None)) };
        unsafe { addr_of_mut!((*th).hookmask).write(Cell::new(0)) };
        unsafe { addr_of_mut!((*th).basehookcount).write(Cell::new(0)) };
        unsafe { addr_of_mut!((*th).allowhook).write(Cell::new(1)) };
        unsafe { addr_of_mut!((*th).hookcount).write(Cell::new(0)) };
        unsafe { addr_of_mut!((*th).openupval).write(Cell::new(null_mut())) };
        unsafe { addr_of_mut!((*th).oldpc).write(Cell::new(0)) };

        // Allocate stack.
        let layout = Layout::array::<StackValue>(2 * 20 + 5).unwrap();
        let stack = unsafe { std::alloc::alloc(layout) as *mut StackValue };

        if stack.is_null() {
            handle_alloc_error(layout);
        }

        for i in 0..(2 * 20 + 5) {
            unsafe { (*stack.offset(i)).val.tt_ = 0 | 0 << 4 };
        }

        unsafe { (*th).stack.set(stack) };
        unsafe { addr_of_mut!((*th).top).write(StackPtr::new((*th).stack.get())) };
        unsafe { addr_of_mut!((*th).stack_last).write(Cell::new((*th).stack.get().add(2 * 20))) };
        unsafe { addr_of_mut!((*th).tbclist).write(Cell::new((*th).stack.get())) };

        // Setup base CI.
        let ci = unsafe { (*th).base_ci.get() };

        unsafe { (*ci).previous = null_mut() };
        unsafe { (*ci).next = (*ci).previous };
        unsafe { (*ci).callstatus = 1 << 1 };
        unsafe { (*ci).func = (*th).top.get() };
        unsafe { (*ci).u.savedpc = null() };
        unsafe { (*ci).nresults = 0 };
        unsafe { (*th).top.write_nil() };
        unsafe { (*th).top.add(1) };
        unsafe { (*ci).top = ((*th).top.get()).offset(20) };
        unsafe { (*th).ci.set(ci) };

        th
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
