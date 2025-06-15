use crate::lapi::{lua_isnumber, lua_tointegerx, lua_tolstring, lua_typename};
use crate::lauxlib::{luaL_argerror, luaL_tolstring, luaL_typeerror};
use crate::{Ref, Str, Thread};
use alloc::boxed::Box;
use core::ops::Deref;

/// Context to invoke Rust function.
pub struct Context {
    th: *const Thread,
    args: usize,
}

impl Context {
    #[inline(always)]
    pub(crate) fn new(th: *const Thread, args: usize) -> Self {
        Self { th, args }
    }

    /// Returns `true` if this call has no arguments.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.args == 0
    }

    /// Returns a number of arguments for this call.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.args
    }

    /// Gets string argument.
    ///
    /// Note that `arg` is **zero-based**, not one.
    ///
    /// # Panics
    /// If `arg` greater or equal [`Self::len()`].
    #[inline(always)]
    pub fn get_str(&self, arg: usize, convert: bool) -> Result<&Str, Box<dyn core::error::Error>> {
        assert!(arg < self.args);

        let arg = (arg + 1) as i32;
        let t = self.th;
        let s = unsafe { lua_tolstring(t, arg, convert) };

        if s.is_null() {
            Err(unsafe { luaL_typeerror(t, arg, lua_typename(4)) })
        } else {
            Ok(unsafe { &*s })
        }
    }

    /// Gets string argument.
    ///
    /// Note that `arg` is **zero-based**, not one.
    ///
    /// # Panics
    /// If `arg` greater or equal [`Self::len()`].
    #[inline(always)]
    pub fn get_nilable_str(
        &self,
        arg: usize,
        convert: bool,
    ) -> Result<Option<&Str>, Box<dyn core::error::Error>> {
        assert!(arg < self.args);

        if unsafe { ((*self.th).get(arg + 1).tt_ & 0xf) == 0 } {
            Ok(None)
        } else {
            self.get_str(arg, convert).map(Some)
        }
    }

    /// Gets argument `arg` and convert it to Lua integer.
    ///
    /// Note that `arg` is **zero-based**, not one.
    ///
    /// # Panics
    /// If `arg` greater or equal [`Self::len()`].
    #[inline(always)]
    pub fn to_int(&self, arg: usize) -> Result<i64, Box<dyn core::error::Error>> {
        assert!(arg < self.args);

        let arg = (arg + 1) as i32;
        let mut isnum = 0;
        let d: i64 = unsafe { lua_tointegerx(self.th, arg, &mut isnum) };

        if isnum == 0 {
            Err(if unsafe { lua_isnumber(self.th, arg) } != 0 {
                unsafe { luaL_argerror(self.th, arg, "number has no integer representation") }
            } else {
                unsafe { luaL_typeerror(self.th, arg, lua_typename(3)) }
            })
        } else {
            Ok(d)
        }
    }

    /// Gets argument `arg` and convert it to Lua integer.
    ///
    /// Note that `arg` is **zero-based**, not one.
    ///
    /// # Panics
    /// If `arg` greater or equal [`Self::len()`].
    #[inline(always)]
    pub fn to_nilable_int(&self, arg: usize) -> Result<Option<i64>, Box<dyn core::error::Error>> {
        assert!(arg < self.args);

        if unsafe { ((*self.th).get(arg + 1).tt_ & 0xf) == 0 } {
            Ok(None)
        } else {
            self.to_int(arg).map(Some)
        }
    }

    /// Get argument `arg` and convert it to Lua string.
    ///
    /// Note that `arg` is **zero-based**, not one.
    ///
    /// This has the same semantic as `luaL_tolstring`, which mean it does not modify the argument.
    ///
    /// # Panics
    /// If `arg` greater or equal [`Self::len()`].
    #[inline(always)]
    pub fn to_str(&self, arg: usize) -> Result<Ref<Str>, Box<dyn core::error::Error>> {
        assert!(arg < self.args);

        let t = self.th;
        let s = unsafe { luaL_tolstring(t, (arg + 1) as i32)? };
        let s = unsafe { Ref::new((*t).hdr.global_owned(), s) };

        unsafe { (*t).top.sub(1) };

        Ok(s)
    }
}

/// Context to invoke Rust yield function.
pub struct YieldContext<'a>(&'a Context);

impl<'a> Deref for YieldContext<'a> {
    type Target = Context;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.0
    }
}

/// Context to invoke Rust async function.
pub struct AsyncContext<'a>(&'a Context);

impl<'a> Deref for AsyncContext<'a> {
    type Target = Context;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.0
    }
}
