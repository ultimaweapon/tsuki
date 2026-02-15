pub(crate) use self::legacy::*;
pub(crate) use self::node::*;
pub(crate) use self::rust::*;

use self::iter::Iter;
use crate::lmem::luaM_free_;
use crate::ltm::{TM_EQ, TM_GC, luaT_gettm};
use crate::{Lua, Object, Ref, Str, UnsafeValue, Value};
use alloc::vec::Vec;
use core::alloc::Layout;
use core::cell::Cell;
use core::convert::identity;
use core::marker::PhantomPinned;
use core::mem::zeroed;
use core::ptr::{addr_of_mut, null, null_mut};
use thiserror::Error;

mod iter;
mod legacy;
mod node;
mod rust;

/// Lua table.
///
/// Use [Lua::create_table()] or [Context::create_table()](crate::Context::create_table()) to create
/// the value of this type.
#[repr(C)]
pub struct Table<A> {
    pub(crate) hdr: Object<A>,
    pub(crate) flags: Cell<u8>,
    pub(crate) lsizenode: Cell<u8>,
    pub(crate) alimit: Cell<u32>,
    pub(crate) array: Cell<*mut UnsafeValue<A>>,
    pub(crate) node: Cell<*mut Node<A>>,
    pub(crate) lastfree: Cell<*mut Node<A>>,
    pub(crate) metatable: Cell<*const Self>,
    absent_key: UnsafeValue<A>,
    pin: PhantomPinned,
}

impl<A> Table<A> {
    #[inline(never)]
    pub(crate) unsafe fn new(g: *const Lua<A>) -> *const Self {
        let layout = Layout::new::<Self>();
        let o = unsafe { (*g).gc.alloc(5 | 0 << 4, layout).cast::<Self>() };
        let absent_key = UnsafeValue {
            value_: unsafe { zeroed() },
            tt_: 0 | 2 << 4,
        };

        unsafe { addr_of_mut!((*o).flags).write(Cell::new(!(!(0 as u32) << TM_EQ + 1) as u8)) };
        unsafe { addr_of_mut!((*o).lsizenode).write(Cell::new(0)) };
        unsafe { addr_of_mut!((*o).alimit).write(Cell::new(0)) };
        unsafe { addr_of_mut!((*o).array).write(Cell::new(null_mut())) };
        unsafe { addr_of_mut!((*o).node).write(Cell::new(&raw const (*g).dummy_node as _)) };
        unsafe { addr_of_mut!((*o).lastfree).write(Cell::new(null_mut())) };
        unsafe { addr_of_mut!((*o).metatable).write(Cell::new(null_mut())) };
        unsafe { addr_of_mut!((*o).absent_key).write(absent_key) };

        o
    }

    /// Returns metatable for this table.
    pub fn metatable(&self) -> Option<Ref<'_, Self>> {
        let v = self.metatable.get();

        match v.is_null() {
            true => None,
            false => Some(unsafe { Ref::new(v) }),
        }
    }

    /// Set metatable for this table.
    ///
    /// Use [Self::remove_metatable()] if you want to remove the metatable.
    ///
    /// # Panics
    /// If `v` was created from different [Lua](crate::Lua) instance.
    pub fn set_metatable(&self, v: &Self) -> Result<(), MetatableError> {
        // Check if metatable come from the same Lua.
        if v.hdr.global != self.hdr.global {
            panic!("attempt to set metatable created from a different Lua");
        }

        // Prevent __gc metamethod.
        if v.flags.get() & 1 << TM_GC == 0 {
            if unsafe { !luaT_gettm(v, TM_GC).is_null() } {
                return Err(MetatableError::HasGc);
            }
        }

        // Set metatable.
        self.metatable.set(v);

        if self.hdr.marked.get() & 1 << 5 != 0 && v.hdr.marked.get() & (1 << 3 | 1 << 4) != 0 {
            unsafe { self.hdr.global().gc.barrier(&self.hdr, &v.hdr) };
        }

        Ok(())
    }

    /// Removes metatable from this table.
    #[inline(always)]
    pub fn remove_metatable(&self) {
        self.metatable.set(null());
    }

    /// Returns the length of this table.
    ///
    /// This has the same behavior as `#` operator on a table, which may have unexpected behavior if
    /// you don't know how it works. See Lua
    /// [docs](https://www.lua.org/manual/5.4/manual.html#3.4.7) for more details.
    ///
    /// This is equivalent to `lua_rawlen` with a table.
    pub fn len(&self) -> i64 {
        unsafe { luaH_getn(self) as i64 }
    }

    /// Returns `true` if the table contains a value for the specified key.
    ///
    /// # Panics
    /// If `k` was created from different [Lua](crate::Lua) instance.
    pub fn contains_key(&self, k: impl Into<UnsafeValue<A>>) -> bool {
        // Check if key come from the same Lua.
        let k = k.into();

        if unsafe { (k.tt_ & 1 << 6 != 0) && (*k.value_.gc).global != self.hdr.global } {
            panic!("attempt to check the table with key from a different Lua");
        }

        // Get value.
        let v = unsafe { luaH_get(self, &k) };

        unsafe { (*v).tt_ & 0xf != 0 }
    }

    /// Returns `true` if the table contains a value for `k`.
    ///
    /// This method will trigger GC if new string is allocated.
    pub fn contains_str_key<K>(&self, k: K) -> bool
    where
        K: AsRef<str> + AsRef<[u8]> + Into<Vec<u8>>,
    {
        let k = unsafe { Str::from_str(self.hdr.global, k) };
        let v = unsafe { UnsafeValue::from_obj(k.unwrap_or_else(identity).cast()) };
        let v = unsafe { luaH_get(self, &v) };
        let v = unsafe { (*v).tt_ & 0xf != 0 };

        if k.is_ok() {
            self.hdr.global().gc.step();
        }

        v
    }

    /// Returns `true` if the table contains a value for `k`.
    ///
    /// This method will trigger GC if new string is allocated.
    pub fn contains_bytes_key<K>(&self, k: K) -> bool
    where
        K: AsRef<[u8]> + Into<Vec<u8>>,
    {
        let k = unsafe { Str::from_bytes(self.hdr.global, k) };
        let v = unsafe { UnsafeValue::from_obj(k.unwrap_or_else(identity).cast()) };
        let v = unsafe { luaH_get(self, &v) };
        let v = unsafe { (*v).tt_ & 0xf != 0 };

        if k.is_ok() {
            self.hdr.global().gc.step();
        }

        v
    }

    /// Returns a value corresponding to the key.
    ///
    /// # Panics
    /// If `k` come from different [Lua](crate::Lua) instance.
    pub fn get(&self, k: impl Into<UnsafeValue<A>>) -> Value<'_, A> {
        let k = k.into();

        if unsafe { (k.tt_ & 1 << 6 != 0) && (*k.value_.gc).global != self.hdr.global } {
            panic!("attempt to get the table with key from a different Lua");
        }

        unsafe { Value::from_unsafe(luaH_get(self, &k)) }
    }

    /// Returns a value corresponding to `k`.
    ///
    /// This method will trigger GC if new string is allocated.
    #[inline]
    pub fn get_str_key<K>(&self, k: K) -> Value<'_, A>
    where
        K: AsRef<str> + AsRef<[u8]> + Into<Vec<u8>>,
    {
        let k = unsafe { Str::from_str(self.hdr.global, k) };
        let v = unsafe { UnsafeValue::from_obj(k.unwrap_or_else(identity).cast()) };
        let v = unsafe { luaH_get(self, &v) };

        if k.is_ok() {
            self.hdr.global().gc.step();
        }

        unsafe { Value::from_unsafe(v) }
    }

    /// Returns a value corresponding to `k`.
    ///
    /// This method will trigger GC if new string is allocated.
    pub fn get_bytes_key<K>(&self, k: K) -> Value<'_, A>
    where
        K: AsRef<[u8]> + Into<Vec<u8>>,
    {
        let k = unsafe { Str::from_bytes(self.hdr.global, k) };
        let v = unsafe { UnsafeValue::from_obj(k.unwrap_or_else(identity).cast()) };
        let v = unsafe { luaH_get(self, &v) };

        if k.is_ok() {
            self.hdr.global().gc.step();
        }

        unsafe { Value::from_unsafe(v) }
    }

    /// # Panics
    /// If `k` come from different [Lua](crate::Lua) instance.
    pub(crate) fn get_raw(&self, k: impl Into<UnsafeValue<A>>) -> *const UnsafeValue<A> {
        let k = k.into();

        if unsafe { (k.tt_ & 1 << 6 != 0) && (*k.value_.gc).global != self.hdr.global } {
            panic!("attempt to get the table with key from a different Lua");
        }

        unsafe { luaH_get(self, &k) }
    }

    #[inline(always)]
    pub(crate) unsafe fn get_raw_unchecked(
        &self,
        k: impl Into<UnsafeValue<A>>,
    ) -> *const UnsafeValue<A> {
        unsafe { luaH_get(self, &k.into()) }
    }

    #[inline(always)]
    pub(crate) fn get_raw_int_key(&self, k: i64) -> &UnsafeValue<A> {
        unsafe { &*luaH_getint(self, k) }
    }

    #[inline(always)]
    pub(crate) fn get_raw_str_key<K>(&self, k: K) -> *const UnsafeValue<A>
    where
        K: AsRef<[u8]> + Into<Vec<u8>>,
    {
        let k = unsafe { Str::from_bytes(self.hdr.global, k).unwrap_or_else(identity) };
        let k = unsafe { UnsafeValue::from_obj(k.cast()) };

        unsafe { luaH_get(self, &k) }
    }

    /// Inserts a key-value pair into this table.
    ///
    /// # Panics
    /// If `k` or `v` was created from different [Lua](crate::Lua) instance.
    pub fn set(
        &self,
        k: impl Into<UnsafeValue<A>>,
        v: impl Into<UnsafeValue<A>>,
    ) -> Result<(), TableError> {
        // Check if key come from the same Lua.
        let k = k.into();

        if unsafe { (k.tt_ & 1 << 6 != 0) && (*k.value_.gc).global != self.hdr.global } {
            panic!("attempt to set the table with key from a different Lua");
        }

        // Check if value come from the same Lua.
        let v = v.into();

        if unsafe { (v.tt_ & 1 << 6 != 0) && (*v.value_.gc).global != self.hdr.global } {
            panic!("attempt to set the table with value from a different Lua");
        }

        unsafe { self.set_unchecked(k, v) }
    }

    /// # Safety
    /// `k` and `v` must come from the same [Lua](crate::Lua) instance.
    pub unsafe fn set_unchecked(
        &self,
        k: impl Into<UnsafeValue<A>>,
        v: impl Into<UnsafeValue<A>>,
    ) -> Result<(), TableError> {
        let k = k.into();
        let v = v.into();

        unsafe { luaH_set(self, &k, &v)? };

        self.flags
            .set((self.flags.get() as u32 & !!(!(0 as u32) << TM_EQ + 1)) as u8);

        if (v.tt_ & 1 << 6 != 0) && (self.hdr.marked.get() & 1 << 5 != 0) {
            if unsafe { (*v.value_.gc).marked.get() & (1 << 3 | 1 << 4) != 0 } {
                unsafe { self.hdr.global().gc.barrier_back(&self.hdr) };
            }
        }

        Ok(())
    }

    /// Inserts a value with string key into this table.
    ///
    /// This method will trigger GC if new string is allocated.
    ///
    /// # Panics
    /// If `v` was created from different [Lua](crate::Lua) instance.
    #[inline(never)]
    pub fn set_str_key<K>(&self, k: K, v: impl Into<UnsafeValue<A>>)
    where
        K: AsRef<str> + AsRef<[u8]> + Into<Vec<u8>>,
    {
        // Check if value come from the same Lua.
        let v = v.into();

        if unsafe { (v.tt_ & 1 << 6 != 0) && (*v.value_.gc).global != self.hdr.global } {
            panic!("attempt to set the table with value from a different Lua");
        }

        // Set.
        let s = unsafe { Str::from_str(self.hdr.global, k) };
        let k = unsafe { UnsafeValue::from_obj(s.unwrap_or_else(identity).cast()) };

        // SAFETY: Key was created from the same Lua on the above.
        // SAFETY: We have checked the value on the above.
        // SAFETY: Key is a string so error is not possible.
        unsafe { self.set_unchecked(k, v).unwrap_unchecked() };

        if s.is_ok() {
            self.hdr.global().gc.step();
        }
    }

    /// Inserts a value with string key into this table without checking if `v` created from the
    /// same [Lua] instance.
    ///
    /// This method will trigger GC if new string is allocated.
    ///
    /// # Safety
    /// `v` must created from the same [Lua] instance.
    pub unsafe fn set_str_key_unchecked<K>(&self, k: K, v: impl Into<UnsafeValue<A>>)
    where
        K: AsRef<str> + AsRef<[u8]> + Into<Vec<u8>>,
    {
        let s = unsafe { Str::from_str(self.hdr.global, k) };
        let k = unsafe { UnsafeValue::from_obj(s.unwrap_or_else(identity).cast()) };

        unsafe { self.set_unchecked(k, v).unwrap_unchecked() };

        if s.is_ok() {
            self.hdr.global().gc.step();
        }
    }

    /// Inserts a value with string key into this table.
    ///
    /// This method will trigger GC if new string is allocated.
    ///
    /// # Panics
    /// If `v` was created from different [Lua](crate::Lua) instance.
    pub fn set_bytes_key<K>(&self, k: K, v: impl Into<UnsafeValue<A>>)
    where
        K: AsRef<[u8]> + Into<Vec<u8>>,
    {
        // Check if value come from the same Lua.
        let v = v.into();

        if unsafe { (v.tt_ & 1 << 6 != 0) && (*v.value_.gc).global != self.hdr.global } {
            panic!("attempt to set the table with value from a different Lua");
        }

        // Set.
        let s = unsafe { Str::from_bytes(self.hdr.global, k) };
        let k = unsafe { UnsafeValue::from_obj(s.unwrap_or_else(identity).cast()) };

        // SAFETY: Key was created from the same Lua on the above.
        // SAFETY: We have checked the value on the above.
        // SAFETY: Key is a string so error is not possible.
        unsafe { self.set_unchecked(k, v).unwrap_unchecked() };

        if s.is_ok() {
            self.hdr.global().gc.step();
        }
    }

    /// Inserts a value with string key into this table without checking if `v` created from the
    /// same [Lua] instance.
    ///
    /// This method will trigger GC if new string is allocated.
    ///
    /// # Safety
    /// `v` must created from the same [Lua] instance.
    pub unsafe fn set_bytes_key_unchecked<K>(&self, k: K, v: impl Into<UnsafeValue<A>>)
    where
        K: AsRef<[u8]> + Into<Vec<u8>>,
    {
        let s = unsafe { Str::from_bytes(self.hdr.global, k) };
        let k = unsafe { UnsafeValue::from_obj(s.unwrap_or_else(identity).cast()) };

        unsafe { self.set_unchecked(k, v).unwrap_unchecked() };

        if s.is_ok() {
            self.hdr.global().gc.step();
        }
    }

    pub(crate) unsafe fn set_slot_unchecked(
        &self,
        s: *const UnsafeValue<A>,
        k: impl Into<UnsafeValue<A>>,
        v: impl Into<UnsafeValue<A>>,
    ) -> Result<(), TableError> {
        let k = k.into();
        let v = v.into();

        unsafe { luaH_finishset(self, &k, s, &v)? };

        self.flags
            .set((self.flags.get() as u32 & !!(!0 << TM_EQ + 1)) as u8);

        if v.tt_ & 1 << 6 != 0 && self.hdr.marked.get() & 1 << 5 != 0 {
            if unsafe { (*v.value_.gc).marked.is_white() } {
                unsafe { self.hdr.global().gc.barrier_back(&self.hdr) };
            }
        }

        Ok(())
    }

    #[inline(never)]
    pub(crate) unsafe fn next_raw(
        &self,
        key: &UnsafeValue<A>,
    ) -> Result<Option<[UnsafeValue<A>; 2]>, KeyMissing> {
        // Get from array table.
        let asize = unsafe { luaH_realasize(self) };
        let mut i = unsafe { findindex(self, key, asize)? };
        let array = self.array.get();

        while i < asize {
            let val = unsafe { array.add(i.try_into().unwrap()) };

            if unsafe { !((*val).tt_ & 0xf == 0) } {
                let key = i64::from(i + 1).into();

                return Ok(unsafe { Some([key, val.read()]) });
            }

            i += 1
        }

        // Get from hash table.
        i = i - asize;

        while i < (1 << self.lsizenode.get()) {
            let n = unsafe { self.node.get().add(i.try_into().unwrap()) };

            if unsafe { !((*n).tt_ & 0xf == 0) } {
                // Copy key.
                let k = UnsafeValue {
                    tt_: unsafe { (*n).key_tt },
                    value_: unsafe { (*n).key_val },
                };

                // Copy value.
                let v = UnsafeValue {
                    tt_: unsafe { (*n).tt_ },
                    value_: unsafe { (*n).value_ },
                };

                return Ok(Some([k, v]));
            }

            i = i + 1;
        }

        Ok(None)
    }
}

impl<A> Drop for Table<A> {
    fn drop(&mut self) {
        unsafe { freehash(self) };
        unsafe {
            luaM_free_(
                self.array.get().cast(),
                (luaH_realasize(self) as usize).wrapping_mul(size_of::<UnsafeValue<A>>()),
            )
        };
    }
}

impl<'a, A> IntoIterator for &'a Table<A> {
    type Item = Result<(Value<'a, A>, Value<'a, A>), KeyMissing>;
    type IntoIter = Iter<'a, A>;

    #[inline(always)]
    fn into_iter(self) -> Self::IntoIter {
        Iter::new(self)
    }
}

/// Represents an error when the operation on a table fails.
#[derive(Debug, Error)]
pub enum TableError {
    /// Key is `nil`.
    #[error("key is nil")]
    NilKey,

    /// Key is NaN.
    #[error("key is NaN")]
    NanKey,
}

/// Error when attempt to set an invalid metatable.
#[derive(Debug, Error)]
pub enum MetatableError {
    /// The metatable as `__gc`.
    #[error("the metatable contains __gc metamethod, which Tsuki does not support")]
    HasGc,
}

/// Error when attempt to enumerating a table with a key that was removed.
///
/// This error can happens only if you remove current key from the table during enumeration.
#[derive(Debug, Error)]
#[error("invalid key to 'next'")]
pub struct KeyMissing;
