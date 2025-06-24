#![no_std]

pub use self::context::*;
pub use self::function::*;
pub use self::gc::Ref;
pub use self::parser::*;
pub use self::string::*;
pub use self::table::*;
pub use self::thread::*;
pub use self::ty::*;

use self::gc::{Gc, Object, luaC_barrier_, luaC_freeallobjects};
use self::lapi::lua_settop;
use self::ldo::luaD_protectedparser;
use self::llex::luaX_init;
use self::lobject::Udata;
use self::ltm::luaT_init;
use self::lzio::Zio;
use self::value::{UnsafeValue, UntaggedValue};
use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec::Vec;
use core::any::TypeId;
use core::cell::{Cell, RefCell, UnsafeCell};
use core::ffi::c_int;
use core::hint::unreachable_unchecked;
use core::marker::PhantomPinned;
use core::ops::Deref;
use core::pin::Pin;
use core::ptr::{null, null_mut};
use core::task::RawWakerVTable;
use hashbrown::HashMap;
use rustc_hash::FxBuildHasher;
use thiserror::Error;

pub mod builtin;

mod context;
mod function;
mod gc;
mod lapi;
mod lauxlib;
mod lcode;
mod lctype;
mod ldebug;
mod ldo;
mod lfunc;
mod llex;
mod lmem;
mod lobject;
mod lopcodes;
mod lparser;
mod lstate;
mod lstring;
mod ltm;
mod lzio;
mod parser;
mod string;
mod table;
mod thread;
mod ty;
mod value;
mod vm;

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

#[inline(always)]
unsafe fn lua_pop(th: *const Thread, n: c_int) -> Result<(), Box<dyn core::error::Error>> {
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
pub struct Lua {
    currentwhite: Cell<u8>,
    all: Cell<*const Object>,
    refs: Cell<*const Object>,
    gc: Gc,
    GCestimate: Cell<usize>,
    lastatomic: Cell<usize>,
    strt: StringTable,
    l_registry: UnsafeCell<UnsafeValue>,
    nilvalue: UnsafeCell<UnsafeValue>,
    seed: u32,
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
    tmname: [Cell<*const Str>; 25],
    primitive_mt: [Cell<*const Table>; 9],
    userdata_mt: RefCell<HashMap<TypeId, *const Table, FxBuildHasher>>,
    _phantom: PhantomPinned,
}

impl Lua {
    /// Create a new [`Lua`] with random seed to hash Lua string.
    ///
    /// Note that all built-in functions (e.g. `print`) are not enabled by default.
    #[cfg(feature = "rand")]
    pub fn new() -> Pin<Rc<Self>> {
        Self::with_seed(rand::random())
    }

    /// Create a new [`Lua`] with a seed to hash Lua string.
    ///
    /// You can use [`Lua::new()`] instead if `rand` feature is enabled (which is default) or you
    /// can pass `0` as a seed if
    /// [HashDoS](https://en.wikipedia.org/wiki/Collision_attack#Hash_flooding) attack is not
    /// possible for your application.
    ///
    /// Note that all built-in functions (e.g. `print`) are not enabled by default.
    pub fn with_seed(seed: u32) -> Pin<Rc<Self>> {
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
        let registry = unsafe { Table::new(g.deref()) };
        let io: *mut UnsafeValue = g.l_registry.get();

        unsafe { (*io).value_.gc = registry as *mut Object };
        unsafe { (*io).tt_ = 5 | 0 << 4 | 1 << 6 };

        unsafe { luaH_resize(registry, 2, 0) };

        // Create dummy object for LUA_RIDX_MAINTHREAD.
        let io_0 = unsafe { (*registry).array.get().offset(1 - 1) as *mut UnsafeValue };

        unsafe { (*io_0).value_.gc = Table::new(g.deref()).cast() };
        unsafe { (*io_0).tt_ = 5 | 0 << 4 | 1 << 6 };

        // Create LUA_RIDX_GLOBALS.
        let io_1 = unsafe { (*registry).array.get().offset(2 - 1) as *mut UnsafeValue };

        unsafe { (*io_1).value_.gc = Table::new(g.deref()).cast() };
        unsafe { (*io_1).tt_ = 5 | 0 << 4 | 1 << 6 };

        // Initialize internal module.
        unsafe { luaT_init(g.deref()) };
        unsafe { luaX_init(g.deref()) };

        g.gcstp.set(0);
        g
    }

    /// Setup [basic library](https://www.lua.org/manual/5.4/manual.html#6.1).
    ///
    /// Note that `print` only available with `std` feature.
    pub fn setup_base(&self) {
        let g = self.global();

        unsafe { g.set_str_key_unchecked("assert", Fp(crate::builtin::base::assert)) };
        unsafe { g.set_str_key_unchecked("error", Fp(crate::builtin::base::error)) };
        unsafe { g.set_str_key_unchecked("getmetatable", Fp(crate::builtin::base::getmetatable)) };
        unsafe { g.set_str_key_unchecked("load", Fp(crate::builtin::base::load)) };
        unsafe { g.set_str_key_unchecked("pcall", Fp(crate::builtin::base::pcall)) };
        #[cfg(feature = "std")]
        unsafe {
            g.set_str_key_unchecked("print", Fp(crate::builtin::base::print))
        };
        unsafe { g.set_str_key_unchecked("setmetatable", Fp(crate::builtin::base::setmetatable)) };
        unsafe { g.set_str_key_unchecked("type", Fp(crate::builtin::base::r#type)) };
        unsafe { g.set_str_key_unchecked("_G", g) };
    }

    /// Setup [string library](https://www.lua.org/manual/5.4/manual.html#6.4).
    pub fn setup_string(&self) {
        // Setup string table.
        let g = unsafe { Table::new(self) };

        unsafe { (*g).set_str_key_unchecked("sub", Fp(crate::builtin::string::sub)) };

        // Set global.
        let g = unsafe { UnsafeValue::from_obj(g.cast()) };

        unsafe { self.global().set_str_key_unchecked("string", g) };

        // Set metatable.
        let mt = unsafe { Table::new(self) };

        unsafe { (*mt).set_str_key_unchecked("__index", g) };

        self.primitive_mt[4].set(mt);
    }

    /// Setup [mathematical library](https://www.lua.org/manual/5.4/manual.html#6.7).
    pub fn setup_math(&self) {
        // Setup math table.
        let g = unsafe { Table::new(self) };

        // Set global.
        let g = unsafe { UnsafeValue::from_obj(g.cast()) };

        unsafe { self.global().set_str_key_unchecked("math", g) };
    }

    /// Load a Lua chunk.
    pub fn load(&self, info: ChunkInfo, chunk: impl AsRef<[u8]>) -> Result<Ref<LuaFn>, ParseError> {
        let chunk = chunk.as_ref();
        let z = Zio {
            n: chunk.len(),
            p: chunk.as_ptr().cast(),
        };

        // Load.
        let f = unsafe { luaD_protectedparser(self, z, info)? };

        if !(*f).upvals.is_empty() {
            let gt = unsafe {
                (*((*self.l_registry.get()).value_.gc.cast::<Table>()))
                    .array
                    .get()
                    .offset(2 - 1)
            };

            let io1: *mut UnsafeValue = unsafe { (*(*f).upvals[0].get()).v.get() };

            unsafe { (*io1).value_ = (*gt).value_ };
            unsafe { (*io1).tt_ = (*gt).tt_ };

            if unsafe { (*gt).tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0 } {
                if unsafe {
                    (*(*f).upvals[0].get()).hdr.marked.get() & 1 << 5 != 0
                        && (*(*gt).value_.gc).marked.get() & (1 << 3 | 1 << 4) != 0
                } {
                    unsafe { luaC_barrier_(self, (*f).upvals[0].get().cast(), (*gt).value_.gc) };
                }
            }
        }

        Ok(f)
    }

    /// Create a new Lua thread (AKA coroutine).
    #[inline(always)]
    pub fn spawn(&self) -> Ref<Thread> {
        unsafe { Ref::new(Thread::new(self)) }
    }

    /// Create a Lua string.
    pub fn create_str<T>(&self, v: T) -> Ref<Str>
    where
        T: AsRef<str> + AsRef<[u8]> + Into<Vec<u8>>,
    {
        unsafe { Ref::new(Str::from_str(self, v)) }
    }

    /// Create a Lua table.
    pub fn create_table(&self) -> Ref<Table> {
        unsafe { Ref::new(Table::new(self)) }
    }

    /// Returns a global table.
    #[inline(always)]
    pub fn global(&self) -> &Table {
        let reg = unsafe { (*self.l_registry.get()).value_.gc.cast::<Table>() };
        let tab = unsafe { (*reg).array.get().add(2 - 1) };
        let tab = unsafe { (*tab).value_.gc.cast::<Table>() };

        unsafe { &*tab }
    }

    unsafe fn get_mt(&self, o: *const UnsafeValue) -> *const Table {
        match unsafe { (*o).tt_ & 0xf } {
            5 => unsafe { (*(*o).value_.gc.cast::<Table>()).metatable.get() },
            7 => unsafe { (*(*o).value_.gc.cast::<Udata>()).metatable },
            v => self.primitive_mt[usize::from(v)].get(),
        }
    }

    fn reset_gray(&self) {
        self.grayagain.set(null_mut());
        self.gray.set(null_mut());
        self.ephemeron.set(null_mut());
        self.allweak.set(null_mut());
        self.weak.set(null_mut());
    }

    fn to_rc(&self) -> Pin<Rc<Self>> {
        unsafe { Rc::increment_strong_count(self) };
        unsafe { Pin::new_unchecked(Rc::from_raw(self)) }
    }
}

impl Drop for Lua {
    fn drop(&mut self) {
        unsafe { luaC_freeallobjects(self) };
    }
}

/// Lua value.
pub enum Value {
    Nil,
    Bool(bool),
    Fp(fn(Context<Args>) -> Result<Context<Ret>, Box<dyn core::error::Error>>),
    Int(i64),
    Float(f64),
    Str(Ref<Str>),
    Table(Ref<Table>),
    LuaFn(Ref<LuaFn>),
    Thread(Ref<Thread>),
}

impl Value {
    unsafe fn from_unsafe(v: *const UnsafeValue) -> Self {
        match unsafe { (*v).tt_ & 0xf } {
            0 => Self::Nil,
            1 => Self::Bool(unsafe { ((*v).tt_ & 0x30) != 0 }),
            2 => match unsafe { ((*v).tt_ >> 4) & 3 } {
                0 => Self::Fp(unsafe { (*v).value_.f }),
                1 => todo!(),
                2 => todo!(),
                3 => todo!(),
                _ => unsafe { unreachable_unchecked() },
            },
            3 => match unsafe { ((*v).tt_ >> 4) & 3 } {
                0 => Self::Int(unsafe { (*v).value_.i }),
                1 => Self::Float(unsafe { (*v).value_.n }),
                _ => unreachable!(),
            },
            4 => Self::Str(unsafe { Ref::new((*v).value_.gc.cast()) }),
            5 => Self::Table(unsafe { Ref::new((*v).value_.gc.cast()) }),
            6 => match unsafe { ((*v).tt_ >> 4) & 3 } {
                0 => Self::LuaFn(unsafe { Ref::new((*v).value_.gc.cast()) }),
                1 => todo!(),
                2 => todo!(),
                3 => todo!(),
                _ => unsafe { unreachable_unchecked() },
            },
            7 => todo!(),
            8 => Self::Thread(unsafe { Ref::new((*v).value_.gc.cast()) }),
            _ => unreachable!(),
        }
    }
}

/// Unit struct to create `nil` value.
pub struct Nil;

/// Non-Yieldable Rust function.
#[derive(Clone, Copy)]
pub struct Fp(pub fn(Context<Args>) -> Result<Context<Ret>, Box<dyn core::error::Error>>);

#[derive(Clone, Copy)]
pub struct YieldFp(pub fn(Context<Args>) -> Result<Context<Ret>, Box<dyn core::error::Error>>);

#[derive(Clone, Copy)]
pub struct AsyncFp(
    pub  fn(
        Context<Args>,
    )
        -> Box<dyn Future<Output = Result<Context<Ret>, Box<dyn core::error::Error>>> + '_>,
);

/// Represents an error when arithmetic operation fails.
#[derive(Debug, Error)]
pub enum ArithError {
    #[error("attempt to perform 'n%0'")]
    ModZero,

    #[error("attempt to divide by zero")]
    DivZero,
}

/// Represents an error when Lua stack is overflow.
#[derive(Debug, Error)]
#[error("stack overflow")]
pub struct StackOverflow;

static NON_YIELDABLE_WAKER: RawWakerVTable = RawWakerVTable::new(
    |_| unimplemented!(),
    |_| unimplemented!(),
    |_| unimplemented!(),
    |_| {},
);
