use crate::llex::luaX_init;
use crate::ltm::luaT_init;
use crate::table::{luaH_new, luaH_resize};
use crate::value::{UnsafeValue, UntaggedValue};
use crate::{Fp, Gc, Lua, Object, Str, StringTable};
use alloc::rc::Rc;
use core::cell::{Cell, UnsafeCell};
use core::marker::PhantomPinned;
use core::ops::Deref;
use core::pin::Pin;
use core::ptr::{null, null_mut};

/// Struct to build the instance of [`Lua`].
pub struct Builder {
    g: Pin<Rc<Lua>>,
}

impl Builder {
    /// Create a new [`Builder`] with a seed to hash Lua string.
    ///
    /// You can use [`Builder::default()`] instead if `rand` feature is enabled (which is default)
    /// or you can pass `0` as a seed if
    /// [HashDoS](https://en.wikipedia.org/wiki/Collision_attack#Hash_flooding) attack is not
    /// possible for your application.
    ///
    /// Note that all built-in functions (e.g. `print`) are not enabled by default.
    pub fn new(seed: u32) -> Self {
        let g = Rc::pin(Lua {
            currentwhite: Cell::new(1 << 3),
            all: Cell::new(null()),
            refs: Cell::new(null()),
            gc: Gc::new(size_of::<Self>()),
            GCestimate: Cell::new(0), // TODO: Lua does not initialize this.
            lastatomic: Cell::new(0),
            strt: StringTable::new(),
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
            primitive_mt: [
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
            userdata_mt: Default::default(),
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
        unsafe { luaT_init(g.deref()) };
        unsafe { luaX_init(g.deref()) };

        g.gcstp.set(0);

        Self { g }
    }

    /// Enable [basic library](https://www.lua.org/manual/5.4/manual.html#6.1).
    ///
    /// Note that `print` only available with `std` feature.
    pub fn enable_base(self) -> Self {
        let g = self.g.deref();
        let global = |k: &str, v: UnsafeValue| unsafe {
            let k = UnsafeValue::from_str(Str::new(g, k));

            g.global().set_unchecked(k, v).unwrap();
        };

        global("assert", Fp(crate::builtin::base::assert).into());
        global("error", Fp(crate::builtin::base::error).into());
        global("pcall", Fp(crate::builtin::base::pcall).into());
        #[cfg(feature = "std")]
        global("print", Fp(crate::builtin::base::print).into());

        self
    }

    #[inline(always)]
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
