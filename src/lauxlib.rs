#![allow(
    dead_code,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments,
    unused_mut
)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::lapi::{
    lua_absindex, lua_call, lua_checkstack, lua_concat, lua_copy, lua_createtable, lua_getfield,
    lua_getmetatable, lua_gettop, lua_isinteger, lua_isnumber, lua_isstring, lua_len, lua_next,
    lua_pushboolean, lua_pushcclosure, lua_pushinteger, lua_pushlstring, lua_pushnil,
    lua_pushstring, lua_pushvalue, lua_rawequal, lua_rawget, lua_rotate, lua_setfield,
    lua_setglobal, lua_setmetatable, lua_settop, lua_toboolean, lua_tointegerx, lua_tolstring,
    lua_tonumberx, lua_topointer, lua_touserdata, lua_type, lua_typename,
};
use crate::ldebug::{lua_getinfo, lua_getstack};
use crate::lstate::{CallInfo, lua_Debug};
use crate::{Fp, Thread, lua_pop};
use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use core::ffi::{CStr, c_void};
use core::fmt::{Display, Write};
use core::ptr::null_mut;
use libc::{FILE, strcmp, strlen, strncmp};

#[derive(Copy, Clone)]
#[repr(C)]
pub struct luaL_Reg {
    pub name: *const libc::c_char,
    pub func: Option<Fp>,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct LoadF {
    pub n: libc::c_int,
    pub f: *mut FILE,
    pub buff: [libc::c_char; 1024],
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct LoadS {
    pub s: *const libc::c_char,
    pub size: usize,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct UBox {
    pub box_0: *mut libc::c_void,
    pub bsize: usize,
}

unsafe fn findfield(
    mut L: *const Thread,
    mut objidx: libc::c_int,
    mut level: libc::c_int,
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
                lua_pushstring(L, b".\0" as *const u8 as *const libc::c_char);
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
    mut L: *const Thread,
    mut ar: &mut lua_Debug,
) -> Result<libc::c_int, Box<dyn core::error::Error>> {
    let mut top: libc::c_int = lua_gettop(L);
    lua_getinfo(L, b"f\0" as *const u8 as *const libc::c_char, ar);
    lua_getfield(
        L,
        -(1000000 as libc::c_int) - 1000 as libc::c_int,
        "_LOADED",
    )?;
    luaL_checkstack(
        L,
        6,
        b"not enough stack\0" as *const u8 as *const libc::c_char,
    )?;
    if findfield(L, top + 1 as libc::c_int, 2 as libc::c_int)? != 0 {
        let mut name: *const libc::c_char = lua_tolstring(L, -(1 as libc::c_int), 0 as *mut usize);
        if strncmp(name, b"_G.\0" as *const u8 as *const libc::c_char, 3) == 0 as libc::c_int {
            lua_pushstring(L, name.offset(3 as libc::c_int as isize));
            lua_rotate(L, -(2 as libc::c_int), -(1 as libc::c_int));
            lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
        }
        lua_copy(L, -(1 as libc::c_int), top + 1 as libc::c_int);
        lua_settop(L, top + 1 as libc::c_int)?;
        return Ok(1 as libc::c_int);
    } else {
        lua_settop(L, top)?;
        return Ok(0 as libc::c_int);
    };
}

unsafe fn pushfuncname(
    mut L: *mut Thread,
    dst: &mut String,
    mut ar: &mut lua_Debug,
) -> Result<(), Box<dyn core::error::Error>> {
    if pushglobalfuncname(L, ar)? != 0 {
        let n = CStr::from_ptr(lua_tolstring(L, -1, 0 as *mut usize)).to_string_lossy();

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

unsafe fn lastlevel(mut L: *mut Thread) -> libc::c_int {
    let mut ar: lua_Debug = lua_Debug {
        event: 0,
        name: 0 as *const libc::c_char,
        namewhat: 0 as *const libc::c_char,
        what: 0 as *const libc::c_char,
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
        let mut m: libc::c_int = (li + le) / 2 as libc::c_int;
        if lua_getstack(L, m, &mut ar) != 0 {
            li = m + 1 as libc::c_int;
        } else {
            le = m;
        }
    }
    return le - 1 as libc::c_int;
}

pub unsafe fn luaL_traceback(
    mut L: *mut Thread,
    mut L1: *mut Thread,
    mut msg: *const libc::c_char,
    mut level: libc::c_int,
) -> Result<(), Box<dyn core::error::Error>> {
    let mut b = String::new();
    let mut ar: lua_Debug = lua_Debug {
        event: 0,
        name: 0 as *const libc::c_char,
        namewhat: 0 as *const libc::c_char,
        what: 0 as *const libc::c_char,
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
    let mut last: libc::c_int = lastlevel(L1);
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
            let mut n: libc::c_int = last - level - 11 as libc::c_int + 1 as libc::c_int;

            write!(b, "\n\t...\t(skipping {} levels)", n).unwrap();

            level += n;
        } else {
            lua_getinfo(L1, b"Slnt\0" as *const u8 as *const libc::c_char, &mut ar);

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

pub unsafe fn luaL_argerror(
    mut L: *const Thread,
    mut arg: libc::c_int,
    extramsg: impl Display,
) -> Result<libc::c_int, Box<dyn core::error::Error>> {
    let mut ar: lua_Debug = lua_Debug {
        event: 0,
        name: 0 as *const libc::c_char,
        namewhat: 0 as *const libc::c_char,
        what: 0 as *const libc::c_char,
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
    lua_getinfo(L, b"n\0" as *const u8 as *const libc::c_char, &mut ar);
    if strcmp(ar.namewhat, b"method\0" as *const u8 as *const libc::c_char) == 0 as libc::c_int {
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
        ar.name = if pushglobalfuncname(L, &mut ar)? != 0 {
            lua_tolstring(L, -(1 as libc::c_int), 0 as *mut usize)
        } else {
            b"?\0" as *const u8 as *const libc::c_char
        };
    }
    return luaL_error(
        L,
        format!(
            "bad argument #{arg} to '{}' ({extramsg})",
            CStr::from_ptr(ar.name).to_string_lossy()
        ),
    );
}

pub unsafe fn luaL_typeerror(
    mut L: *const Thread,
    mut arg: libc::c_int,
    expect: impl Display,
) -> Result<libc::c_int, Box<dyn core::error::Error>> {
    let actual = if luaL_getmetafield(L, arg, c"__name".as_ptr())? == 4 {
        CStr::from_ptr(lua_tolstring(L, -1, 0 as *mut usize)).to_string_lossy()
    } else if lua_type(L, arg) == 2 {
        "light userdata".into()
    } else {
        lua_typename(lua_type(L, arg)).into()
    };

    return luaL_argerror(L, arg, format_args!("{expect} expected, got {actual}"));
}

unsafe fn tag_error(
    mut L: *const Thread,
    mut arg: libc::c_int,
    mut tag: libc::c_int,
) -> Result<(), Box<dyn core::error::Error>> {
    luaL_typeerror(L, arg, lua_typename(tag))?;
    Ok(())
}

pub unsafe fn luaL_where(mut L: *const Thread, level: libc::c_int) -> Cow<'static, str> {
    let mut ar: lua_Debug = lua_Debug {
        event: 0,
        name: 0 as *const libc::c_char,
        namewhat: 0 as *const libc::c_char,
        what: 0 as *const libc::c_char,
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
        lua_getinfo(L, b"Sl\0" as *const u8 as *const libc::c_char, &mut ar);

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

pub unsafe fn luaL_error(
    L: *const Thread,
    m: impl Display,
) -> Result<libc::c_int, Box<dyn core::error::Error>> {
    Err(format!("{}{}", luaL_where(L, 1), m).into())
}

pub unsafe fn luaL_fileresult(
    mut L: *mut Thread,
    mut stat: libc::c_int,
    mut fname: *const libc::c_char,
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
    mut L: *mut Thread,
    mut tname: *const libc::c_char,
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
        b"__name\0" as *const u8 as *const libc::c_char,
    )?;
    lua_pushvalue(L, -(1 as libc::c_int));
    lua_setfield(L, -(1000000 as libc::c_int) - 1000 as libc::c_int, tname)?;
    return Ok(1 as libc::c_int);
}

pub unsafe fn luaL_setmetatable(
    mut L: *mut Thread,
    mut tname: *const libc::c_char,
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
    mut L: *mut Thread,
    mut ud: libc::c_int,
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
    mut L: *mut Thread,
    mut ud: libc::c_int,
    name: &str,
) -> Result<*mut libc::c_void, Box<dyn core::error::Error>> {
    let mut p: *mut libc::c_void = luaL_testudata(L, ud, name)?;
    (((p != 0 as *mut libc::c_void) as libc::c_int != 0 as libc::c_int) as libc::c_int
        as libc::c_long
        != 0
        || luaL_typeerror(L, ud, name)? != 0) as libc::c_int;
    return Ok(p);
}

pub unsafe fn luaL_checkoption<'a>(
    mut L: *mut Thread,
    mut arg: libc::c_int,
    mut def: *const libc::c_char,
    lst: impl IntoIterator<Item = &'a str>,
) -> Result<libc::c_int, Box<dyn core::error::Error>> {
    let name: *const libc::c_char = if !def.is_null() {
        luaL_optlstring(L, arg, def, 0 as *mut usize)?
    } else {
        luaL_checklstring(L, arg, 0 as *mut usize)?
    };

    let name = CStr::from_ptr(name);

    if let Some(i) = lst
        .into_iter()
        .position(|v| v.as_bytes() == name.to_bytes())
    {
        return Ok(i.try_into().unwrap());
    }

    return luaL_argerror(
        L,
        arg,
        format_args!("invalid option '{}'", name.to_string_lossy()),
    );
}

pub unsafe fn luaL_checkstack(
    mut L: *const Thread,
    space: usize,
    mut msg: *const libc::c_char,
) -> Result<(), Box<dyn core::error::Error>> {
    if lua_checkstack(L, space).is_err() {
        if !msg.is_null() {
            luaL_error(
                L,
                format!("stack overflow ({})", CStr::from_ptr(msg).to_string_lossy()),
            )?;
        } else {
            luaL_error(L, "stack overflow")?;
        }
    }

    Ok(())
}

pub unsafe fn luaL_checktype(
    mut L: *const Thread,
    mut arg: libc::c_int,
    mut t: libc::c_int,
) -> Result<(), Box<dyn core::error::Error>> {
    if ((lua_type(L, arg) != t) as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        tag_error(L, arg, t)?;
    }
    Ok(())
}

pub unsafe fn luaL_checkany(
    mut L: *const Thread,
    mut arg: libc::c_int,
) -> Result<(), Box<dyn core::error::Error>> {
    if ((lua_type(L, arg) == -(1 as libc::c_int)) as libc::c_int != 0 as libc::c_int) as libc::c_int
        as libc::c_long
        != 0
    {
        luaL_argerror(L, arg, "value expected")?;
    }
    Ok(())
}

pub unsafe fn luaL_checklstring(
    mut L: *const Thread,
    mut arg: libc::c_int,
    mut len: *mut usize,
) -> Result<*const libc::c_char, Box<dyn core::error::Error>> {
    let mut s: *const libc::c_char = lua_tolstring(L, arg, len);
    if (s.is_null() as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long != 0 {
        tag_error(L, arg, 4 as libc::c_int)?;
    }
    return Ok(s);
}

pub unsafe fn luaL_optlstring(
    mut L: *const Thread,
    mut arg: libc::c_int,
    mut def: *const libc::c_char,
    mut len: *mut usize,
) -> Result<*const libc::c_char, Box<dyn core::error::Error>> {
    if lua_type(L, arg) <= 0 as libc::c_int {
        if !len.is_null() {
            *len = if !def.is_null() { strlen(def) } else { 0 };
        }
        return Ok(def);
    } else {
        return luaL_checklstring(L, arg, len);
    };
}

pub unsafe fn luaL_checknumber(
    mut L: *const Thread,
    mut arg: libc::c_int,
) -> Result<f64, Box<dyn core::error::Error>> {
    let mut isnum: libc::c_int = 0;
    let mut d: f64 = lua_tonumberx(L, arg, &mut isnum);
    if ((isnum == 0) as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long != 0 {
        tag_error(L, arg, 3 as libc::c_int)?;
    }
    return Ok(d);
}

pub unsafe fn luaL_optnumber(
    mut L: *const Thread,
    mut arg: libc::c_int,
    mut def: f64,
) -> Result<f64, Box<dyn core::error::Error>> {
    return if lua_type(L, arg) <= 0 as libc::c_int {
        Ok(def)
    } else {
        luaL_checknumber(L, arg)
    };
}

unsafe fn interror(
    mut L: *const Thread,
    mut arg: libc::c_int,
) -> Result<(), Box<dyn core::error::Error>> {
    if lua_isnumber(L, arg) != 0 {
        luaL_argerror(L, arg, "number has no integer representation")?;
    } else {
        tag_error(L, arg, 3 as libc::c_int)?;
    };
    Ok(())
}

pub unsafe fn luaL_checkinteger(
    mut L: *const Thread,
    mut arg: libc::c_int,
) -> Result<i64, Box<dyn core::error::Error>> {
    let mut isnum: libc::c_int = 0;
    let mut d: i64 = lua_tointegerx(L, arg, &mut isnum);
    if ((isnum == 0) as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long != 0 {
        interror(L, arg)?;
    }
    return Ok(d);
}

pub unsafe fn luaL_optinteger(
    mut L: *const Thread,
    mut arg: libc::c_int,
    mut def: i64,
) -> Result<i64, Box<dyn core::error::Error>> {
    return if lua_type(L, arg) <= 0 as libc::c_int {
        Ok(def)
    } else {
        luaL_checkinteger(L, arg)
    };
}

unsafe fn getS(
    ud: *mut c_void,
    mut size: *mut usize,
) -> Result<*const libc::c_char, Box<dyn core::error::Error>> {
    let mut ls: *mut LoadS = ud as *mut LoadS;
    if (*ls).size == 0 as libc::c_int as usize {
        return Ok(0 as *const libc::c_char);
    }
    *size = (*ls).size;
    (*ls).size = 0 as libc::c_int as usize;
    return Ok((*ls).s);
}

pub unsafe fn luaL_getmetafield(
    mut L: *const Thread,
    mut obj: libc::c_int,
    mut event: *const libc::c_char,
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
    mut L: *const Thread,
    mut obj: libc::c_int,
    mut event: *const libc::c_char,
) -> Result<libc::c_int, Box<dyn core::error::Error>> {
    obj = lua_absindex(L, obj);

    if luaL_getmetafield(L, obj, event)? == 0 as libc::c_int {
        return Ok(0 as libc::c_int);
    }

    lua_pushvalue(L, obj);
    lua_call(L, 1, 1)?;

    return Ok(1);
}

pub unsafe fn luaL_len(
    mut L: *const Thread,
    mut idx: libc::c_int,
) -> Result<i64, Box<dyn core::error::Error>> {
    let mut l: i64 = 0;
    let mut isnum: libc::c_int = 0;
    lua_len(L, idx)?;
    l = lua_tointegerx(L, -(1 as libc::c_int), &mut isnum);
    if ((isnum == 0) as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long != 0 {
        luaL_error(L, "object length is not an integer")?;
    }
    lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
    return Ok(l);
}

pub unsafe fn luaL_tolstring(
    mut L: *const Thread,
    mut idx: libc::c_int,
    mut len: *mut usize,
) -> Result<*const libc::c_char, Box<dyn core::error::Error>> {
    idx = lua_absindex(L, idx);

    if luaL_callmeta(L, idx, b"__tostring\0" as *const u8 as *const libc::c_char)? != 0 {
        if lua_isstring(L, -(1 as libc::c_int)) == 0 {
            luaL_error(L, "'__tostring' must return a string")?;
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
                        b"true\0" as *const u8 as *const libc::c_char
                    } else {
                        b"false\0" as *const u8 as *const libc::c_char
                    },
                );
            }
            0 => {
                lua_pushstring(L, b"nil\0" as *const u8 as *const libc::c_char);
            }
            _ => {
                let mut tt = luaL_getmetafield(L, idx, c"__name".as_ptr())?;
                let kind = if tt == 4 {
                    CStr::from_ptr(lua_tolstring(L, -1, 0 as *mut usize)).to_string_lossy()
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

    Ok(lua_tolstring(L, -(1 as libc::c_int), len))
}

pub unsafe fn luaL_setfuncs(
    mut L: *const Thread,
    mut l: *const luaL_Reg,
    mut nup: libc::c_int,
) -> Result<(), Box<dyn core::error::Error>> {
    luaL_checkstack(
        L,
        nup.try_into().unwrap(),
        b"too many upvalues\0" as *const u8 as *const libc::c_char,
    )?;

    while !((*l).name).is_null() {
        match (*l).func {
            Some(f) => {
                let mut i: libc::c_int = 0;

                while i < nup {
                    lua_pushvalue(L, -nup);
                    i += 1;
                }

                lua_pushcclosure(L, f, nup);
            }
            None => lua_pushboolean(L, 0),
        }

        lua_setfield(L, -(nup + 2 as libc::c_int), (*l).name)?;
        l = l.offset(1);
    }

    lua_settop(L, -nup - 1 as libc::c_int)
}

pub unsafe fn luaL_getsubtable(
    mut L: *const Thread,
    mut idx: libc::c_int,
    mut fname: *const libc::c_char,
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

pub unsafe fn luaL_requiref(
    mut L: *const Thread,
    mut modname: *const libc::c_char,
    mut openf: Fp,
    mut glb: libc::c_int,
) -> Result<(), Box<dyn core::error::Error>> {
    luaL_getsubtable(
        L,
        -(1000000 as libc::c_int) - 1000 as libc::c_int,
        b"_LOADED\0" as *const u8 as *const libc::c_char,
    )?;
    lua_getfield(L, -(1 as libc::c_int), CStr::from_ptr(modname).to_bytes())?;
    if lua_toboolean(L, -(1 as libc::c_int)) == 0 {
        lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
        lua_pushcclosure(L, openf, 0 as libc::c_int);
        lua_pushstring(L, modname);
        lua_call(L, 1, 1)?;
        lua_pushvalue(L, -(1 as libc::c_int));
        lua_setfield(L, -(3 as libc::c_int), modname)?;
    }
    lua_rotate(L, -(2 as libc::c_int), -(1 as libc::c_int));
    lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
    if glb != 0 {
        lua_pushvalue(L, -(1 as libc::c_int));
        lua_setglobal(L, modname)?;
    }
    Ok(())
}
