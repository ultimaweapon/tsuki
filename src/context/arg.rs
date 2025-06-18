use super::{Args, Context};
use crate::lapi::lua_typename;
use crate::lauxlib::{luaL_argerror, luaL_tolstring};
use crate::lobject::luaO_tostring;
use crate::lvm::{F2Ieq, luaV_tointeger};
use crate::value::UnsafeValue;
use crate::{Ref, Str, luaH_get};
use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use core::cmp::min;
use core::fmt::Display;
use core::mem::MaybeUninit;
use core::num::NonZero;
use core::ptr::null_mut;

/// Argument passed from Lua to Rust function.
pub struct Arg<'a, 'b> {
    cx: &'a Context<'b, Args>,
    index: NonZero<usize>,
}

impl<'a, 'b> Arg<'a, 'b> {
    #[inline(always)]
    pub(super) fn new(cx: &'a Context<'b, Args>, index: NonZero<usize>) -> Self {
        Self { cx, index }
    }

    /// Check if this argument exists.
    ///
    /// Other methods like [`Self::get_str()`] already validate if the argument exists. This method
    /// can be used in case you want to verify if the argument exists but don't need its value.
    #[inline(always)]
    pub fn exists(&self) -> Result<(), Box<dyn core::error::Error>> {
        if self.index.get() > self.cx.payload.0 {
            Err(self.error("value expected"))
        } else {
            Ok(())
        }
    }

    /// Checks if this argument is Lua string and return it.
    #[inline(always)]
    pub fn get_str(&self, convert: bool) -> Result<&'a Str, Box<dyn core::error::Error>> {
        let expect = lua_typename(4);
        let v = self.get_raw(expect)?;

        match unsafe { (*v).tt_ & 0xf } {
            4 => (),
            3 if convert => unsafe { luaO_tostring(self.cx.th.hdr.global, v) },
            _ => return Err(unsafe { self.type_error(expect, v) }),
        }

        Ok(unsafe { &*(*v).value_.gc.cast::<Str>() })
    }

    /// Checks if this argument is Lua string and return it. Returns [`None`] if this argument is
    /// `nil` or does not exists.
    #[inline(always)]
    pub fn get_nilable_str(
        &self,
        convert: bool,
    ) -> Result<Option<&'a Str>, Box<dyn core::error::Error>> {
        // Get argument.
        let v = self.get_raw_or_null();

        if v.is_null() {
            return Ok(None);
        }

        // Check type.
        match unsafe { (*v).tt_ & 0xf } {
            0 => return Ok(None),
            4 => (),
            3 if convert => unsafe { luaO_tostring(self.cx.th.hdr.global, v) },
            _ => return Err(unsafe { self.type_error(lua_typename(4), v) }),
        }

        Ok(Some(unsafe { &*(*v).value_.gc.cast::<Str>() }))
    }

    /// Gets the argument and convert it to Lua integer.
    #[inline(always)]
    pub fn to_int(&self) -> Result<i64, Box<dyn core::error::Error>> {
        // Check if integer.
        let expect = lua_typename(3);
        let raw = self.get_raw(expect)?;

        if unsafe { (*raw).tt_ == 3 | 0 << 4 } {
            return Ok(unsafe { (*raw).value_.i });
        }

        // Convert to integer.
        let mut val = MaybeUninit::uninit();

        if unsafe { luaV_tointeger(raw, val.as_mut_ptr(), F2Ieq) != 0 } {
            Ok(unsafe { val.assume_init() })
        } else if unsafe { (*raw).tt_ == 3 | 1 << 4 } {
            Err(self.error("number has no integer representation"))
        } else {
            Err(unsafe { self.type_error(expect, raw) })
        }
    }

    /// Gets the argument and convert it to Lua integer. Returns [`None`] if this argument is `nil`
    /// or does not exists.
    #[inline(always)]
    pub fn to_nilable_int(&self) -> Result<Option<i64>, Box<dyn core::error::Error>> {
        // Check type.
        let raw = self.get_raw_or_null();

        if unsafe { raw.is_null() || (*raw).tt_ & 0xf == 0 } {
            return Ok(None);
        } else if unsafe { (*raw).tt_ == 3 | 0 << 4 } {
            return Ok(Some(unsafe { (*raw).value_.i }));
        };

        // Convert to integer.
        let mut val = MaybeUninit::uninit();

        if unsafe { luaV_tointeger(raw, val.as_mut_ptr(), F2Ieq) != 0 } {
            Ok(Some(unsafe { val.assume_init() }))
        } else if unsafe { (*raw).tt_ == 3 | 1 << 4 } {
            Err(self.error("number has no integer representation"))
        } else {
            Err(unsafe { self.type_error(lua_typename(3), raw) })
        }
    }

    /// Gets the argument and convert it to Lua string.
    ///
    /// This has the same semantic as `luaL_tolstring`, which mean it does not modify the argument.
    #[inline(never)]
    pub fn to_str(&self) -> Result<Ref<Str>, Box<dyn core::error::Error>> {
        let a = min(self.index.get(), self.cx.payload.0 + 1);
        let t = self.cx.th;
        let s = unsafe { luaL_tolstring(t, a as i32)? };
        let s = unsafe { Ref::new(t.hdr.global_owned(), s) };

        unsafe { (*t).top.sub(1) };

        Ok(s)
    }

    /// Create an error for this argument.
    ///
    /// `reason` will become the value of [`core::error::Error::source()`] on the returned error.
    /// The [`core::fmt::Display`] that implemented on the returned error does not include `reason`.
    #[inline(always)]
    pub fn error(
        &self,
        reason: impl Into<Box<dyn core::error::Error>>,
    ) -> Box<dyn core::error::Error> {
        unsafe { luaL_argerror(self.cx.th, self.index, reason) }
    }

    #[inline(always)]
    fn get_raw(
        &self,
        expect: impl Display,
    ) -> Result<*mut UnsafeValue, Box<dyn core::error::Error>> {
        let th = self.cx.th;
        let ci = th.ci.get();

        if self.index.get() > self.cx.payload.0 {
            Err(self.invalid_type(expect, lua_typename(-1)))
        } else {
            Ok(unsafe { &raw mut (*(*ci).func.add(self.index.get())).val })
        }
    }

    #[inline(always)]
    fn get_raw_or_null(&self) -> *mut UnsafeValue {
        let th = self.cx.th;
        let ci = th.ci.get();

        if self.index.get() > self.cx.payload.0 {
            null_mut()
        } else {
            unsafe { &raw mut (*(*ci).func.add(self.index.get())).val }
        }
    }

    #[inline(never)]
    unsafe fn type_error(
        &self,
        expect: impl Display,
        actual: *const UnsafeValue,
    ) -> Box<dyn core::error::Error> {
        let g = self.cx.th.hdr.global();
        let mt = unsafe { g.get_mt(actual) };
        let actual: Cow<str> = if mt.is_null() {
            lua_typename(unsafe { ((*actual).tt_ & 0xf).into() }).into()
        } else {
            let key = unsafe { UnsafeValue::from_str(Str::new(g, "__name")) };
            let val = unsafe { luaH_get(mt, &key) };

            match unsafe { (*val).tt_ & 0xf } {
                4 => {
                    String::from_utf8_lossy(unsafe { (*(*val).value_.gc.cast::<Str>()).as_bytes() })
                }
                _ => lua_typename(unsafe { ((*actual).tt_ & 0xf).into() }).into(),
            }
        };

        self.error(format!("{expect} expected, got {actual}"))
    }

    #[inline(never)]
    fn invalid_type(
        &self,
        expect: impl Display,
        actual: impl Display,
    ) -> Box<dyn core::error::Error> {
        self.error(format!("{expect} expected, got {actual}"))
    }
}
