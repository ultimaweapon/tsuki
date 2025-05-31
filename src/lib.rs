pub use self::error::*;
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
    luaL_optlstring, luaL_requiref, luaL_setfuncs, luaL_tolstring, luaL_typeerror,
};
pub use self::lbaselib::luaopen_base;
pub use self::lmathlib::luaopen_math;
pub use self::lstate::lua_closethread;
pub use self::lstrlib::luaopen_string;
pub use self::ltablib::luaopen_table;
pub use self::thread::*;

use self::llex::luaX_init;
use self::lmem::luaM_free_;
use self::lobject::{StackValue, TString, TValue, Table, Value};
use self::lstring::luaS_init;
use self::ltable::{luaH_new, luaH_resize};
use self::ltm::luaT_init;
use std::alloc::{Layout, handle_alloc_error};
use std::cell::{Cell, RefCell, UnsafeCell};
use std::ffi::c_int;
use std::marker::PhantomPinned;
use std::ops::Deref;
use std::pin::Pin;
use std::ptr::{addr_of_mut, null, null_mut};
use std::rc::Rc;

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
mod thread;

#[inline(always)]
pub unsafe fn lua_pop(td: *mut Thread, n: c_int) -> Result<(), Box<dyn std::error::Error>> {
    unsafe { lua_settop(td, -(n) - 1) }
}

#[inline(always)]
unsafe extern "C" fn api_incr_top(td: *mut Thread) {
    unsafe { (*td).top.add(1) };

    if unsafe { (*td).top.get() > (*(*td).ci.get()).top } {
        panic!("stack overflow");
    }
}

/// Global states shared with all Lua threads.
pub struct Lua {
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
    fixedgc: Cell<*mut Object>,
    twups: Cell<*const Thread>,
    tmname: [Cell<*mut TString>; 25],
    mt: [Cell<*mut Table>; 9],
    handle_table: RefCell<Vec<*const Object>>,
    handle_free: RefCell<Vec<usize>>,
    _phantom: PhantomPinned,
}

impl Lua {
    pub fn new() -> Result<Pin<Rc<Self>>, Box<dyn std::error::Error>> {
        let g = Rc::pin(Self {
            gc: Gc::new(size_of::<Self>()),
            GCestimate: Cell::new(0), // TODO: Lua does not initialize this.
            lastatomic: Cell::new(0),
            strt: UnsafeCell::new(StringTable {
                hash: null_mut(),
                nuse: 0,
                size: 0,
            }),
            l_registry: UnsafeCell::new(TValue {
                value_: Value { i: 0 },
                tt_: (0 | 0 << 4),
            }),
            nilvalue: UnsafeCell::new(TValue {
                value_: Value { i: 0 },
                tt_: (0 | 0 << 4),
            }),
            seed: rand::random(),
            gcstate: Cell::new(8),
            gcstopem: Cell::new(0),
            gcstp: Cell::new(2),
            gcpause: Cell::new((200 as libc::c_int / 4 as libc::c_int) as u8),
            gcstepmul: Cell::new((100 as libc::c_int / 4 as libc::c_int) as u8),
            gcstepsize: Cell::new(13 as libc::c_int as u8),
            sweepgc: Cell::new(null_mut()),
            gray: Cell::new(null_mut()),
            grayagain: Cell::new(null_mut()),
            weak: Cell::new(null_mut()),
            ephemeron: Cell::new(null_mut()),
            allweak: Cell::new(null_mut()),
            fixedgc: Cell::new(null_mut()),
            twups: Cell::new(null_mut()),
            tmname: [
                Cell::new(null_mut()),
                Cell::new(null_mut()),
                Cell::new(null_mut()),
                Cell::new(null_mut()),
                Cell::new(null_mut()),
                Cell::new(null_mut()),
                Cell::new(null_mut()),
                Cell::new(null_mut()),
                Cell::new(null_mut()),
                Cell::new(null_mut()),
                Cell::new(null_mut()),
                Cell::new(null_mut()),
                Cell::new(null_mut()),
                Cell::new(null_mut()),
                Cell::new(null_mut()),
                Cell::new(null_mut()),
                Cell::new(null_mut()),
                Cell::new(null_mut()),
                Cell::new(null_mut()),
                Cell::new(null_mut()),
                Cell::new(null_mut()),
                Cell::new(null_mut()),
                Cell::new(null_mut()),
                Cell::new(null_mut()),
                Cell::new(null_mut()),
            ],
            mt: [
                Cell::new(null_mut()),
                Cell::new(null_mut()),
                Cell::new(null_mut()),
                Cell::new(null_mut()),
                Cell::new(null_mut()),
                Cell::new(null_mut()),
                Cell::new(null_mut()),
                Cell::new(null_mut()),
                Cell::new(null_mut()),
            ],
            handle_table: RefCell::default(),
            handle_free: RefCell::default(),
            _phantom: PhantomPinned,
        });

        // Setup registry.
        let td = g.spawn();
        let registry: *mut Table = unsafe { luaH_new(td)? };
        let io: *mut TValue = g.l_registry.get();

        unsafe { (*io).value_.gc = registry as *mut Object };
        unsafe { (*io).tt_ = 5 | 0 << 4 | 1 << 6 };

        unsafe { luaH_resize(td, registry, 2, 0) }?;

        // Create dummy object for LUA_RIDX_MAINTHREAD.
        let io_0 = unsafe { ((*registry).array).offset(1 - 1) as *mut TValue };

        unsafe { (*io_0).value_.gc = luaH_new(td)? as *mut Object };
        unsafe { (*io_0).tt_ = 5 | 0 << 4 | 1 << 6 };

        // Create LUA_RIDX_GLOBALS.
        let io_1 = unsafe { ((*registry).array).offset(2 - 1) as *mut TValue };

        unsafe { (*io_1).value_.gc = luaH_new(td)? as *mut Object };
        unsafe { (*io_1).tt_ = 5 | 0 << 4 | 1 << 6 };

        // Initialize internal module.
        unsafe { luaS_init(td) };
        unsafe { luaT_init(td)? };
        unsafe { luaX_init(td)? };

        g.gcstp.set(0);

        Ok(g)
    }

    pub fn spawn(self: &Pin<Rc<Self>>) -> *mut Thread {
        // Create new thread.
        let td = unsafe { self.gc.alloc(8, Layout::new::<Thread>()) as *mut Thread };

        unsafe { (*td).global = self.deref() };
        unsafe { addr_of_mut!((*td).stack).write(Cell::new(null_mut())) };
        unsafe { addr_of_mut!((*td).ci).write(Cell::new(null_mut())) };
        unsafe { addr_of_mut!((*td).nci).write(Cell::new(0)) };
        unsafe { addr_of_mut!((*td).twups).write(Cell::new(td)) };
        unsafe { addr_of_mut!((*td).hook).write(Cell::new(None)) };
        unsafe { addr_of_mut!((*td).hookmask).write(Cell::new(0)) };
        unsafe { addr_of_mut!((*td).basehookcount).write(Cell::new(0)) };
        unsafe { addr_of_mut!((*td).allowhook).write(Cell::new(1)) };
        unsafe { addr_of_mut!((*td).hookcount).write(Cell::new(0)) };
        unsafe { addr_of_mut!((*td).openupval).write(Cell::new(null_mut())) };
        unsafe { addr_of_mut!((*td).oldpc).write(Cell::new(0)) };

        // Allocate stack.
        let layout = Layout::array::<StackValue>(2 * 20 + 5).unwrap();
        let stack = unsafe { std::alloc::alloc(layout) as *mut StackValue };

        if stack.is_null() {
            handle_alloc_error(layout);
        }

        for i in 0..(2 * 20 + 5) {
            unsafe { (*stack.offset(i)).val.tt_ = 0 | 0 << 4 };
        }

        unsafe { (*td).stack.set(stack) };
        unsafe { addr_of_mut!((*td).top).write(StackPtr::new((*td).stack.get())) };
        unsafe { addr_of_mut!((*td).stack_last).write(Cell::new((*td).stack.get().add(2 * 20))) };
        unsafe { addr_of_mut!((*td).tbclist).write(Cell::new((*td).stack.get())) };

        // Setup base CI.
        let ci = unsafe { (*td).base_ci.get() };

        unsafe { (*ci).previous = null_mut() };
        unsafe { (*ci).next = (*ci).previous };
        unsafe { (*ci).callstatus = 1 << 1 };
        unsafe { (*ci).func = (*td).top.get() };
        unsafe { (*ci).u.savedpc = null() };
        unsafe { (*ci).nresults = 0 };
        unsafe { (*td).top.write_nil() };
        unsafe { (*td).top.add(1) };
        unsafe { (*ci).top = ((*td).top.get()).offset(20) };
        unsafe { (*td).ci.set(ci) };

        td
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
