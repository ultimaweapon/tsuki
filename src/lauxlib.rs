#![allow(non_camel_case_types, non_snake_case, unused_assignments)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::lapi::{
    lua_checkstack, lua_concat, lua_copy, lua_getfield, lua_getmetatable, lua_gettop, lua_next,
    lua_pushlstring, lua_pushnil, lua_pushstring, lua_rawequal, lua_rawget, lua_rotate, lua_settop,
    lua_tolstring, lua_type, lua_typename,
};
use crate::ldebug::{lua_getinfo, lua_getstack};
use crate::lstate::lua_Debug;
use crate::{Thread, lua_pop};
use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use core::ffi::{CStr, c_char};
use core::fmt::{Display, Formatter, Write};
use core::num::NonZero;
use libc::strcmp;

unsafe fn findfield<D>(
    L: *const Thread<D>,
    objidx: libc::c_int,
    level: libc::c_int,
) -> Result<libc::c_int, Box<dyn core::error::Error>> {
    if level == 0 as libc::c_int || !(lua_type(L, -(1 as libc::c_int)) == 5 as libc::c_int) {
        return Ok(0 as libc::c_int);
    }
    lua_pushnil(L);
    while lua_next(L, -2)? != 0 {
        if lua_type(L, -(2 as libc::c_int)) == 4 as libc::c_int {
            if lua_rawequal(L, objidx, -(1 as libc::c_int))? != 0 {
                lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
                return Ok(1 as libc::c_int);
            } else if findfield(L, objidx, level - 1 as libc::c_int)? != 0 {
                lua_pushstring(L, b".\0" as *const u8 as *const c_char);
                lua_copy(L, -(1 as libc::c_int), -(3 as libc::c_int));
                lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
                lua_concat(L, 3 as libc::c_int)?;
                return Ok(1 as libc::c_int);
            }
        }
        lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
    }
    return Ok(0 as libc::c_int);
}

unsafe fn pushglobalfuncname<D>(
    L: *const Thread<D>,
    ar: &mut lua_Debug<D>,
) -> Result<libc::c_int, Box<dyn core::error::Error>> {
    let top: libc::c_int = lua_gettop(L);
    luaL_checkstack(L, 8, b"not enough stack\0" as *const u8 as *const c_char)?;
    lua_getinfo(L, b"f\0" as *const u8 as *const c_char, ar);
    lua_getfield(
        L,
        -(1000000 as libc::c_int) - 1000 as libc::c_int,
        "_LOADED",
    )?;

    if findfield(L, top + 1 as libc::c_int, 2 as libc::c_int)? != 0 {
        let name = (*lua_tolstring(L, -1, true)).as_bytes();

        if let Some(name) = name.strip_prefix(b"_G.") {
            lua_pushlstring(L, name);
            lua_rotate(L, -(2 as libc::c_int), -(1 as libc::c_int));
            lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
        }

        lua_copy(L, -(1 as libc::c_int), top + 1 as libc::c_int);
        lua_settop(L, top + 1 as libc::c_int)?;
        return Ok(1 as libc::c_int);
    } else {
        lua_settop(L, top)?;
        return Ok(0 as libc::c_int);
    }
}

unsafe fn pushfuncname<D>(
    L: *mut Thread<D>,
    dst: &mut String,
    ar: &mut lua_Debug<D>,
) -> Result<(), Box<dyn core::error::Error>> {
    if pushglobalfuncname(L, ar)? != 0 {
        let n = lua_tolstring(L, -1, true);
        let n = String::from_utf8_lossy((*n).as_bytes());

        dst.push_str("function '");
        dst.push_str(&n);
        dst.push('\'');

        lua_pop(L, 1)?;
    } else if *(*ar).namewhat as libc::c_int != '\0' as i32 {
        dst.push_str(&CStr::from_ptr((*ar).namewhat).to_string_lossy());
        dst.push_str(" '");
        dst.push_str(&CStr::from_ptr((*ar).name).to_string_lossy());
        dst.push('\'');
    } else if *(*ar).what as libc::c_int == 'm' as i32 {
        dst.push_str("main chunk");
    } else if let Some(v) = &ar.source {
        write!(dst, "function <{}:{}>", v.name(), (*ar).linedefined).unwrap();
    } else {
        dst.push('?');
    }

    Ok(())
}

unsafe fn lastlevel<D>(L: *mut Thread<D>) -> libc::c_int {
    let mut ar = lua_Debug::default();
    let mut li: libc::c_int = 1 as libc::c_int;
    let mut le: libc::c_int = 1 as libc::c_int;
    while lua_getstack(L, le, &mut ar) != 0 {
        li = le;
        le *= 2 as libc::c_int;
    }
    while li < le {
        let m: libc::c_int = (li + le) / 2 as libc::c_int;
        if lua_getstack(L, m, &mut ar) != 0 {
            li = m + 1 as libc::c_int;
        } else {
            le = m;
        }
    }
    return le - 1 as libc::c_int;
}

pub unsafe fn luaL_traceback<D>(
    L: *mut Thread<D>,
    L1: *mut Thread<D>,
    msg: *const c_char,
    mut level: libc::c_int,
) -> Result<(), Box<dyn core::error::Error>> {
    let mut b = String::new();
    let mut ar = lua_Debug::default();
    let last: libc::c_int = lastlevel(L1);
    let mut limit2show: libc::c_int = if last - level > 10 as libc::c_int + 11 as libc::c_int {
        10 as libc::c_int
    } else {
        -(1 as libc::c_int)
    };

    if !msg.is_null() {
        b.push_str(&CStr::from_ptr(msg).to_string_lossy());
        b.push('\n');
    }

    b.push_str("stack traceback:");

    loop {
        let fresh1 = level;
        level = level + 1;
        if !(lua_getstack(L1, fresh1, &mut ar) != 0) {
            break;
        }
        let fresh2 = limit2show;
        limit2show = limit2show - 1;
        if fresh2 == 0 as libc::c_int {
            let n: libc::c_int = last - level - 11 as libc::c_int + 1 as libc::c_int;

            write!(b, "\n\t...\t(skipping {} levels)", n).unwrap();

            level += n;
        } else {
            lua_getinfo(L1, b"Slnt\0" as *const u8 as *const c_char, &mut ar);

            if ar.currentline <= 0 {
                write!(
                    b,
                    "\n\t{}: in ",
                    ar.source.as_ref().map(|v| v.name()).unwrap_or("")
                )
                .unwrap();
            } else {
                write!(
                    b,
                    "\n\t{}:{}: in ",
                    ar.source.as_ref().map(|v| v.name()).unwrap_or(""),
                    ar.currentline,
                )
                .unwrap();
            }

            pushfuncname(L, &mut b, &mut ar)?;

            if ar.istailcall != 0 {
                b.push_str("\n\t(...tail calls...)");
            }
        }
    }

    lua_pushlstring(L, b);
    Ok(())
}

/// `arg` is used only for display.
#[inline(never)]
pub unsafe fn luaL_argerror<D>(
    L: *const Thread<D>,
    mut arg: NonZero<usize>,
    reason: impl Into<Box<dyn core::error::Error>>,
) -> Box<dyn core::error::Error> {
    let mut ar = lua_Debug::default();

    if lua_getstack(L, 0 as libc::c_int, &mut ar) == 0 {
        return Box::new(ArgError {
            message: format!("bad argument #{arg}"),
            reason: reason.into(),
        });
    }

    lua_getinfo(L, b"n\0" as *const u8 as *const c_char, &mut ar);

    if strcmp(ar.namewhat, b"method\0" as *const u8 as *const c_char) == 0 as libc::c_int {
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

#[inline(never)]
pub unsafe fn luaL_typeerror<D>(
    L: *const Thread<D>,
    arg: libc::c_int,
    expect: impl Display,
) -> Box<dyn core::error::Error> {
    let actual = match luaL_getmetafield(L, arg, c"__name".as_ptr()) {
        Ok(4) => String::from_utf8_lossy((*lua_tolstring(L, -1, true)).as_bytes()),
        Ok(_) => lua_typename(lua_type(L, arg)).into(),
        Err(e) => return e,
    };

    return luaL_argerror(
        L,
        arg.try_into().and_then(|v: usize| v.try_into()).unwrap(),
        format!("{expect} expected, got {actual}"),
    );
}

unsafe fn tag_error<D>(
    L: *const Thread<D>,
    arg: libc::c_int,
    tag: libc::c_int,
) -> Box<dyn core::error::Error> {
    luaL_typeerror(L, arg, lua_typename(tag))
}

pub unsafe fn luaL_where<D>(L: *const Thread<D>, level: libc::c_int) -> Cow<'static, str> {
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
    if lua_checkstack(L, space).is_err() {
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

pub unsafe fn luaL_getmetafield<D>(
    L: *const Thread<D>,
    obj: libc::c_int,
    event: *const c_char,
) -> Result<libc::c_int, Box<dyn core::error::Error>> {
    if lua_getmetatable(L, obj) == 0 {
        return Ok(0 as libc::c_int);
    } else {
        let mut tt: libc::c_int = 0;
        lua_pushstring(L, event);
        tt = lua_rawget(L, -(2 as libc::c_int));
        if tt == 0 as libc::c_int {
            lua_settop(L, -(2 as libc::c_int) - 1 as libc::c_int)?;
        } else {
            lua_rotate(L, -(2 as libc::c_int), -(1 as libc::c_int));
            lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
        }
        return Ok(tt);
    };
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
