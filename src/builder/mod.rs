use crate::lauxlib::luaL_requiref;
use crate::lbaselib::luaopen_base;
use crate::llex::luaX_init;
use crate::lmathlib::luaopen_math;
use crate::lobject::{TValue, Table, Value};
use crate::lstring::luaS_init;
use crate::lstrlib::luaopen_string;
use crate::ltable::{luaH_new, luaH_resize};
use crate::ltablib::luaopen_table;
use crate::ltm::luaT_init;
use crate::{Gc, Lua, Object, Ref, StringTable, Thread, lua_pop};
use std::cell::{Cell, UnsafeCell};
use std::marker::PhantomPinned;
use std::ops::Deref;
use std::pin::Pin;
use std::ptr::{null, null_mut};
use std::rc::Rc;

/// Struct to build the instance of [`Lua`].
pub struct Builder {
    g: Pin<Rc<Lua>>,
    th: Ref<Thread>,
}

impl Builder {
    /// Use [`Self::enable_all()`] to enable all Lua built-in libraries. You can also enable only
    /// selected library with `enable_*` (e.g. [`Self::enable_base()`]).
    pub fn new() -> Self {
        let g = Rc::pin(Lua {
            all: Cell::new(null()),
            refs: Cell::new(null()),
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
            fixedgc: Cell::new(null()),
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
            _phantom: PhantomPinned,
        });

        // Setup registry.
        let th = g.spawn();
        let registry: *mut Table = unsafe { luaH_new(g.deref()) };
        let io: *mut TValue = g.l_registry.get();

        unsafe { (*io).value_.gc = registry as *mut Object };
        unsafe { (*io).tt_ = 5 | 0 << 4 | 1 << 6 };

        unsafe { luaH_resize(th, registry, 2, 0) };

        // Create dummy object for LUA_RIDX_MAINTHREAD.
        let io_0 = unsafe { ((*registry).array).offset(1 - 1) as *mut TValue };

        unsafe { (*io_0).value_.gc = luaH_new(g.deref()).cast() };
        unsafe { (*io_0).tt_ = 5 | 0 << 4 | 1 << 6 };

        // Create LUA_RIDX_GLOBALS.
        let io_1 = unsafe { ((*registry).array).offset(2 - 1) as *mut TValue };

        unsafe { (*io_1).value_.gc = luaH_new(g.deref()).cast() };
        unsafe { (*io_1).tt_ = 5 | 0 << 4 | 1 << 6 };

        // Initialize internal module.
        unsafe { luaS_init(th) };
        unsafe { luaT_init(g.deref()) };
        unsafe { luaX_init(g.deref()) };

        g.gcstp.set(0);

        Self {
            th: unsafe { Ref::new(g.clone(), th) },
            g,
        }
    }

    /// Enable all built-in libraries.
    ///
    /// This has the same effect as calling [`Self::enable_base()`], [`Self::enable_string()`],
    /// [`Self::enable_table()`] and [`Self::enable_math()`] individually.
    pub fn enable_all(self) -> Self {
        self.enable_base()
            .enable_string()
            .enable_table()
            .enable_math()
    }

    /// Enable [basic library](https://www.lua.org/manual/5.4/manual.html#6.1).
    pub fn enable_base(self) -> Self {
        unsafe { luaL_requiref(self.th.deref(), c"_G".as_ptr(), luaopen_base, 0).unwrap() };
        unsafe { lua_pop(self.th.deref(), 1).unwrap() };
        self
    }

    /// Enable [string library](https://www.lua.org/manual/5.4/manual.html#6.4).
    pub fn enable_string(self) -> Self {
        unsafe { luaL_requiref(self.th.deref(), c"string".as_ptr(), luaopen_string, 1).unwrap() };
        unsafe { lua_pop(self.th.deref(), 1).unwrap() };
        self
    }

    /// Enable [table library](https://www.lua.org/manual/5.4/manual.html#6.6).
    pub fn enable_table(self) -> Self {
        unsafe { luaL_requiref(self.th.deref(), c"table".as_ptr(), luaopen_table, 1).unwrap() };
        unsafe { lua_pop(self.th.deref(), 1).unwrap() };
        self
    }

    /// Enable [mathematical library](https://www.lua.org/manual/5.4/manual.html#6.7).
    pub fn enable_math(self) -> Self {
        unsafe { luaL_requiref(self.th.deref(), c"math".as_ptr(), luaopen_math, 1).unwrap() };
        unsafe { lua_pop(self.th.deref(), 1).unwrap() };
        self
    }

    pub fn build(self) -> Pin<Rc<Lua>> {
        self.g
    }
}
