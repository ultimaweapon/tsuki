pub use self::builder::*;
pub use self::error::*;
pub use self::function::*;
pub use self::gc::*;
pub use self::parser::*;
pub use self::table::*;
pub use self::thread::*;

use self::lapi::lua_settop;
use self::ldo::luaD_protectedparser;
use self::lmem::luaM_free_;
use self::lobject::{TString, TValue, Table};
use self::lzio::Zio;
use std::cell::{Cell, UnsafeCell};
use std::ffi::c_int;
use std::marker::PhantomPinned;
use std::ops::Deref;
use std::pin::Pin;
use std::ptr::null_mut;
use std::rc::Rc;
use thiserror::Error;

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
unsafe fn lua_pop(th: *const Thread, n: c_int) -> Result<(), Box<dyn std::error::Error>> {
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

            let io1: *mut TValue = unsafe { (*(*f).upvals[0].get()).v.get() };

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

/// Lua value.
pub enum Value {}

/// Represents an error when arithmetic operation fails.
#[derive(Debug, Error)]
pub enum ArithError {
    #[error("attempt to perform 'n%0'")]
    ModZero,

    #[error("attempt to divide by zero")]
    DivZero,
}
