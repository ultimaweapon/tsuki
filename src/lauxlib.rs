#![allow(non_camel_case_types, non_snake_case, unused_assignments)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::Thread;
use crate::lapi::{
    lua_checkstack, lua_concat, lua_copy, lua_getfield, lua_gettop, lua_next, lua_pushlstring,
    lua_pushnil, lua_pushstring, lua_rawequal, lua_rotate, lua_settop, lua_tolstring, lua_type,
};
use crate::ldebug::{lua_getinfo, lua_getstack};
use crate::lstate::lua_Debug;
use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use core::ffi::{CStr, c_char};
use core::fmt::{Display, Formatter};
use core::num::NonZero;
use libc::strcmp;

type c_int = i32;

unsafe fn findfield<D>(
    L: *const Thread<D>,
    objidx: c_int,
    level: c_int,
) -> Result<c_int, Box<dyn core::error::Error>> {
    if level == 0 as c_int || !(lua_type(L, -(1 as c_int)) == 5 as c_int) {
        return Ok(0 as c_int);
    }
    lua_pushnil(L);
    while lua_next(L, -2)? != 0 {
        if lua_type(L, -(2 as c_int)) == 4 as c_int {
            if lua_rawequal(L, objidx, -(1 as c_int))? != 0 {
                lua_settop(L, -(1 as c_int) - 1 as c_int)?;
                return Ok(1 as c_int);
            } else if findfield(L, objidx, level - 1 as c_int)? != 0 {
                lua_pushstring(L, b".\0" as *const u8 as *const c_char);
                lua_copy(L, -(1 as c_int), -(3 as c_int));
                lua_settop(L, -(1 as c_int) - 1 as c_int)?;
                lua_concat(L, 3 as c_int)?;
                return Ok(1 as c_int);
            }
        }
        lua_settop(L, -(1 as c_int) - 1 as c_int)?;
    }
    return Ok(0 as c_int);
}

unsafe fn pushglobalfuncname<D>(
    L: *const Thread<D>,
    ar: &mut lua_Debug<D>,
) -> Result<c_int, Box<dyn core::error::Error>> {
    let top: c_int = lua_gettop(L);
    luaL_checkstack(L, 8, b"not enough stack\0" as *const u8 as *const c_char)?;
    lua_getinfo(L, b"f\0" as *const u8 as *const c_char, ar);
    lua_getfield(L, -(1000000 as c_int) - 1000 as c_int, "_LOADED")?;

    if findfield(L, top + 1 as c_int, 2 as c_int)? != 0 {
        let name = (*lua_tolstring(L, -1, true)).as_bytes();

        if let Some(name) = name.strip_prefix(b"_G.") {
            lua_pushlstring(L, name);
            lua_rotate(L, -(2 as c_int), -(1 as c_int));
            lua_settop(L, -(1 as c_int) - 1 as c_int)?;
        }

        lua_copy(L, -(1 as c_int), top + 1 as c_int);
        lua_settop(L, top + 1 as c_int)?;
        return Ok(1 as c_int);
    } else {
        lua_settop(L, top)?;
        return Ok(0 as c_int);
    }
}

/// `arg` is used only for display.
#[inline(never)]
pub unsafe fn luaL_argerror<D>(
    L: *const Thread<D>,
    mut arg: NonZero<usize>,
    reason: impl Into<Box<dyn core::error::Error>>,
) -> Box<dyn core::error::Error> {
    let mut ar = lua_Debug::default();

    if lua_getstack(L, 0 as c_int, &mut ar) == 0 {
        return Box::new(ArgError {
            message: format!("bad argument #{arg}"),
            reason: reason.into(),
        });
    }

    lua_getinfo(L, b"n\0" as *const u8 as *const c_char, &mut ar);

    if strcmp(ar.namewhat, b"method\0" as *const u8 as *const c_char) == 0 as c_int {
        arg = match NonZero::new(arg.get() - 1) {
            Some(v) => v,
            None => {
                return Box::new(ArgError {
                    message: format!(
                        "calling '{}' on bad self",
                        CStr::from_ptr(ar.name).to_string_lossy()
                    ),
                    reason: reason.into(),
                });
            }
        };
    }

    if (ar.name).is_null() {
        ar.name = match pushglobalfuncname(L, &mut ar) {
            Ok(0) => b"?\0" as *const u8 as *const c_char,
            Ok(_) => (*lua_tolstring(L, -1, true)).contents.as_ptr(),
            Err(e) => return e,
        };
    }

    Box::new(ArgError {
        message: format!(
            "bad argument #{arg} to '{}'",
            CStr::from_ptr(ar.name).to_string_lossy()
        ),
        reason: reason.into(),
    })
}

pub unsafe fn luaL_where<D>(L: *const Thread<D>, level: c_int) -> Cow<'static, str> {
    let mut ar = lua_Debug::default();

    if lua_getstack(L, level, &mut ar) != 0 {
        lua_getinfo(L, b"Sl\0" as *const u8 as *const c_char, &mut ar);

        if ar.currentline > 0 {
            return format!(
                "{}:{}: ",
                ar.source.as_ref().map(|v| v.name()).unwrap_or(""),
                ar.currentline,
            )
            .into();
        }
    }

    "".into()
}

pub unsafe fn luaL_error<D>(L: *const Thread<D>, m: impl Display) -> Box<dyn core::error::Error> {
    format!("{}{}", luaL_where(L, 1), m).into()
}

pub unsafe fn luaL_checkstack<D>(
    L: *const Thread<D>,
    space: usize,
    msg: *const c_char,
) -> Result<(), Box<dyn core::error::Error>> {
    if lua_checkstack(L, space, 0).is_err() {
        if !msg.is_null() {
            return Err(luaL_error(
                L,
                format!("stack overflow ({})", CStr::from_ptr(msg).to_string_lossy()),
            ));
        } else {
            return Err(luaL_error(L, "stack overflow"));
        }
    }

    Ok(())
}

/// Represents an error when argument to Rust function is not valid.
#[derive(Debug)]
struct ArgError {
    message: String,
    reason: Box<dyn core::error::Error>,
}

impl core::error::Error for ArgError {
    #[inline(always)]
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        Some(self.reason.as_ref())
    }
}

impl Display for ArgError {
    #[inline(always)]
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        self.message.fmt(f)
    }
}
