#![no_std]

pub use self::builder::*;
pub use self::context::*;
pub use self::function::*;
pub use self::gc::Ref;
pub use self::module::*;
pub use self::parser::*;
pub use self::string::*;
pub use self::table::*;
pub use self::thread::*;

use self::gc::{Gc, Object, luaC_barrier_, luaC_freeallobjects};
use self::lapi::lua_settop;
use self::ldo::luaD_protectedparser;
use self::lobject::Udata;
use self::lzio::Zio;
use self::value::UnsafeValue;
use alloc::boxed::Box;
use alloc::rc::Rc;
use core::any::TypeId;
use core::cell::{Cell, RefCell, UnsafeCell};
use core::ffi::c_int;
use core::marker::PhantomPinned;
use core::ops::Deref;
use core::pin::Pin;
use core::ptr::null_mut;
use core::task::RawWakerVTable;
use hashbrown::HashMap;
use rustc_hash::FxBuildHasher;
use thiserror::Error;

mod builder;
mod builtin;
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
mod lvm;
mod lzio;
mod module;
mod parser;
mod string;
mod table;
mod thread;
mod value;

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
///
/// Use [`Builder`] to get an instance of this type.
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
    /// Load a Lua chunk.
    pub fn load(
        self: &Pin<Rc<Self>>,
        info: ChunkInfo,
        chunk: impl AsRef<[u8]>,
    ) -> Result<Ref<LuaFn>, ParseError> {
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
                    unsafe {
                        luaC_barrier_(self.deref(), (*f).upvals[0].get().cast(), (*gt).value_.gc)
                    };
                }
            }
        }

        Ok(f)
    }

    /// Create a new Lua thread (AKA coroutine).
    #[inline(always)]
    pub fn spawn(self: &Pin<Rc<Self>>) -> Ref<Thread> {
        Thread::new(self)
    }

    /// Returns a global table.
    #[inline(always)]
    pub fn global(&self) -> &Table {
        let reg = unsafe { (*self.l_registry.get()).value_.gc.cast::<Table>() };
        let tab = unsafe { (*reg).array.get().add(2 - 1) };
        let tab = unsafe { (*tab).value_.gc.cast::<Table>() };

        unsafe { &*tab }
    }

    /// Create a Lua string.
    pub fn create_str(&self, v: impl AsRef<str>) -> Ref<Str> {
        unsafe { Ref::new(self.to_rc(), Str::new(self, v.as_ref())) }
    }

    pub fn create_table(&self) -> Ref<Table> {
        todo!()
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
pub enum Value {}

/// Non-Yieldable Rust function.
#[derive(Clone, Copy)]
pub struct Fp(pub fn(Context<Args>) -> Result<Context<()>, Box<dyn core::error::Error>>);

#[derive(Clone, Copy)]
pub struct YieldFp(pub fn(Context<Args>) -> Result<Context<()>, Box<dyn core::error::Error>>);

#[derive(Clone, Copy)]
pub struct AsyncFp(
    pub  for<'a> fn(
        Context<'a, Args>,
    ) -> Box<
        dyn Future<Output = Result<Context<'a, ()>, Box<dyn core::error::Error>>> + 'a,
    >,
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
