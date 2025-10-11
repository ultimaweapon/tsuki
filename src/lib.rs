//! Lua 5.4 ported to Rust.
//!
//! # Quickstart
//!
//! ```
//! use tsuki::builtin::{BaseLib, CoroLib, MathLib, StringLib, TableLib, Utf8Lib};
//! use tsuki::{Args, Context, Lua, Ret, Value, fp};
//!
//! fn main() {
//!     // Set up.
//!     let lua = Lua::new(());
//!
//!     lua.use_module(None, true, BaseLib).unwrap();
//!     lua.use_module(None, true, CoroLib).unwrap();
//!     lua.use_module(None, true, MathLib).unwrap();
//!     lua.use_module(None, true, StringLib).unwrap();
//!     lua.use_module(None, true, TableLib).unwrap();
//!     lua.use_module(None, true, Utf8Lib).unwrap();
//!
//!     lua.global().set_str_key("myfunc", fp!(myfunc));
//!
//!     // Run on main thread.
//!     let chunk = lua.load("abc.lua", "return myfunc()").unwrap();
//!     let result = lua.call::<Value<_>>(chunk, ()).unwrap();
//!
//!     match result {
//!         Value::Str(v) => assert_eq!(v.as_str(), Some("Hello world!")),
//!         _ => todo!(),
//!     }
//! }
//!
//! fn myfunc(cx: Context<(), Args>) -> Result<Context<(), Ret>, Box<dyn core::error::Error>> {
//!     cx.push_str("Hello world!")?;
//!     Ok(cx.into())
//! }
//! ```
//!
//! # Types that can be converted to UnsafeValue.
//!
//! You can pass the value of the following types for `impl Into<UnsafeValue>`:
//!
//! - [`Nil`]
//! - [`bool`]
//! - [`Fp`]
//! - [`AsyncFp`]
//! - [`i8`]
//! - [`i16`]
//! - [`i32`]
//! - [`i64`]
//! - [`u8`]
//! - [`u16`]
//! - [`u32`]
//! - [`f32`]
//! - [`f64`]
//! - [`Number`]
//! - Reference to [`Str`]
//! - Reference to [`Table`]
//! - Reference to [`LuaFn`]
//! - Reference to [`UserData`]
//! - Reference to [`Thread`]
//! - [`Ref`]
//! - [`Value`]
//! - [`Arg`] or reference to it
//!
//! The value will be converted to corresponding Lua value. Tsuki does not expose [`UnsafeValue`] by
//! design so you cannot construct its value. Tsuki also never handout the value of [`UnsafeValue`].
#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub use self::context::*;
pub use self::function::*;
pub use self::gc::Ref;
pub use self::module::*;
pub use self::parser::*;
pub use self::registry::*;
pub use self::string::*;
pub use self::table::*;
pub use self::thread::*;
pub use self::ty::*;
pub use self::userdata::*;

use self::gc::{Gc, Object};
use self::lapi::lua_settop;
use self::ldebug::lua_getinfo;
use self::ldo::luaD_protectedparser;
use self::llex::{TK_WHILE, luaX_tokens};
use self::lstate::{CallInfo, lua_Debug};
use self::ltm::{
    TM_ADD, TM_BAND, TM_BNOT, TM_BOR, TM_BXOR, TM_CALL, TM_CLOSE, TM_CONCAT, TM_DIV, TM_EQ, TM_GC,
    TM_IDIV, TM_INDEX, TM_LE, TM_LEN, TM_LT, TM_MOD, TM_MODE, TM_MUL, TM_NEWINDEX, TM_POW, TM_SHL,
    TM_SHR, TM_SUB, TM_UNM, luaT_gettm,
};
use self::lzio::Zio;
use self::value::{UnsafeValue, UntaggedValue};
use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::any::{Any, TypeId};
use core::cell::{Cell, UnsafeCell};
use core::error::Error;
use core::ffi::c_int;
use core::fmt::{Display, Formatter};
use core::hint::unreachable_unchecked;
use core::marker::PhantomPinned;
use core::ops::Deref;
use core::pin::Pin;
use core::ptr::null;
use core::task::RawWakerVTable;
use thiserror::Error;

pub mod builtin;

mod context;
mod function;
mod gc;
mod hasher;
mod lapi;
mod lauxlib;
mod lcode;
mod lctype;
mod ldebug;
mod ldo;
mod lfunc;
mod libc;
mod llex;
mod lmem;
mod lobject;
mod lparser;
mod lstate;
mod lstring;
mod ltm;
mod lzio;
mod module;
mod parser;
mod registry;
mod string;
mod table;
mod thread;
mod ty;
mod userdata;
mod value;
mod vm;

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

#[inline(always)]
unsafe fn lua_pop<D>(th: *const Thread<D>, n: c_int) -> Result<(), Box<dyn Error>> {
    unsafe { lua_settop(th, -(n) - 1) }
}

#[inline(always)]
unsafe fn api_incr_top<D>(th: *const Thread<D>) {
    unsafe { (*th).top.add(1) };

    if unsafe { (*th).top.get() > (*(*th).ci.get()).top } {
        panic!("stack overflow");
    }
}

/// Helper macro to construct [`Fp`] or [`AsyncFp`].
#[macro_export]
macro_rules! fp {
    ($f:path) => {
        $crate::Fp::new($f)
    };
    ($f:path as async) => {{
        #[cfg(not(feature = "std"))]
        use ::alloc::boxed::Box;

        $crate::AsyncFp::new(|cx| Box::pin($f(cx)))
    }};
}

/// Global states shared with all Lua threads.
#[repr(C)] // Force gc field to be the first field.
pub struct Lua<T> {
    gc: Gc<T>,
    strt: StringTable<T>,
    l_registry: UnsafeCell<UnsafeValue<T>>,
    nilvalue: UnsafeCell<UnsafeValue<T>>,
    dummy_node: Node<T>,
    seed: u32,
    active_rust_call: Cell<usize>,
    modules_locked: Cell<bool>,
    associated_data: T,
    phantom: PhantomPinned,
}

impl<T> Lua<T> {
    /// Create a new [Lua] with a random seed to hash Lua string.
    ///
    /// You can retrieve `associated_data` later with [Self::associated_data()] or
    /// [Context::associated_data()].
    ///
    /// Note that all built-in functions (e.g. `print`) are not enabled by default.
    #[cfg(feature = "rand")]
    #[cfg_attr(docsrs, doc(cfg(feature = "rand")))]
    pub fn new(associated_data: T) -> Pin<Rc<Self>> {
        Self::with_seed(associated_data, rand::random())
    }

    /// Create a new [Lua] with a seed to hash Lua string.
    ///
    /// You can use [Self::new()] instead if `rand` feature is enabled (which is default) or you
    /// can pass `0` as a seed if
    /// [HashDoS](https://en.wikipedia.org/wiki/Collision_attack#Hash_flooding) attack is not
    /// possible for your application.
    ///
    /// You can retrieve `associated_data` later with [Self::associated_data()] or
    /// [Context::associated_data()].
    ///
    /// Note that all built-in functions (e.g. `print`) are not enabled by default.
    pub fn with_seed(associated_data: T, seed: u32) -> Pin<Rc<Self>> {
        let g = Rc::pin(Lua {
            gc: unsafe { Gc::new() }, // SAFETY: gc in the first field on Lua.
            strt: StringTable::new(),
            l_registry: UnsafeCell::new(Nil.into()),
            nilvalue: UnsafeCell::new(Nil.into()),
            dummy_node: Node {
                u: NodeKey {
                    value_: UntaggedValue { gc: null() },
                    tt_: 0 | 1 << 4,
                    key_tt: 0 | 0 << 4,
                    next: 0,
                    key_val: UntaggedValue { gc: null() },
                },
            },
            seed,
            active_rust_call: Cell::new(0),
            modules_locked: Cell::new(false),
            associated_data,
            phantom: PhantomPinned,
        });

        // Setup registry.
        let reg = unsafe { Table::new(g.deref()) };

        unsafe { g.gc.set_root(reg.cast()) };
        unsafe { g.l_registry.get().write(UnsafeValue::from_obj(reg.cast())) };
        unsafe { luaH_resize(reg, 6, 0) };

        // Create main thread.
        let reg = unsafe { (*reg).array.get() };
        let main = Thread::new(g.deref());

        unsafe { reg.add(0).write(UnsafeValue::from_obj(main.cast())) };

        // Create LUA_RIDX_GLOBALS.
        let glb = unsafe { Table::new(g.deref()) };

        unsafe { reg.add(1).write(UnsafeValue::from_obj(glb.cast())) };

        // Create table for metatables.
        let mts = unsafe { Table::new(g.deref()) };

        unsafe { luaH_resize(mts, 9, 0) };

        for i in 0..9 {
            let e = unsafe { (*mts).array.get().add(i) };

            unsafe { (*e).tt_ = 1 | 0 << 4 };
            unsafe { (*e).value_.gc = null() };
        }

        unsafe { reg.add(2).write(UnsafeValue::from_obj(mts.cast())) };

        // Create table for event names.
        let events = unsafe { Table::new(g.deref()) };
        let entries = &[
            (TM_INDEX, "__index"),
            (TM_NEWINDEX, "__newindex"),
            (TM_GC, "__gc"),
            (TM_MODE, "__mode"),
            (TM_LEN, "__len"),
            (TM_EQ, "__eq"),
            (TM_ADD, "__add"),
            (TM_SUB, "__sub"),
            (TM_MUL, "__mul"),
            (TM_MOD, "__mod"),
            (TM_POW, "__pow"),
            (TM_DIV, "__div"),
            (TM_IDIV, "__idiv"),
            (TM_BAND, "__band"),
            (TM_BOR, "__bor"),
            (TM_BXOR, "__bxor"),
            (TM_SHL, "__shl"),
            (TM_SHR, "__shr"),
            (TM_UNM, "__unm"),
            (TM_BNOT, "__bnot"),
            (TM_LT, "__lt"),
            (TM_LE, "__le"),
            (TM_CONCAT, "__concat"),
            (TM_CALL, "__call"),
            (TM_CLOSE, "__close"),
        ];

        unsafe { luaH_resize(events, entries.len().try_into().unwrap(), 0) };

        for &(k, v) in entries {
            let v = unsafe { Str::from_str(g.deref(), v) };
            let v = unsafe { UnsafeValue::from_obj(v.cast()) };

            unsafe { (*events).set_unchecked(k, v).unwrap_unchecked() };
        }

        unsafe { reg.add(3).write(UnsafeValue::from_obj(events.cast())) };

        // Create table for Lua tokens.
        let tokens = unsafe { Table::new(g.deref()) };
        let n = TK_WHILE - (255 + 1) + 1;

        unsafe { luaH_resize(tokens, 0, n.try_into().unwrap()) };

        for i in 0..n {
            let k = unsafe { Str::from_str(g.deref(), luaX_tokens[i as usize]) };
            let k = unsafe { UnsafeValue::from_obj(k.cast()) };

            unsafe { (*tokens).set_unchecked(k, i + 1).unwrap_unchecked() };
        }

        unsafe { reg.add(4).write(UnsafeValue::from_obj(tokens.cast())) };

        // Create table for modules.
        let mods = unsafe { Table::new(g.deref()) };

        unsafe { reg.add(5).write(UnsafeValue::from_obj(mods.cast())) };

        g
    }

    /// Returns associated data that passed to [Self::new()] or [Self::with_seed()].
    #[inline(always)]
    pub fn associated_data(&self) -> &T {
        &self.associated_data
    }

    /// Load a Lua module that implemented in Rust.
    ///
    /// Supply `name` if you want to use different name than [Module::NAME].
    ///
    /// If `global` is `true` this will **overwrite** the global variable with the same name as the
    /// module.
    ///
    /// The error can be either [ModuleExists], [RecursiveCall] or the one that returned from
    /// [Module::open()].
    ///
    /// # Panics
    /// If [Module::open()] returns a value created from different Lua instance.
    pub fn use_module<'a, M>(
        &'a self,
        name: Option<&str>,
        global: bool,
        module: M,
    ) -> Result<(), Box<dyn Error>>
    where
        M: Module<T>,
        M::Instance<'a>: Into<UnsafeValue<T>>,
    {
        // Prevent recursive call.
        let lock = match ModulesLock::new(&self.modules_locked) {
            Some(v) => v,
            None => return Err(Box::new(RecursiveCall::new(Self::use_module::<M>))),
        };

        // Check if exists.
        let name = name.unwrap_or(M::NAME);
        let n = unsafe { UnsafeValue::from_obj(Str::from_str(self, name).cast()) };
        let t = self.modules();
        let s = unsafe { t.get_raw_unchecked(n) };

        if unsafe { ((*s).tt_ & 0xf) != 0 } {
            return Err(Box::new(ModuleExists));
        }

        // Open the module. We need a strong reference to name here since the module can trigger GC.
        let n = unsafe { Ref::new(n.value_.gc.cast::<Str<T>>()) };
        let m = module.open(self)?.into();

        if (m.tt_ & 0xf) == 0 {
            return Ok(());
        } else if unsafe { (m.tt_ & 1 << 6) != 0 && (*m.value_.gc).global != self } {
            panic!("the module instance was created from different Lua instance");
        }

        // SAFETY: n is not nil or NaN.
        unsafe { t.set_slot_unchecked(s, n.deref(), m).unwrap_unchecked() };

        if global {
            unsafe { self.global().set_unchecked(n, m).unwrap_unchecked() };
        }

        drop(lock);

        Ok(())
    }

    /// Set metatable for Lua string.
    ///
    /// # Panics
    /// - If `mt` was created from different [Lua](crate::Lua) instance.
    /// - If `mt` contains `__gc`.
    pub fn set_str_metatable(&self, mt: &Table<T>) {
        if mt.hdr.global != self {
            panic!("attempt to set string metatable created from a different Lua");
        }

        // Prevent __gc metamethod.
        if unsafe { mt.flags.get() & 1 << TM_GC == 0 && !luaT_gettm(mt, TM_GC).is_null() } {
            panic!("__gc metamethod is not supported");
        }

        unsafe { self.metatables().set_unchecked(4, mt).unwrap_unchecked() };
    }

    /// Register a metatable for userdata `V`. If the metatable for `V` already exists it will be
    /// replaced.
    ///
    /// This does not change the metatable for the userdata that already created.
    ///
    /// # Panics
    /// - If `mt` come from different [Lua](crate::Lua) instance.
    /// - If `mt` contains `__gc`.
    pub fn register_metatable<V: Any>(&self, mt: &Table<T>) {
        if mt.hdr.global != self {
            panic!("attempt to register a metatable created from a different Lua");
        }

        // Prevent __gc metamethod.
        if unsafe { mt.flags.get() & 1 << TM_GC == 0 && !luaT_gettm(mt, TM_GC).is_null() } {
            panic!("__gc metamethod is not supported");
        }

        // Get type ID.
        let k = unsafe { RustId::new(self, TypeId::of::<V>()) };
        let k = unsafe { UnsafeValue::from_obj(k.cast()) };

        unsafe { self.metatables().set_unchecked(k, mt).unwrap_unchecked() };
    }

    /// Sets a value to registry.
    ///
    /// # Panics
    /// If `v` was created from different [Lua](crate::Lua) instance.
    pub fn set_registry<'a, K>(&self, v: <K::Value<'a> as RegValue<T>>::In<'a>)
    where
        K: RegKey<T>,
        K::Value<'a>: RegValue<T>,
    {
        let v = K::Value::into_unsafe(v);

        if unsafe { (v.tt_ & 1 << 6) != 0 && (*v.value_.gc).global != self } {
            panic!("attempt to set registry value created from different Lua instance");
        }

        // Set.
        let r = unsafe { (*self.l_registry.get()).value_.gc.cast::<Table<T>>() };
        let k = unsafe { RustId::new(self, TypeId::of::<K>()) };
        let k = unsafe { UnsafeValue::from_obj(k.cast()) };

        // SAFETY: k is not nil or NaN.
        unsafe { (*r).set_unchecked(k, v).unwrap_unchecked() };
    }

    /// Returns value on registry that was set with [Self::set_registry()].
    pub fn registry<'a, K>(&'a self) -> Option<<K::Value<'a> as RegValue<T>>::Out<'a>>
    where
        K: RegKey<T>,
        K::Value<'a>: RegValue<T>,
    {
        let id = TypeId::of::<K>();
        let reg = unsafe { &*(*self.l_registry.get()).value_.gc.cast::<Table<T>>() };
        let s = unsafe { luaH_getid(reg, &id) };

        match unsafe { (*s).tt_ & 0xf } {
            0 => None,
            _ => Some(unsafe { K::Value::from_unsafe(s) }),
        }
    }

    /// Returns a global table.
    #[inline(always)]
    pub fn global(&self) -> &Table<T> {
        let reg = unsafe { (*self.l_registry.get()).value_.gc.cast::<Table<T>>() };
        let tab = unsafe { (*reg).array.get().add(1) };
        let tab = unsafe { (*tab).value_.gc.cast::<Table<T>>() };

        unsafe { &*tab }
    }

    /// Create a Lua string.
    #[inline(always)]
    pub fn create_str<V>(&self, v: V) -> Ref<'_, Str<T>>
    where
        V: AsRef<str> + AsRef<[u8]> + Into<Vec<u8>>,
    {
        self.gc.step();

        unsafe { Ref::new(Str::from_str(self, v)) }
    }

    /// Create a Lua table.
    #[inline(always)]
    pub fn create_table(&self) -> Ref<'_, Table<T>> {
        self.gc.step();

        unsafe { Ref::new(Table::new(self)) }
    }

    /// Create a full userdata.
    ///
    /// The metatable for the userdata that was registered with [Self::register_metatable()] will be
    /// loaded during creation. A call to [Self::register_metatable()] has no effect for any
    /// userdata that already created.
    #[inline(always)]
    pub fn create_ud<V: Any>(&self, v: V) -> Ref<'_, UserData<T, V>> {
        self.gc.step();

        unsafe { Ref::new(UserData::new(self, v).cast()) }
    }

    /// Create a new Lua thread (AKA coroutine).
    pub fn create_thread(&self) -> Ref<'_, Thread<T>> {
        self.gc.step();

        unsafe { Ref::new(Thread::new(self)) }
    }

    /// Load a Lua chunk.
    pub fn load(
        &self,
        info: impl Into<ChunkInfo>,
        chunk: impl AsRef<[u8]>,
    ) -> Result<Ref<'_, LuaFn<T>>, ParseError> {
        let chunk = chunk.as_ref();
        let z = Zio {
            n: chunk.len(),
            p: chunk.as_ptr().cast(),
        };

        // Load.
        let f = unsafe { luaD_protectedparser(self, z, info.into())? };

        if !(*f).upvals.is_empty() {
            let gt = unsafe {
                (*((*self.l_registry.get()).value_.gc.cast::<Table<T>>()))
                    .array
                    .get()
                    .offset(2 - 1)
            };

            let io1 = unsafe { (*(*f).upvals[0].get()).v.get() };

            unsafe { (*io1).value_ = (*gt).value_ };
            unsafe { (*io1).tt_ = (*gt).tt_ };

            if unsafe { (*gt).tt_ & 1 << 6 != 0 } {
                if unsafe {
                    (*(*f).upvals[0].get()).hdr.marked.get() & 1 << 5 != 0
                        && (*(*gt).value_.gc).marked.get() & (1 << 3 | 1 << 4) != 0
                } {
                    unsafe {
                        self.gc
                            .barrier((*f).upvals[0].get().cast(), (*gt).value_.gc)
                    };
                }
            }
        }

        Ok(f)
    }

    /// Call a function or callable value on main thread.
    ///
    /// See [`Thread::call()`] for more details.
    #[inline(always)]
    pub fn call<'a, R: Outputs<'a, T>>(
        &'a self,
        f: impl Into<UnsafeValue<T>>,
        args: impl Inputs<T>,
    ) -> Result<R, Box<dyn Error>> {
        self.main().call(f, args)
    }

    #[inline(always)]
    fn main(&self) -> &Thread<T> {
        let reg = unsafe { (*self.l_registry.get()).value_.gc.cast::<Table<T>>() };
        let val = unsafe { (*reg).array.get().add(0) };
        let val = unsafe { (*val).value_.gc.cast::<Thread<T>>() };

        unsafe { &*val }
    }

    unsafe fn metatable(&self, o: *const UnsafeValue<T>) -> *const Table<T> {
        match unsafe { (*o).tt_ & 0xf } {
            5 => unsafe { (*(*o).value_.gc.cast::<Table<T>>()).metatable.get() },
            7 => unsafe { (*(*o).value_.gc.cast::<UserData<T, ()>>()).mt },
            v => unsafe { self.metatables().get_raw_int_key(v.into()).value_.gc.cast() },
        }
    }

    #[inline(always)]
    fn metatables(&self) -> &Table<T> {
        let reg = unsafe { (*self.l_registry.get()).value_.gc.cast::<Table<T>>() };
        let tab = unsafe { (*reg).array.get().add(2) };
        let tab = unsafe { (*tab).value_.gc.cast::<Table<T>>() };

        unsafe { &*tab }
    }

    #[inline(always)]
    fn events(&self) -> &Table<T> {
        let reg = unsafe { (*self.l_registry.get()).value_.gc.cast::<Table<T>>() };
        let tab = unsafe { (*reg).array.get().add(3) };
        let tab = unsafe { (*tab).value_.gc.cast::<Table<T>>() };

        unsafe { &*tab }
    }

    #[inline(always)]
    fn tokens(&self) -> &Table<T> {
        let reg = unsafe { (*self.l_registry.get()).value_.gc.cast::<Table<T>>() };
        let tab = unsafe { (*reg).array.get().add(4) };
        let tab = unsafe { (*tab).value_.gc.cast::<Table<T>>() };

        unsafe { &*tab }
    }

    #[inline(always)]
    fn modules(&self) -> &Table<T> {
        let reg = unsafe { (*self.l_registry.get()).value_.gc.cast::<Table<T>>() };
        let tab = unsafe { (*reg).array.get().add(5) };
        let tab = unsafe { (*tab).value_.gc.cast::<Table<T>>() };

        unsafe { &*tab }
    }
}

/// RAII struct to toggle [Lua::modules_locked].
struct ModulesLock<'a>(&'a Cell<bool>);

impl<'a> ModulesLock<'a> {
    #[inline(always)]
    fn new(locked: &'a Cell<bool>) -> Option<Self> {
        if locked.get() {
            return None;
        }

        locked.set(true);

        Some(Self(locked))
    }
}

impl<'a> Drop for ModulesLock<'a> {
    #[inline(always)]
    fn drop(&mut self) {
        self.0.set(false);
    }
}

/// Lua value.
pub enum Value<'a, D> {
    Nil,
    Bool(bool),
    Fp(fn(Context<D, Args>) -> Result<Context<D, Ret>, Box<dyn Error>>),
    AsyncFp(
        fn(
            Context<D, Args>,
        ) -> Pin<Box<dyn Future<Output = Result<Context<D, Ret>, Box<dyn Error>>> + '_>>,
    ),
    Int(i64),
    Float(f64),
    Str(Ref<'a, Str<D>>),
    Table(Ref<'a, Table<D>>),
    LuaFn(Ref<'a, LuaFn<D>>),
    UserData(Ref<'a, UserData<D, dyn Any>>),
    Thread(Ref<'a, Thread<D>>),
}

impl<'a, D> Value<'a, D> {
    /// Constructs [`Value`] from [`Arg`].
    ///
    /// Returns [`None`] if argument `v` does not exists.
    #[inline(always)]
    pub fn from_arg(v: &Arg<'_, 'a, D>) -> Option<Self> {
        let v = v.get_raw_or_null();

        match v.is_null() {
            true => None,
            false => Some(unsafe { Self::from_unsafe(v) }),
        }
    }

    /// Returns `true` if this value is [`Value::Nil`].
    pub const fn is_nil(&self) -> bool {
        match self {
            Self::Nil => true,
            _ => false,
        }
    }

    #[inline(never)]
    unsafe fn from_unsafe(v: *const UnsafeValue<D>) -> Self {
        match unsafe { (*v).tt_ & 0xf } {
            0 => Self::Nil,
            1 => Self::Bool(unsafe { ((*v).tt_ & 0x30) != 0 }),
            2 => match unsafe { ((*v).tt_ >> 4) & 3 } {
                0 => Self::Fp(unsafe { (*v).value_.f }),
                1 => todo!(),
                2 => Self::AsyncFp(unsafe { (*v).value_.a }),
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
            7 => Self::UserData(unsafe { Ref::new((*v).value_.gc.cast()) }),
            8 => Self::Thread(unsafe { Ref::new((*v).value_.gc.cast()) }),
            _ => unreachable!(),
        }
    }
}

/// Unit struct to create `nil` value.
pub struct Nil;

/// Non-Yieldable Rust function.
pub struct Fp<D>(fn(Context<D, Args>) -> Result<Context<D, Ret>, Box<dyn Error>>);

impl<D> Fp<D> {
    /// Construct a new [`Fp`] from a function pointer.
    ///
    /// [`fp`] macro is more convenience than this function.
    pub const fn new(v: fn(Context<D, Args>) -> Result<Context<D, Ret>, Box<dyn Error>>) -> Self {
        Self(v)
    }
}

impl<D> Clone for Fp<D> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for Fp<D> {}

pub struct YieldFp<D>(fn(Context<D, Args>) -> Result<Context<D, Ret>, Box<dyn Error>>);

impl<D> Clone for YieldFp<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for YieldFp<D> {}

/// Asynchronous Rust function.
///
/// Each call into async function from Lua always incur one heap allocation so create async function
/// only when necessary.
pub struct AsyncFp<D>(
    fn(
        Context<D, Args>,
    ) -> Pin<Box<dyn Future<Output = Result<Context<D, Ret>, Box<dyn Error>>> + '_>>,
);

impl<D> AsyncFp<D> {
    /// Construct a new [`AsyncFp`] from a function pointer.
    ///
    /// [`fp`] macro is more convenience than this function.
    pub const fn new(
        v: fn(
            Context<D, Args>,
        )
            -> Pin<Box<dyn Future<Output = Result<Context<D, Ret>, Box<dyn Error>>> + '_>>,
    ) -> Self {
        Self(v)
    }
}

impl<D> Clone for AsyncFp<D> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for AsyncFp<D> {}

/// Helper enum to encapsulates either integer or float.
#[derive(Clone, Copy, PartialEq)]
pub enum Number {
    Int(i64),
    Float(f64),
}

impl From<i64> for Number {
    #[inline(always)]
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<f64> for Number {
    #[inline(always)]
    fn from(value: f64) -> Self {
        Self::Float(value)
    }
}

/// Type of operator.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Ops {
    Add,
    Sub,
    Mul,
    Mod,
    Pow,
    NumDiv,
    IntDiv,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    Neg,
    Not,
}

impl Ops {
    pub const fn from_u8(v: u8) -> Option<Self> {
        match v {
            v if v == Self::Add as u8 => Some(Self::Add),
            v if v == Self::Sub as u8 => Some(Self::Sub),
            v if v == Self::Mul as u8 => Some(Self::Mul),
            v if v == Self::Mod as u8 => Some(Self::Mod),
            v if v == Self::Pow as u8 => Some(Self::Pow),
            v if v == Self::NumDiv as u8 => Some(Self::NumDiv),
            v if v == Self::IntDiv as u8 => Some(Self::IntDiv),
            v if v == Self::And as u8 => Some(Self::And),
            v if v == Self::Or as u8 => Some(Self::Or),
            v if v == Self::Xor as u8 => Some(Self::Xor),
            v if v == Self::Shl as u8 => Some(Self::Shl),
            v if v == Self::Shr as u8 => Some(Self::Shr),
            v if v == Self::Neg as u8 => Some(Self::Neg),
            v if v == Self::Not as u8 => Some(Self::Not),
            _ => None,
        }
    }
}

/// Represents an error when [`Fp`] or [`AsyncFp`] return an error.
#[derive(Debug)]
pub struct CallError {
    chunk: Option<(String, u32)>,
    reason: Box<dyn Error>,
}

impl CallError {
    unsafe fn new<D>(
        th: *const Thread<D>,
        caller: *mut CallInfo<D>,
        mut reason: Box<dyn Error>,
    ) -> Box<Self> {
        // Forward ourself.
        reason = match reason.downcast() {
            Ok(v) => return v,
            Err(e) => e,
        };

        // Traverse up until reaching a Lua function.
        let mut ci = unsafe { (*th).ci.get() };
        let mut chunk = None;

        while unsafe { ci != caller && ci != (*th).base_ci.get() } {
            let mut ar = lua_Debug {
                i_ci: ci,
                ..Default::default()
            };

            unsafe { lua_getinfo(th, c"Sl".as_ptr(), &mut ar) };

            if let Some(v) = ar.source {
                chunk = Some((v.name, u32::try_from(ar.currentline).unwrap()));
                break;
            }

            ci = unsafe { (*ci).previous };
        }

        Box::new(Self { chunk, reason })
    }

    /// Returns chunk name and line number if this error triggered from Lua.
    pub fn location(&self) -> Option<(&str, u32)> {
        self.chunk.as_ref().map(|(n, l)| (n.as_str(), *l))
    }
}

impl Error for CallError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.reason.source()
    }
}

impl Display for CallError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        self.reason.fmt(f)
    }
}

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

/// Represents an error when [Lua::use_module()] fails due to the module already exists.
#[derive(Debug, Error)]
#[error("module with the same name already exists")]
pub struct ModuleExists;

/// Represents an error when a function that cannot be recursive call itself either directly or
/// indirectly.
#[derive(Debug, Error)]
#[error("a call to '{0}' cannot be recursive")]
pub struct RecursiveCall(&'static str);

impl RecursiveCall {
    fn new<F>(_: F) -> Self {
        Self(core::any::type_name::<F>())
    }
}

static NON_YIELDABLE_WAKER: RawWakerVTable = RawWakerVTable::new(
    |_| unimplemented!(),
    |_| unimplemented!(),
    |_| unimplemented!(),
    |_| {},
);
