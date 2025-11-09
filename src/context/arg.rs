use super::{Args, Context};
use crate::lapi::lua_typename;
use crate::lauxlib::luaL_argerror;
use crate::ldo::luaD_call;
use crate::value::UnsafeValue;
use crate::vm::{F2Ieq, luaV_tointeger, luaV_tonumber_};
use crate::{
    Float, Fp, LuaFn, NON_YIELDABLE_WAKER, Number, Ref, Str, Table, Type, UserData, Value,
};
use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use core::any::{Any, type_name};
use core::convert::identity;
use core::fmt::{Display, Write};
use core::num::NonZero;
use core::pin::pin;
use core::ptr::{null, null_mut};
use core::task::{Poll, Waker};
use thiserror::Error;

/// Argument passed from Lua to Rust function.
///
/// Use [Context::arg()] to get the value of this type.
pub struct Arg<'a, 'b, A> {
    cx: &'a Context<'b, A, Args>,
    index: NonZero<usize>,
}

impl<'a, 'b, A> Arg<'a, 'b, A> {
    #[inline(always)]
    pub(super) fn new(cx: &'a Context<'b, A, Args>, index: NonZero<usize>) -> Self {
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
    /// ```
    /// # use tsuki::context::{Args, Context, Ret, ArgNotFound};
    /// # fn f(cx: Context<(), Args>) -> Result<Context<(), Ret>, Box<dyn core::error::Error>> {
    /// # let arg = cx.arg(1);
    /// if !arg.is_exists() {
    ///     return Err(arg.error(ArgNotFound));
    /// }
    /// # Ok(cx.into())
    /// # }
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

    /// Gets metatable for this argument.
    ///
    /// Returns [None] if this argument does not exists.
    pub fn metatable(&self) -> Option<Option<Ref<'b, Table<A>>>> {
        // Get argument.
        let v = self.get_raw_or_null();

        if v.is_null() {
            return None;
        }

        // Get metatable.
        let g = self.cx.th.hdr.global();
        let mt = unsafe { g.metatable(v) };
        let mt = match mt.is_null() {
            true => None,
            false => Some(unsafe { Ref::new(mt) }),
        };

        Some(mt)
    }

    /// Checks if this argument is an integer and return it.
    ///
    /// This method will return [None] if this argument does not exists or not an integer.
    #[inline(always)]
    pub fn as_int(&self) -> Option<i64> {
        let v = self.get_raw_or_null();

        if v.is_null() {
            None
        } else if unsafe { (*v).tt_ == 3 | 0 << 4 } {
            Some(unsafe { (*v).value_.i })
        } else {
            None
        }
    }

    /// Checks if this argument is a number and return it.
    ///
    /// This method will return [None] if this argument does not exists or not a number.
    #[inline(always)]
    pub fn as_num(&self) -> Option<Number> {
        let v = self.get_raw_or_null();

        if v.is_null() {
            None
        } else if unsafe { (*v).tt_ == 3 | 0 << 4 } {
            Some(Number::Int(unsafe { (*v).value_.i }))
        } else if unsafe { (*v).tt_ == 3 | 1 << 4 } {
            Some(Number::Float(unsafe { (*v).value_.n }))
        } else {
            None
        }
    }

    /// Checks if this argument is a string and return it.
    ///
    /// This method will accept a number if `convert` is `true`. In this case the argument will be
    /// converted to a string **in-place**.
    #[inline(always)]
    pub fn as_str(&self, convert: bool) -> Option<&'a Str<A>> {
        let v = self.get_raw_or_null();

        if v.is_null() {
            return None;
        }

        match unsafe { (*v).tt_ & 0xf } {
            3 if convert => Some(unsafe { &*self.convert_str(v) }),
            4 => Some(unsafe { &*(*v).value_.gc.cast() }),
            _ => None,
        }
    }

    /// Checks if this argument is a table and return it.
    ///
    /// This method will return [None] if this argument does not exists or not a table.
    #[inline(always)]
    pub fn as_table(&self) -> Option<&'a Table<A>> {
        let v = self.get_raw_or_null();

        if v.is_null() {
            None
        } else if unsafe { (*v).tt_ & 0xf == 5 } {
            Some(unsafe { &*(*v).value_.gc.cast() })
        } else {
            None
        }
    }

    /// Checks if this argument is a Rust function and return it.
    ///
    /// This method will return [None] if this argument does not exists or not a Rust function.
    #[inline(always)]
    pub fn as_fp(&self) -> Option<Fp<A>> {
        let v = self.get_raw_or_null();

        if v.is_null() {
            None
        } else if unsafe { (*v).tt_ & 0x3f == 0x02 } {
            Some(Fp(unsafe { (*v).value_.f }))
        } else {
            None
        }
    }

    /// Checks if this argument is a Lua function and return it.
    ///
    /// This method will return [None] if this argument does not exists or not a Lua function.
    #[inline(always)]
    pub fn as_lua_fn(&self) -> Option<&'a LuaFn<A>> {
        let v = self.get_raw_or_null();

        if v.is_null() {
            None
        } else if unsafe { (*v).tt_ & 0x3f == 0x06 } {
            Some(unsafe { &*(*v).value_.gc.cast() })
        } else {
            None
        }
    }

    /// Get address of argument value (if any).
    ///
    /// This has the same semantic as `lua_topointer`.
    #[inline(always)]
    pub fn as_ptr(&self) -> *const u8 {
        let v = self.get_raw_or_null();

        if v.is_null() {
            return null();
        }

        match unsafe { (*v).tt_ & 0x3f } {
            2 => unsafe { (*v).value_.f as *const u8 },
            18 | 50 => todo!(),
            34 => unsafe { (*v).value_.a as *const u8 },
            7 => unsafe { (*(*v).value_.gc.cast::<UserData<A, ()>>()).ptr.cast() },
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
    pub fn get(&self) -> Option<Value<'b, A>> {
        let v = self.get_raw_or_null();

        if v.is_null() {
            None
        } else {
            Some(unsafe { Value::from_unsafe(v) })
        }
    }

    /// Checks if this argument is a string and return it.
    ///
    /// This method **does not** convert a number to string. Use [Self::to_str()] if you want that
    /// behavior.
    #[inline(always)]
    pub fn get_str(&self) -> Result<&'a Str<A>, Box<dyn core::error::Error>> {
        let expect = lua_typename(4);
        let v = self.get_raw(expect)?;

        match unsafe { (*v).tt_ & 0xf } {
            4 => Ok(unsafe { &*(*v).value_.gc.cast() }),
            _ => Err(unsafe { self.type_error(expect, v) }),
        }
    }

    /// Checks if this argument is a string and return it.
    ///
    /// This method returns [`None`] in the following cases:
    ///
    /// - This argument is `nil`.
    /// - This argument does not exists and `required` is `false`.
    ///
    /// This method **does not** convert a number to string. Use [Self::to_nilable_str()] if you
    /// want that behavior.
    #[inline(always)]
    pub fn get_nilable_str(
        &self,
        required: bool,
    ) -> Result<Option<&'a Str<A>>, Box<dyn core::error::Error>> {
        // Get argument.
        let expect = "nil or string";
        let v = self.get_raw_or_null();

        if v.is_null() {
            match required {
                true => return Err(unsafe { self.type_error(expect, v) }),
                false => return Ok(None),
            }
        }

        // Check type.
        match unsafe { (*v).tt_ & 0xf } {
            0 => Ok(None),
            4 => Ok(Some(unsafe { &*(*v).value_.gc.cast() })),
            _ => Err(unsafe { self.type_error(expect, v) }),
        }
    }

    /// Checks if this argument is a table and return it.
    #[inline(always)]
    pub fn get_table(&self) -> Result<&'a Table<A>, Box<dyn core::error::Error>> {
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
    ) -> Result<Option<&'a Table<A>>, Box<dyn core::error::Error>> {
        // Check if argument exists.
        let expect = "nil or table";
        let v = self.get_raw_or_null();

        if v.is_null() {
            match required {
                true => return Err(unsafe { self.type_error(expect, v) }),
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

    /// Checks if this argument is a userdata `T` and return it.
    #[inline(always)]
    pub fn get_ud<T: Any>(&self) -> Result<&'a UserData<A, T>, Box<dyn core::error::Error>> {
        let expect = type_name::<T>();
        let v = self.get_raw(expect)?;
        let ud = match unsafe { (*v).tt_ & 0xf } {
            7 => unsafe { (*v).value_.gc.cast::<UserData<A, dyn Any>>() },
            _ => return Err(unsafe { self.type_error(expect, v) }),
        };

        match unsafe { (*ud).downcast() } {
            Some(v) => Ok(v),
            None => Err(unsafe { self.type_error(expect, v) }),
        }
    }

    /// Checks if this argument is a userdata `T` and return it.
    ///
    /// This method returns [`None`] in the following cases:
    ///
    /// - This argument is `nil`.
    /// - This argument does not exists and `required` is `false`.
    #[inline(always)]
    pub fn get_nilable_ud<T: Any>(
        &self,
        required: bool,
    ) -> Result<Option<&'a UserData<A, T>>, Box<dyn core::error::Error>> {
        // Check if argument exists.
        let name = type_name::<T>();
        let expect = format_args!("nil or {name}");
        let v = self.get_raw_or_null();

        if v.is_null() {
            match required {
                true => return Err(unsafe { self.type_error(expect, v) }),
                false => return Ok(None),
            }
        }

        // Check type.
        let ud = match unsafe { (*v).tt_ & 0xf } {
            0 => return Ok(None),
            7 => unsafe { (*v).value_.gc.cast::<UserData<A, dyn Any>>() },
            _ => return Err(unsafe { self.type_error(expect, v) }),
        };

        match unsafe { (*ud).downcast() } {
            Some(v) => Ok(Some(v)),
            None => Err(unsafe { self.type_error(expect, v) }),
        }
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
        let raw = self.get_raw(lua_typename(3))?;

        if unsafe { (*raw).tt_ == 3 | 0 << 4 } {
            Ok(unsafe { (*raw).value_.i })
        } else {
            unsafe { self.convert_int(raw) }
        }
    }

    #[inline(never)]
    unsafe fn convert_int(
        &self,
        raw: *const UnsafeValue<A>,
    ) -> Result<i64, Box<dyn core::error::Error>> {
        if let Some(val) = unsafe { luaV_tointeger(raw, F2Ieq) } {
            Ok(val)
        } else if unsafe { (*raw).tt_ == 3 | 1 << 4 } {
            Err(self.error("number has no integer representation"))
        } else {
            Err(unsafe { self.type_error(lua_typename(3), raw) })
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
                true => return Err(unsafe { self.type_error(expect, raw) }),
                false => return Ok(None),
            }
        } else if unsafe { (*raw).tt_ & 0xf == 0 } {
            return Ok(None);
        } else if unsafe { (*raw).tt_ == 3 | 0 << 4 } {
            return Ok(Some(unsafe { (*raw).value_.i }));
        };

        // Convert to integer.
        if let Some(val) = unsafe { luaV_tointeger(raw, F2Ieq) } {
            Ok(Some(val))
        } else if unsafe { (*raw).tt_ == 3 | 1 << 4 } {
            Err(self.error("number has no integer representation"))
        } else {
            Err(unsafe { self.type_error(expect, raw) })
        }
    }

    /// Gets the argument and convert it to Lua floating-point.
    ///
    /// This has the same semantic as `luaL_checknumber`.
    #[inline(always)]
    pub fn to_float(&self) -> Result<Float, Box<dyn core::error::Error>> {
        // Check if number.
        let expect = lua_typename(3);
        let raw = self.get_raw(expect)?;

        if unsafe { (*raw).tt_ == 3 | 1 << 4 } {
            return Ok(unsafe { (*raw).value_.n });
        }

        // Convert to number.
        if let Some(val) = unsafe { luaV_tonumber_(raw) } {
            Ok(val)
        } else {
            Err(unsafe { self.type_error(expect, raw) })
        }
    }

    /// Gets the argument and convert it to Lua floating-point.
    ///
    /// This method returns [`None`] in the following cases:
    ///
    /// - This argument is `nil`.
    /// - This argument does not exists and `required` is `false`.
    ///
    /// This has the same semantic as `luaL_optnumber`.
    #[inline(always)]
    pub fn to_nilable_float(
        &self,
        required: bool,
    ) -> Result<Option<Float>, Box<dyn core::error::Error>> {
        // Check type.
        let expect = "nil or number";
        let raw = self.get_raw_or_null();

        if raw.is_null() {
            match required {
                true => return Err(unsafe { self.type_error(expect, raw) }),
                false => return Ok(None),
            }
        } else if unsafe { (*raw).tt_ & 0xf == 0 } {
            return Ok(None);
        } else if unsafe { (*raw).tt_ == 3 | 1 << 4 } {
            return Ok(Some(unsafe { (*raw).value_.n }));
        };

        // Convert to number.
        if let Some(val) = unsafe { luaV_tonumber_(raw) } {
            Ok(Some(val))
        } else {
            Err(unsafe { self.type_error(expect, raw) })
        }
    }

    /// Checks if this argument is a string or number and return it as string.
    ///
    /// This has the same semantic as `luaL_checklstring`, which mean it will convert the argument
    /// **in-place** if it is a number.
    ///
    /// This method will trigger GC if new string is allocated.
    #[inline(always)]
    pub fn to_str(&self) -> Result<&'a Str<A>, Box<dyn core::error::Error>> {
        let expect = lua_typename(4);
        let v = self.get_raw(expect)?;

        match unsafe { (*v).tt_ & 0xf } {
            3 => Ok(unsafe { &*self.convert_str(v) }),
            4 => Ok(unsafe { &*(*v).value_.gc.cast() }),
            _ => Err(unsafe { self.type_error(expect, v) }),
        }
    }

    /// Checks if this argument is a string or number and return it as string.
    ///
    /// This method returns [None] in the following cases:
    ///
    /// - This argument is `nil`.
    /// - This argument does not exists and `required` is `false`.
    ///
    /// This has the same semantic as `luaL_checklstring`, which mean it will convert the argument
    /// **in-place** if it is a number.
    ///
    /// This method will trigger GC if new string is allocated.
    #[inline(always)]
    pub fn to_nilable_str(
        &self,
        required: bool,
    ) -> Result<Option<&'a Str<A>>, Box<dyn core::error::Error>> {
        // Get argument.
        let expect = "nil or string";
        let v = self.get_raw_or_null();

        if v.is_null() {
            match required {
                true => return Err(unsafe { self.type_error(expect, v) }),
                false => return Ok(None),
            }
        }

        // Check type.
        match unsafe { (*v).tt_ & 0xf } {
            0 => Ok(None),
            3 => Ok(Some(unsafe { &*self.convert_str(v) })),
            4 => Ok(Some(unsafe { &*(*v).value_.gc.cast() })),
            _ => Err(unsafe { self.type_error(expect, v) }),
        }
    }

    #[inline(never)]
    unsafe fn convert_str(&self, v: *mut UnsafeValue<A>) -> *const Str<A> {
        // Convert to string.
        let s = if unsafe { (*v).tt_ & 0x3f == 0x03 } {
            unsafe { (*v).value_.i.to_string() }
        } else {
            unsafe { (*v).value_.n.to_string() }
        };

        // Create string.
        let g = self.cx.th.hdr.global();
        let s = unsafe { Str::from_str(g, s) };
        let r = s.unwrap_or_else(identity);

        unsafe { (*v).tt_ = (*r).hdr.tt | 1 << 6 };
        unsafe { (*v).value_.gc = r.cast() };

        if s.is_ok() {
            g.gc.step();
        }

        r
    }

    /// Gets the argument and convert it to Lua string suitable for display.
    ///
    /// This method does not modify the argument and treat non-existent argument as `nil` the same
    /// as `luaL_tolstring`. Note that this method requires `__tostring` metamethod to return a
    /// UTF-8 string. It also required `__name` metavalue to be UTF-8 string.
    ///
    /// The returned [`Str`] guarantee to be a UTF-8 string. If this argument is a string but it is
    /// not UTF-8 this method will return a new [`Str`] with content `string: CONTENT_IN_LOWER_HEX`
    /// instead.
    pub fn display(&self) -> Result<Ref<'b, Str<A>>, Box<dyn core::error::Error>> {
        // Try __tostring metamethod.
        let t = self.cx.th;
        let g = t.hdr.global();
        let arg = self.get_raw_or_null();
        let mt = match arg.is_null() {
            true => null(),
            false => unsafe { g.metatable(arg) },
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
                {
                    let f = unsafe { t.top.get().sub(2) };
                    let f = pin!(unsafe { luaD_call(t, f, 1) });
                    let w = unsafe { Waker::new(null(), &NON_YIELDABLE_WAKER) };

                    match f.poll(&mut core::task::Context::from_waker(&w)) {
                        Poll::Ready(Ok(_)) => (),
                        Poll::Ready(Err(e)) => return Err(e), // Requires unsized coercion.
                        Poll::Pending => unreachable!(),
                    }
                }

                unsafe { t.top.sub(1) };

                // Get result.
                let r = unsafe { t.top.read(0) };

                match r.tt_ & 0xf {
                    3 => unsafe {
                        // Convert to string.
                        let s = if r.tt_ & 0x3f == 0x03 {
                            r.value_.i.to_string()
                        } else {
                            r.value_.n.to_string()
                        };

                        // Create string.
                        let s = Str::from_str(g, s);
                        let r = Ref::new(s.unwrap_or_else(identity));

                        if s.is_ok() {
                            g.gc.step();
                        }

                        return Ok(r);
                    },
                    4 => unsafe {
                        let r = r.value_.gc.cast::<Str<A>>();

                        if !(*r).is_utf8() {
                            return Err(self.error("'__tostring' must return a UTF-8 string"));
                        }

                        return Ok(Ref::new(r));
                    },
                    _ => return Err("'__tostring' must return a string".into()),
                }
            }
        }

        // Get type.
        let ty = match arg.is_null() {
            true => None,
            false => Some(unsafe { (*arg).tt_ & 0xf }),
        };

        // Check type.
        let v = match ty {
            Some(0) => unsafe { Str::from_str(g, "nil").unwrap_or_else(identity) },
            Some(1) => match unsafe { ((*arg).tt_ >> 4) & 3 } {
                0 => unsafe { Str::from_str(g, "false").unwrap_or_else(identity) },
                _ => unsafe { Str::from_str(g, "true").unwrap_or_else(identity) },
            },
            Some(3) => match unsafe { ((*arg).tt_ >> 4) & 3 } {
                0 => unsafe {
                    Str::from_str(g, (*arg).value_.i.to_string()).unwrap_or_else(identity)
                },
                1 => unsafe {
                    let v = (*arg).value_.n;

                    Str::from_str(g, v.to_string()).unwrap_or_else(identity)
                },
                _ => unreachable!(),
            },
            Some(4) if unsafe { (*(*arg).value_.gc.cast::<Str<A>>()).is_utf8() } => unsafe {
                (*arg).value_.gc.cast::<Str<A>>()
            },
            Some(v) => unsafe {
                // Get __name from metatable.
                let kind = match (mt.is_null() == false)
                    .then(move || (*mt).get_raw_str_key("__name"))
                    .filter(|&v| (*v).tt_ & 0xf == 4)
                    .map(|v| (*v).value_.gc.cast::<Str<A>>())
                {
                    Some(v) => (*v)
                        .as_str()
                        .ok_or_else(|| self.error("'__name' must be UTF-8 string"))?,
                    None => lua_typename(v.into()),
                };

                // Build value.
                let mut buf = String::with_capacity(kind.len() + 2 + 18);

                write!(buf, "{kind}: ").unwrap();

                match v {
                    2 => write!(buf, "{:p}", (*arg).value_.f).unwrap(),
                    18 | 50 => todo!(),
                    34 => write!(buf, "{:p}", (*arg).value_.a).unwrap(),
                    4 => {
                        let v = (*arg).value_.gc.cast::<Str<A>>();
                        let v = (*v).as_bytes();

                        buf.reserve(v.len().saturating_mul(2).saturating_sub(18));

                        for b in v {
                            write!(buf, "{b:x}").unwrap();
                        }
                    }
                    5 | 6 | 8 => write!(buf, "{:p}", (*arg).value_.gc).unwrap(),
                    7 => write!(
                        buf,
                        "{:p}",
                        (*(*arg).value_.gc.cast::<UserData<A, ()>>()).ptr
                    )
                    .unwrap(),
                    _ => unreachable!(),
                }

                Str::from_str(g, buf).unwrap_or_else(identity)
            },
            None => unsafe {
                Str::from_str(g, format!("{}: {:p}", lua_typename(-1), null::<()>()))
                    .unwrap_or_else(identity)
            },
        };

        Ok(unsafe { Ref::new(v) })
    }

    /// Create invalid type error for this argument.
    #[inline(always)]
    pub fn invalid_type(&self, expect: impl Display) -> Box<dyn core::error::Error> {
        let v = self.get_raw_or_null();

        unsafe { self.type_error(expect, v) }
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
    ) -> Result<*mut UnsafeValue<A>, Box<dyn core::error::Error>> {
        let th = self.cx.th;
        let ci = th.ci.get();

        if self.index.get() > self.cx.payload.0 {
            Err(unsafe { self.type_error(expect, null()) })
        } else {
            Ok(unsafe { th.stack.get().add((*ci).func + self.index.get()).cast() })
        }
    }

    #[inline(always)]
    pub(crate) fn get_raw_or_null(&self) -> *mut UnsafeValue<A> {
        let th = self.cx.th;
        let ci = th.ci.get();

        if self.index.get() > self.cx.payload.0 {
            null_mut()
        } else {
            unsafe { th.stack.get().add((*ci).func + self.index.get()).cast() }
        }
    }

    #[inline(never)]
    unsafe fn type_error(
        &self,
        expect: impl Display,
        actual: *const UnsafeValue<A>,
    ) -> Box<dyn core::error::Error> {
        // Check if no value.
        if actual.is_null() {
            return self.error(format!("{} expected, got {}", expect, lua_typename(-1)));
        }

        // Get type name.
        let actual = self.cx.type_name(unsafe { actual.read() });

        self.error(format!("{} expected, got {}", expect, actual))
    }
}

/// Represents an error when [`Arg`] does not exists.
#[derive(Debug, Error)]
#[error("value expected")]
pub struct ArgNotFound;
