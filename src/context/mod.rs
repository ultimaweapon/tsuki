pub use self::arg::*;

use crate::lapi::lua_checkstack;
use crate::ldo::luaD_call;
use crate::lobject::luaO_arith;
use crate::value::UnsafeValue;
use crate::vm::{F2Ieq, luaV_finishget, luaV_lessthan, luaV_objlen, luaV_tointeger};
use crate::{
    CallError, ChunkInfo, LuaFn, NON_YIELDABLE_WAKER, Ops, ParseError, Ref, RegKey, RegValue,
    StackOverflow, Str, Table, Thread, Type, UserData, luaH_get, luaH_getint,
};
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::any::Any;
use core::cell::Cell;
use core::mem::MaybeUninit;
use core::num::NonZero;
use core::pin::pin;
use core::ptr::null;
use core::task::{Poll, Waker};

mod arg;

/// Context to invoke [Fp](crate::Fp) and [AsyncFp](crate::AsyncFp).
///
/// This provides [`Self::arg()`] to get arguments passed from Lua and [`Self::push()`] to returns
/// the values back to Lua. It also contains other methods like [`Self::create_table()`].
///
/// This type has two variants, which is indicated by `T` (either [`Args`] or [`Ret`]). The method
/// to get arguments will only available on [`Args`] variant. [`Ret`] variant is used to return the
/// values back to Lua. [`Args`] variant can be converted to [`Ret`] variant using standard Rust
/// [`From`] and [`Into`] or [`Self::into_results()`] (the former will returns all pushed values).
/// There is no way to get [`Args`] variant back once you converted it.
pub struct Context<'a, D, T> {
    th: &'a Thread<D>,
    ret: Cell<usize>,
    payload: T,
}

impl<'a, D, T> Context<'a, D, T> {
    #[inline(always)]
    pub(crate) fn new(th: &'a Thread<D>, payload: T) -> Self {
        Self {
            th,
            ret: Cell::new(0),
            payload,
        }
    }

    /// Returns associated data that passed to [Lua::new()](crate::Lua::new()) or
    /// [Lua::with_seed()](crate::Lua::with_seed()).
    #[inline(always)]
    pub fn associated_data(&self) -> &D {
        &self.th.hdr.global().associated_data
    }

    /// Sets a value to registry.
    ///
    /// # Panics
    /// If `v` was created from different [Lua](crate::Lua) instance.
    pub fn set_registry<'b, K>(&self, v: <K::Value<'b> as RegValue<D>>::In<'b>)
    where
        K: RegKey<D>,
        K::Value<'b>: RegValue<D>,
    {
        self.th.hdr.global().set_registry::<K>(v);
    }

    /// Returns value on registry that was set with
    /// [Lua::set_registry()](crate::Lua::set_registry()) or [Self::set_registry()].
    pub fn registry<K>(&self) -> Option<<K::Value<'a> as RegValue<D>>::Out<'a>>
    where
        K: RegKey<D>,
        K::Value<'a>: RegValue<D>,
    {
        self.th.hdr.global().registry::<K>()
    }

    /// Create a Lua string.
    #[inline(always)]
    pub fn create_str<V>(&self, v: V) -> Ref<'a, Str<D>>
    where
        V: AsRef<str> + AsRef<[u8]> + Into<Vec<u8>>,
    {
        self.th.hdr.global().gc.step();

        unsafe { Ref::new(Str::from_str(self.th.hdr.global, v)) }
    }

    /// Create a Lua table.
    #[inline(always)]
    pub fn create_table(&self) -> Ref<'a, Table<D>> {
        self.th.hdr.global().gc.step();

        unsafe { Ref::new(Table::new(self.th.hdr.global)) }
    }

    /// Create a full userdata.
    ///
    /// The metatable for the userdata that was registered with
    /// [Lua::register_metatable()](crate::Lua::register_metatable()) will be loaded during
    /// creation. A call to [Lua::register_metatable()](crate::Lua::register_metatable()) has no
    /// effect for any userdata that already created.
    #[inline(always)]
    pub fn create_ud<V: Any>(&self, v: V) -> Ref<'a, UserData<D, V>> {
        self.th.hdr.global().gc.step();

        unsafe { Ref::new(UserData::new(self.th.hdr.global, v).cast()) }
    }

    /// Create a new Lua thread (AKA coroutine).
    pub fn create_thread(&self) -> Ref<'a, Thread<D>> {
        self.th.hdr.global().gc.step();

        unsafe { Ref::new(Thread::new(self.th.hdr.global())) }
    }

    /// Load a Lua chunk.
    #[inline(always)]
    pub fn load(
        &self,
        info: impl Into<ChunkInfo>,
        chunk: impl AsRef<[u8]>,
    ) -> Result<Ref<'a, LuaFn<D>>, ParseError> {
        self.th.hdr.global().load(info, chunk)
    }

    /// Returns length of `v`.
    ///
    /// This has the same semantic as `luaL_len`, which mean it is honor `__len` metamethod.
    ///
    /// # Panics
    /// If `v` come from different [Lua](crate::Lua) instance.
    pub fn get_value_len(
        &self,
        v: impl Into<UnsafeValue<D>>,
    ) -> Result<i64, Box<dyn core::error::Error>> {
        // Check if value come from the same Lua.
        let v = v.into();

        if unsafe { (v.tt_ & 1 << 6 != 0) && (*v.value_.gc).global != self.th.hdr.global } {
            panic!("attempt to get a length of the value created from a different Lua");
        }

        // Get length.
        let l = unsafe { luaV_objlen(self.th, &v)? };

        if l.tt_ == 3 | 0 << 4 {
            return Ok(unsafe { l.value_.i });
        }

        // Try convert to integer.
        let mut v = MaybeUninit::uninit();

        if unsafe { luaV_tointeger(&l, v.as_mut_ptr(), F2Ieq) == 0 } {
            return Err("object length is not an integer".into());
        }

        Ok(unsafe { v.assume_init() })
    }

    /// Check if `lhs` less than `rhs` according to Lua operator `<`.
    ///
    /// # Panics
    /// If either `lhs` or `rhs` come from different [Lua](crate::Lua) instance.
    #[inline(always)]
    pub fn is_value_lt(
        &self,
        lhs: impl Into<UnsafeValue<D>>,
        rhs: impl Into<UnsafeValue<D>>,
    ) -> Result<bool, Box<dyn core::error::Error>> {
        // Check if the first operand created from the same Lua.
        let lhs = lhs.into();

        if unsafe { (lhs.tt_ & 1 << 6 != 0) && (*lhs.value_.gc).global != self.th.hdr.global } {
            panic!("attempt to compare a value created from a different Lua");
        }

        // Check if the second operand created from the same Lua.
        let rhs = rhs.into();

        if unsafe { (rhs.tt_ & 1 << 6 != 0) && (*rhs.value_.gc).global != self.th.hdr.global } {
            panic!("attempt to compare a value created from a different Lua");
        }

        Ok(unsafe { luaV_lessthan(self.th, &lhs, &rhs)? != 0 })
    }

    /// Push value to the result of this call.
    ///
    /// # Panics
    /// If `v` come from different [Lua](crate::Lua) instance.
    pub fn push(&self, v: impl Into<UnsafeValue<D>>) -> Result<(), StackOverflow> {
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

    /// Push value to the result of this call without checking if `v` created from different
    /// [Lua](crate::Lua) instance.
    ///
    /// # Safety
    /// `v` must created from the same [Lua](crate::Lua) instance.
    pub unsafe fn push_unchecked(&self, v: impl Into<UnsafeValue<D>>) -> Result<(), StackOverflow> {
        let v = v.into();

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

    /// Push a next key-value pair from `t` to the result of this call.
    ///
    /// Returns `false` if no key-value next to `k`. In this case no any value is pushed.
    ///
    /// # Panics
    /// If `t` or `k` created from different [Lua](crate::Lua) instance.
    pub fn push_next(
        &self,
        t: &Table<D>,
        k: impl Into<UnsafeValue<D>>,
    ) -> Result<bool, Box<dyn core::error::Error>> {
        unsafe { lua_checkstack(self.th, 2)? };

        // Check if table come from the same Lua.
        if t.hdr.global != self.th.hdr.global {
            panic!("attempt to push a value from a table created from different Lua");
        }

        // Check if key come from the same Lua.
        let k = k.into();

        if unsafe { (k.tt_ & 1 << 6 != 0) && (*k.value_.gc).global != self.th.hdr.global } {
            panic!("attempt to push a value created from different Lua");
        }

        // Get pair.
        let [k, v] = match unsafe { t.next_raw(&k)? } {
            Some(v) => v,
            None => return Ok(false),
        };

        unsafe { self.th.top.write(k) };
        unsafe { self.th.top.add(1) };
        unsafe { self.th.top.write(v) };
        unsafe { self.th.top.add(1) };
        self.ret.set(self.ret.get() + 2);

        Ok(true)
    }

    /// Push a value for `k` from `t` to the result of this call.
    ///
    /// # Panics
    /// If `t` or `k` created from different [Lua](crate::Lua) instance.
    pub fn push_from_table(
        &self,
        t: &Table<D>,
        k: impl Into<UnsafeValue<D>>,
    ) -> Result<Type, StackOverflow> {
        unsafe { lua_checkstack(self.th, 1)? };

        // Check if table come from the same Lua.
        if t.hdr.global != self.th.hdr.global {
            panic!("attempt to push a value from a table created from different Lua");
        }

        // Get value and push it.
        let v = t.get_raw(k);

        unsafe { self.th.top.write(*v) };
        unsafe { self.th.top.add(1) };
        self.ret.set(self.ret.get() + 1);

        Ok(unsafe { Type::from_tt((*v).tt_) })
    }

    /// Push a value for `k` from `t` to the result of this call.
    ///
    /// # Panics
    /// If `t` created from different [Lua](crate::Lua) instance.
    pub fn push_from_str_key<K>(&self, t: &Table<D>, k: K) -> Result<Type, StackOverflow>
    where
        K: AsRef<[u8]> + Into<Vec<u8>>,
    {
        unsafe { lua_checkstack(self.th, 1)? };

        // Check if table come from the same Lua.
        if t.hdr.global != self.th.hdr.global {
            panic!("attempt to push a value from a table created from different Lua");
        }

        // Get value and push it.
        let v = t.get_raw_str_key(k);

        unsafe { self.th.top.write(*v) };
        unsafe { self.th.top.add(1) };
        self.ret.set(self.ret.get() + 1);

        Ok(unsafe { Type::from_tt((*v).tt_) })
    }

    /// Push a value for `k` from `t` to the result of this call.
    ///
    /// This method honor `__index` metavalue.
    ///
    /// # Panics
    /// If `t` or `k` come from different [Lua](crate::Lua) instance.
    pub fn push_from_index(
        &self,
        t: impl Into<UnsafeValue<D>>,
        k: impl Into<UnsafeValue<D>>,
    ) -> Result<Type, Box<dyn core::error::Error>> {
        unsafe { lua_checkstack(self.th, 1)? };

        // Check if table come from the same Lua.
        let t = t.into();

        if unsafe { (t.tt_ & 1 << 6 != 0) && (*t.value_.gc).global != self.th.hdr.global } {
            panic!("attempt to push a value created from different Lua");
        }

        // Check if key come from the same Lua.
        let mut k = k.into();

        if unsafe { (k.tt_ & 1 << 6 != 0) && (*k.value_.gc).global != self.th.hdr.global } {
            panic!("attempt to push a value created from different Lua");
        }

        // Try table.
        let mut slot = null();
        let ok = if !(t.tt_ == 5 | 0 << 4 | 1 << 6) {
            false
        } else {
            let t = unsafe { t.value_.gc.cast::<Table<D>>() };

            slot = unsafe { luaH_get(t, &k) };

            unsafe { !((*slot).tt_ & 0xf == 0) }
        };

        // Get value.
        let v = if ok {
            unsafe { slot.read() }
        } else {
            // Try __index.
            unsafe { luaV_finishget(self.th, &t, &mut k, slot)? }
        };

        unsafe { self.th.top.write(v) };
        unsafe { self.th.top.add(1) };
        self.ret.set(self.ret.get() + 1);

        Ok(Type::from_tt(v.tt_))
    }

    /// Push a value for `k` from `t` to the result of this call.
    ///
    /// This method honor `__index` metavalue.
    ///
    /// # Panics
    /// If `t` come from different [Lua](crate::Lua) instance.
    pub fn push_from_index_with_int(
        &self,
        t: impl Into<UnsafeValue<D>>,
        k: i64,
    ) -> Result<Type, Box<dyn core::error::Error>> {
        unsafe { lua_checkstack(self.th, 1)? };

        // Check if table come from the same Lua.
        let t = t.into();

        if unsafe { (t.tt_ & 1 << 6 != 0) && (*t.value_.gc).global != self.th.hdr.global } {
            panic!("attempt to push a value created from different Lua");
        }

        // Try table.
        let mut slot = null();
        let ok = if !(t.tt_ == 5 | 0 << 4 | 1 << 6) {
            false
        } else {
            let t = unsafe { t.value_.gc.cast::<Table<D>>() };

            slot = unsafe { luaH_getint(t, k) };

            unsafe { !((*slot).tt_ & 0xf == 0) }
        };

        // Get value.
        let v = if ok {
            unsafe { slot.read() }
        } else {
            // Try __index.
            let mut k = UnsafeValue::from(k);

            unsafe { luaV_finishget(self.th, &t, &mut k, slot)? }
        };

        unsafe { self.th.top.write(v) };
        unsafe { self.th.top.add(1) };
        self.ret.set(self.ret.get() + 1);

        Ok(Type::from_tt(v.tt_))
    }

    /// Push the result of addition between `lhs` and `rhs`, returns the type of pushed value.
    ///
    /// This method honor `__add` metavalue.
    ///
    /// # Panics
    /// If either `lhs` or `rhs` come from different [Lua](crate::Lua) instance.
    pub fn push_add(
        &self,
        lhs: impl Into<UnsafeValue<D>>,
        rhs: impl Into<UnsafeValue<D>>,
    ) -> Result<Type, Box<dyn core::error::Error>> {
        // Check operands.
        let lhs = lhs.into();
        let rhs = rhs.into();

        if unsafe { (lhs.tt_ & 1 << 6 != 0) && (*lhs.value_.gc).global != self.th.hdr.global } {
            panic!("attempt to perform addition on a value created from different Lua");
        }

        if unsafe { (rhs.tt_ & 1 << 6 != 0) && (*rhs.value_.gc).global != self.th.hdr.global } {
            panic!("attempt to perform addition on a value created from different Lua");
        }

        // Perform addition.
        let r = unsafe { luaO_arith(self.th, Ops::Add, &lhs, &rhs)? };

        unsafe { lua_checkstack(self.th, 1)? };
        unsafe { self.th.top.write(r) };
        unsafe { self.th.top.add(1) };

        self.ret.update(|v| v + 1);

        Ok(Type::from_tt(r.tt_))
    }

    /// Push the result of subtraction `lhs` with `rhs`, returns the type of pushed value.
    ///
    /// This method honor `__sub` metavalue.
    ///
    /// # Panics
    /// If either `lhs` or `rhs` come from different [Lua](crate::Lua) instance.
    pub fn push_sub(
        &self,
        lhs: impl Into<UnsafeValue<D>>,
        rhs: impl Into<UnsafeValue<D>>,
    ) -> Result<Type, Box<dyn core::error::Error>> {
        // Check operands.
        let lhs = lhs.into();
        let rhs = rhs.into();

        if unsafe { (lhs.tt_ & 1 << 6 != 0) && (*lhs.value_.gc).global != self.th.hdr.global } {
            panic!("attempt to perform subtraction on a value created from different Lua");
        }

        if unsafe { (rhs.tt_ & 1 << 6 != 0) && (*rhs.value_.gc).global != self.th.hdr.global } {
            panic!("attempt to perform subtraction on a value created from different Lua");
        }

        // Perform subtraction.
        let r = unsafe { luaO_arith(self.th, Ops::Sub, &lhs, &rhs)? };

        unsafe { lua_checkstack(self.th, 1)? };
        unsafe { self.th.top.write(r) };
        unsafe { self.th.top.add(1) };

        self.ret.update(|v| v + 1);

        Ok(Type::from_tt(r.tt_))
    }

    /// Push the result of `lhs` modulo `rhs`, returns the type of pushed value.
    ///
    /// This method honor `__mod` metavalue.
    ///
    /// # Panics
    /// If either `lhs` or `rhs` was created from different [Lua](crate::Lua) instance.
    pub fn push_rem(
        &self,
        lhs: impl Into<UnsafeValue<D>>,
        rhs: impl Into<UnsafeValue<D>>,
    ) -> Result<Type, Box<dyn core::error::Error>> {
        // Check operands.
        let lhs = lhs.into();
        let rhs = rhs.into();

        if unsafe { (lhs.tt_ & 1 << 6 != 0) && (*lhs.value_.gc).global != self.th.hdr.global } {
            panic!("attempt to perform modulo on a value created from different Lua");
        }

        if unsafe { (rhs.tt_ & 1 << 6 != 0) && (*rhs.value_.gc).global != self.th.hdr.global } {
            panic!("attempt to perform modulo on a value created from different Lua");
        }

        // Perform subtraction.
        let r = unsafe { luaO_arith(self.th, Ops::Mod, &lhs, &rhs)? };

        unsafe { lua_checkstack(self.th, 1)? };
        unsafe { self.th.top.write(r) };
        unsafe { self.th.top.add(1) };

        self.ret.update(|v| v + 1);

        Ok(Type::from_tt(r.tt_))
    }

    /// Push the result of `lhs` raised to the power of `rhs`, returns the type of pushed value.
    ///
    /// This method honor `__pow` metavalue.
    ///
    /// # Panics
    /// If either `lhs` or `rhs` was created from different [Lua](crate::Lua) instance.
    pub fn push_pow(
        &self,
        lhs: impl Into<UnsafeValue<D>>,
        rhs: impl Into<UnsafeValue<D>>,
    ) -> Result<Type, Box<dyn core::error::Error>> {
        // Check operands.
        let lhs = lhs.into();
        let rhs = rhs.into();

        if unsafe { (lhs.tt_ & 1 << 6 != 0) && (*lhs.value_.gc).global != self.th.hdr.global } {
            panic!("attempt to perform exponentiation on a value created from different Lua");
        }

        if unsafe { (rhs.tt_ & 1 << 6 != 0) && (*rhs.value_.gc).global != self.th.hdr.global } {
            panic!("attempt to perform exponentiation on a value created from different Lua");
        }

        // Perform subtraction.
        let r = unsafe { luaO_arith(self.th, Ops::Pow, &lhs, &rhs)? };

        unsafe { lua_checkstack(self.th, 1)? };
        unsafe { self.th.top.write(r) };
        unsafe { self.th.top.add(1) };

        self.ret.update(|v| v + 1);

        Ok(Type::from_tt(r.tt_))
    }

    /// Push the result of negation on `v`, returns the type of pushed value.
    ///
    /// This method honor `__unm` metavalue.
    ///
    /// # Panics
    /// If `v` was created from different [Lua](crate::Lua) instance.
    pub fn push_neg(
        &self,
        v: impl Into<UnsafeValue<D>>,
    ) -> Result<Type, Box<dyn core::error::Error>> {
        // Check operands.
        let v = v.into();

        if unsafe { (v.tt_ & 1 << 6 != 0) && (*v.value_.gc).global != self.th.hdr.global } {
            panic!("attempt to perform negation on a value created from different Lua");
        }

        // Perform subtraction.
        let r = unsafe { luaO_arith(self.th, Ops::Neg, &v, &v)? };

        unsafe { lua_checkstack(self.th, 1)? };
        unsafe { self.th.top.write(r) };
        unsafe { self.th.top.add(1) };

        self.ret.update(|v| v + 1);

        Ok(Type::from_tt(r.tt_))
    }

    /// Reserves capacity for at least `additional` more elements to be pushed.
    ///
    /// Usually you don't need this method unless you try to push a large amount of results.
    ///
    /// This has the same semantic as `lua_checkstack`.
    pub fn reserve(&self, additional: usize) -> Result<(), StackOverflow> {
        unsafe { lua_checkstack(self.th, additional) }
    }

    /// Call `f` with values above it as arguments.
    ///
    /// Use negative `f` to refer from the top of stack (e.g. `-1` mean one value from the top of
    /// stack).
    ///
    /// # Panics
    /// If `f` is not a valid stack index.
    pub fn forward(
        self,
        f: impl TryInto<NonZero<isize>>,
    ) -> Result<Context<'a, D, Ret>, Box<dyn core::error::Error>> {
        // Get function index.
        let f = match f.try_into() {
            Ok(v) => v,
            Err(_) => panic!("zero is not a valid stack index"),
        };

        // Convert negative index.
        let ci = self.th.ci.get();
        let top = unsafe { self.th.top.get().offset_from_unsigned((*ci).func) };
        let f = match usize::try_from(f.get()) {
            Ok(v) => {
                if v >= top {
                    panic!("{f} is not a valid stack index");
                }

                v
            }
            Err(_) => match top.saturating_sub(f.get().unsigned_abs()) {
                0 => panic!("{f} is not a valid stack index"),
                v => v,
            },
        };

        // Invoke.
        let rem = f - 1;
        let f = unsafe { (*ci).func.add(f) };
        let cx = Context {
            th: self.th,
            ret: Cell::new(0),
            payload: Ret(rem),
        };

        {
            let f = unsafe { pin!(luaD_call(self.th, f, -1)) };
            let w = unsafe { Waker::new(null(), &NON_YIELDABLE_WAKER) };

            match f.poll(&mut core::task::Context::from_waker(&w)) {
                Poll::Ready(Ok(_)) => (),
                Poll::Ready(Err(e)) => return Err(e),
                Poll::Pending => unreachable!(),
            }
        }

        // Get number of results.
        let ret = unsafe { (*ci).func.add(rem + 1) };
        let ret = unsafe { cx.th.top.get().offset_from_unsigned(ret) };

        cx.ret.set(ret);

        Ok(cx)
    }

    /// Call `f` with values above it as arguments.
    ///
    /// # Panics
    /// If `f` is not a valid stack index.
    pub fn try_forward(
        self,
        f: impl TryInto<NonZero<usize>>,
    ) -> Result<TryCall<'a, D>, Box<dyn core::error::Error>> {
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
        let cx = Context {
            th: self.th,
            ret: Cell::new(0),
            payload: Ret(rem),
        };

        {
            let f = unsafe { pin!(luaD_call(self.th, f, -1)) };
            let w = unsafe { Waker::new(null(), &NON_YIELDABLE_WAKER) };

            match f.poll(&mut core::task::Context::from_waker(&w)) {
                Poll::Ready(Ok(_)) => (),
                Poll::Ready(Err(e)) => return Ok(TryCall::Err(cx, e)),
                Poll::Pending => unreachable!(),
            }
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
    pub fn into_results(self, i: impl TryInto<NonZero<isize>>) -> Context<'a, D, Ret> {
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

impl<'a, D> Context<'a, D, Args> {
    /// Returns number of arguments for this call.
    #[inline(always)]
    pub fn args(&self) -> usize {
        self.payload.0
    }

    /// Note that this method does not verify if argument `n` actually exists. The verification will
    /// be done by the returned [Arg].
    ///
    /// # Panics
    /// If `n` is zero.
    #[inline(always)]
    pub fn arg(&self, n: impl TryInto<NonZero<usize>>) -> Arg<'_, 'a, D> {
        let n = match n.try_into() {
            Ok(v) => v,
            Err(_) => panic!("zero is not a valid argument index"),
        };

        Arg::new(self, n)
    }
}

impl<'a, D> Context<'a, D, Ret> {
    /// Insert `v` at `i` by shift all above values.
    ///
    /// # Panics
    /// - If `i` is lower than the first result or not a valid stack index.
    /// - If `v` come from different [Lua](crate::Lua) instance.
    pub fn insert(
        &self,
        i: impl TryInto<NonZero<usize>>,
        v: impl Into<UnsafeValue<D>>,
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

        for i in (0..(top - i.get())).rev() {
            let src = unsafe { src.add(i) };
            let dst = unsafe { dst.add(i) };

            unsafe { (*dst).tt_ = (*src).tt_ };
            unsafe { (*dst).value_ = (*src).value_ };
        }

        unsafe { (*src).tt_ = v.tt_ };
        unsafe { (*src).value_ = v.value_ };

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

    /// Shortens the call results, keeping the first `len` values and remove the rest.
    ///
    /// This has no effect if `len` is equal or greater than call results.
    pub fn truncate(&mut self, len: usize) {
        let remove = match self.ret.get().checked_sub(len) {
            Some(v) => v,
            None => return,
        };

        unsafe { self.th.top.sub(remove) };
        self.ret.set(len);
    }

    pub(crate) fn results(&self) -> usize {
        self.ret.get()
    }
}

impl<'a, D> From<Context<'a, D, Args>> for Context<'a, D, Ret> {
    #[inline(always)]
    fn from(value: Context<'a, D, Args>) -> Self {
        Self {
            th: value.th,
            ret: value.ret,
            payload: Ret(value.payload.0),
        }
    }
}

/// Success result of [`Context::try_forward()`].
pub enum TryCall<'a, D> {
    Ok(Context<'a, D, Ret>),
    Err(Context<'a, D, Ret>, Box<CallError>),
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
