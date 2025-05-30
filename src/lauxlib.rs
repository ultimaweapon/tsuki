#![allow(
    dead_code,
    mutable_transmutes,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments,
    unused_mut
)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::lapi::{
    lua_absindex, lua_call, lua_checkstack, lua_closeslot, lua_concat, lua_copy, lua_createtable,
    lua_getfield, lua_getmetatable, lua_gettop, lua_isinteger, lua_isnumber, lua_isstring, lua_len,
    lua_load, lua_newuserdatauv, lua_next, lua_pushboolean, lua_pushcclosure, lua_pushinteger,
    lua_pushlightuserdata, lua_pushlstring, lua_pushnil, lua_pushstring, lua_pushvalue,
    lua_rawequal, lua_rawget, lua_rawgeti, lua_rawlen, lua_rawseti, lua_rotate, lua_setfield,
    lua_setglobal, lua_setmetatable, lua_settop, lua_toboolean, lua_toclose, lua_tointegerx,
    lua_tolstring, lua_tonumberx, lua_topointer, lua_touserdata, lua_type, lua_typename,
};
use crate::ldebug::{lua_getinfo, lua_getstack};
use crate::lstate::{CallInfo, lua_CFunction, lua_Debug};
use crate::{Thread, lua_pop};
use libc::{FILE, free, memcpy, realloc, strcmp, strlen, strncmp, strstr};
use std::borrow::Cow;
use std::ffi::{CStr, c_char, c_int, c_void};
use std::fmt::Display;
use std::ptr::{null, null_mut};

#[derive(Copy, Clone)]
#[repr(C)]
pub struct luaL_Buffer {
    pub(crate) b: *mut libc::c_char,
    pub(crate) size: usize,
    pub(crate) n: usize,
    pub(crate) L: *mut Thread,
    pub(crate) init: C2RustUnnamed,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub union C2RustUnnamed {
    pub n: f64,
    pub u: libc::c_double,
    pub s: *mut libc::c_void,
    pub i: i64,
    pub l: libc::c_long,
    pub b: [libc::c_char; 1024],
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct luaL_Reg {
    pub name: *const libc::c_char,
    pub func: Option<lua_CFunction>,
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
    mut L: *mut Thread,
    mut objidx: libc::c_int,
    mut level: libc::c_int,
) -> Result<c_int, Box<dyn std::error::Error>> {
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
                lua_pushstring(L, b".\0" as *const u8 as *const libc::c_char)?;
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
    mut L: *mut Thread,
    mut ar: *mut lua_Debug,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut top: libc::c_int = lua_gettop(L);
    lua_getinfo(L, b"f\0" as *const u8 as *const libc::c_char, ar)?;
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
        let mut name: *const libc::c_char = lua_tolstring(L, -(1 as libc::c_int), 0 as *mut usize)?;
        if strncmp(name, b"_G.\0" as *const u8 as *const libc::c_char, 3) == 0 as libc::c_int {
            lua_pushstring(L, name.offset(3 as libc::c_int as isize))?;
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
    mut ar: *mut lua_Debug,
) -> Result<(), Box<dyn std::error::Error>> {
    if pushglobalfuncname(L, ar)? != 0 {
        lua_pushlstring(
            L,
            format!(
                "function '{}'",
                CStr::from_ptr(lua_tolstring(L, -(1 as libc::c_int), 0 as *mut usize)?)
                    .to_string_lossy(),
            ),
        )?;
        lua_rotate(L, -(2 as libc::c_int), -(1 as libc::c_int));
        lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
    } else if *(*ar).namewhat as libc::c_int != '\0' as i32 {
        lua_pushlstring(
            L,
            format!(
                "{} '{}'",
                CStr::from_ptr((*ar).namewhat).to_string_lossy(),
                CStr::from_ptr((*ar).name).to_string_lossy(),
            ),
        )?;
    } else if *(*ar).what as libc::c_int == 'm' as i32 {
        lua_pushstring(L, b"main chunk\0" as *const u8 as *const libc::c_char)?;
    } else if *(*ar).what as libc::c_int != 'C' as i32 {
        lua_pushlstring(
            L,
            format!(
                "function <{}:{}>",
                CStr::from_ptr(((*ar).short_src).as_mut_ptr()).to_string_lossy(),
                (*ar).linedefined,
            ),
        )?;
    } else {
        lua_pushstring(L, b"?\0" as *const u8 as *const libc::c_char)?;
    };

    Ok(())
}

unsafe extern "C" fn lastlevel(mut L: *mut Thread) -> libc::c_int {
    let mut ar: lua_Debug = lua_Debug {
        event: 0,
        name: 0 as *const libc::c_char,
        namewhat: 0 as *const libc::c_char,
        what: 0 as *const libc::c_char,
        source: 0 as *const libc::c_char,
        srclen: 0,
        currentline: 0,
        linedefined: 0,
        lastlinedefined: 0,
        nups: 0,
        nparams: 0,
        isvararg: 0,
        istailcall: 0,
        ftransfer: 0,
        ntransfer: 0,
        short_src: [0; 60],
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
) -> Result<(), Box<dyn std::error::Error>> {
    let mut b: luaL_Buffer = luaL_Buffer {
        b: 0 as *mut libc::c_char,
        size: 0,
        n: 0,
        L: 0 as *mut Thread,
        init: C2RustUnnamed { n: 0. },
    };
    let mut ar: lua_Debug = lua_Debug {
        event: 0,
        name: 0 as *const libc::c_char,
        namewhat: 0 as *const libc::c_char,
        what: 0 as *const libc::c_char,
        source: 0 as *const libc::c_char,
        srclen: 0,
        currentline: 0,
        linedefined: 0,
        lastlinedefined: 0,
        nups: 0,
        nparams: 0,
        isvararg: 0,
        istailcall: 0,
        ftransfer: 0,
        ntransfer: 0,
        short_src: [0; 60],
        i_ci: 0 as *mut CallInfo,
    };
    let mut last: libc::c_int = lastlevel(L1);
    let mut limit2show: libc::c_int = if last - level > 10 as libc::c_int + 11 as libc::c_int {
        10 as libc::c_int
    } else {
        -(1 as libc::c_int)
    };
    luaL_buffinit(L, &mut b);
    if !msg.is_null() {
        luaL_addstring(&mut b, msg)?;
        (b.n < b.size || !luaL_prepbuffsize(&mut b, 1)?.is_null()) as libc::c_int;
        let fresh0 = b.n;
        b.n = (b.n).wrapping_add(1);
        *(b.b).offset(fresh0 as isize) = '\n' as i32 as libc::c_char;
    }
    luaL_addstring(
        &mut b,
        b"stack traceback:\0" as *const u8 as *const libc::c_char,
    )?;
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

            lua_pushlstring(L, format!("\n\t...\t(skipping {} levels)", n))?;
            luaL_addvalue(&mut b)?;

            level += n;
        } else {
            lua_getinfo(L1, b"Slnt\0" as *const u8 as *const libc::c_char, &mut ar)?;

            if ar.currentline <= 0 {
                lua_pushlstring(
                    L,
                    format!(
                        "\n\t{}: in \0",
                        CStr::from_ptr((ar.short_src).as_mut_ptr()).to_string_lossy()
                    ),
                )?;
            } else {
                lua_pushlstring(
                    L,
                    format!(
                        "\n\t{}:{}: in ",
                        CStr::from_ptr((ar.short_src).as_mut_ptr()).to_string_lossy(),
                        ar.currentline,
                    ),
                )?;
            }

            luaL_addvalue(&mut b)?;
            pushfuncname(L, &mut ar)?;
            luaL_addvalue(&mut b)?;
            if ar.istailcall != 0 {
                luaL_addstring(
                    &mut b,
                    b"\n\t(...tail calls...)\0" as *const u8 as *const libc::c_char,
                )?;
            }
        }
    }
    luaL_pushresult(&mut b)
}

pub unsafe fn luaL_argerror(
    mut L: *mut Thread,
    mut arg: libc::c_int,
    extramsg: impl Display,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut ar: lua_Debug = lua_Debug {
        event: 0,
        name: 0 as *const libc::c_char,
        namewhat: 0 as *const libc::c_char,
        what: 0 as *const libc::c_char,
        source: 0 as *const libc::c_char,
        srclen: 0,
        currentline: 0,
        linedefined: 0,
        lastlinedefined: 0,
        nups: 0,
        nparams: 0,
        isvararg: 0,
        istailcall: 0,
        ftransfer: 0,
        ntransfer: 0,
        short_src: [0; 60],
        i_ci: 0 as *mut CallInfo,
    };
    if lua_getstack(L, 0 as libc::c_int, &mut ar) == 0 {
        return luaL_error(L, format!("bad argument #{arg} ({extramsg})"));
    }
    lua_getinfo(L, b"n\0" as *const u8 as *const libc::c_char, &mut ar)?;
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
            lua_tolstring(L, -(1 as libc::c_int), 0 as *mut usize)?
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
    mut L: *mut Thread,
    mut arg: libc::c_int,
    expect: impl Display,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let actual = if luaL_getmetafield(L, arg, c"__name".as_ptr())? == 4 {
        CStr::from_ptr(lua_tolstring(L, -1, 0 as *mut usize)?).to_string_lossy()
    } else if lua_type(L, arg) == 2 {
        "light userdata".into()
    } else {
        lua_typename(lua_type(L, arg)).into()
    };

    return luaL_argerror(L, arg, format_args!("{expect} expected, got {actual}"));
}

unsafe fn tag_error(
    mut L: *mut Thread,
    mut arg: libc::c_int,
    mut tag: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    luaL_typeerror(L, arg, lua_typename(tag))?;
    Ok(())
}

pub unsafe fn luaL_where(
    mut L: *mut Thread,
    level: c_int,
) -> Result<Cow<'static, str>, Box<dyn std::error::Error>> {
    let mut ar: lua_Debug = lua_Debug {
        event: 0,
        name: 0 as *const libc::c_char,
        namewhat: 0 as *const libc::c_char,
        what: 0 as *const libc::c_char,
        source: 0 as *const libc::c_char,
        srclen: 0,
        currentline: 0,
        linedefined: 0,
        lastlinedefined: 0,
        nups: 0,
        nparams: 0,
        isvararg: 0,
        istailcall: 0,
        ftransfer: 0,
        ntransfer: 0,
        short_src: [0; 60],
        i_ci: 0 as *mut CallInfo,
    };

    if lua_getstack(L, level, &mut ar) != 0 {
        lua_getinfo(L, b"Sl\0" as *const u8 as *const libc::c_char, &mut ar)?;

        if ar.currentline > 0 {
            return Ok(format!(
                "{}:{}: ",
                CStr::from_ptr(ar.short_src.as_ptr()).to_string_lossy(),
                ar.currentline,
            )
            .into());
        }
    }

    Ok("".into())
}

pub unsafe fn luaL_error(
    L: *mut Thread,
    m: impl Display,
) -> Result<c_int, Box<dyn std::error::Error>> {
    Err(format!("{}{}", luaL_where(L, 1)?, m).into())
}

pub unsafe fn luaL_fileresult(
    mut L: *mut Thread,
    mut stat: libc::c_int,
    mut fname: *const libc::c_char,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let en = std::io::Error::last_os_error();

    if stat != 0 {
        lua_pushboolean(L, 1 as libc::c_int);
        return Ok(1 as libc::c_int);
    } else {
        lua_pushnil(L);

        if !fname.is_null() {
            lua_pushlstring(
                L,
                format!("{}: {}", CStr::from_ptr(fname).to_string_lossy(), en),
            )?;
        } else {
            lua_pushlstring(L, en.to_string())?;
        }

        lua_pushinteger(L, en.raw_os_error().unwrap().into());
        return Ok(3 as libc::c_int);
    };
}

pub unsafe fn luaL_newmetatable(
    mut L: *mut Thread,
    mut tname: *const libc::c_char,
) -> Result<c_int, Box<dyn std::error::Error>> {
    if lua_getfield(
        L,
        -(1000000 as libc::c_int) - 1000 as libc::c_int,
        CStr::from_ptr(tname).to_bytes(),
    )? != 0
    {
        return Ok(0 as libc::c_int);
    }
    lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
    lua_createtable(L, 0 as libc::c_int, 2 as libc::c_int)?;
    lua_pushstring(L, tname)?;
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
) -> Result<(), Box<dyn std::error::Error>> {
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
) -> Result<*mut libc::c_void, Box<dyn std::error::Error>> {
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
) -> Result<*mut libc::c_void, Box<dyn std::error::Error>> {
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
) -> Result<c_int, Box<dyn std::error::Error>> {
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
    mut L: *mut Thread,
    space: usize,
    mut msg: *const libc::c_char,
) -> Result<(), Box<dyn std::error::Error>> {
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
    mut L: *mut Thread,
    mut arg: libc::c_int,
    mut t: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    if ((lua_type(L, arg) != t) as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        tag_error(L, arg, t)?;
    }
    Ok(())
}

pub unsafe fn luaL_checkany(
    mut L: *mut Thread,
    mut arg: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    if ((lua_type(L, arg) == -(1 as libc::c_int)) as libc::c_int != 0 as libc::c_int) as libc::c_int
        as libc::c_long
        != 0
    {
        luaL_argerror(L, arg, "value expected")?;
    }
    Ok(())
}

pub unsafe fn luaL_checklstring(
    mut L: *mut Thread,
    mut arg: libc::c_int,
    mut len: *mut usize,
) -> Result<*const libc::c_char, Box<dyn std::error::Error>> {
    let mut s: *const libc::c_char = lua_tolstring(L, arg, len)?;
    if (s.is_null() as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long != 0 {
        tag_error(L, arg, 4 as libc::c_int)?;
    }
    return Ok(s);
}

pub unsafe fn luaL_optlstring(
    mut L: *mut Thread,
    mut arg: libc::c_int,
    mut def: *const libc::c_char,
    mut len: *mut usize,
) -> Result<*const libc::c_char, Box<dyn std::error::Error>> {
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
    mut L: *mut Thread,
    mut arg: libc::c_int,
) -> Result<f64, Box<dyn std::error::Error>> {
    let mut isnum: libc::c_int = 0;
    let mut d: f64 = lua_tonumberx(L, arg, &mut isnum);
    if ((isnum == 0) as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long != 0 {
        tag_error(L, arg, 3 as libc::c_int)?;
    }
    return Ok(d);
}

pub unsafe fn luaL_optnumber(
    mut L: *mut Thread,
    mut arg: libc::c_int,
    mut def: f64,
) -> Result<f64, Box<dyn std::error::Error>> {
    return if lua_type(L, arg) <= 0 as libc::c_int {
        Ok(def)
    } else {
        luaL_checknumber(L, arg)
    };
}

unsafe fn interror(
    mut L: *mut Thread,
    mut arg: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    if lua_isnumber(L, arg) != 0 {
        luaL_argerror(L, arg, "number has no integer representation")?;
    } else {
        tag_error(L, arg, 3 as libc::c_int)?;
    };
    Ok(())
}

pub unsafe fn luaL_checkinteger(
    mut L: *mut Thread,
    mut arg: libc::c_int,
) -> Result<i64, Box<dyn std::error::Error>> {
    let mut isnum: libc::c_int = 0;
    let mut d: i64 = lua_tointegerx(L, arg, &mut isnum);
    if ((isnum == 0) as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long != 0 {
        interror(L, arg)?;
    }
    return Ok(d);
}

pub unsafe fn luaL_optinteger(
    mut L: *mut Thread,
    mut arg: libc::c_int,
    mut def: i64,
) -> Result<i64, Box<dyn std::error::Error>> {
    return if lua_type(L, arg) <= 0 as libc::c_int {
        Ok(def)
    } else {
        luaL_checkinteger(L, arg)
    };
}

unsafe fn resizebox(
    mut L: *mut Thread,
    mut idx: libc::c_int,
    mut newsize: usize,
) -> Result<*mut c_void, Box<dyn std::error::Error>> {
    let mut box_0: *mut UBox = lua_touserdata(L, idx) as *mut UBox;
    let temp = if newsize == 0 {
        free((*box_0).box_0);
        0 as *mut libc::c_void
    } else {
        realloc((*box_0).box_0, newsize)
    };

    if ((temp.is_null() && newsize > 0 as libc::c_int as usize) as libc::c_int != 0 as libc::c_int)
        as libc::c_int as libc::c_long
        != 0
    {
        return Err("not enough memory".into());
    }
    (*box_0).box_0 = temp;
    (*box_0).bsize = newsize;
    return Ok(temp);
}

unsafe fn boxgc(mut L: *mut Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    resizebox(L, 1 as libc::c_int, 0 as libc::c_int as usize)?;
    Ok(0)
}

static mut boxmt: [luaL_Reg; 2] = [
    {
        let mut init = luaL_Reg {
            name: b"__close\0" as *const u8 as *const libc::c_char,
            func: Some(boxgc),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: 0 as *const libc::c_char,
            func: None,
        };
        init
    },
];

unsafe fn newbox(mut L: *mut Thread) -> Result<(), Box<dyn std::error::Error>> {
    let mut box_0 = lua_newuserdatauv(L, ::core::mem::size_of::<UBox>(), 0)? as *mut UBox;
    (*box_0).box_0 = 0 as *mut libc::c_void;
    (*box_0).bsize = 0 as libc::c_int as usize;
    if luaL_newmetatable(L, b"_UBOX*\0" as *const u8 as *const libc::c_char)? != 0 {
        luaL_setfuncs(L, &raw const boxmt as *const luaL_Reg, 0)?;
    }
    lua_setmetatable(L, -(2 as libc::c_int)).unwrap();
    Ok(())
}

unsafe fn newbuffsize(
    mut B: *mut luaL_Buffer,
    mut sz: usize,
) -> Result<usize, Box<dyn std::error::Error>> {
    let mut newsize: usize = (*B).size / 2 as libc::c_int as usize * 3 as libc::c_int as usize;
    if (((!(0 as libc::c_int as usize)).wrapping_sub(sz) < (*B).n) as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        return luaL_error((*B).L, "buffer too large").map(|v| v as usize);
    }
    if newsize < ((*B).n).wrapping_add(sz) {
        newsize = ((*B).n).wrapping_add(sz);
    }
    return Ok(newsize);
}

unsafe fn prepbuffsize(
    mut B: *mut luaL_Buffer,
    mut sz: usize,
    mut boxidx: libc::c_int,
) -> Result<*mut libc::c_char, Box<dyn std::error::Error>> {
    if ((*B).size).wrapping_sub((*B).n) >= sz {
        return Ok(((*B).b).offset((*B).n as isize));
    } else {
        let mut L: *mut Thread = (*B).L;
        let mut newbuff: *mut libc::c_char = 0 as *mut libc::c_char;
        let mut newsize: usize = newbuffsize(B, sz)?;
        if (*B).b != ((*B).init.b).as_mut_ptr() {
            newbuff = resizebox(L, boxidx, newsize)? as *mut libc::c_char;
        } else {
            lua_rotate(L, boxidx, -(1 as libc::c_int));
            lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
            newbox(L)?;
            lua_rotate(L, boxidx, 1 as libc::c_int);
            lua_toclose(L, boxidx)?;
            newbuff = resizebox(L, boxidx, newsize)? as *mut libc::c_char;
            memcpy(
                newbuff as *mut libc::c_void,
                (*B).b as *const libc::c_void,
                ((*B).n).wrapping_mul(::core::mem::size_of::<libc::c_char>()),
            );
        }
        (*B).b = newbuff;
        (*B).size = newsize;
        return Ok(newbuff.offset((*B).n as isize));
    };
}

pub unsafe fn luaL_prepbuffsize(
    mut B: *mut luaL_Buffer,
    mut sz: usize,
) -> Result<*mut c_char, Box<dyn std::error::Error>> {
    return prepbuffsize(B, sz, -(1 as libc::c_int));
}

pub unsafe fn luaL_addlstring(
    B: *mut luaL_Buffer,
    s: *const c_char,
    l: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    if l > 0 {
        let b = prepbuffsize(B, l, -1)?;
        memcpy(b.cast(), s.cast(), l);
        (*B).n += l;
    }
    Ok(())
}

pub unsafe fn luaL_addstring(
    mut B: *mut luaL_Buffer,
    mut s: *const libc::c_char,
) -> Result<(), Box<dyn std::error::Error>> {
    luaL_addlstring(B, s, strlen(s))
}

pub unsafe fn luaL_pushresult(mut B: *mut luaL_Buffer) -> Result<(), Box<dyn std::error::Error>> {
    let mut L: *mut Thread = (*B).L;
    lua_pushlstring(L, std::slice::from_raw_parts((*B).b.cast(), (*B).n))?;
    if (*B).b != ((*B).init.b).as_mut_ptr() {
        lua_closeslot(L, -(2 as libc::c_int))?;
    }
    lua_rotate(L, -(2 as libc::c_int), -(1 as libc::c_int));
    lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)
}

pub unsafe fn luaL_pushresultsize(
    mut B: *mut luaL_Buffer,
    mut sz: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    (*B).n = ((*B).n).wrapping_add(sz);
    luaL_pushresult(B)
}

pub unsafe fn luaL_addvalue(mut B: *mut luaL_Buffer) -> Result<(), Box<dyn std::error::Error>> {
    let mut L: *mut Thread = (*B).L;
    let mut len: usize = 0;
    let mut s: *const libc::c_char = lua_tolstring(L, -(1 as libc::c_int), &mut len)?;
    let mut b: *mut libc::c_char = prepbuffsize(B, len, -(2 as libc::c_int))?;
    memcpy(
        b as *mut libc::c_void,
        s as *const libc::c_void,
        len.wrapping_mul(::core::mem::size_of::<libc::c_char>()),
    );
    (*B).n = ((*B).n).wrapping_add(len);
    lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)
}

pub unsafe fn luaL_buffinit(mut L: *mut Thread, mut B: *mut luaL_Buffer) {
    (*B).L = L;
    (*B).b = ((*B).init.b).as_mut_ptr();
    (*B).n = 0 as libc::c_int as usize;
    (*B).size = (16 as libc::c_int as libc::c_ulong)
        .wrapping_mul(::core::mem::size_of::<*mut libc::c_void>() as libc::c_ulong)
        .wrapping_mul(::core::mem::size_of::<f64>() as libc::c_ulong) as libc::c_int
        as usize;
    lua_pushlightuserdata(L, B as *mut libc::c_void);
}

pub unsafe fn luaL_buffinitsize(
    mut L: *mut Thread,
    mut B: *mut luaL_Buffer,
    mut sz: usize,
) -> Result<*mut c_char, Box<dyn std::error::Error>> {
    luaL_buffinit(L, B);
    return prepbuffsize(B, sz, -(1 as libc::c_int));
}

pub unsafe fn luaL_ref(
    mut L: *mut Thread,
    mut t: libc::c_int,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut ref_0: libc::c_int = 0;
    if lua_type(L, -(1 as libc::c_int)) == 0 as libc::c_int {
        lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
        return Ok(-(1 as libc::c_int));
    }
    t = lua_absindex(L, t);
    if lua_rawgeti(L, t, (2 as libc::c_int + 1 as libc::c_int) as i64) == 0 as libc::c_int {
        ref_0 = 0 as libc::c_int;
        lua_pushinteger(L, 0 as libc::c_int as i64);
        lua_rawseti(L, t, (2 as libc::c_int + 1 as libc::c_int) as i64)?;
    } else {
        ref_0 = lua_tointegerx(L, -(1 as libc::c_int), 0 as *mut libc::c_int) as libc::c_int;
    }
    lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
    if ref_0 != 0 as libc::c_int {
        lua_rawgeti(L, t, ref_0 as i64);
        lua_rawseti(L, t, (2 as libc::c_int + 1 as libc::c_int) as i64)?;
    } else {
        ref_0 = lua_rawlen(L, t) as libc::c_int + 1 as libc::c_int;
    }
    lua_rawseti(L, t, ref_0 as i64)?;
    return Ok(ref_0);
}

pub unsafe fn luaL_unref(
    mut L: *mut Thread,
    mut t: libc::c_int,
    mut ref_0: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    if ref_0 >= 0 as libc::c_int {
        t = lua_absindex(L, t);
        lua_rawgeti(L, t, (2 as libc::c_int + 1 as libc::c_int) as i64);
        lua_rawseti(L, t, ref_0 as i64)?;
        lua_pushinteger(L, ref_0 as i64);
        lua_rawseti(L, t, (2 as libc::c_int + 1 as libc::c_int) as i64)?;
    }
    Ok(())
}

pub unsafe fn luaL_loadfilex(
    L: *mut Thread,
    filename: *const c_char,
    mode: *const c_char,
) -> Result<(), Box<dyn std::error::Error>> {
    todo!()
}

unsafe fn getS(
    ud: *mut c_void,
    mut size: *mut usize,
) -> Result<*const c_char, Box<dyn std::error::Error>> {
    let mut ls: *mut LoadS = ud as *mut LoadS;
    if (*ls).size == 0 as libc::c_int as usize {
        return Ok(0 as *const libc::c_char);
    }
    *size = (*ls).size;
    (*ls).size = 0 as libc::c_int as usize;
    return Ok((*ls).s);
}

pub unsafe fn luaL_loadbufferx(
    L: *mut Thread,
    chunk: impl AsRef<[u8]>,
    name: *const c_char,
    mode: *const c_char,
) -> Result<(), Box<dyn std::error::Error>> {
    let chunk = chunk.as_ref();
    let mut ls = LoadS {
        s: chunk.as_ptr().cast(),
        size: chunk.len(),
    };

    lua_load(L, getS, &mut ls as *mut LoadS as *mut c_void, name, mode)
}

pub unsafe fn luaL_loadstring(
    L: *mut Thread,
    s: *const c_char,
) -> Result<(), Box<dyn std::error::Error>> {
    luaL_loadbufferx(L, CStr::from_ptr(s).to_bytes(), s, null())
}

pub unsafe fn luaL_getmetafield(
    mut L: *mut Thread,
    mut obj: libc::c_int,
    mut event: *const libc::c_char,
) -> Result<c_int, Box<dyn std::error::Error>> {
    if lua_getmetatable(L, obj) == 0 {
        return Ok(0 as libc::c_int);
    } else {
        let mut tt: libc::c_int = 0;
        lua_pushstring(L, event)?;
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
    mut L: *mut Thread,
    mut obj: libc::c_int,
    mut event: *const libc::c_char,
) -> Result<c_int, Box<dyn std::error::Error>> {
    obj = lua_absindex(L, obj);

    if luaL_getmetafield(L, obj, event)? == 0 as libc::c_int {
        return Ok(0 as libc::c_int);
    }

    lua_pushvalue(L, obj);
    lua_call(L, 1, 1)?;

    return Ok(1);
}

pub unsafe fn luaL_len(
    mut L: *mut Thread,
    mut idx: libc::c_int,
) -> Result<i64, Box<dyn std::error::Error>> {
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
    mut L: *mut Thread,
    mut idx: libc::c_int,
    mut len: *mut usize,
) -> Result<*const libc::c_char, Box<dyn std::error::Error>> {
    idx = lua_absindex(L, idx);

    if luaL_callmeta(L, idx, b"__tostring\0" as *const u8 as *const libc::c_char)? != 0 {
        if lua_isstring(L, -(1 as libc::c_int)) == 0 {
            luaL_error(L, "'__tostring' must return a string")?;
        }
    } else {
        match lua_type(L, idx) {
            3 => {
                if lua_isinteger(L, idx) != 0 {
                    lua_pushlstring(L, lua_tointegerx(L, idx, 0 as *mut libc::c_int).to_string())?;
                } else {
                    // Lua expect 0.0 as "0.0". The problem is there is no way to force Rust to
                    // output "0.0" so we need to do this manually.
                    let v = lua_tonumberx(L, idx, null_mut());

                    if v.fract() == 0.0 {
                        lua_pushlstring(L, format!("{v:.1}"))?;
                    } else {
                        lua_pushlstring(L, v.to_string())?;
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
                )?;
            }
            0 => {
                lua_pushstring(L, b"nil\0" as *const u8 as *const libc::c_char)?;
            }
            _ => {
                let mut tt: libc::c_int =
                    luaL_getmetafield(L, idx, b"__name\0" as *const u8 as *const libc::c_char)?;
                let kind = if tt == 4 {
                    CStr::from_ptr(lua_tolstring(L, -1, 0 as *mut usize)?).to_string_lossy()
                } else {
                    lua_typename(lua_type(L, idx)).into()
                };

                lua_pushlstring(L, format!("{}: {:p}", kind, lua_topointer(L, idx)))?;

                if tt != 0 as libc::c_int {
                    lua_rotate(L, -(2 as libc::c_int), -(1 as libc::c_int));
                    lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
                }
            }
        }
    }

    lua_tolstring(L, -(1 as libc::c_int), len)
}

pub unsafe fn luaL_setfuncs(
    mut L: *mut Thread,
    mut l: *const luaL_Reg,
    mut nup: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
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
    mut L: *mut Thread,
    mut idx: libc::c_int,
    mut fname: *const libc::c_char,
) -> Result<c_int, Box<dyn std::error::Error>> {
    if lua_getfield(L, idx, CStr::from_ptr(fname).to_bytes())? == 5 {
        return Ok(1 as libc::c_int);
    } else {
        lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
        idx = lua_absindex(L, idx);
        lua_createtable(L, 0 as libc::c_int, 0 as libc::c_int)?;
        lua_pushvalue(L, -(1 as libc::c_int));
        lua_setfield(L, idx, fname)?;
        return Ok(0 as libc::c_int);
    };
}

pub unsafe fn luaL_requiref(
    mut L: *mut Thread,
    mut modname: *const libc::c_char,
    mut openf: lua_CFunction,
    mut glb: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    luaL_getsubtable(
        L,
        -(1000000 as libc::c_int) - 1000 as libc::c_int,
        b"_LOADED\0" as *const u8 as *const libc::c_char,
    )?;
    lua_getfield(L, -(1 as libc::c_int), CStr::from_ptr(modname).to_bytes())?;
    if lua_toboolean(L, -(1 as libc::c_int)) == 0 {
        lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
        lua_pushcclosure(L, openf, 0 as libc::c_int);
        lua_pushstring(L, modname)?;
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

pub unsafe fn luaL_addgsub(
    mut b: *mut luaL_Buffer,
    mut s: *const libc::c_char,
    mut p: *const libc::c_char,
    mut r: *const libc::c_char,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut wild: *const libc::c_char = 0 as *const libc::c_char;
    let mut l: usize = strlen(p);
    loop {
        wild = strstr(s, p);
        if wild.is_null() {
            break;
        }
        luaL_addlstring(b, s, wild.offset_from(s) as libc::c_long as usize)?;
        luaL_addstring(b, r)?;
        s = wild.offset(l as isize);
    }
    luaL_addstring(b, s)
}

pub unsafe fn luaL_gsub(
    mut L: *mut Thread,
    mut s: *const libc::c_char,
    mut p: *const libc::c_char,
    mut r: *const libc::c_char,
) -> Result<*const libc::c_char, Box<dyn std::error::Error>> {
    let mut b: luaL_Buffer = luaL_Buffer {
        b: 0 as *mut libc::c_char,
        size: 0,
        n: 0,
        L: 0 as *mut Thread,
        init: C2RustUnnamed { n: 0. },
    };

    luaL_buffinit(L, &mut b);
    luaL_addgsub(&mut b, s, p, r)?;
    luaL_pushresult(&mut b)?;

    lua_tolstring(L, -(1 as libc::c_int), 0 as *mut usize)
}
