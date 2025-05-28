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

use crate::lapi::{lua_copy, lua_getfield, lua_pushboolean, lua_rawgeti, lua_rawseti};
use crate::lauxlib::{luaL_addgsub, luaL_getsubtable, luaL_gsub};
use crate::lstate::lua_CFunction;
use crate::{
    C2RustUnnamed, Thread, lua_call, lua_createtable, lua_isstring, lua_pushcclosure,
    lua_pushlstring, lua_pushnil, lua_pushstring, lua_pushvalue, lua_rotate, lua_setfield,
    lua_settop, lua_toboolean, lua_tolstring, lua_type, luaL_Buffer, luaL_Reg, luaL_addstring,
    luaL_buffinit, luaL_checklstring, luaL_error, luaL_loadfilex, luaL_optlstring,
    luaL_prepbuffsize, luaL_pushresult, luaL_setfuncs,
};
use libc::{fclose, fopen, strchr};
use std::ffi::{CStr, c_int};
use std::fmt::Write;
use std::ptr::{addr_of, null_mut};

unsafe fn setpath(
    mut L: *mut Thread,
    mut fieldname: *const libc::c_char,
    mut dft: *const libc::c_char,
) -> Result<(), Box<dyn std::error::Error>> {
    lua_pushstring(L, dft)?;
    lua_setfield(L, -(2 as libc::c_int), fieldname)
}

unsafe fn readable(mut filename: *const libc::c_char) -> libc::c_int {
    let mut f = fopen(filename, b"r\0" as *const u8 as *const libc::c_char);
    if f.is_null() {
        return 0 as libc::c_int;
    }
    fclose(f);
    return 1 as libc::c_int;
}

unsafe fn getnextfilename(
    mut path: *mut *mut libc::c_char,
    mut end: *mut libc::c_char,
) -> *const libc::c_char {
    let mut sep: *mut libc::c_char = 0 as *mut libc::c_char;
    let mut name: *mut libc::c_char = *path;
    if name == end {
        return 0 as *const libc::c_char;
    } else if *name as libc::c_int == '\0' as i32 {
        *name = *(b";\0" as *const u8 as *const libc::c_char);
        name = name.offset(1);
    }
    sep = strchr(
        name,
        *(b";\0" as *const u8 as *const libc::c_char) as libc::c_int,
    );
    if sep.is_null() {
        sep = end;
    }
    *sep = '\0' as i32 as libc::c_char;
    *path = sep;
    return name;
}

unsafe fn pusherrornotfound(
    mut L: *mut Thread,
    mut path: *const libc::c_char,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut b: luaL_Buffer = luaL_Buffer {
        b: 0 as *mut libc::c_char,
        size: 0,
        n: 0,
        L: 0 as *mut Thread,
        init: C2RustUnnamed { n: 0. },
    };
    luaL_buffinit(L, &mut b);
    luaL_addstring(&mut b, b"no file '\0" as *const u8 as *const libc::c_char)?;
    luaL_addgsub(
        &mut b,
        path,
        b";\0" as *const u8 as *const libc::c_char,
        b"'\n\tno file '\0" as *const u8 as *const libc::c_char,
    )?;
    luaL_addstring(&mut b, b"'\0" as *const u8 as *const libc::c_char)?;
    luaL_pushresult(&mut b)
}

unsafe fn searchpath(
    mut L: *mut Thread,
    mut name: *const libc::c_char,
    mut path: *const libc::c_char,
    mut sep: *const libc::c_char,
    mut dirsep: *const libc::c_char,
) -> Result<*const libc::c_char, Box<dyn std::error::Error>> {
    let mut buff: luaL_Buffer = luaL_Buffer {
        b: 0 as *mut libc::c_char,
        size: 0,
        n: 0,
        L: 0 as *mut Thread,
        init: C2RustUnnamed { n: 0. },
    };
    let mut pathname: *mut libc::c_char = 0 as *mut libc::c_char;
    let mut endpathname: *mut libc::c_char = 0 as *mut libc::c_char;
    let mut filename: *const libc::c_char = 0 as *const libc::c_char;
    if *sep as libc::c_int != '\0' as i32 && !(strchr(name, *sep as libc::c_int)).is_null() {
        name = luaL_gsub(L, name, sep, dirsep)?;
    }
    luaL_buffinit(L, &mut buff);
    luaL_addgsub(
        &mut buff,
        path,
        b"?\0" as *const u8 as *const libc::c_char,
        name,
    )?;
    (buff.n < buff.size || !(luaL_prepbuffsize(&mut buff, 1 as libc::c_int as usize))?.is_null())
        as libc::c_int;
    let fresh2 = buff.n;
    buff.n = (buff.n).wrapping_add(1);
    *(buff.b).offset(fresh2 as isize) = '\0' as i32 as libc::c_char;
    pathname = buff.b;
    endpathname = pathname
        .offset(buff.n as isize)
        .offset(-(1 as libc::c_int as isize));
    loop {
        filename = getnextfilename(&mut pathname, endpathname);
        if filename.is_null() {
            break;
        }
        if readable(filename) != 0 {
            return lua_pushstring(L, filename);
        }
    }
    luaL_pushresult(&mut buff)?;
    pusherrornotfound(L, lua_tolstring(L, -(1 as libc::c_int), 0 as *mut usize)?)?;
    return Ok(0 as *const libc::c_char);
}

unsafe fn ll_searchpath(mut L: *mut Thread) -> Result<libc::c_int, Box<dyn std::error::Error>> {
    let mut f: *const libc::c_char = searchpath(
        L,
        luaL_checklstring(L, 1 as libc::c_int, 0 as *mut usize)?,
        luaL_checklstring(L, 2 as libc::c_int, 0 as *mut usize)?,
        luaL_optlstring(
            L,
            3 as libc::c_int,
            b".\0" as *const u8 as *const libc::c_char,
            0 as *mut usize,
        )?,
        luaL_optlstring(
            L,
            4 as libc::c_int,
            b"/\0" as *const u8 as *const libc::c_char,
            0 as *mut usize,
        )?,
    )?;

    if !f.is_null() {
        return Ok(1 as libc::c_int);
    } else {
        lua_pushnil(L);
        lua_rotate(L, -(2 as libc::c_int), 1 as libc::c_int);
        return Ok(2 as libc::c_int);
    };
}

unsafe fn findfile(
    mut L: *mut Thread,
    mut name: *const libc::c_char,
    pname: &str,
    mut dirsep: *const libc::c_char,
) -> Result<*const libc::c_char, Box<dyn std::error::Error>> {
    let mut path: *const libc::c_char = 0 as *const libc::c_char;

    lua_getfield(
        L,
        -(1000000 as libc::c_int) - 1000 as libc::c_int - 1 as libc::c_int,
        pname,
    )?;

    path = lua_tolstring(L, -(1 as libc::c_int), 0 as *mut usize)?;

    if path == 0 as *mut libc::c_void as *const libc::c_char {
        luaL_error(L, format_args!("'package.{pname}' must be a string"))?;
    }

    return searchpath(
        L,
        name,
        path,
        b".\0" as *const u8 as *const libc::c_char,
        dirsep,
    );
}

unsafe fn searcher_Lua(mut L: *mut Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let name: *const libc::c_char = luaL_checklstring(L, 1 as libc::c_int, 0 as *mut usize)?;
    let filename = findfile(
        L,
        name,
        "path",
        #[cfg(unix)]
        c"/".as_ptr(),
        #[cfg(windows)]
        c"\\".as_ptr(),
    )?;

    if filename.is_null() {
        return Ok(1 as libc::c_int);
    }

    match luaL_loadfilex(L, filename, 0 as *const libc::c_char) {
        Ok(_) => {
            lua_pushstring(L, filename)?;
            return Ok(2 as libc::c_int);
        }
        Err(e) => luaL_error(
            L,
            format_args!(
                "error loading module '{}' from file '{}':\n\t{}",
                CStr::from_ptr(name).to_string_lossy(),
                CStr::from_ptr(filename).to_string_lossy(),
                e
            ),
        ),
    }
}

unsafe fn searcher_preload(L: *mut Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let name = CStr::from_ptr(luaL_checklstring(L, 1 as libc::c_int, 0 as *mut usize)?);

    lua_getfield(
        L,
        -(1000000 as libc::c_int) - 1000 as libc::c_int,
        "_PRELOAD",
    )?;

    if lua_getfield(L, -(1 as libc::c_int), name.to_bytes())? == 0 as libc::c_int {
        lua_pushlstring(
            L,
            format!("no field package.preload['{}']", name.to_string_lossy()),
        )?;
        return Ok(1 as libc::c_int);
    } else {
        lua_pushstring(L, b":preload:\0" as *const u8 as *const libc::c_char)?;
        return Ok(2 as libc::c_int);
    };
}

unsafe fn findloader(mut L: *mut Thread, name: &CStr) -> Result<(), Box<dyn std::error::Error>> {
    if (lua_getfield(
        L,
        -(1000000 as libc::c_int) - 1000 as libc::c_int - 1 as libc::c_int,
        "searchers",
    )? != 5) as libc::c_int as libc::c_long
        != 0
    {
        luaL_error(L, "'package.searchers' must be a table")?;
    }

    let mut msg = String::new();
    let mut i = 1 as libc::c_int;

    loop {
        if ((lua_rawgeti(L, 3 as libc::c_int, i as i64) == 0 as libc::c_int) as libc::c_int
            != 0 as libc::c_int) as libc::c_int as libc::c_long
            != 0
        {
            lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
            luaL_error(
                L,
                format_args!("module '{}' not found:{}", name.to_string_lossy(), msg),
            )?;
        }

        lua_pushstring(L, name.as_ptr())?;
        lua_call(L, 1 as libc::c_int, 2 as libc::c_int)?;

        if lua_type(L, -(2 as libc::c_int)) == 6 as libc::c_int {
            return Ok(());
        } else if lua_isstring(L, -(2 as libc::c_int)) != 0 {
            let e = CStr::from_ptr(lua_tolstring(L, -2, null_mut())?);

            write!(msg, "\n\t{}", e.to_string_lossy())?;
        }

        lua_settop(L, -(2 as libc::c_int) - 1 as libc::c_int)?;

        i += 1;
    }
}

unsafe fn ll_require(mut L: *mut Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let name = luaL_checklstring(L, 1, 0 as *mut usize)?;
    let name = CStr::from_ptr(name);

    lua_settop(L, 1)?;
    lua_getfield(
        L,
        -(1000000 as libc::c_int) - 1000 as libc::c_int,
        b"_LOADED",
    )?;
    lua_getfield(L, 2, name.to_bytes())?;

    if lua_toboolean(L, -1) != 0 {
        return Ok(1 as libc::c_int);
    }

    lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
    findloader(L, name)?;
    lua_rotate(L, -(2 as libc::c_int), 1 as libc::c_int);
    lua_pushvalue(L, 1 as libc::c_int);
    lua_pushvalue(L, -(3 as libc::c_int));
    lua_call(L, 2 as libc::c_int, 1 as libc::c_int)?;
    if !(lua_type(L, -(1 as libc::c_int)) == 0 as libc::c_int) {
        lua_setfield(L, 2 as libc::c_int, name.as_ptr())?;
    } else {
        lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
    }
    if lua_getfield(L, 2 as libc::c_int, name.to_bytes())? == 0 as libc::c_int {
        lua_pushboolean(L, 1 as libc::c_int);
        lua_copy(L, -(1 as libc::c_int), -(2 as libc::c_int));
        lua_setfield(L, 2 as libc::c_int, name.as_ptr())?;
    }
    lua_rotate(L, -(2 as libc::c_int), 1 as libc::c_int);
    return Ok(2 as libc::c_int);
}

static mut pk_funcs: [luaL_Reg; 6] = [
    {
        let mut init = luaL_Reg {
            name: b"searchpath\0" as *const u8 as *const libc::c_char,
            func: Some(ll_searchpath),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"preload\0" as *const u8 as *const libc::c_char,
            func: None,
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"path\0" as *const u8 as *const libc::c_char,
            func: None,
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"searchers\0" as *const u8 as *const libc::c_char,
            func: None,
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"loaded\0" as *const u8 as *const libc::c_char,
            func: None,
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

static mut ll_funcs: [luaL_Reg; 2] = [
    {
        let mut init = luaL_Reg {
            name: b"require\0" as *const u8 as *const libc::c_char,
            func: Some(ll_require),
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

unsafe fn createsearcherstable(mut L: *mut Thread) -> Result<(), Box<dyn std::error::Error>> {
    static searchers: [lua_CFunction; 2] = [searcher_preload, searcher_Lua];

    lua_createtable(
        L,
        (::core::mem::size_of::<[lua_CFunction; 5]>() as libc::c_ulong)
            .wrapping_div(::core::mem::size_of::<lua_CFunction>() as libc::c_ulong)
            .wrapping_sub(1 as libc::c_int as libc::c_ulong) as libc::c_int,
        0 as libc::c_int,
    )?;

    for (i, s) in searchers.iter().enumerate() {
        lua_pushvalue(L, -(2 as libc::c_int));
        lua_pushcclosure(L, *s, 1 as libc::c_int);
        lua_rawseti(L, -(2 as libc::c_int), (i + 1) as i64)?;
    }

    lua_setfield(
        L,
        -(2 as libc::c_int),
        b"searchers\0" as *const u8 as *const libc::c_char,
    )
}

pub unsafe fn luaopen_package(mut L: *mut Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    lua_createtable(
        L,
        0 as libc::c_int,
        (::core::mem::size_of::<[luaL_Reg; 8]>() as libc::c_ulong)
            .wrapping_div(::core::mem::size_of::<luaL_Reg>() as libc::c_ulong)
            .wrapping_sub(1 as libc::c_int as libc::c_ulong) as libc::c_int,
    )?;
    luaL_setfuncs(L, addr_of!(pk_funcs).cast(), 0 as libc::c_int)?;
    createsearcherstable(L)?;
    setpath(
        L,
        b"path\0" as *const u8 as *const libc::c_char,
        b"./?.lua;./?/init.lua\0" as *const u8 as *const libc::c_char,
    )?;

    if cfg!(windows) {
        lua_pushstring(L, b"\\\n;\n?\n!\n-\n\0" as *const u8 as *const libc::c_char)?;
    } else {
        lua_pushstring(L, b"/\n;\n?\n!\n-\n\0" as *const u8 as *const libc::c_char)?;
    }

    lua_setfield(
        L,
        -(2 as libc::c_int),
        b"config\0" as *const u8 as *const libc::c_char,
    )?;
    luaL_getsubtable(
        L,
        -(1000000 as libc::c_int) - 1000 as libc::c_int,
        b"_LOADED\0" as *const u8 as *const libc::c_char,
    )?;
    lua_setfield(
        L,
        -(2 as libc::c_int),
        b"loaded\0" as *const u8 as *const libc::c_char,
    )?;
    luaL_getsubtable(
        L,
        -(1000000 as libc::c_int) - 1000 as libc::c_int,
        b"_PRELOAD\0" as *const u8 as *const libc::c_char,
    )?;
    lua_setfield(
        L,
        -(2 as libc::c_int),
        b"preload\0" as *const u8 as *const libc::c_char,
    )?;
    lua_rawgeti(
        L,
        -(1000000 as libc::c_int) - 1000 as libc::c_int,
        2 as libc::c_int as i64,
    );
    lua_pushvalue(L, -(2 as libc::c_int));
    luaL_setfuncs(L, addr_of!(ll_funcs).cast(), 1 as libc::c_int)?;
    lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;

    return Ok(1 as libc::c_int);
}
