use crate::lauxlib::luaL_requiref;
use crate::lbaselib::luaopen_base;
use crate::llex::luaX_init;
use crate::lmathlib::luaopen_math;
use crate::lobject::{UnsafeValue, UntaggedValue};
use crate::lstring::luaS_init;
use crate::lstrlib::luaopen_string;
use crate::ltablib::luaopen_table;
use crate::ltm::luaT_init;
use crate::table::{luaH_new, luaH_resize};
use crate::{Gc, Lua, Module, Object, Ref, StringTable, Thread, lua_pop};
use alloc::rc::Rc;
use core::cell::{Cell, UnsafeCell};
use core::marker::PhantomPinned;
use core::ops::Deref;
use core::pin::Pin;
use core::ptr::{null, null_mut};

/// Struct to build the instance of [`Lua`].
pub struct Builder {
    g: Pin<Rc<Lua>>,
    th: Ref<Thread>,
}

impl Builder {
    /// Create a new [`Builder`] with a seed to hash Lua string.
    ///
    /// You can use [`Builder::default()`] instead if `rand` feature is enabled (which is default)
    /// or you can pass `0` as a seed if
    /// [HashDoS](https://en.wikipedia.org/wiki/Collision_attack#Hash_flooding) attack is not
    /// possible for your application.
    ///
    /// Use [`Self::enable_all()`] to enable all Lua built-in libraries. You can also enable only
    /// selected library with `enable_*` (e.g. [`Self::enable_base()`]).
    pub fn new(seed: u32) -> Self {
        let g = Rc::pin(Lua {
            currentwhite: Cell::new(1 << 3),
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
            l_registry: UnsafeCell::new(UnsafeValue {
                value_: UntaggedValue { i: 0 },
                tt_: (0 | 0 << 4),
            }),
            nilvalue: UnsafeCell::new(UnsafeValue {
                value_: UntaggedValue { i: 0 },
                tt_: (0 | 0 << 4),
            }),
            seed,
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
        let registry = unsafe { luaH_new(g.deref()) };
        let io: *mut UnsafeValue = g.l_registry.get();

        unsafe { (*io).value_.gc = registry as *mut Object };
        unsafe { (*io).tt_ = 5 | 0 << 4 | 1 << 6 };

        unsafe { luaH_resize(registry, 2, 0) };

        // Create dummy object for LUA_RIDX_MAINTHREAD.
        let io_0 = unsafe { (*registry).array.get().offset(1 - 1) as *mut UnsafeValue };

        unsafe { (*io_0).value_.gc = luaH_new(g.deref()).cast() };
        unsafe { (*io_0).tt_ = 5 | 0 << 4 | 1 << 6 };

        // Create LUA_RIDX_GLOBALS.
        let io_1 = unsafe { (*registry).array.get().offset(2 - 1) as *mut UnsafeValue };

        unsafe { (*io_1).value_.gc = luaH_new(g.deref()).cast() };
        unsafe { (*io_1).tt_ = 5 | 0 << 4 | 1 << 6 };

        // Initialize internal module.
        unsafe { luaS_init(g.deref()) };
        unsafe { luaT_init(g.deref()) };
        unsafe { luaX_init(g.deref()) };

        g.gcstp.set(0);

        Self {
            th: Thread::new(&g),
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

    /// # Panics
    /// If module with the same name already added.
    pub fn add_module<T: Module>(self, m: T) -> Self {
        todo!()
    }

    pub fn build(self) -> Pin<Rc<Lua>> {
        self.g
    }
}

#[cfg(feature = "rand")]
impl Default for Builder {
    fn default() -> Self {
        Self::new(rand::random())
    }
}
