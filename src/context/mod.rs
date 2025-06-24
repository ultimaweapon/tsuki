pub use self::arg::*;

use crate::lapi::{lua_checkstack, lua_pcall};
use crate::lobject::StackValue;
use crate::value::UnsafeValue;
use crate::{
    ChunkInfo, LuaFn, NON_YIELDABLE_WAKER, ParseError, Ref, StackOverflow, Str, Table, Thread, Type,
};
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::Cell;
use core::num::NonZero;
use core::pin::pin;
use core::ptr::null;
use core::task::{Poll, Waker};

mod arg;

/// Context to invoke Rust function.
pub struct Context<'a, T> {
    th: &'a Thread,
    ret: Cell<usize>,
    payload: T,
}

impl<'a, T> Context<'a, T> {
    #[inline(always)]
    pub(crate) fn new(th: &'a Thread, payload: T) -> Self {
        Self {
            th,
            ret: Cell::new(0),
            payload,
        }
    }

    /// Create a Lua string.
    pub fn create_str<V>(&self, v: V) -> Ref<Str>
    where
        V: AsRef<str> + AsRef<[u8]> + Into<Vec<u8>>,
    {
        let s = unsafe { Str::from_str(self.th.hdr.global, v) };

        unsafe { Ref::new(s) }
    }

    /// Load a Lua chunk.
    pub fn load(&self, info: ChunkInfo, chunk: impl AsRef<[u8]>) -> Result<Ref<LuaFn>, ParseError> {
        self.th.hdr.global().load(info, chunk)
    }

    /// Push value to the result of this call.
    ///
    /// # Panics
    /// If `v` come from different [Lua](crate::Lua) instance.
    pub fn push(&self, v: impl Into<UnsafeValue>) -> Result<(), StackOverflow> {
        // Check if value come from the same Lua.
        let v = v.into();

        if unsafe { (v.tt_ & 1 << 6 != 0) && (*v.value_.gc).global != self.th.hdr.global } {
            panic!("attempt to push a value created from a different Lua");
        }

        // Push.
        unsafe { lua_checkstack(self.th, 1)? };
        unsafe { self.th.top.write(v) };
        unsafe { self.th.top.add(1) };
        self.ret.set(self.ret.get() + 1);

        Ok(())
    }

    /// Push a string to the result of this call.
    ///
    /// This method is more efficient than create a string with [`Self::create_str()`] and push it
    /// via [`Self::push()`].
    pub fn push_str<V>(&self, v: V) -> Result<(), StackOverflow>
    where
        V: AsRef<str> + AsRef<[u8]> + Into<Vec<u8>>,
    {
        unsafe { lua_checkstack(self.th, 1)? };

        // Create string.
        let s = unsafe { Str::from_str(self.th.hdr.global, v) };

        unsafe { self.th.top.write(UnsafeValue::from_obj(s.cast())) };
        unsafe { self.th.top.add(1) };
        self.ret.set(self.ret.get() + 1);

        Ok(())
    }

    /// Push a byte slice as Lua string to the result of this call.
    pub fn push_bytes<V>(&self, v: V) -> Result<(), StackOverflow>
    where
        V: AsRef<[u8]> + Into<Vec<u8>>,
    {
        unsafe { lua_checkstack(self.th, 1)? };

        // Create string.
        let s = unsafe { Str::from_bytes(self.th.hdr.global, v) };

        unsafe { self.th.top.write(UnsafeValue::from_obj(s.cast())) };
        unsafe { self.th.top.add(1) };
        self.ret.set(self.ret.get() + 1);

        Ok(())
    }

    /// Push a value for `k` from `t` to the result of this call.
    pub fn push_from_str_key<K>(&self, t: &Table, k: K) -> Result<Type, StackOverflow>
    where
        K: AsRef<[u8]> + Into<Vec<u8>>,
    {
        unsafe { lua_checkstack(self.th, 1)? };

        // Get value and push it.
        let v = t.get_raw_str_key(k);

        unsafe { self.th.top.write(*v) };
        unsafe { self.th.top.add(1) };
        self.ret.set(self.ret.get() + 1);

        Ok(unsafe { Type::from_tt((*v).tt_) })
    }

    /// Call `f` with values above it as arguments.
    ///
    /// # Panics
    /// If `f` is not a valid stack index.
    pub fn try_forward(
        self,
        f: impl TryInto<NonZero<usize>>,
    ) -> Result<TryCall<'a>, Box<dyn core::error::Error>> {
        // Get function index.
        let f = match f.try_into() {
            Ok(v) => v,
            Err(_) => panic!("zero is not a valid stack index"),
        };

        // Check if index valid.
        let ci = self.th.ci.get();

        if unsafe { f.get() > (self.th.top.get().offset_from_unsigned((*ci).func) - 1) } {
            panic!("{f} is not a valid stack index");
        }

        // Invoke.
        let rem = f.get() - 1;
        let f = unsafe { (*ci).func.add(f.get()) };
        let args = unsafe { self.th.top.get().offset_from_unsigned(f) - 1 };
        let f = unsafe { pin!(lua_pcall(self.th, args, -1)) };
        let w = unsafe { Waker::new(null(), &NON_YIELDABLE_WAKER) };
        let cx = Context {
            th: self.th,
            ret: Cell::new(0),
            payload: Ret(rem),
        };

        match f.poll(&mut core::task::Context::from_waker(&w)) {
            Poll::Ready(Ok(_)) => (),
            Poll::Ready(Err(e)) => return Ok(TryCall::Err(cx, e.chunk, e.reason)),
            Poll::Pending => unreachable!(),
        }

        // Get number of results.
        let ret = unsafe { (*ci).func.add(rem + 1) };
        let ret = unsafe { cx.th.top.get().offset_from_unsigned(ret) };

        cx.ret.set(ret);

        Ok(TryCall::Ok(cx))
    }

    /// Converts all values start at `i` to call results.
    ///
    /// Use negative `i` to refer from the top of stack (e.g. `-1` mean one value from the top of
    /// stack).
    ///
    /// # Panics
    /// If `i` is not a valid stack index.
    pub fn into_results(self, i: impl TryInto<NonZero<isize>>) -> Context<'a, Ret> {
        // Get start index.
        let i = match i.try_into() {
            Ok(v) => v,
            Err(_) => panic!("zero is not a valid stack index"),
        };

        // Convert negative index.
        let ci = self.th.ci.get();
        let top = unsafe { self.th.top.get().offset_from_unsigned((*ci).func) };
        let off = match usize::try_from(i.get()) {
            Ok(v) => v,
            Err(_) => match top.saturating_sub(i.get().unsigned_abs()) {
                0 => panic!("{i} is not a valid stack index"),
                v => v,
            },
        };

        // Check if index valid.
        let ret = match top.checked_sub(off) {
            Some(v) => v,
            None => panic!("{i} is not a valid stack index"),
        };

        Context {
            th: self.th,
            ret: Cell::new(ret),
            payload: Ret(off - 1),
        }
    }
}

impl<'a> Context<'a, Args> {
    /// Returns number of arguments for this call.
    #[inline(always)]
    pub fn args(&self) -> usize {
        self.payload.0
    }

    /// Note that this method does not verify if argument `n` actually exists. The verification will
    /// be done by the returned [`Arg`].
    ///
    /// # Panics
    /// If `n` is zero.
    #[inline(always)]
    pub fn arg(&self, n: impl TryInto<NonZero<usize>>) -> Arg<'_, 'a> {
        let n = match n.try_into() {
            Ok(v) => v,
            Err(_) => panic!("zero is not a valid argument index"),
        };

        Arg::new(self, n)
    }
}

impl<'a> Context<'a, Ret> {
    /// Insert `v` at `i` by shift all above values.
    ///
    /// # Panics
    /// - If `i` is lower than the first result or not a valid stack index.
    /// - If `v` come from different [Lua](crate::Lua) instance.
    pub fn insert(
        &self,
        i: impl TryInto<NonZero<usize>>,
        v: impl Into<UnsafeValue>,
    ) -> Result<(), StackOverflow> {
        // Check if index lower than the first result.
        let i = match i.try_into() {
            Ok(v) => v,
            Err(_) => panic!("zero is not a valid stack index"),
        };

        if i.get() <= self.payload.0 {
            panic!("{i} is lower than the first result");
        }

        // Check if index valid.
        let ci = self.th.ci.get();
        let top = unsafe { self.th.top.get().offset_from_unsigned((*ci).func) };

        if i.get() > top {
            panic!("{i} is not a valid stack index");
        }

        // Check if value come from the same Lua.
        let v = v.into();

        if unsafe { (v.tt_ & 1 << 6 != 0) && (*v.value_.gc).global != self.th.hdr.global } {
            panic!("attempt to push a value created from a different Lua");
        }

        unsafe { lua_checkstack(self.th, 1)? };

        // Insert the value.
        let src = unsafe { (*ci).func.add(i.get()) };
        let dst = unsafe { (*ci).func.add(i.get() + 1) };

        unsafe { src.copy_to(dst, top - i.get()) };
        unsafe { src.write(StackValue { val: v }) };
        unsafe { self.th.top.add(1) };
        self.ret.set(self.ret.get() + 1);

        Ok(())
    }

    /// Removes the last value from call results.
    ///
    /// # Panics
    /// If results is empty.
    pub fn pop(&mut self) {
        let ret = self.ret.get().checked_sub(1).unwrap();

        unsafe { self.th.top.sub(1) };
        self.ret.set(ret);
    }

    pub(crate) fn results(&self) -> usize {
        self.ret.get()
    }
}

impl<'a> From<Context<'a, Args>> for Context<'a, Ret> {
    #[inline(always)]
    fn from(value: Context<'a, Args>) -> Self {
        Self {
            th: value.th,
            ret: value.ret,
            payload: Ret(value.payload.0),
        }
    }
}

/// Success result of [`Context::try_forward()`].
pub enum TryCall<'a> {
    Ok(Context<'a, Ret>),
    Err(
        Context<'a, Ret>,
        Option<(String, u32)>,
        Box<dyn core::error::Error>,
    ),
}

/// Call arguments encapsulated in [`Context`].
pub struct Args(usize);

impl Args {
    #[inline(always)]
    pub(crate) fn new(v: usize) -> Self {
        Self(v)
    }
}

/// Call results encapsulated in [`Context`];
pub struct Ret(usize);
