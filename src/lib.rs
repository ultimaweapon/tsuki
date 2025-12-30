//! Lua 5.4 ported to Rust.
//!
//! # Quickstart
//!
//! ```
//! use tsuki::builtin::{BaseLib, CoroLib, MathLib, StrLib, TableLib, Utf8Lib};
//! use tsuki::context::{Args, Context, Ret};
//! use tsuki::{Lua, Value, fp};
//!
//! fn main() {
//!     // Set up.
//!     let lua = Lua::new(());
//!
//!     lua.use_module(None, true, BaseLib).unwrap();
//!     lua.use_module(None, true, CoroLib).unwrap();
//!     lua.use_module(None, true, MathLib).unwrap();
//!     lua.use_module(None, true, StrLib).unwrap();
//!     lua.use_module(None, true, TableLib).unwrap();
//!     lua.use_module(None, true, Utf8Lib).unwrap();
//!
//!     lua.global().set_str_key("myfunc", fp!(myfunc));
//!
//!     // Run on main thread.
//!     let chunk = lua.load("abc.lua", "return myfunc()").unwrap();
//!     let td = lua.create_thread();
//!     let result = td.call(chunk, ()).unwrap();
//!
//!     match result {
//!         Value::Str(v) => assert_eq!(v.as_utf8(), Some("Hello world!")),
//!         _ => todo!(),
//!     }
//! }
//!
//! fn myfunc(cx: Context<(), Args>) -> Result<Context<(), Ret>, Box<dyn core::error::Error>> {
//!     cx.push_str("Hello world!")?;
//!
//!     Ok(cx.into())
//! }
//! ```
//!
//! # Types that can be converted to UnsafeValue
//!
//! You can pass the value of the following types for `impl Into<UnsafeValue>`:
//!
//! - [Nil]
//! - [bool]
//! - [Fp]
//! - [YieldFp]
//! - [AsyncFp]
//! - [i8]
//! - [i16]
//! - [i32]
//! - [i64]
//! - [u8]
//! - [u16]
//! - [u32]
//! - [f32]
//! - [f64]
//! - [Float]
//! - [Number]
//! - Reference to [Str]
//! - Reference to [Table]
//! - Reference to [LuaFn]
//! - Reference to [UserData]
//! - Reference to [Thread]
//! - [Ref]
//! - [Value] or a reference to it
//! - [Arg] or a reference to it
//!
//! The value will be converted to corresponding Lua value. Tsuki does not expose [UnsafeValue] by
//! design so you cannot construct its value. Tsuki also never handout the value of [UnsafeValue].
//!
//! # Get function argument
//!
//! Use [Context::arg()] to get an argument passed to Rust function:
//!
//! ```
//! # use tsuki::context::{Args, Context, Ret};
//! fn myfunc(cx: Context<(), Args>) -> Result<Context<(), Ret>, Box<dyn core::error::Error>> {
//!     let arg = cx.arg(1); // One-based the same as Lua so this is first argument.
//!     let val = arg.to_int()?;
//!
//!     if val < 0 {
//!         return Err(arg.error("expect positive integer"));
//!     }
//!
//!     // This will return nil since to any values pushed to cx.
//!     Ok(cx.into())
//! }
//! ```
//!
//! # Parsing Lua option
//!
//! Tsuki provides a derive macro [FromStr] to handle this.
//!
//! # Store value in registry
//!
//! You need to create a type per key in registry:
//!
//! ```
//! use tsuki::{RegKey, Table};
//!
//! struct MyKey;
//!
//! impl<A> RegKey<A> for MyKey {
//!     type Value<'a>
//!         = Table<A>
//!     where
//!         A: 'a;
//! }
//! ```
//!
//! Type itself is a key, not its value. Then you can use [Lua::set_registry()] or
//! [Context::set_registry()] to set the value and [Lua::registry()] or [Context::registry()] to
//! retrieve the value.
//!
//! # Store value in Rust collection
//!
//! Tsuki also provides Rust collection that can store Lua values. The following code demonstrate a
//! registry value of [BTreeMap] to map Rust [String] to any Lua value:
//!
//! ```
//! use tsuki::collections::BTreeMap;
//! use tsuki::context::{Args, Context, Ret};
//! use tsuki::{Dynamic, RegKey};
//!
//! fn myfunc(cx: Context<(), Args>) -> Result<Context<(), Ret>, Box<dyn core::error::Error>> {
//!     let v = cx.arg(1);
//!     let r = cx.registry::<MyKey>().unwrap();
//!
//!     r.insert(String::from("abc"), v);
//!
//!     Ok(cx.into())
//! }
//!
//! struct MyKey;
//!
//! impl<A> RegKey<A> for MyKey {
//!     type Value<'a>
//!         = BTreeMap<A, String, Dynamic>
//!     where
//!         A: 'a;
//! }
//! ```
//!
//! See [collections] module for available collections.
#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]

pub use self::function::*;
pub use self::gc::Ref;
pub use self::module::*;
pub use self::number::*;
pub use self::parser::*;
pub use self::registry::*;
pub use self::string::*;
pub use self::table::*;
pub use self::thread::*;
pub use self::ty::*;
pub use self::userdata::*;

use self::collections::{BTreeMap, CollectionValue};
use self::context::{Arg, Args, Context, Ret};
use self::gc::{Gc, Object};
use self::ldebug::{funcinfo, luaG_getfuncline};
use self::ldo::luaD_protectedparser;
use self::llex::{TK_WHILE, luaX_tokens};
use self::lstate::lua_Debug;
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
use core::any::{Any, TypeId, type_name};
use core::cell::{Cell, UnsafeCell};
use core::convert::identity;
use core::error::Error;
use core::fmt::{Display, Formatter};
use core::marker::PhantomPinned;
use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut};
use core::pin::Pin;
use core::ptr::null;
use core::task::RawWakerVTable;
use thiserror::Error;

pub mod builtin;
pub mod collections;
pub mod context;

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
mod llex;
mod lmem;
mod lobject;
mod lparser;
mod lstate;
mod lstring;
mod ltm;
mod lzio;
mod module;
mod number;
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

/// Helper macro to construct [Fp] or [AsyncFp].
#[macro_export]
macro_rules! fp {
    ($f:path) => {
        $crate::Fp::new($f)
    };
    ($f:path as yield) => {
        $crate::YieldFp::new($f)
    };
    ($f:path as async) => {
        $crate::async_fp!($f)
    };
}

#[cfg(feature = "std")]
#[doc(hidden)]
#[macro_export]
macro_rules! async_fp {
    ($f:path) => {
        $crate::AsyncFp::new(|cx| ::std::boxed::Box::pin($f(cx)))
    };
}

#[cfg(not(feature = "std"))]
#[doc(hidden)]
#[macro_export]
macro_rules! async_fp {
    ($f:path) => {
        $crate::AsyncFp::new(|cx| ::alloc::boxed::Box::pin($f(cx)))
    };
}

/// Generate [core::str::FromStr] implementation for enum to parse Lua
/// [option](https://www.lua.org/manual/5.4/manual.html#luaL_checkoption).
///
/// Only enum with unit variants is supported. The name to map will be the same as Lua convention,
/// which is lower-cased without separators:
///
/// ```
/// use tsuki::FromStr;
///
/// #[derive(FromStr)]
/// enum MyOption {
///     Foo,
///     FooBar,
/// }
/// ```
///
/// Will map `foo` to `MyOption::Foo` and `foobar` to `MyOption::FooBar`.
pub use tsuki_macros::FromStr;

/// Generate [Class] implementation from `impl` block.
///
/// This attribute macro inspect all associated functions and methods within the `impl` block and
/// generate a metatable:
///
/// ```
/// use tsuki::context::{Args, Context, Ret};
/// use tsuki::class;
///
/// struct MyUserData;
///
/// #[class(associated_data = ())]
/// impl MyUserData {
///     fn index1(&self, cx: &Context<(), Args>) -> Result<(), Box<dyn core::error::Error>> {
///         Ok(())
///     }
///
///     fn index2(cx: Context<(), Args>) -> Result<Context<(), Ret>, Box<dyn core::error::Error>> {
///         Ok(cx.into())
///     }
///
///     async fn index3(&self, cx: &Context<'_, (), Args>) -> Result<(), Box<dyn core::error::Error>> {
///         Ok(())
///     }
///
///     #[close]
///     fn close(&self, cx: &Context<(), Args>) -> Result<(), Box<dyn core::error::Error>> {
///         Ok(())
///     }
/// }
/// ```
///
/// Will set `index1`, `index2`, `index3` and `close` to the metatable with the same name. The
/// `__index` will be set to the table itself and `__close` will be set to `close`. Use
/// `#[close(hidden)]` to set to `__close` only.
///
/// The `associated_data` is a type of the value passed as `associated_data` to [Lua::new()]. You
/// can specify a type parameter if `MyUserData` can works with multiple types.
pub use tsuki_macros::class;

/// Global states shared with all Lua threads.
#[repr(C)] // Force gc field to be the first field.
pub struct Lua<A> {
    gc: Gc<A>,
    strt: StringTable<A>,
    l_registry: UnsafeCell<UnsafeValue<A>>,
    nilvalue: UnsafeCell<UnsafeValue<A>>,
    dummy_node: Node<A>,
    seed: u32,
    modules_locked: Cell<bool>,
    associated_data: A,
    phantom: PhantomPinned,
}

impl<A> Lua<A> {
    /// Create a new [Lua] with a random seed to hash Lua string.
    ///
    /// You can retrieve `associated_data` later with [Self::associated_data()] or
    /// [Context::associated_data()].
    ///
    /// Note that all built-in functions (e.g. `print`) are not enabled by default.
    #[cfg(feature = "rand")]
    #[cfg_attr(docsrs, doc(cfg(feature = "rand")))]
    pub fn new(associated_data: A) -> Pin<Rc<Self>> {
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
    pub fn with_seed(associated_data: A, seed: u32) -> Pin<Rc<Self>> {
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
            modules_locked: Cell::new(false),
            associated_data,
            phantom: PhantomPinned,
        });

        // Setup registry.
        let reg = unsafe { Table::new(g.deref()) };

        unsafe { g.gc.set_root(reg.cast()) };
        unsafe { g.l_registry.get().write(UnsafeValue::from_obj(reg.cast())) };
        unsafe { luaH_resize(reg, 6, 0) };

        // Create LUA_RIDX_GLOBALS.
        let reg = unsafe { (*reg).array.get() };
        let glb = unsafe { Table::new(g.deref()) };

        unsafe { reg.add(0).write(false.into()) };
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
            let v = unsafe { Str::from_str(g.deref(), v).unwrap_or_else(identity) };
            let v = unsafe { UnsafeValue::from_obj(v.cast()) };

            unsafe { (*events).set_unchecked(k, v).unwrap_unchecked() };
        }

        unsafe { reg.add(3).write(UnsafeValue::from_obj(events.cast())) };

        // Create table for Lua tokens.
        let tokens = unsafe { Table::new(g.deref()) };
        let n = TK_WHILE - (255 + 1) + 1;

        unsafe { luaH_resize(tokens, 0, n.try_into().unwrap()) };

        for i in 0..n {
            let k = unsafe { Str::from_str(g.deref(), luaX_tokens[i as usize]) }
                .unwrap_or_else(identity);
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
    pub fn associated_data(&self) -> &A {
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
        M: Module<A>,
        M::Inst<'a>: Into<UnsafeValue<A>>,
    {
        // Prevent recursive call.
        let lock = match ModulesLock::new(&self.modules_locked) {
            Some(v) => v,
            None => return Err(Box::new(RecursiveCall::new(Self::use_module::<M>))),
        };

        // Check if exists.
        let name = name.unwrap_or(M::NAME);
        let n = unsafe { Str::from_str(self, name).unwrap_or_else(identity) };
        let n = unsafe { UnsafeValue::from_obj(n.cast()) };
        let t = self.modules();
        let s = unsafe { t.get_raw_unchecked(n) };

        if unsafe { ((*s).tt_ & 0xf) != 0 } {
            return Err(Box::new(ModuleExists));
        }

        // Open the module. We need a strong reference to name here since the module can trigger GC.
        let n = unsafe { Ref::new(n.value_.gc.cast::<Str<A>>()) };
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
    pub fn set_str_metatable(&self, mt: &Table<A>) {
        if mt.hdr.global != self {
            panic!("attempt to set string metatable created from a different Lua");
        }

        // Prevent __gc metamethod.
        if unsafe { mt.flags.get() & 1 << TM_GC == 0 && !luaT_gettm(mt, TM_GC).is_null() } {
            panic!("__gc metamethod is not supported");
        }

        unsafe { self.metatables().set_unchecked(4, mt).unwrap_unchecked() };
    }

    /// Register a metatable for userdata `T`. If the metatable for `T` already exists it will be
    /// **replaced**.
    ///
    /// See [Class] if you want to automate the metatable creation for your userdata.
    ///
    /// This does not change the metatable for any userdata that already created.
    ///
    /// # Panics
    /// - If `mt` was created from different [Lua](crate::Lua) instance.
    /// - If `mt` contains `__gc`.
    pub fn register_metatable<T: Any>(&self, mt: &Table<A>) {
        if mt.hdr.global != self {
            panic!("attempt to register a metatable created from a different Lua");
        }

        // Prevent __gc metamethod.
        if unsafe { mt.flags.get() & 1 << TM_GC == 0 && !luaT_gettm(mt, TM_GC).is_null() } {
            panic!("__gc metamethod is not supported");
        }

        // Add to list.
        let k = unsafe { RustId::new(self, TypeId::of::<T>()) };
        let k = unsafe { UnsafeValue::from_obj(k.cast()) };

        unsafe { self.metatables().set_unchecked(k, mt).unwrap_unchecked() };

        self.gc.step();
    }

    /// Register a metatable for userdata `T`. If the metatable for `T` already exists it will be
    /// **replaced**.
    ///
    /// This does not change the metatable for any userdata that already created.
    ///
    /// # Panics
    /// - If [Class::create_metatable()] returns a table that was created from different
    ///   [Lua](crate::Lua) instance.
    /// - If [Class::create_metatable()] returns a table that contains `__gc`.
    pub fn register_class<T: Class<A>>(&self) {
        // Create metatable.
        let mt = T::create_metatable(self);
        let mt = mt.deref();

        if mt.hdr.global != self {
            panic!(
                "{} returns a table that was created from a different Lua",
                type_name::<T>()
            );
        } else if unsafe { mt.flags.get() & 1 << TM_GC == 0 && !luaT_gettm(mt, TM_GC).is_null() } {
            panic!("{} returns a table that contains __gc", type_name::<T>());
        }

        // Add to list.
        let k = unsafe { RustId::new(self, TypeId::of::<T>()) };
        let k = unsafe { UnsafeValue::from_obj(k.cast()) };

        unsafe { self.metatables().set_unchecked(k, mt).unwrap_unchecked() };

        self.gc.step();
    }

    /// Sets a value to registry.
    ///
    /// # Panics
    /// If `v` was created from different [Lua](crate::Lua) instance.
    pub fn set_registry<'a, K>(&self, v: <K::Value<'a> as RegValue<A>>::In<'a>)
    where
        K: RegKey<A>,
        K::Value<'a>: RegValue<A>,
    {
        let v = K::Value::into_unsafe(v);

        if unsafe { (v.tt_ & 1 << 6) != 0 && (*v.value_.gc).global != self } {
            panic!("attempt to set registry value created from different Lua instance");
        }

        // Set.
        let r = unsafe { (*self.l_registry.get()).value_.gc.cast::<Table<A>>() };
        let k = unsafe { RustId::new(self, TypeId::of::<K>()) };
        let k = unsafe { UnsafeValue::from_obj(k.cast()) };

        // SAFETY: k is not nil or NaN.
        unsafe { (*r).set_unchecked(k, v).unwrap_unchecked() };
    }

    /// Returns value on registry that was set with [Self::set_registry()] or
    /// [Context::set_registry()].
    pub fn registry<'a, K>(&'a self) -> Option<<K::Value<'a> as RegValue<A>>::Out<'a>>
    where
        K: RegKey<A>,
        K::Value<'a>: RegValue<A>,
    {
        let id = TypeId::of::<K>();
        let reg = unsafe { &*(*self.l_registry.get()).value_.gc.cast::<Table<A>>() };
        let s = unsafe { luaH_getid(reg, &id) };

        match unsafe { (*s).tt_ & 0xf } {
            0 => None,
            _ => Some(unsafe { K::Value::from_unsafe(s) }),
        }
    }

    /// Returns a global table.
    #[inline(always)]
    pub fn global(&self) -> &Table<A> {
        let reg = unsafe { (*self.l_registry.get()).value_.gc.cast::<Table<A>>() };
        let tab = unsafe { (*reg).array.get().add(1) };
        let tab = unsafe { (*tab).value_.gc.cast::<Table<A>>() };

        unsafe { &*tab }
    }

    /// Create a Lua string with UTF-8 content.
    #[inline(always)]
    pub fn create_str<T>(&self, v: T) -> Ref<'_, Str<A>>
    where
        T: AsRef<str> + AsRef<[u8]> + Into<Vec<u8>>,
    {
        let s = unsafe { Str::from_str(self, v) };
        let v = unsafe { Ref::new(s.unwrap_or_else(identity)) };

        if s.is_ok() {
            self.gc.step();
        }

        v
    }

    /// Create a Lua string with binary content.
    #[inline(always)]
    pub fn create_bytes<T>(&self, v: T) -> Ref<'_, Str<A>>
    where
        T: AsRef<[u8]> + Into<Vec<u8>>,
    {
        let s = unsafe { Str::from_bytes(self, v) };
        let v = unsafe { Ref::new(s.unwrap_or_else(identity)) };

        if s.is_ok() {
            self.gc.step();
        }

        v
    }

    /// Create a Lua table.
    #[inline(always)]
    pub fn create_table(&self) -> Ref<'_, Table<A>> {
        self.gc.step();

        unsafe { Ref::new(Table::new(self)) }
    }

    /// Create a full userdata.
    ///
    /// The metatable for the userdata that was registered with [Self::register_metatable()] will be
    /// loaded during creation. A call to [Self::register_metatable()] has no effect for any
    /// userdata that already created.
    #[inline(always)]
    pub fn create_ud<T: Any>(&self, v: T) -> Ref<'_, UserData<A, T>> {
        self.gc.step();

        unsafe { Ref::new(UserData::new(self, v).cast()) }
    }

    /// Create a new Lua thread (AKA coroutine).
    pub fn create_thread(&self) -> Ref<'_, Thread<A>> {
        self.gc.step();

        unsafe { Ref::new(Thread::new(self)) }
    }

    /// Create a new [BTreeMap] to map Rust value to Lua value.
    ///
    /// `K` can be any Rust type that implement [Ord]. See [collections] module for a list of
    /// possible type for `V`.
    pub fn create_btree_map<K, V>(&self) -> Ref<'_, BTreeMap<A, K, V>>
    where
        K: Ord + 'static,
        V: CollectionValue<A> + 'static,
    {
        self.gc.step();

        unsafe { Ref::new(BTreeMap::new(self)) }
    }

    /// Deserialize [Value] from Serde deserializer.
    ///
    /// This method can only deserialize a value from self-describing formats.
    #[cfg(feature = "serde")]
    #[cfg_attr(docsrs, doc(cfg(feature = "serde")))]
    pub fn deserialize_value<'de, D: serde::Deserializer<'de>>(
        &self,
        deserializer: D,
    ) -> Result<Value<'_, A>, D::Error> {
        deserializer.deserialize_any(self::value::serde::ValueVisitor::new(self))
    }

    /// Load a Lua chunk.
    pub fn load(
        &self,
        name: impl Into<String>,
        chunk: impl AsRef<[u8]>,
    ) -> Result<Ref<'_, LuaFn<A>>, ParseError> {
        let chunk = chunk.as_ref();
        let z = Zio {
            n: chunk.len(),
            p: chunk.as_ptr().cast(),
        };

        // Load.
        let f = unsafe { luaD_protectedparser(self, z, name.into().into())? };

        if !(*f).upvals.is_empty() {
            let gt = unsafe {
                (*((*self.l_registry.get()).value_.gc.cast::<Table<A>>()))
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

    unsafe fn metatable(&self, o: *const UnsafeValue<A>) -> *const Table<A> {
        match unsafe { (*o).tt_ & 0xf } {
            5 => unsafe { (*(*o).value_.gc.cast::<Table<A>>()).metatable.get() },
            7 => unsafe { (*(*o).value_.gc.cast::<UserData<A, ()>>()).mt },
            v => unsafe { self.metatables().get_raw_int_key(v.into()).value_.gc.cast() },
        }
    }

    #[inline(always)]
    fn metatables(&self) -> &Table<A> {
        let reg = unsafe { (*self.l_registry.get()).value_.gc.cast::<Table<A>>() };
        let tab = unsafe { (*reg).array.get().add(2) };
        let tab = unsafe { (*tab).value_.gc.cast::<Table<A>>() };

        unsafe { &*tab }
    }

    #[inline(always)]
    fn events(&self) -> &Table<A> {
        let reg = unsafe { (*self.l_registry.get()).value_.gc.cast::<Table<A>>() };
        let tab = unsafe { (*reg).array.get().add(3) };
        let tab = unsafe { (*tab).value_.gc.cast::<Table<A>>() };

        unsafe { &*tab }
    }

    #[inline(always)]
    fn tokens(&self) -> &Table<A> {
        let reg = unsafe { (*self.l_registry.get()).value_.gc.cast::<Table<A>>() };
        let tab = unsafe { (*reg).array.get().add(4) };
        let tab = unsafe { (*tab).value_.gc.cast::<Table<A>>() };

        unsafe { &*tab }
    }

    #[inline(always)]
    fn modules(&self) -> &Table<A> {
        let reg = unsafe { (*self.l_registry.get()).value_.gc.cast::<Table<A>>() };
        let tab = unsafe { (*reg).array.get().add(5) };
        let tab = unsafe { (*tab).value_.gc.cast::<Table<A>>() };

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

/// Encapsulates a Lua value.
#[repr(u64, align(8))] // Force field to be at offset 8.
pub enum Value<'a, A> {
    /// The value is `nil`.
    Nil = 0 | 0 << 4,
    /// The value is `false`.
    False = 1 | 0 << 4,
    /// The value is `true`.
    True = 1 | 1 << 4,
    /// The value is `function` implemented in Rust.
    Fp(Fp<A>) = 2 | 0 << 4,
    /// The value is `function` implemented in Rust to yield coroutine.
    YieldFp(YieldFp<A>) = 2 | 1 << 4,
    /// The value is `function` implemented in Rust as async function.
    AsyncFp(AsyncFp<A>) = 2 | 2 << 4,
    /// The value is `integer`.
    Int(i64) = 3 | 0 << 4,
    /// The value is `float`.
    Float(Float) = 3 | 1 << 4,
    /// The value is `string`.
    Str(Ref<'a, Str<A>>) = 4 | 0 << 4 | 1 << 6,
    /// The value is `table`.
    Table(Ref<'a, Table<A>>) = 5 | 0 << 4 | 1 << 6,
    /// The value is `function` implemented in Lua.
    LuaFn(Ref<'a, LuaFn<A>>) = 6 | 0 << 4 | 1 << 6,
    /// The value is `full userdata`.
    UserData(Ref<'a, UserData<A, dyn Any>>) = 7 | 0 << 4 | 1 << 6,
    /// The value is `thread`.
    Thread(Ref<'a, Thread<A>>) = 8 | 0 << 4 | 1 << 6,
}

// Make sure all fields live at offset 8.
const _: () = assert!(align_of::<Fp<()>>() <= 8);
const _: () = assert!(align_of::<YieldFp<()>>() <= 8);
const _: () = assert!(align_of::<AsyncFp<()>>() <= 8);
const _: () = assert!(align_of::<i64>() <= 8);
const _: () = assert!(align_of::<Float>() <= 8);
const _: () = assert!(align_of::<Ref<Str<()>>>() <= 8);

impl<'a, A> Value<'a, A> {
    /// Constructs [Value] from [Arg].
    ///
    /// Returns [None] if argument `v` does not exists.
    #[inline]
    pub fn from_arg(v: &Arg<'_, 'a, A>) -> Option<Self> {
        let v = v.get_raw_or_null();

        match v.is_null() {
            true => None,
            false => Some(unsafe { Self::from_unsafe(v) }),
        }
    }

    /// Returns `true` if this value is [Value::Nil].
    #[inline(always)]
    pub const fn is_nil(&self) -> bool {
        matches!(self, Self::Nil)
    }

    /// Returns [Type] for this value.
    #[inline(always)]
    pub fn ty(&self) -> Type {
        // SAFETY: Value has #[repr(u64)].
        let t = unsafe { (self as *const Self as *const u64).read() };

        // SAFETY: Low-order byte has the same value as Type.
        unsafe { core::mem::transmute((t & 0xf) as u8) }
    }

    /// Returns `false` if this value is either [Value::Nil] or [Value::False] otherwise `true`.
    ///
    /// This has the same semantic as `lua_toboolean`.
    #[inline(always)]
    pub const fn to_bool(&self) -> bool {
        !matches!(self, Self::Nil | Self::False)
    }

    #[inline(never)]
    unsafe fn from_unsafe(v: *const UnsafeValue<A>) -> Self {
        let mut r = MaybeUninit::<Self>::uninit();
        let p = r.as_mut_ptr().cast::<u64>();
        let t = unsafe { (*v).tt_ };

        match t & 0xf {
            0 => unsafe { p.write(0 | 0 << 4) },
            1 | 2 | 3 => unsafe {
                p.write(t.into());
                p.add(1).cast::<UntaggedValue<A>>().write((*v).value_);
            },
            4 | 5 | 6 | 7 | 8 => unsafe {
                let v = (*v).value_.gc;

                p.write(t.into());
                p.add(1).cast::<*const Object<A>>().write(v);

                core::mem::forget(Ref::new_inline(v));
            },
            _ => unreachable!(),
        }

        unsafe { r.assume_init() }
    }
}

/// Unit struct to create `nil` value.
pub struct Nil;

/// Non-Yieldable Rust function.
#[repr(transparent)]
pub struct Fp<A>(fn(Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn Error>>);

impl<A> Fp<A> {
    /// Construct a new [Fp] from a function pointer.
    ///
    /// [fp] macro is more convenience than this function.
    #[inline(always)]
    pub const fn new(v: fn(Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn Error>>) -> Self {
        Self(v)
    }
}

impl<A> Clone for Fp<A> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

impl<A> Copy for Fp<A> {}

/// Rust function to yield Lua values.
#[repr(transparent)]
pub struct YieldFp<A>(fn(Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn Error>>);

impl<A> YieldFp<A> {
    /// Construct a new [YieldFp] from a function pointer.
    ///
    /// [fp] macro is more convenience than this function.
    #[inline(always)]
    pub const fn new(v: fn(Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn Error>>) -> Self {
        Self(v)
    }
}

impl<A> Clone for YieldFp<A> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A> Copy for YieldFp<A> {}

/// Asynchronous Rust function.
///
/// Note that this function can only be called from Lua. In other words, it cannot be called from
/// [Fp] or this function either directly or indirectly.
///
/// Each call into async function from Lua always incur one heap allocation so create async function
/// only when necessary.
///
/// You need to use [Thread::async_call()] to be able to call this function from Lua.
#[repr(transparent)]
pub struct AsyncFp<A>(
    fn(
        Context<A, Args>,
    ) -> Pin<Box<dyn Future<Output = Result<Context<A, Ret>, Box<dyn Error>>> + '_>>,
);

impl<A> AsyncFp<A> {
    /// Construct a new [AsyncFp] from a function pointer.
    ///
    /// [fp] macro is more convenience than this function.
    #[inline(always)]
    pub const fn new(
        v: fn(
            Context<A, Args>,
        )
            -> Pin<Box<dyn Future<Output = Result<Context<A, Ret>, Box<dyn Error>>> + '_>>,
    ) -> Self {
        Self(v)
    }
}

impl<A> Clone for AsyncFp<A> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

impl<A> Copy for AsyncFp<A> {}

/// Unit struct to store any value in registry or collection.
pub struct Dynamic;

/// Type of operator.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum Ops {
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
    const fn from_u8(v: u8) -> Option<Self> {
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

/// A wrapper of [Vec] to provide [core::fmt::Write].
#[derive(Default)]
struct Buffer(Vec<u8>);

impl Deref for Buffer {
    type Target = Vec<u8>;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Buffer {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl AsRef<[u8]> for Buffer {
    #[inline(always)]
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl core::fmt::Write for Buffer {
    #[inline]
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.0.extend_from_slice(s.as_bytes());
        Ok(())
    }
}

impl From<Buffer> for Vec<u8> {
    #[inline(always)]
    fn from(value: Buffer) -> Self {
        value.0
    }
}

/// Represents an error when [`Fp`] or [`AsyncFp`] return an error.
#[derive(Debug)]
pub struct CallError {
    chunk: Option<(Rc<String>, u32)>,
    reason: Box<dyn Error>,
}

impl CallError {
    unsafe fn new<A>(th: *const Thread<A>, mut reason: Box<dyn Error>) -> Box<Self> {
        // Forward ourself.
        reason = match reason.downcast() {
            Ok(v) => return v,
            Err(e) => e,
        };

        // Traverse up until reaching a Lua function.
        let mut ci = unsafe { (*th).ci.get() };
        let mut chunk = None;

        while unsafe { ci != (*th).base_ci.get() } {
            let mut ar = lua_Debug::default();

            let func = unsafe { (*th).stack.get().add((*ci).func) };
            let cl = if unsafe {
                (*func).tt_ == 6 | 0 << 4 | 1 << 6 || (*func).tt_ == 6 | 2 << 4 | 1 << 6
            } {
                unsafe { (*func).value_.gc }
            } else {
                null()
            };

            unsafe { funcinfo(&mut ar, cl) };

            ar.currentline = if unsafe { !ci.is_null() && (*ci).callstatus & 1 << 1 == 0 } {
                unsafe {
                    luaG_getfuncline(
                        (*(*func).value_.gc.cast::<LuaFn<A>>()).p.get(),
                        ((*ci).pc - 1) as _,
                    )
                }
            } else {
                -1
            };

            if let Some(v) = ar.chunk {
                chunk = Some((v, u32::try_from(ar.currentline).unwrap()));
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

static YIELDABLE_WAKER: RawWakerVTable = RawWakerVTable::new(
    |_| unimplemented!(),
    |_| unimplemented!(),
    |_| unimplemented!(),
    |_| {},
);
