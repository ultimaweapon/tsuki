use super::{Args, Context};
use crate::lapi::{lua_pcall, lua_typename};
use crate::lauxlib::luaL_argerror;
use crate::lobject::{Udata, luaO_tostring};
use crate::value::UnsafeValue;
use crate::vm::{F2Ieq, luaV_objlen, luaV_tointeger, luaV_tonumber_};
use crate::{NON_YIELDABLE_WAKER, Ref, Str, Table, Type, Value, luaH_get};
use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use core::fmt::Display;
use core::mem::{MaybeUninit, offset_of};
use core::num::NonZero;
use core::pin::pin;
use core::ptr::{null, null_mut};
use core::task::{Poll, Waker};
use thiserror::Error;

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
    /// You can use [`Self::exists()`] if you want to return an error if this argument does not
    /// exists.
    #[inline(always)]
    pub fn is_exists(&self) -> bool {
        self.index.get() <= self.cx.payload.0
    }

    /// Check if this argument exists.
    ///
    /// This has the same effect as:
    ///
    /// ```ignore
    /// if !arg.is_exists() {
    ///     return Err(arg.error(ArgNotFound));
    /// }
    /// ```
    ///
    /// Other methods like [`Self::get_str()`] already validate if the argument exists. This method
    /// can be used in case you want to verify if the argument exists but don't need its value.
    ///
    /// This has the same semantic as `luaL_checkany`.
    #[inline(always)]
    pub fn exists(self) -> Result<Self, Box<dyn core::error::Error>> {
        if self.is_exists() {
            Ok(self)
        } else {
            Err(self.error(ArgNotFound))
        }
    }

    /// Returns type of this argument.
    ///
    /// Use [`Self::is_int()`] if you want to check if argument is Lua integer.
    #[inline(always)]
    pub fn ty(&self) -> Option<Type> {
        let v = self.get_raw_or_null();

        if v.is_null() {
            None
        } else {
            Some(Type::from_tt(unsafe { (*v).tt_ }))
        }
    }

    /// Check if this argument is Lua integer.
    ///
    /// This has the same semantic as `lua_isinteger`, except it return [`None`] if the argument
    /// does not exists instead of `false`.
    #[inline(always)]
    pub fn is_int(&self) -> Option<bool> {
        let v = self.get_raw_or_null();

        if v.is_null() {
            None
        } else {
            Some(unsafe { (*v).tt_ == 3 | 0 << 4 })
        }
    }

    /// Returns length of the value.
    ///
    /// This has the same semantic as `luaL_len`, which mean it is honor `__len` metamethod.
    pub fn len(&self) -> Result<i64, Box<dyn core::error::Error>> {
        // Get argument.
        let v = self.get_raw_or_null();

        if v.is_null() {
            return Err(self.error(ArgNotFound));
        }

        // Get length.
        let l = unsafe { luaV_objlen(self.cx.th, v)? };

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

    /// Get address of argument value (if any).
    ///
    /// This has the same semantic as `lua_topointer`, except it does not return the address of
    /// userdata.
    #[inline(always)]
    pub fn as_ptr(&self) -> *const u8 {
        let v = self.get_raw_or_null();

        if v.is_null() {
            return null();
        }

        match unsafe { (*v).tt_ & 0x3f } {
            2 => unsafe { (*v).value_.f as *const u8 },
            18 | 34 | 50 => todo!(),
            _ => unsafe {
                if (*v).tt_ & 1 << 6 != 0 {
                    (*v).value_.gc.cast()
                } else {
                    null()
                }
            },
        }
    }

    /// Returns the value of this argument.
    ///
    /// This method is expensive compared to a specialized method like [`Self::get_str()`]. Use this
    /// method only when you need [`Value`]. If you want to check type of this argument use
    /// [`Self::ty()`] instead since it much faster.
    pub fn get(&self) -> Option<Value> {
        let v = self.get_raw_or_null();

        if v.is_null() {
            None
        } else {
            Some(unsafe { Value::from_unsafe(v) })
        }
    }

    /// Checks if this argument is Lua string or number and return it as string.
    ///
    /// This has the same semantic as `luaL_checklstring`, which mean if this argument is a number
    /// it will convert to string **in-place**.
    #[inline(always)]
    pub fn get_str(&self) -> Result<&'a Str, Box<dyn core::error::Error>> {
        let expect = lua_typename(4);
        let v = self.get_raw(expect)?;

        match unsafe { (*v).tt_ & 0xf } {
            3 => unsafe { luaO_tostring(self.cx.th.hdr.global, v) },
            4 => (),
            _ => return Err(unsafe { self.type_error(expect, v) }),
        }

        Ok(unsafe { &*(*v).value_.gc.cast::<Str>() })
    }

    /// Checks if this argument is Lua string or number and return it as string.
    ///
    /// This method returns [`None`] in the following cases:
    ///
    /// - This argument is `nil`.
    /// - This argument does not exists and `required` is `false`.
    ///
    /// This has the same semantic as `luaL_optlstring`, which mean if this argument is a number it
    /// will convert to string **in-place**.
    #[inline(always)]
    pub fn get_nilable_str(
        &self,
        required: bool,
    ) -> Result<Option<&'a Str>, Box<dyn core::error::Error>> {
        // Get argument.
        let expect = "nil or string";
        let v = self.get_raw_or_null();

        if v.is_null() {
            match required {
                true => return Err(self.invalid_type(expect, lua_typename(-1))),
                false => return Ok(None),
            }
        }

        // Check type.
        match unsafe { (*v).tt_ & 0xf } {
            0 => return Ok(None),
            3 => unsafe { luaO_tostring(self.cx.th.hdr.global, v) },
            4 => (),
            _ => return Err(unsafe { self.type_error(expect, v) }),
        }

        Ok(Some(unsafe { &*(*v).value_.gc.cast::<Str>() }))
    }

    /// Checks if this argument is a table and return it.
    #[inline(always)]
    pub fn get_table(&self) -> Result<&'a Table, Box<dyn core::error::Error>> {
        let expect = lua_typename(5);
        let v = self.get_raw(expect)?;

        match unsafe { (*v).tt_ & 0xf } {
            5 => Ok(unsafe { &*(*v).value_.gc.cast() }),
            _ => Err(unsafe { self.type_error(expect, v) }),
        }
    }

    /// Checks if this argument is a table and return it.
    ///
    /// This method returns [`None`] in the following cases:
    ///
    /// - This argument is `nil`.
    /// - This argument does not exists and `required` is `false`.
    #[inline(always)]
    pub fn get_nilable_table(
        &self,
        required: bool,
    ) -> Result<Option<&'a Table>, Box<dyn core::error::Error>> {
        // Check if argument exists.
        let expect = "nil or table";
        let v = self.get_raw_or_null();

        if v.is_null() {
            match required {
                true => return Err(self.invalid_type(expect, lua_typename(-1))),
                false => return Ok(None),
            }
        }

        // Check type.
        match unsafe { (*v).tt_ & 0xf } {
            0 => Ok(None),
            5 => Ok(Some(unsafe { &*(*v).value_.gc.cast() })),
            _ => Err(unsafe { self.type_error(expect, v) }),
        }
    }

    /// Gets metatable for this argument.
    pub fn get_metatable(&self) -> Option<Option<Ref<Table>>> {
        // Get argument.
        let v = self.get_raw_or_null();

        if v.is_null() {
            return None;
        }

        // Get metatable.
        let g = self.cx.th.hdr.global();
        let mt = unsafe { g.get_mt(v) };
        let mt = match mt.is_null() {
            true => None,
            false => Some(unsafe { Ref::new(mt) }),
        };

        Some(mt)
    }

    /// Gets the argument and convert it to Lua boolean.
    ///
    /// This method has the same mechanism as Lua conditional check, which mean it only returns
    /// `false` in the following cases:
    ///
    /// - This argument has `false` value.
    /// - This argument is `nil`.
    ///
    /// All other values will cause this method to return `true`, including **zero**.
    ///
    /// This has the same semantic as `lua_toboolean`, except it return [`None`] if the argument
    /// does not exists instead of `false`.
    #[inline(always)]
    pub fn to_bool(&self) -> Option<bool> {
        let raw = self.get_raw_or_null();

        if raw.is_null() {
            None
        } else if unsafe { (*raw).tt_ == 1 | 0 << 4 || (*raw).tt_ & 0xf == 0 } {
            Some(false)
        } else {
            Some(true)
        }
    }

    /// Gets the argument and convert it to Lua integer.
    ///
    /// This has the same semantic as `luaL_checkinteger`.
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

    /// Gets the argument and convert it to Lua integer.
    ///
    /// This method returns [`None`] in the following cases:
    ///
    /// - This argument is `nil`.
    /// - This argument does not exists and `required` is `false`.
    ///
    /// This has the same semantic as `luaL_optinteger`.
    pub fn to_nilable_int(
        &self,
        required: bool,
    ) -> Result<Option<i64>, Box<dyn core::error::Error>> {
        // Check type.
        let expect = "nil or number";
        let raw = self.get_raw_or_null();

        if raw.is_null() {
            match required {
                true => return Err(self.invalid_type(expect, lua_typename(-1))),
                false => return Ok(None),
            }
        } else if unsafe { (*raw).tt_ & 0xf == 0 } {
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
            Err(unsafe { self.type_error(expect, raw) })
        }
    }

    /// Gets the argument and convert it to Lua number.
    ///
    /// This has the same semantic as `luaL_checknumber`.
    #[inline(always)]
    pub fn to_num(&self) -> Result<f64, Box<dyn core::error::Error>> {
        // Check if number.
        let expect = lua_typename(3);
        let raw = self.get_raw(expect)?;

        if unsafe { (*raw).tt_ == 3 | 1 << 4 } {
            return Ok(unsafe { (*raw).value_.n });
        }

        // Convert to number.
        let mut val = MaybeUninit::uninit();

        if unsafe { luaV_tonumber_(raw, val.as_mut_ptr()) != 0 } {
            Ok(unsafe { val.assume_init() })
        } else {
            Err(unsafe { self.type_error(expect, raw) })
        }
    }

    /// Gets the argument and convert it to Lua number.
    ///
    /// This method returns [`None`] in the following cases:
    ///
    /// - This argument is `nil`.
    /// - This argument does not exists and `required` is `false`.
    ///
    /// This has the same semantic as `luaL_optnumber`.
    #[inline(always)]
    pub fn to_nilable_num(
        &self,
        required: bool,
    ) -> Result<Option<f64>, Box<dyn core::error::Error>> {
        // Check type.
        let expect = "nil or number";
        let raw = self.get_raw_or_null();

        if raw.is_null() {
            match required {
                true => return Err(self.invalid_type(expect, lua_typename(-1))),
                false => return Ok(None),
            }
        } else if unsafe { (*raw).tt_ & 0xf == 0 } {
            return Ok(None);
        } else if unsafe { (*raw).tt_ == 3 | 1 << 4 } {
            return Ok(Some(unsafe { (*raw).value_.n }));
        };

        // Convert to number.
        let mut val = MaybeUninit::uninit();

        if unsafe { luaV_tonumber_(raw, val.as_mut_ptr()) != 0 } {
            Ok(Some(unsafe { val.assume_init() }))
        } else {
            Err(unsafe { self.type_error(expect, raw) })
        }
    }

    /// Gets the argument and convert it to Lua string suitable for display.
    ///
    /// This has the same semantic as `luaL_tolstring`, which mean it does not modify the argument
    /// and treat non-existent argument as `nil`.
    pub fn display(&self) -> Result<Ref<Str>, Box<dyn core::error::Error>> {
        // Try __tostring metamethod.
        let t = self.cx.th;
        let g = t.hdr.global();
        let arg = self.get_raw_or_null();
        let mt = match arg.is_null() {
            true => null(),
            false => unsafe { g.get_mt(arg) },
        };

        if !mt.is_null() {
            let v = unsafe { (*mt).get_raw_str_key("__tostring") };

            if unsafe { (*v).tt_ & 0xf != 0 } {
                // Assume extra stack.
                unsafe { t.top.write(*v) };
                unsafe { t.top.add(1) };
                unsafe { t.top.write(*arg) };
                unsafe { t.top.add(1) };

                // Invoke.
                let f = pin!(unsafe { lua_pcall(t, 1, 1) });
                let w = unsafe { Waker::new(null(), &NON_YIELDABLE_WAKER) };

                match f.poll(&mut core::task::Context::from_waker(&w)) {
                    Poll::Ready(v) => v?,
                    Poll::Pending => unreachable!(),
                }

                unsafe { t.top.sub(1) };

                // Get result.
                let mut r = unsafe { t.top.read(0) };

                match r.tt_ & 0xf {
                    3 => unsafe { luaO_tostring(g, &mut r) },
                    4 => (),
                    _ => return Err("'__tostring' must return a string".into()),
                }

                return Ok(unsafe { Ref::new(r.value_.gc.cast::<Str>()) });
            }
        }

        // Get type.
        let ty = match arg.is_null() {
            true => None,
            false => Some(unsafe { (*arg).tt_ & 0xf }),
        };

        // Check type.
        let v = match ty {
            Some(0) => unsafe { Str::from_str(g, "nil") },
            Some(1) => match unsafe { ((*arg).tt_ >> 4) & 3 } {
                0 => unsafe { Str::from_str(g, "false") },
                _ => unsafe { Str::from_str(g, "true") },
            },
            Some(3) => match unsafe { ((*arg).tt_ >> 4) & 3 } {
                0 => unsafe { Str::from_str(g, (*arg).value_.i.to_string()) },
                1 => unsafe {
                    // Lua expect 0.0 as "0.0". The problem is there is no way to force Rust to
                    // output "0.0" so we need to do this manually.
                    let v = (*arg).value_.n;

                    if v.fract() == 0.0 {
                        Str::from_str(g, format!("{v:.1}"))
                    } else {
                        Str::from_str(g, v.to_string())
                    }
                },
                _ => unreachable!(),
            },
            Some(4) => unsafe { (*arg).value_.gc.cast::<Str>() },
            Some(v) => unsafe {
                // Get __name from metatable.
                let kind = (mt.is_null() == false)
                    .then(|| (*mt).get_raw_str_key("__name"))
                    .filter(|&v| (*v).tt_ & 0xf == 4)
                    .map(|v| (*v).value_.gc.cast::<Str>())
                    .map(|v| match (*v).as_str() {
                        Some(v) => Cow::Borrowed(v),
                        None => String::from_utf8_lossy((*v).as_bytes()),
                    })
                    .unwrap_or_else(|| lua_typename(v.into()).into());
                let v = match v {
                    2 => (*arg).value_.f as *const (),
                    18 | 34 | 50 => todo!(),
                    5 | 6 | 8 => (*arg).value_.gc.cast(),
                    7 => (*arg)
                        .value_
                        .gc
                        .byte_add(
                            offset_of!(Udata, uv)
                                + size_of::<UnsafeValue>()
                                    * usize::from((*((*arg).value_.gc.cast::<Udata>())).nuvalue),
                        )
                        .cast(),
                    _ => unreachable!(),
                };

                Str::from_str(g, format!("{}: {:p}", kind, v))
            },
            None => unsafe {
                Str::from_str(g, format!("{}: {:p}", lua_typename(-1), null::<()>()))
            },
        };

        Ok(unsafe { Ref::new(v) })
    }

    /// Create an error for this argument.
    ///
    /// `reason` will become the value of [`core::error::Error::source()`] on the returned error.
    /// The [`core::fmt::Display`] that implemented on the returned error does not include `reason`.
    ///
    /// Use [`ArgNotFound`] if this argument is required but does not exists.
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
    pub(crate) fn get_raw_or_null(&self) -> *mut UnsafeValue {
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
            let key = unsafe { UnsafeValue::from_obj(Str::from_str(g, "__name").cast()) };
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

/// Represents an error when [`Arg`] does not exists.
#[derive(Debug, Error)]
#[error("value expected")]
pub struct ArgNotFound;
