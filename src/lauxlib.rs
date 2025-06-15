#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments
)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::lapi::{
    lua_absindex, lua_call, lua_checkstack, lua_concat, lua_copy, lua_createtable, lua_getfield,
    lua_getmetatable, lua_gettop, lua_isinteger, lua_isstring, lua_len, lua_next, lua_pushboolean,
    lua_pushinteger, lua_pushlstring, lua_pushnil, lua_pushstring, lua_pushvalue, lua_rawequal,
    lua_rawget, lua_rotate, lua_setfield, lua_setmetatable, lua_settop, lua_toboolean,
    lua_tointegerx, lua_tolstring, lua_tonumberx, lua_topointer, lua_touserdata, lua_type,
    lua_typename,
};
use crate::ldebug::{lua_getinfo, lua_getstack};
use crate::lstate::{CallInfo, lua_Debug};
use crate::{NON_YIELDABLE_WAKER, Str, Thread, lua_pop};
use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use core::ffi::{CStr, c_char, c_void};
use core::fmt::{Display, Write};
use core::pin::pin;
use core::ptr::{null, null_mut};
use core::task::{Context, Poll, Waker};
use libc::strcmp;

#[derive(Copy, Clone)]
#[repr(C)]
pub struct LoadS {
    pub s: *const c_char,
    pub size: usize,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct UBox {
    pub box_0: *mut libc::c_void,
    pub bsize: usize,
}

unsafe fn findfield(
    L: *const Thread,
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

unsafe fn pushglobalfuncname(
    L: *const Thread,
    ar: &mut lua_Debug,
) -> Result<libc::c_int, Box<dyn core::error::Error>> {
    let top: libc::c_int = lua_gettop(L);
    lua_getinfo(L, b"f\0" as *const u8 as *const c_char, ar);
    lua_getfield(
        L,
        -(1000000 as libc::c_int) - 1000 as libc::c_int,
        "_LOADED",
    )?;
    luaL_checkstack(L, 6, b"not enough stack\0" as *const u8 as *const c_char)?;

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

unsafe fn pushfuncname(
    L: *mut Thread,
    dst: &mut String,
    ar: &mut lua_Debug,
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

unsafe fn lastlevel(L: *mut Thread) -> libc::c_int {
    let mut ar: lua_Debug = lua_Debug {
        event: 0,
        name: 0 as *const c_char,
        namewhat: 0 as *const c_char,
        what: 0 as *const c_char,
        source: None,
        currentline: 0,
        linedefined: 0,
        lastlinedefined: 0,
        nups: 0,
        nparams: 0,
        isvararg: 0,
        istailcall: 0,
        ftransfer: 0,
        ntransfer: 0,
        i_ci: 0 as *mut CallInfo,
    };
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

pub unsafe fn luaL_traceback(
    L: *mut Thread,
    L1: *mut Thread,
    msg: *const c_char,
    mut level: libc::c_int,
) -> Result<(), Box<dyn core::error::Error>> {
    let mut b = String::new();
    let mut ar: lua_Debug = lua_Debug {
        event: 0,
        name: 0 as *const c_char,
        namewhat: 0 as *const c_char,
        what: 0 as *const c_char,
        source: None,
        currentline: 0,
        linedefined: 0,
        lastlinedefined: 0,
        nups: 0,
        nparams: 0,
        isvararg: 0,
        istailcall: 0,
        ftransfer: 0,
        ntransfer: 0,
        i_ci: 0 as *mut CallInfo,
    };
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

#[inline(never)]
pub unsafe fn luaL_argerror(
    L: *const Thread,
    mut arg: libc::c_int,
    extramsg: impl Display,
) -> Box<dyn core::error::Error> {
    let mut ar: lua_Debug = lua_Debug {
        event: 0,
        name: 0 as *const c_char,
        namewhat: 0 as *const c_char,
        what: 0 as *const c_char,
        source: None,
        currentline: 0,
        linedefined: 0,
        lastlinedefined: 0,
        nups: 0,
        nparams: 0,
        isvararg: 0,
        istailcall: 0,
        ftransfer: 0,
        ntransfer: 0,
        i_ci: 0 as *mut CallInfo,
    };
    if lua_getstack(L, 0 as libc::c_int, &mut ar) == 0 {
        return luaL_error(L, format!("bad argument #{arg} ({extramsg})"));
    }
    lua_getinfo(L, b"n\0" as *const u8 as *const c_char, &mut ar);
    if strcmp(ar.namewhat, b"method\0" as *const u8 as *const c_char) == 0 as libc::c_int {
        arg -= 1;

        if arg == 0 as libc::c_int {
            return luaL_error(
                L,
                format!(
                    "calling '{}' on bad self ({extramsg})",
                    CStr::from_ptr(ar.name).to_string_lossy()
                ),
            );
        }
    }
    if (ar.name).is_null() {
        ar.name = match pushglobalfuncname(L, &mut ar) {
            Ok(0) => b"?\0" as *const u8 as *const c_char,
            Ok(_) => (*lua_tolstring(L, -1, true)).contents.as_ptr(),
            Err(e) => return e,
        };
    }

    luaL_error(
        L,
        format!(
            "bad argument #{arg} to '{}' ({extramsg})",
            CStr::from_ptr(ar.name).to_string_lossy()
        ),
    )
}

#[inline(never)]
pub unsafe fn luaL_typeerror(
    L: *const Thread,
    arg: libc::c_int,
    expect: impl Display,
) -> Box<dyn core::error::Error> {
    let actual = match luaL_getmetafield(L, arg, c"__name".as_ptr()) {
        Ok(4) => String::from_utf8_lossy((*lua_tolstring(L, -1, true)).as_bytes()),
        Ok(_) => lua_typename(lua_type(L, arg)).into(),
        Err(e) => return e,
    };

    return luaL_argerror(L, arg, format_args!("{expect} expected, got {actual}"));
}

unsafe fn tag_error(
    L: *const Thread,
    arg: libc::c_int,
    tag: libc::c_int,
) -> Box<dyn core::error::Error> {
    luaL_typeerror(L, arg, lua_typename(tag))
}

pub unsafe fn luaL_where(L: *const Thread, level: libc::c_int) -> Cow<'static, str> {
    let mut ar: lua_Debug = lua_Debug {
        event: 0,
        name: 0 as *const c_char,
        namewhat: 0 as *const c_char,
        what: 0 as *const c_char,
        source: None,
        currentline: 0,
        linedefined: 0,
        lastlinedefined: 0,
        nups: 0,
        nparams: 0,
        isvararg: 0,
        istailcall: 0,
        ftransfer: 0,
        ntransfer: 0,
        i_ci: 0 as *mut CallInfo,
    };

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

pub unsafe fn luaL_error(L: *const Thread, m: impl Display) -> Box<dyn core::error::Error> {
    format!("{}{}", luaL_where(L, 1), m).into()
}

pub unsafe fn luaL_fileresult(
    L: *mut Thread,
    stat: libc::c_int,
    fname: *const c_char,
) -> libc::c_int {
    let en = std::io::Error::last_os_error();

    if stat != 0 {
        lua_pushboolean(L, 1 as libc::c_int);
        return 1 as libc::c_int;
    } else {
        lua_pushnil(L);

        if !fname.is_null() {
            lua_pushlstring(
                L,
                format!("{}: {}", CStr::from_ptr(fname).to_string_lossy(), en),
            );
        } else {
            lua_pushlstring(L, en.to_string());
        }

        lua_pushinteger(L, en.raw_os_error().unwrap().into());
        return 3 as libc::c_int;
    };
}

pub unsafe fn luaL_newmetatable(
    L: *mut Thread,
    tname: *const c_char,
) -> Result<libc::c_int, Box<dyn core::error::Error>> {
    if lua_getfield(
        L,
        -(1000000 as libc::c_int) - 1000 as libc::c_int,
        CStr::from_ptr(tname).to_bytes(),
    )? != 0
    {
        return Ok(0 as libc::c_int);
    }
    lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
    lua_createtable(L, 0 as libc::c_int, 2 as libc::c_int);
    lua_pushstring(L, tname);
    lua_setfield(
        L,
        -(2 as libc::c_int),
        b"__name\0" as *const u8 as *const c_char,
    )?;
    lua_pushvalue(L, -(1 as libc::c_int));
    lua_setfield(L, -(1000000 as libc::c_int) - 1000 as libc::c_int, tname)?;
    return Ok(1 as libc::c_int);
}

pub unsafe fn luaL_setmetatable(
    L: *mut Thread,
    tname: *const c_char,
) -> Result<(), Box<dyn core::error::Error>> {
    lua_getfield(
        L,
        -(1000000 as libc::c_int) - 1000 as libc::c_int,
        CStr::from_ptr(tname).to_bytes(),
    )?;

    if let Err(e) = lua_setmetatable(L, -2) {
        lua_pop(L, 1)?;
        return Err(e);
    }

    Ok(())
}

pub unsafe fn luaL_testudata(
    L: *mut Thread,
    ud: libc::c_int,
    tname: &str,
) -> Result<*mut libc::c_void, Box<dyn core::error::Error>> {
    let mut p: *mut libc::c_void = lua_touserdata(L, ud);
    if !p.is_null() {
        if lua_getmetatable(L, ud) != 0 {
            lua_getfield(L, -(1000000 as libc::c_int) - 1000 as libc::c_int, tname)?;
            if lua_rawequal(L, -(1 as libc::c_int), -(2 as libc::c_int))? == 0 {
                p = 0 as *mut libc::c_void;
            }
            lua_settop(L, -(2 as libc::c_int) - 1 as libc::c_int)?;
            return Ok(p);
        }
    }
    return Ok(0 as *mut libc::c_void);
}

pub unsafe fn luaL_checkudata(
    L: *mut Thread,
    ud: libc::c_int,
    name: &str,
) -> Result<*mut libc::c_void, Box<dyn core::error::Error>> {
    let p: *mut libc::c_void = luaL_testudata(L, ud, name)?;

    if p != 0 as *mut libc::c_void {
        Ok(p)
    } else {
        Err(luaL_typeerror(L, ud, name))
    }
}

pub unsafe fn luaL_checkstack(
    L: *const Thread,
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

pub unsafe fn luaL_checktype(
    L: *const Thread,
    arg: libc::c_int,
    t: libc::c_int,
) -> Result<(), Box<dyn core::error::Error>> {
    if ((lua_type(L, arg) != t) as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        return Err(tag_error(L, arg, t));
    }
    Ok(())
}

pub unsafe fn luaL_checkany(
    L: *const Thread,
    arg: libc::c_int,
) -> Result<(), Box<dyn core::error::Error>> {
    if ((lua_type(L, arg) == -(1 as libc::c_int)) as libc::c_int != 0 as libc::c_int) as libc::c_int
        as libc::c_long
        != 0
    {
        return Err(luaL_argerror(L, arg, "value expected"));
    }
    Ok(())
}

pub unsafe fn luaL_checknumber(
    L: *const Thread,
    arg: libc::c_int,
) -> Result<f64, Box<dyn core::error::Error>> {
    let mut isnum: libc::c_int = 0;
    let d: f64 = lua_tonumberx(L, arg, &mut isnum);
    if ((isnum == 0) as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long != 0 {
        return Err(tag_error(L, arg, 3 as libc::c_int));
    }
    return Ok(d);
}

pub unsafe fn luaL_optnumber(
    L: *const Thread,
    arg: libc::c_int,
    def: f64,
) -> Result<f64, Box<dyn core::error::Error>> {
    return if lua_type(L, arg) <= 0 as libc::c_int {
        Ok(def)
    } else {
        luaL_checknumber(L, arg)
    };
}

unsafe fn getS(
    ud: *mut c_void,
    size: *mut usize,
) -> Result<*const c_char, Box<dyn core::error::Error>> {
    let ls: *mut LoadS = ud as *mut LoadS;
    if (*ls).size == 0 as libc::c_int as usize {
        return Ok(0 as *const c_char);
    }
    *size = (*ls).size;
    (*ls).size = 0 as libc::c_int as usize;
    return Ok((*ls).s);
}

pub unsafe fn luaL_getmetafield(
    L: *const Thread,
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

pub unsafe fn luaL_callmeta(
    L: *const Thread,
    mut obj: libc::c_int,
    event: *const c_char,
) -> Result<libc::c_int, Box<dyn core::error::Error>> {
    obj = lua_absindex(L, obj);

    if luaL_getmetafield(L, obj, event)? == 0 as libc::c_int {
        return Ok(0 as libc::c_int);
    }

    lua_pushvalue(L, obj);

    // Invoke.
    let f = pin!(lua_call(L, 1, 1));
    let w = Waker::new(null(), &NON_YIELDABLE_WAKER);

    match f.poll(&mut Context::from_waker(&w)) {
        Poll::Ready(v) => v?,
        Poll::Pending => unreachable!(),
    }

    return Ok(1);
}

pub unsafe fn luaL_len(
    L: *const Thread,
    idx: libc::c_int,
) -> Result<i64, Box<dyn core::error::Error>> {
    let mut l: i64 = 0;
    let mut isnum: libc::c_int = 0;
    lua_len(L, idx)?;
    l = lua_tointegerx(L, -(1 as libc::c_int), &mut isnum);
    if ((isnum == 0) as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long != 0 {
        return Err(luaL_error(L, "object length is not an integer"));
    }
    lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
    return Ok(l);
}

#[inline(never)]
pub unsafe fn luaL_tolstring(
    L: *const Thread,
    mut idx: libc::c_int,
) -> Result<*const Str, Box<dyn core::error::Error>> {
    idx = lua_absindex(L, idx);

    if luaL_callmeta(L, idx, b"__tostring\0" as *const u8 as *const c_char)? != 0 {
        if lua_isstring(L, -(1 as libc::c_int)) == 0 {
            return Err(luaL_error(L, "'__tostring' must return a string"));
        }
    } else {
        match lua_type(L, idx) {
            3 => {
                if lua_isinteger(L, idx) != 0 {
                    lua_pushlstring(L, lua_tointegerx(L, idx, 0 as *mut libc::c_int).to_string());
                } else {
                    // Lua expect 0.0 as "0.0". The problem is there is no way to force Rust to
                    // output "0.0" so we need to do this manually.
                    let v = lua_tonumberx(L, idx, null_mut());

                    if v.fract() == 0.0 {
                        lua_pushlstring(L, format!("{v:.1}"));
                    } else {
                        lua_pushlstring(L, v.to_string());
                    }
                }
            }
            4 => lua_pushvalue(L, idx),
            1 => {
                lua_pushstring(
                    L,
                    if lua_toboolean(L, idx) != 0 {
                        b"true\0" as *const u8 as *const c_char
                    } else {
                        b"false\0" as *const u8 as *const c_char
                    },
                );
            }
            0 => {
                lua_pushstring(L, b"nil\0" as *const u8 as *const c_char);
            }
            _ => {
                let tt = luaL_getmetafield(L, idx, c"__name".as_ptr())?;
                let kind = if tt == 4 {
                    String::from_utf8_lossy((*lua_tolstring(L, -1, true)).as_bytes())
                } else {
                    lua_typename(lua_type(L, idx)).into()
                };

                lua_pushlstring(L, format!("{}: {:p}", kind, lua_topointer(L, idx)));

                if tt != 0 as libc::c_int {
                    lua_rotate(L, -(2 as libc::c_int), -(1 as libc::c_int));
                    lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
                }
            }
        }
    }

    Ok(lua_tolstring(L, -1, false))
}

pub unsafe fn luaL_getsubtable(
    L: *const Thread,
    mut idx: libc::c_int,
    fname: *const c_char,
) -> Result<libc::c_int, Box<dyn core::error::Error>> {
    if lua_getfield(L, idx, CStr::from_ptr(fname).to_bytes())? == 5 {
        return Ok(1 as libc::c_int);
    } else {
        lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
        idx = lua_absindex(L, idx);
        lua_createtable(L, 0 as libc::c_int, 0 as libc::c_int);
        lua_pushvalue(L, -(1 as libc::c_int));
        lua_setfield(L, idx, fname)?;
        return Ok(0 as libc::c_int);
    };
}
