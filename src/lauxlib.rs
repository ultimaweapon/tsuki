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
#![allow(unused_variables)]

use crate::lapi::{
    lua_absindex, lua_callk, lua_checkstack, lua_closeslot, lua_concat, lua_copy, lua_createtable,
    lua_error, lua_getallocf, lua_getfield, lua_getmetatable, lua_gettop, lua_isinteger,
    lua_isnumber, lua_isstring, lua_len, lua_load, lua_newuserdatauv, lua_next, lua_pushboolean,
    lua_pushcclosure, lua_pushinteger, lua_pushlightuserdata, lua_pushlstring, lua_pushnil,
    lua_pushstring, lua_pushvalue, lua_rawequal, lua_rawget, lua_rawgeti, lua_rawlen, lua_rawseti,
    lua_rotate, lua_setfield, lua_setglobal, lua_setmetatable, lua_settop, lua_setwarnf,
    lua_toboolean, lua_toclose, lua_tointegerx, lua_tolstring, lua_tonumberx, lua_topointer,
    lua_touserdata, lua_type, lua_typename, lua_version,
};
use crate::ldebug::{lua_getinfo, lua_getstack};
use crate::lstate::{
    CallInfo, lua_Alloc, lua_CFunction, lua_Debug, lua_KContext, lua_State, lua_newstate,
};
use libc::{FILE, free, memcpy, realloc, strcmp, strlen, strncmp, strstr};
use std::ffi::{CStr, c_char, c_int, c_void};
use std::fmt::Display;
use std::ptr::null;

#[derive(Copy, Clone)]
#[repr(C)]
pub struct luaL_Buffer {
    pub b: *mut libc::c_char,
    pub size: usize,
    pub n: usize,
    pub L: *mut lua_State,
    pub init: C2RustUnnamed,
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

unsafe extern "C" fn findfield(
    mut L: *mut lua_State,
    mut objidx: libc::c_int,
    mut level: libc::c_int,
) -> libc::c_int {
    if level == 0 as libc::c_int || !(lua_type(L, -(1 as libc::c_int)) == 5 as libc::c_int) {
        return 0 as libc::c_int;
    }
    lua_pushnil(L);
    while lua_next(L, -(2 as libc::c_int)) != 0 {
        if lua_type(L, -(2 as libc::c_int)) == 4 as libc::c_int {
            if lua_rawequal(L, objidx, -(1 as libc::c_int)) != 0 {
                lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int);
                return 1 as libc::c_int;
            } else if findfield(L, objidx, level - 1 as libc::c_int) != 0 {
                lua_pushstring(L, b".\0" as *const u8 as *const libc::c_char);
                lua_copy(L, -(1 as libc::c_int), -(3 as libc::c_int));
                lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int);
                lua_concat(L, 3 as libc::c_int);
                return 1 as libc::c_int;
            }
        }
        lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int);
    }
    return 0 as libc::c_int;
}

unsafe extern "C" fn pushglobalfuncname(
    mut L: *mut lua_State,
    mut ar: *mut lua_Debug,
) -> libc::c_int {
    let mut top: libc::c_int = lua_gettop(L);
    lua_getinfo(L, b"f\0" as *const u8 as *const libc::c_char, ar);
    lua_getfield(
        L,
        -(1000000 as libc::c_int) - 1000 as libc::c_int,
        "_LOADED",
    );
    luaL_checkstack(
        L,
        6 as libc::c_int,
        b"not enough stack\0" as *const u8 as *const libc::c_char,
    );
    if findfield(L, top + 1 as libc::c_int, 2 as libc::c_int) != 0 {
        let mut name: *const libc::c_char = lua_tolstring(L, -(1 as libc::c_int), 0 as *mut usize);
        if strncmp(name, b"_G.\0" as *const u8 as *const libc::c_char, 3) == 0 as libc::c_int {
            lua_pushstring(L, name.offset(3 as libc::c_int as isize));
            lua_rotate(L, -(2 as libc::c_int), -(1 as libc::c_int));
            lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int);
        }
        lua_copy(L, -(1 as libc::c_int), top + 1 as libc::c_int);
        lua_settop(L, top + 1 as libc::c_int);
        return 1 as libc::c_int;
    } else {
        lua_settop(L, top);
        return 0 as libc::c_int;
    };
}

unsafe extern "C" fn pushfuncname(mut L: *mut lua_State, mut ar: *mut lua_Debug) {
    if pushglobalfuncname(L, ar) != 0 {
        lua_pushlstring(
            L,
            format!(
                "function '{}'",
                CStr::from_ptr(lua_tolstring(L, -(1 as libc::c_int), 0 as *mut usize))
                    .to_string_lossy(),
            ),
        );
        lua_rotate(L, -(2 as libc::c_int), -(1 as libc::c_int));
        lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int);
    } else if *(*ar).namewhat as libc::c_int != '\0' as i32 {
        lua_pushlstring(
            L,
            format!(
                "{} '{}'",
                CStr::from_ptr((*ar).namewhat).to_string_lossy(),
                CStr::from_ptr((*ar).name).to_string_lossy(),
            ),
        );
    } else if *(*ar).what as libc::c_int == 'm' as i32 {
        lua_pushstring(L, b"main chunk\0" as *const u8 as *const libc::c_char);
    } else if *(*ar).what as libc::c_int != 'C' as i32 {
        lua_pushlstring(
            L,
            format!(
                "function <{}:{}>",
                CStr::from_ptr(((*ar).short_src).as_mut_ptr()).to_string_lossy(),
                (*ar).linedefined,
            ),
        );
    } else {
        lua_pushstring(L, b"?\0" as *const u8 as *const libc::c_char);
    };
}

unsafe extern "C" fn lastlevel(mut L: *mut lua_State) -> libc::c_int {
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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaL_traceback(
    mut L: *mut lua_State,
    mut L1: *mut lua_State,
    mut msg: *const libc::c_char,
    mut level: libc::c_int,
) {
    let mut b: luaL_Buffer = luaL_Buffer {
        b: 0 as *mut libc::c_char,
        size: 0,
        n: 0,
        L: 0 as *mut lua_State,
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
        luaL_addstring(&mut b, msg);
        (b.n < b.size || !(luaL_prepbuffsize(&mut b, 1 as libc::c_int as usize)).is_null())
            as libc::c_int;
        let fresh0 = b.n;
        b.n = (b.n).wrapping_add(1);
        *(b.b).offset(fresh0 as isize) = '\n' as i32 as libc::c_char;
    }
    luaL_addstring(
        &mut b,
        b"stack traceback:\0" as *const u8 as *const libc::c_char,
    );
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

            lua_pushlstring(L, format!("\n\t...\t(skipping {} levels)", n));
            luaL_addvalue(&mut b);

            level += n;
        } else {
            lua_getinfo(L1, b"Slnt\0" as *const u8 as *const libc::c_char, &mut ar);

            if ar.currentline <= 0 {
                lua_pushlstring(
                    L,
                    format!(
                        "\n\t{}: in \0",
                        CStr::from_ptr((ar.short_src).as_mut_ptr()).to_string_lossy()
                    ),
                );
            } else {
                lua_pushlstring(
                    L,
                    format!(
                        "\n\t{}:{}: in ",
                        CStr::from_ptr((ar.short_src).as_mut_ptr()).to_string_lossy(),
                        ar.currentline,
                    ),
                );
            }

            luaL_addvalue(&mut b);
            pushfuncname(L, &mut ar);
            luaL_addvalue(&mut b);
            if ar.istailcall != 0 {
                luaL_addstring(
                    &mut b,
                    b"\n\t(...tail calls...)\0" as *const u8 as *const libc::c_char,
                );
            }
        }
    }
    luaL_pushresult(&mut b);
}

pub unsafe extern "C" fn luaL_argerror(
    mut L: *mut lua_State,
    mut arg: libc::c_int,
    extramsg: impl Display,
) -> libc::c_int {
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
        ar.name = if pushglobalfuncname(L, &mut ar) != 0 {
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

pub unsafe extern "C" fn luaL_typeerror(
    mut L: *mut lua_State,
    mut arg: libc::c_int,
    expect: impl Display,
) -> libc::c_int {
    let actual = if luaL_getmetafield(L, arg, c"__name".as_ptr()) == 4 {
        CStr::from_ptr(lua_tolstring(L, -1, 0 as *mut usize)).to_string_lossy()
    } else if lua_type(L, arg) == 2 {
        "light userdata".into()
    } else {
        lua_typename(L, lua_type(L, arg)).into()
    };

    return luaL_argerror(L, arg, format_args!("{expect} expected, got {actual}"));
}

unsafe extern "C" fn tag_error(mut L: *mut lua_State, mut arg: libc::c_int, mut tag: libc::c_int) {
    luaL_typeerror(L, arg, lua_typename(L, tag));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaL_where(mut L: *mut lua_State, mut level: libc::c_int) {
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
        lua_getinfo(L, b"Sl\0" as *const u8 as *const libc::c_char, &mut ar);

        if ar.currentline > 0 {
            lua_pushlstring(
                L,
                format!(
                    "{}:{}: ",
                    CStr::from_ptr((ar.short_src).as_mut_ptr()).to_string_lossy(),
                    ar.currentline,
                ),
            );
            return;
        }
    }

    lua_pushstring(L, b"\0" as *const u8 as *const libc::c_char);
}

pub unsafe extern "C" fn luaL_error(mut L: *mut lua_State, m: impl AsRef<str>) -> libc::c_int {
    luaL_where(L, 1 as libc::c_int);
    lua_pushlstring(L, m.as_ref());
    lua_concat(L, 2 as libc::c_int);
    return lua_error(L);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaL_fileresult(
    mut L: *mut lua_State,
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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaL_newmetatable(
    mut L: *mut lua_State,
    mut tname: *const libc::c_char,
) -> libc::c_int {
    if lua_getfield(
        L,
        -(1000000 as libc::c_int) - 1000 as libc::c_int,
        CStr::from_ptr(tname).to_bytes(),
    ) != 0 as libc::c_int
    {
        return 0 as libc::c_int;
    }
    lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int);
    lua_createtable(L, 0 as libc::c_int, 2 as libc::c_int);
    lua_pushstring(L, tname);
    lua_setfield(
        L,
        -(2 as libc::c_int),
        b"__name\0" as *const u8 as *const libc::c_char,
    );
    lua_pushvalue(L, -(1 as libc::c_int));
    lua_setfield(L, -(1000000 as libc::c_int) - 1000 as libc::c_int, tname);
    return 1 as libc::c_int;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaL_setmetatable(mut L: *mut lua_State, mut tname: *const libc::c_char) {
    lua_getfield(
        L,
        -(1000000 as libc::c_int) - 1000 as libc::c_int,
        CStr::from_ptr(tname).to_bytes(),
    );
    lua_setmetatable(L, -(2 as libc::c_int));
}

#[unsafe(no_mangle)]
pub unsafe fn luaL_testudata(
    mut L: *mut lua_State,
    mut ud: libc::c_int,
    tname: &str,
) -> *mut libc::c_void {
    let mut p: *mut libc::c_void = lua_touserdata(L, ud);
    if !p.is_null() {
        if lua_getmetatable(L, ud) != 0 {
            lua_getfield(L, -(1000000 as libc::c_int) - 1000 as libc::c_int, tname);
            if lua_rawequal(L, -(1 as libc::c_int), -(2 as libc::c_int)) == 0 {
                p = 0 as *mut libc::c_void;
            }
            lua_settop(L, -(2 as libc::c_int) - 1 as libc::c_int);
            return p;
        }
    }
    return 0 as *mut libc::c_void;
}

#[unsafe(no_mangle)]
pub unsafe fn luaL_checkudata(
    mut L: *mut lua_State,
    mut ud: libc::c_int,
    name: &str,
) -> *mut libc::c_void {
    let mut p: *mut libc::c_void = luaL_testudata(L, ud, name);
    (((p != 0 as *mut libc::c_void) as libc::c_int != 0 as libc::c_int) as libc::c_int
        as libc::c_long
        != 0
        || luaL_typeerror(L, ud, name) != 0) as libc::c_int;
    return p;
}

pub unsafe extern "C" fn luaL_checkoption<'a>(
    mut L: *mut lua_State,
    mut arg: libc::c_int,
    mut def: *const libc::c_char,
    lst: impl IntoIterator<Item = &'a str>,
) -> libc::c_int {
    let name: *const libc::c_char = if !def.is_null() {
        luaL_optlstring(L, arg, def, 0 as *mut usize)
    } else {
        luaL_checklstring(L, arg, 0 as *mut usize)
    };

    let name = CStr::from_ptr(name);

    if let Some(i) = lst
        .into_iter()
        .position(|v| v.as_bytes() == name.to_bytes())
    {
        return i.try_into().unwrap();
    }

    return luaL_argerror(
        L,
        arg,
        format_args!("invalid option '{}'", name.to_string_lossy()),
    );
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaL_checkstack(
    mut L: *mut lua_State,
    mut space: libc::c_int,
    mut msg: *const libc::c_char,
) {
    if ((lua_checkstack(L, space) == 0) as libc::c_int != 0 as libc::c_int) as libc::c_int
        as libc::c_long
        != 0
    {
        if !msg.is_null() {
            luaL_error(
                L,
                format!("stack overflow ({})", CStr::from_ptr(msg).to_string_lossy()),
            );
        } else {
            luaL_error(L, "stack overflow");
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaL_checktype(
    mut L: *mut lua_State,
    mut arg: libc::c_int,
    mut t: libc::c_int,
) {
    if ((lua_type(L, arg) != t) as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        tag_error(L, arg, t);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaL_checkany(mut L: *mut lua_State, mut arg: libc::c_int) {
    if ((lua_type(L, arg) == -(1 as libc::c_int)) as libc::c_int != 0 as libc::c_int) as libc::c_int
        as libc::c_long
        != 0
    {
        luaL_argerror(L, arg, "value expected");
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaL_checklstring(
    mut L: *mut lua_State,
    mut arg: libc::c_int,
    mut len: *mut usize,
) -> *const libc::c_char {
    let mut s: *const libc::c_char = lua_tolstring(L, arg, len);
    if (s.is_null() as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long != 0 {
        tag_error(L, arg, 4 as libc::c_int);
    }
    return s;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaL_optlstring(
    mut L: *mut lua_State,
    mut arg: libc::c_int,
    mut def: *const libc::c_char,
    mut len: *mut usize,
) -> *const libc::c_char {
    if lua_type(L, arg) <= 0 as libc::c_int {
        if !len.is_null() {
            *len = if !def.is_null() { strlen(def) } else { 0 };
        }
        return def;
    } else {
        return luaL_checklstring(L, arg, len);
    };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaL_checknumber(mut L: *mut lua_State, mut arg: libc::c_int) -> f64 {
    let mut isnum: libc::c_int = 0;
    let mut d: f64 = lua_tonumberx(L, arg, &mut isnum);
    if ((isnum == 0) as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long != 0 {
        tag_error(L, arg, 3 as libc::c_int);
    }
    return d;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaL_optnumber(
    mut L: *mut lua_State,
    mut arg: libc::c_int,
    mut def: f64,
) -> f64 {
    return if lua_type(L, arg) <= 0 as libc::c_int {
        def
    } else {
        luaL_checknumber(L, arg)
    };
}

unsafe extern "C" fn interror(mut L: *mut lua_State, mut arg: libc::c_int) {
    if lua_isnumber(L, arg) != 0 {
        luaL_argerror(L, arg, "number has no integer representation");
    } else {
        tag_error(L, arg, 3 as libc::c_int);
    };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaL_checkinteger(mut L: *mut lua_State, mut arg: libc::c_int) -> i64 {
    let mut isnum: libc::c_int = 0;
    let mut d: i64 = lua_tointegerx(L, arg, &mut isnum);
    if ((isnum == 0) as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long != 0 {
        interror(L, arg);
    }
    return d;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaL_optinteger(
    mut L: *mut lua_State,
    mut arg: libc::c_int,
    mut def: i64,
) -> i64 {
    return if lua_type(L, arg) <= 0 as libc::c_int {
        def
    } else {
        luaL_checkinteger(L, arg)
    };
}

unsafe extern "C" fn resizebox(
    mut L: *mut lua_State,
    mut idx: libc::c_int,
    mut newsize: usize,
) -> *mut libc::c_void {
    let mut ud: *mut libc::c_void = 0 as *mut libc::c_void;
    let mut allocf: lua_Alloc = lua_getallocf(L, &mut ud);
    let mut box_0: *mut UBox = lua_touserdata(L, idx) as *mut UBox;
    let mut temp: *mut libc::c_void =
        allocf.expect("non-null function pointer")(ud, (*box_0).box_0, (*box_0).bsize, newsize);
    if ((temp.is_null() && newsize > 0 as libc::c_int as usize) as libc::c_int != 0 as libc::c_int)
        as libc::c_int as libc::c_long
        != 0
    {
        lua_pushstring(
            L,
            b"not enough memory\0" as *const u8 as *const libc::c_char,
        );
        lua_error(L);
    }
    (*box_0).box_0 = temp;
    (*box_0).bsize = newsize;
    return temp;
}

unsafe fn boxgc(mut L: *mut lua_State) -> Result<c_int, Box<dyn std::error::Error>> {
    resizebox(L, 1 as libc::c_int, 0 as libc::c_int as usize);
    Ok(0)
}

static mut boxmt: [luaL_Reg; 3] = [
    {
        let mut init = luaL_Reg {
            name: b"__gc\0" as *const u8 as *const libc::c_char,
            func: Some(boxgc),
        };
        init
    },
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

unsafe extern "C" fn newbox(mut L: *mut lua_State) {
    let mut box_0: *mut UBox =
        lua_newuserdatauv(L, ::core::mem::size_of::<UBox>(), 0 as libc::c_int) as *mut UBox;
    (*box_0).box_0 = 0 as *mut libc::c_void;
    (*box_0).bsize = 0 as libc::c_int as usize;
    if luaL_newmetatable(L, b"_UBOX*\0" as *const u8 as *const libc::c_char) != 0 {
        luaL_setfuncs(L, &raw const boxmt as *const luaL_Reg, 0);
    }
    lua_setmetatable(L, -(2 as libc::c_int));
}

unsafe extern "C" fn newbuffsize(mut B: *mut luaL_Buffer, mut sz: usize) -> usize {
    let mut newsize: usize = (*B).size / 2 as libc::c_int as usize * 3 as libc::c_int as usize;
    if (((!(0 as libc::c_int as usize)).wrapping_sub(sz) < (*B).n) as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        return luaL_error((*B).L, "buffer too large") as usize;
    }
    if newsize < ((*B).n).wrapping_add(sz) {
        newsize = ((*B).n).wrapping_add(sz);
    }
    return newsize;
}

unsafe extern "C" fn prepbuffsize(
    mut B: *mut luaL_Buffer,
    mut sz: usize,
    mut boxidx: libc::c_int,
) -> *mut libc::c_char {
    if ((*B).size).wrapping_sub((*B).n) >= sz {
        return ((*B).b).offset((*B).n as isize);
    } else {
        let mut L: *mut lua_State = (*B).L;
        let mut newbuff: *mut libc::c_char = 0 as *mut libc::c_char;
        let mut newsize: usize = newbuffsize(B, sz);
        if (*B).b != ((*B).init.b).as_mut_ptr() {
            newbuff = resizebox(L, boxidx, newsize) as *mut libc::c_char;
        } else {
            lua_rotate(L, boxidx, -(1 as libc::c_int));
            lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int);
            newbox(L);
            lua_rotate(L, boxidx, 1 as libc::c_int);
            lua_toclose(L, boxidx);
            newbuff = resizebox(L, boxidx, newsize) as *mut libc::c_char;
            memcpy(
                newbuff as *mut libc::c_void,
                (*B).b as *const libc::c_void,
                ((*B).n).wrapping_mul(::core::mem::size_of::<libc::c_char>()),
            );
        }
        (*B).b = newbuff;
        (*B).size = newsize;
        return newbuff.offset((*B).n as isize);
    };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaL_prepbuffsize(
    mut B: *mut luaL_Buffer,
    mut sz: usize,
) -> *mut libc::c_char {
    return prepbuffsize(B, sz, -(1 as libc::c_int));
}

pub unsafe fn luaL_addlstring(B: *mut luaL_Buffer, s: *const c_char, l: usize) {
    if l > 0 {
        let b = prepbuffsize(B, l, -1);

        memcpy(b.cast(), s.cast(), l);

        (*B).n += l;
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaL_addstring(mut B: *mut luaL_Buffer, mut s: *const libc::c_char) {
    luaL_addlstring(B, s, strlen(s));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaL_pushresult(mut B: *mut luaL_Buffer) {
    let mut L: *mut lua_State = (*B).L;
    lua_pushlstring(L, std::slice::from_raw_parts((*B).b.cast(), (*B).n));
    if (*B).b != ((*B).init.b).as_mut_ptr() {
        lua_closeslot(L, -(2 as libc::c_int));
    }
    lua_rotate(L, -(2 as libc::c_int), -(1 as libc::c_int));
    lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaL_pushresultsize(mut B: *mut luaL_Buffer, mut sz: usize) {
    (*B).n = ((*B).n).wrapping_add(sz);
    luaL_pushresult(B);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaL_addvalue(mut B: *mut luaL_Buffer) {
    let mut L: *mut lua_State = (*B).L;
    let mut len: usize = 0;
    let mut s: *const libc::c_char = lua_tolstring(L, -(1 as libc::c_int), &mut len);
    let mut b: *mut libc::c_char = prepbuffsize(B, len, -(2 as libc::c_int));
    memcpy(
        b as *mut libc::c_void,
        s as *const libc::c_void,
        len.wrapping_mul(::core::mem::size_of::<libc::c_char>()),
    );
    (*B).n = ((*B).n).wrapping_add(len);
    lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaL_buffinit(mut L: *mut lua_State, mut B: *mut luaL_Buffer) {
    (*B).L = L;
    (*B).b = ((*B).init.b).as_mut_ptr();
    (*B).n = 0 as libc::c_int as usize;
    (*B).size = (16 as libc::c_int as libc::c_ulong)
        .wrapping_mul(::core::mem::size_of::<*mut libc::c_void>() as libc::c_ulong)
        .wrapping_mul(::core::mem::size_of::<f64>() as libc::c_ulong) as libc::c_int
        as usize;
    lua_pushlightuserdata(L, B as *mut libc::c_void);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaL_buffinitsize(
    mut L: *mut lua_State,
    mut B: *mut luaL_Buffer,
    mut sz: usize,
) -> *mut libc::c_char {
    luaL_buffinit(L, B);
    return prepbuffsize(B, sz, -(1 as libc::c_int));
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaL_ref(mut L: *mut lua_State, mut t: libc::c_int) -> libc::c_int {
    let mut ref_0: libc::c_int = 0;
    if lua_type(L, -(1 as libc::c_int)) == 0 as libc::c_int {
        lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int);
        return -(1 as libc::c_int);
    }
    t = lua_absindex(L, t);
    if lua_rawgeti(L, t, (2 as libc::c_int + 1 as libc::c_int) as i64) == 0 as libc::c_int {
        ref_0 = 0 as libc::c_int;
        lua_pushinteger(L, 0 as libc::c_int as i64);
        lua_rawseti(L, t, (2 as libc::c_int + 1 as libc::c_int) as i64);
    } else {
        ref_0 = lua_tointegerx(L, -(1 as libc::c_int), 0 as *mut libc::c_int) as libc::c_int;
    }
    lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int);
    if ref_0 != 0 as libc::c_int {
        lua_rawgeti(L, t, ref_0 as i64);
        lua_rawseti(L, t, (2 as libc::c_int + 1 as libc::c_int) as i64);
    } else {
        ref_0 = lua_rawlen(L, t) as libc::c_int + 1 as libc::c_int;
    }
    lua_rawseti(L, t, ref_0 as i64);
    return ref_0;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaL_unref(
    mut L: *mut lua_State,
    mut t: libc::c_int,
    mut ref_0: libc::c_int,
) {
    if ref_0 >= 0 as libc::c_int {
        t = lua_absindex(L, t);
        lua_rawgeti(L, t, (2 as libc::c_int + 1 as libc::c_int) as i64);
        lua_rawseti(L, t, ref_0 as i64);
        lua_pushinteger(L, ref_0 as i64);
        lua_rawseti(L, t, (2 as libc::c_int + 1 as libc::c_int) as i64);
    }
}

pub unsafe fn luaL_loadfilex(
    L: *mut lua_State,
    filename: *const c_char,
    mode: *const c_char,
) -> c_int {
    todo!()
}

unsafe fn getS(ud: *mut c_void, mut size: *mut usize) -> *const c_char {
    let mut ls: *mut LoadS = ud as *mut LoadS;
    if (*ls).size == 0 as libc::c_int as usize {
        return 0 as *const libc::c_char;
    }
    *size = (*ls).size;
    (*ls).size = 0 as libc::c_int as usize;
    return (*ls).s;
}

pub unsafe fn luaL_loadbufferx(
    L: *mut lua_State,
    chunk: impl AsRef<[u8]>,
    name: *const c_char,
    mode: *const c_char,
) -> c_int {
    let chunk = chunk.as_ref();
    let mut ls = LoadS {
        s: chunk.as_ptr().cast(),
        size: chunk.len(),
    };

    lua_load(L, getS, &mut ls as *mut LoadS as *mut c_void, name, mode)
}

pub unsafe fn luaL_loadstring(L: *mut lua_State, s: *const c_char) -> c_int {
    luaL_loadbufferx(L, CStr::from_ptr(s).to_bytes(), s, null())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaL_getmetafield(
    mut L: *mut lua_State,
    mut obj: libc::c_int,
    mut event: *const libc::c_char,
) -> libc::c_int {
    if lua_getmetatable(L, obj) == 0 {
        return 0 as libc::c_int;
    } else {
        let mut tt: libc::c_int = 0;
        lua_pushstring(L, event);
        tt = lua_rawget(L, -(2 as libc::c_int));
        if tt == 0 as libc::c_int {
            lua_settop(L, -(2 as libc::c_int) - 1 as libc::c_int);
        } else {
            lua_rotate(L, -(2 as libc::c_int), -(1 as libc::c_int));
            lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int);
        }
        return tt;
    };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaL_callmeta(
    mut L: *mut lua_State,
    mut obj: libc::c_int,
    mut event: *const libc::c_char,
) -> libc::c_int {
    obj = lua_absindex(L, obj);
    if luaL_getmetafield(L, obj, event) == 0 as libc::c_int {
        return 0 as libc::c_int;
    }
    lua_pushvalue(L, obj);
    lua_callk(
        L,
        1 as libc::c_int,
        1 as libc::c_int,
        0 as libc::c_int as lua_KContext,
        None,
    );
    return 1 as libc::c_int;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaL_len(mut L: *mut lua_State, mut idx: libc::c_int) -> i64 {
    let mut l: i64 = 0;
    let mut isnum: libc::c_int = 0;
    lua_len(L, idx);
    l = lua_tointegerx(L, -(1 as libc::c_int), &mut isnum);
    if ((isnum == 0) as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long != 0 {
        luaL_error(L, "object length is not an integer");
    }
    lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int);
    return l;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaL_tolstring(
    mut L: *mut lua_State,
    mut idx: libc::c_int,
    mut len: *mut usize,
) -> *const libc::c_char {
    idx = lua_absindex(L, idx);
    if luaL_callmeta(L, idx, b"__tostring\0" as *const u8 as *const libc::c_char) != 0 {
        if lua_isstring(L, -(1 as libc::c_int)) == 0 {
            luaL_error(L, "'__tostring' must return a string");
        }
    } else {
        match lua_type(L, idx) {
            3 => {
                if lua_isinteger(L, idx) != 0 {
                    lua_pushlstring(L, lua_tointegerx(L, idx, 0 as *mut libc::c_int).to_string());
                } else {
                    lua_pushlstring(L, lua_tonumberx(L, idx, 0 as *mut libc::c_int).to_string());
                }
            }
            4 => {
                lua_pushvalue(L, idx);
            }
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
                let mut tt: libc::c_int =
                    luaL_getmetafield(L, idx, b"__name\0" as *const u8 as *const libc::c_char);
                let kind = if tt == 4 {
                    CStr::from_ptr(lua_tolstring(L, -1, 0 as *mut usize)).to_string_lossy()
                } else {
                    lua_typename(L, lua_type(L, idx)).into()
                };

                lua_pushlstring(L, format!("{}: {:p}", kind, lua_topointer(L, idx)));

                if tt != 0 as libc::c_int {
                    lua_rotate(L, -(2 as libc::c_int), -(1 as libc::c_int));
                    lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int);
                }
            }
        }
    }
    return lua_tolstring(L, -(1 as libc::c_int), len);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaL_setfuncs(
    mut L: *mut lua_State,
    mut l: *const luaL_Reg,
    mut nup: libc::c_int,
) {
    luaL_checkstack(
        L,
        nup,
        b"too many upvalues\0" as *const u8 as *const libc::c_char,
    );

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

        lua_setfield(L, -(nup + 2 as libc::c_int), (*l).name);
        l = l.offset(1);
    }

    lua_settop(L, -nup - 1 as libc::c_int);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaL_getsubtable(
    mut L: *mut lua_State,
    mut idx: libc::c_int,
    mut fname: *const libc::c_char,
) -> libc::c_int {
    if lua_getfield(L, idx, CStr::from_ptr(fname).to_bytes()) == 5 {
        return 1 as libc::c_int;
    } else {
        lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int);
        idx = lua_absindex(L, idx);
        lua_createtable(L, 0 as libc::c_int, 0 as libc::c_int);
        lua_pushvalue(L, -(1 as libc::c_int));
        lua_setfield(L, idx, fname);
        return 0 as libc::c_int;
    };
}

pub unsafe fn luaL_requiref(
    mut L: *mut lua_State,
    mut modname: *const libc::c_char,
    mut openf: lua_CFunction,
    mut glb: libc::c_int,
) {
    luaL_getsubtable(
        L,
        -(1000000 as libc::c_int) - 1000 as libc::c_int,
        b"_LOADED\0" as *const u8 as *const libc::c_char,
    );
    lua_getfield(L, -(1 as libc::c_int), CStr::from_ptr(modname).to_bytes());
    if lua_toboolean(L, -(1 as libc::c_int)) == 0 {
        lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int);
        lua_pushcclosure(L, openf, 0 as libc::c_int);
        lua_pushstring(L, modname);
        lua_callk(
            L,
            1 as libc::c_int,
            1 as libc::c_int,
            0 as libc::c_int as lua_KContext,
            None,
        );
        lua_pushvalue(L, -(1 as libc::c_int));
        lua_setfield(L, -(3 as libc::c_int), modname);
    }
    lua_rotate(L, -(2 as libc::c_int), -(1 as libc::c_int));
    lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int);
    if glb != 0 {
        lua_pushvalue(L, -(1 as libc::c_int));
        lua_setglobal(L, modname);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaL_addgsub(
    mut b: *mut luaL_Buffer,
    mut s: *const libc::c_char,
    mut p: *const libc::c_char,
    mut r: *const libc::c_char,
) {
    let mut wild: *const libc::c_char = 0 as *const libc::c_char;
    let mut l: usize = strlen(p);
    loop {
        wild = strstr(s, p);
        if wild.is_null() {
            break;
        }
        luaL_addlstring(b, s, wild.offset_from(s) as libc::c_long as usize);
        luaL_addstring(b, r);
        s = wild.offset(l as isize);
    }
    luaL_addstring(b, s);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaL_gsub(
    mut L: *mut lua_State,
    mut s: *const libc::c_char,
    mut p: *const libc::c_char,
    mut r: *const libc::c_char,
) -> *const libc::c_char {
    let mut b: luaL_Buffer = luaL_Buffer {
        b: 0 as *mut libc::c_char,
        size: 0,
        n: 0,
        L: 0 as *mut lua_State,
        init: C2RustUnnamed { n: 0. },
    };
    luaL_buffinit(L, &mut b);
    luaL_addgsub(&mut b, s, p, r);
    luaL_pushresult(&mut b);
    return lua_tolstring(L, -(1 as libc::c_int), 0 as *mut usize);
}

unsafe extern "C" fn l_alloc(
    mut ud: *mut libc::c_void,
    mut ptr: *mut libc::c_void,
    mut osize: usize,
    mut nsize: usize,
) -> *mut libc::c_void {
    if nsize == 0 as libc::c_int as usize {
        free(ptr);
        return 0 as *mut libc::c_void;
    } else {
        return realloc(ptr, nsize);
    };
}

unsafe extern "C" fn checkcontrol(
    mut L: *mut lua_State,
    mut message: *const libc::c_char,
    mut tocont: libc::c_int,
) -> libc::c_int {
    if tocont != 0 || {
        let fresh5 = message;
        message = message.offset(1);
        *fresh5 as libc::c_int != '@' as i32
    } {
        return 0 as libc::c_int;
    } else {
        if strcmp(message, b"off\0" as *const u8 as *const libc::c_char) == 0 as libc::c_int {
            lua_setwarnf(
                L,
                Some(
                    warnfoff
                        as unsafe extern "C" fn(
                            *mut libc::c_void,
                            *const libc::c_char,
                            libc::c_int,
                        ) -> (),
                ),
                L as *mut libc::c_void,
            );
        } else if strcmp(message, b"on\0" as *const u8 as *const libc::c_char) == 0 as libc::c_int {
            lua_setwarnf(
                L,
                Some(
                    warnfon
                        as unsafe extern "C" fn(
                            *mut libc::c_void,
                            *const libc::c_char,
                            libc::c_int,
                        ) -> (),
                ),
                L as *mut libc::c_void,
            );
        }
        return 1 as libc::c_int;
    };
}

unsafe extern "C" fn warnfoff(
    mut ud: *mut libc::c_void,
    mut message: *const libc::c_char,
    mut tocont: libc::c_int,
) {
    checkcontrol(ud as *mut lua_State, message, tocont);
}

unsafe extern "C" fn warnfcont(
    mut ud: *mut libc::c_void,
    mut message: *const libc::c_char,
    mut tocont: libc::c_int,
) {
    let mut L: *mut lua_State = ud as *mut lua_State;

    eprint!("{}", CStr::from_ptr(message).to_string_lossy());

    if tocont != 0 {
        lua_setwarnf(
            L,
            Some(
                warnfcont
                    as unsafe extern "C" fn(
                        *mut libc::c_void,
                        *const libc::c_char,
                        libc::c_int,
                    ) -> (),
            ),
            L as *mut libc::c_void,
        );
    } else {
        eprintln!();

        lua_setwarnf(
            L,
            Some(
                warnfon
                    as unsafe extern "C" fn(
                        *mut libc::c_void,
                        *const libc::c_char,
                        libc::c_int,
                    ) -> (),
            ),
            L as *mut libc::c_void,
        );
    };
}

unsafe extern "C" fn warnfon(
    mut ud: *mut libc::c_void,
    mut message: *const libc::c_char,
    mut tocont: libc::c_int,
) {
    if checkcontrol(ud as *mut lua_State, message, tocont) != 0 {
        return;
    }

    eprint!("Lua warning: ");

    warnfcont(ud, message, tocont);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaL_newstate() -> *mut lua_State {
    let mut L: *mut lua_State = lua_newstate(
        Some(
            l_alloc
                as unsafe extern "C" fn(
                    *mut libc::c_void,
                    *mut libc::c_void,
                    usize,
                    usize,
                ) -> *mut libc::c_void,
        ),
        0 as *mut libc::c_void,
    );

    if (L != 0 as *mut lua_State) as libc::c_int as libc::c_long != 0 {
        lua_setwarnf(
            L,
            Some(
                warnfoff
                    as unsafe extern "C" fn(
                        *mut libc::c_void,
                        *const libc::c_char,
                        libc::c_int,
                    ) -> (),
            ),
            L as *mut libc::c_void,
        );
    }

    return L;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaL_checkversion_(mut L: *mut lua_State, mut ver: f64, mut sz: usize) {
    let mut v: f64 = lua_version(L);
    if sz
        != ::core::mem::size_of::<i64>()
            .wrapping_mul(16)
            .wrapping_add(::core::mem::size_of::<f64>())
    {
        luaL_error(L, "core and library have incompatible numeric types");
    } else if v != ver {
        luaL_error(
            L,
            format!("version mismatch: app. needs {ver}, Lua core provides {v}"),
        );
    }
}
