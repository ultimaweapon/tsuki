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
    lua_call, lua_copy, lua_geti, lua_getmetatable, lua_gettop, lua_isstring, lua_next, lua_pcall,
    lua_pushboolean, lua_pushcclosure, lua_pushinteger, lua_pushlstring, lua_pushnil,
    lua_pushstring, lua_pushvalue, lua_rawequal, lua_rawget, lua_rawgeti, lua_rawlen, lua_rawset,
    lua_rotate, lua_setfield, lua_setmetatable, lua_settop, lua_setupvalue, lua_stringtonumber,
    lua_toboolean, lua_tolstring, lua_type, lua_typename,
};
use crate::lauxlib::{
    luaL_Reg, luaL_argerror, luaL_checkany, luaL_checkinteger, luaL_checklstring, luaL_checkstack,
    luaL_checktype, luaL_error, luaL_getmetafield, luaL_optinteger, luaL_optlstring, luaL_setfuncs,
    luaL_tolstring, luaL_typeerror, luaL_where,
};
use crate::ldebug::luaG_runerror;
use crate::{ChunkInfo, Thread, api_incr_top};
use libc::{isalnum, isdigit, strspn, toupper};
use std::boxed::Box;
use std::ffi::{CStr, c_char, c_int, c_void};
use std::io::Write;
use std::ptr::{null, null_mut};
use std::string::{String, ToString};
use std::{format, print, println};

unsafe fn luaB_print(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut n: libc::c_int = lua_gettop(L);
    let mut i: libc::c_int = 0;
    i = 1 as libc::c_int;
    while i <= n {
        let mut l: usize = 0;
        let s: *const libc::c_char = luaL_tolstring(L, i, &mut l)?;
        let s = std::slice::from_raw_parts(s.cast(), l);

        if i > 1 {
            print!("\t");
        }

        print!("{}", String::from_utf8_lossy(s));

        lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
        i += 1;
    }

    println!();

    return Ok(0 as libc::c_int);
}

unsafe fn luaB_warn(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut n: libc::c_int = lua_gettop(L);
    let mut i: libc::c_int = 0;
    luaL_checklstring(L, 1 as libc::c_int, 0 as *mut usize)?;
    i = 2 as libc::c_int;
    while i <= n {
        luaL_checklstring(L, i, 0 as *mut usize)?;
        i += 1;
    }

    // Print to stderr.
    let mut dst = std::io::stderr().lock();

    for i in 1..=n {
        let mut len = 0;
        let msg = lua_tolstring(L, i, &mut len);
        let msg = std::slice::from_raw_parts(msg.cast(), len);

        dst.write_all(msg).unwrap();
    }

    writeln!(dst).unwrap();

    Ok(0)
}

unsafe extern "C" fn b_str2int(
    mut s: *const c_char,
    mut base: libc::c_int,
    mut pn: *mut i64,
) -> *const libc::c_char {
    let mut n: u64 = 0 as libc::c_int as u64;
    let mut neg: libc::c_int = 0 as libc::c_int;
    s = s.offset(strspn(s, b" \x0C\n\r\t\x0B\0" as *const u8 as *const libc::c_char) as isize);
    if *s as libc::c_int == '-' as i32 {
        s = s.offset(1);
        neg = 1 as libc::c_int;
    } else if *s as libc::c_int == '+' as i32 {
        s = s.offset(1);
    }
    if isalnum(*s as libc::c_uchar as libc::c_int) == 0 {
        return 0 as *const libc::c_char;
    }
    loop {
        let mut digit: libc::c_int = if isdigit(*s as libc::c_uchar as libc::c_int) != 0 {
            *s as libc::c_int - '0' as i32
        } else {
            toupper(*s as libc::c_uchar as libc::c_int) - 'A' as i32 + 10 as libc::c_int
        };
        if digit >= base {
            return 0 as *const libc::c_char;
        }
        n = (n * base as u64).wrapping_add(digit as u64);
        s = s.offset(1);
        if !(isalnum(*s as libc::c_uchar as libc::c_int) != 0) {
            break;
        }
    }
    s = s.offset(strspn(s, b" \x0C\n\r\t\x0B\0" as *const u8 as *const libc::c_char) as isize);
    *pn = (if neg != 0 {
        (0 as libc::c_uint as u64).wrapping_sub(n)
    } else {
        n
    }) as i64;
    return s;
}

unsafe fn luaB_tonumber(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    if lua_type(L, 2 as libc::c_int) <= 0 as libc::c_int {
        if lua_type(L, 1 as libc::c_int) == 3 as libc::c_int {
            lua_settop(L, 1 as libc::c_int)?;
            return Ok(1 as libc::c_int);
        } else {
            let mut l: usize = 0;
            let mut s: *const libc::c_char = lua_tolstring(L, 1 as libc::c_int, &mut l);
            if !s.is_null() && lua_stringtonumber(L, s) == l.wrapping_add(1 as libc::c_int as usize)
            {
                return Ok(1 as libc::c_int);
            }
            luaL_checkany(L, 1 as libc::c_int)?;
        }
    } else {
        let mut l_0: usize = 0;
        let mut s_0: *const libc::c_char = 0 as *const libc::c_char;
        let mut n: i64 = 0 as libc::c_int as i64;
        let mut base: i64 = luaL_checkinteger(L, 2 as libc::c_int)?;
        luaL_checktype(L, 1 as libc::c_int, 4 as libc::c_int)?;
        s_0 = lua_tolstring(L, 1 as libc::c_int, &mut l_0);
        (((2 as libc::c_int as i64 <= base && base <= 36 as libc::c_int as i64) as libc::c_int
            != 0 as libc::c_int) as libc::c_int as libc::c_long
            != 0
            || luaL_argerror(L, 2 as libc::c_int, "base out of range")? != 0)
            as libc::c_int;
        if b_str2int(s_0, base as libc::c_int, &mut n) == s_0.offset(l_0 as isize) {
            lua_pushinteger(L, n);
            return Ok(1 as libc::c_int);
        }
    }
    lua_pushnil(L);
    return Ok(1 as libc::c_int);
}

unsafe fn luaB_error(L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut len = 0;
    let msg = luaL_checklstring(L, 1, &mut len)?;
    let msg = std::slice::from_raw_parts(msg.cast(), len);
    let msg = String::from_utf8_lossy(msg);
    let lv = luaL_optinteger(L, 2, 1)? as libc::c_int;

    lua_settop(L, 1)?;

    if lv > 0 {
        Err(format!("{}{}", luaL_where(L, lv), msg).into())
    } else {
        Err(msg.into())
    }
}

unsafe fn luaB_getmetatable(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    luaL_checkany(L, 1 as libc::c_int)?;
    if lua_getmetatable(L, 1 as libc::c_int) == 0 {
        lua_pushnil(L);
        return Ok(1 as libc::c_int);
    }
    luaL_getmetafield(
        L,
        1 as libc::c_int,
        b"__metatable\0" as *const u8 as *const libc::c_char,
    )?;
    return Ok(1 as libc::c_int);
}

unsafe fn luaB_setmetatable(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut t: libc::c_int = lua_type(L, 2 as libc::c_int);
    luaL_checktype(L, 1 as libc::c_int, 5 as libc::c_int)?;
    (((t == 0 as libc::c_int || t == 5 as libc::c_int) as libc::c_int != 0 as libc::c_int)
        as libc::c_int as libc::c_long
        != 0
        || luaL_typeerror(L, 2 as libc::c_int, "nil or table")? != 0) as libc::c_int;
    if ((luaL_getmetafield(
        L,
        1 as libc::c_int,
        b"__metatable\0" as *const u8 as *const libc::c_char,
    )? != 0 as libc::c_int) as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        return luaL_error(L, "cannot change a protected metatable");
    }
    lua_settop(L, 2 as libc::c_int)?;
    lua_setmetatable(L, 1 as libc::c_int)?;
    return Ok(1 as libc::c_int);
}

unsafe fn luaB_rawequal(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    luaL_checkany(L, 1 as libc::c_int)?;
    luaL_checkany(L, 2 as libc::c_int)?;
    lua_pushboolean(L, lua_rawequal(L, 1 as libc::c_int, 2 as libc::c_int)?);
    return Ok(1 as libc::c_int);
}

unsafe fn luaB_rawlen(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut t: libc::c_int = lua_type(L, 1 as libc::c_int);
    (((t == 5 as libc::c_int || t == 4 as libc::c_int) as libc::c_int != 0 as libc::c_int)
        as libc::c_int as libc::c_long
        != 0
        || luaL_typeerror(L, 1 as libc::c_int, "table or string")? != 0) as libc::c_int;
    lua_pushinteger(L, lua_rawlen(L, 1 as libc::c_int) as i64);
    return Ok(1 as libc::c_int);
}

unsafe fn luaB_rawget(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    luaL_checktype(L, 1 as libc::c_int, 5 as libc::c_int)?;
    luaL_checkany(L, 2 as libc::c_int)?;
    lua_settop(L, 2 as libc::c_int)?;
    lua_rawget(L, 1 as libc::c_int);
    return Ok(1 as libc::c_int);
}

unsafe fn luaB_rawset(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    luaL_checktype(L, 1 as libc::c_int, 5 as libc::c_int)?;
    luaL_checkany(L, 2 as libc::c_int)?;
    luaL_checkany(L, 3 as libc::c_int)?;
    lua_settop(L, 3 as libc::c_int)?;

    if let Err(e) = lua_rawset(L, 1) {
        luaG_runerror(L, e)?;
    }

    return Ok(1 as libc::c_int);
}

unsafe fn luaB_type(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut t: libc::c_int = lua_type(L, 1 as libc::c_int);
    (((t != -(1 as libc::c_int)) as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
        || luaL_argerror(L, 1 as libc::c_int, "value expected")? != 0) as libc::c_int;
    lua_pushlstring(L, lua_typename(t));
    return Ok(1 as libc::c_int);
}

unsafe fn luaB_next(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    luaL_checktype(L, 1 as libc::c_int, 5 as libc::c_int)?;
    lua_settop(L, 2 as libc::c_int)?;

    if lua_next(L, 1)? != 0 {
        Ok(2)
    } else {
        lua_pushnil(L);
        Ok(1)
    }
}

unsafe fn luaB_pairs(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    luaL_checkany(L, 1 as libc::c_int)?;
    if luaL_getmetafield(
        L,
        1 as libc::c_int,
        b"__pairs\0" as *const u8 as *const libc::c_char,
    )? == 0 as libc::c_int
    {
        lua_pushcclosure(L, luaB_next, 0);
        lua_pushvalue(L, 1 as libc::c_int);
        lua_pushnil(L);
    } else {
        lua_pushvalue(L, 1 as libc::c_int);
        lua_call(L, 1, 3)?;
    }
    return Ok(3 as libc::c_int);
}

unsafe fn ipairsaux(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut i: i64 = luaL_checkinteger(L, 2 as libc::c_int)?;
    i = (i as u64).wrapping_add(1 as libc::c_int as u64) as i64;
    lua_pushinteger(L, i);

    if lua_geti(L, 1 as libc::c_int, i)? == 0 as libc::c_int {
        Ok(1)
    } else {
        Ok(2)
    }
}

unsafe fn luaB_ipairs(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    luaL_checkany(L, 1 as libc::c_int)?;
    lua_pushcclosure(L, ipairsaux, 0 as libc::c_int);
    lua_pushvalue(L, 1 as libc::c_int);
    lua_pushinteger(L, 0 as libc::c_int as i64);
    return Ok(3 as libc::c_int);
}

unsafe fn generic_reader(
    ud: *mut c_void,
    mut size: *mut usize,
) -> Result<*const c_char, Box<dyn std::error::Error>> {
    let L = ud.cast();

    luaL_checkstack(
        L,
        2,
        b"too many nested functions\0" as *const u8 as *const libc::c_char,
    )?;
    lua_pushvalue(L, 1 as libc::c_int);
    lua_call(L, 0, 1)?;
    if lua_type(L, -(1 as libc::c_int)) == 0 as libc::c_int {
        lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
        *size = 0 as libc::c_int as usize;
        return Ok(0 as *const libc::c_char);
    } else if ((lua_isstring(L, -(1 as libc::c_int)) == 0) as libc::c_int != 0 as libc::c_int)
        as libc::c_int as libc::c_long
        != 0
    {
        luaL_error(L, "reader function must return a string")?;
    }
    lua_copy(L, -(1 as libc::c_int), 5 as libc::c_int);
    lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;

    Ok(lua_tolstring(L, 5 as libc::c_int, size))
}

unsafe fn luaB_load(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut l: usize = 0;
    let mut s: *const libc::c_char = luaL_checklstring(L, 1 as libc::c_int, &mut l)?;
    let mut mode: *const libc::c_char = luaL_optlstring(L, 3, null(), 0 as *mut usize)?;
    let mut env: libc::c_int = if !(lua_type(L, 4 as libc::c_int) == -(1 as libc::c_int)) {
        4 as libc::c_int
    } else {
        0 as libc::c_int
    };

    if !mode.is_null() {
        return luaL_argerror(L, 3, "mode is not supported");
    }

    let name = luaL_optlstring(L, 2, null(), null_mut())?;
    let s = std::slice::from_raw_parts(s.cast(), l);
    let name = if name.is_null() {
        String::new()
    } else {
        CStr::from_ptr(name).to_string_lossy().into_owned()
    };

    match (*L).global().load(ChunkInfo { name: name.clone() }, s) {
        Ok(f) => {
            if env != 0 as libc::c_int {
                lua_pushvalue(L, env);
                if (lua_setupvalue(L, -(2 as libc::c_int), 1 as libc::c_int)).is_null() {
                    lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
                }
            }

            let io = &raw mut (*(*L).top.get()).val;

            (*io).value_.gc = &f.hdr;
            (*io).tt_ = f.hdr.tt | 1 << 6;

            api_incr_top(L);

            Ok(1)
        }
        Err(e) => {
            lua_pushnil(L);
            lua_pushlstring(L, format!("{name}:{e}"));

            Ok(2)
        }
    }
}

unsafe fn dofilecont(mut L: *mut Thread, mut d1: libc::c_int) -> libc::c_int {
    return lua_gettop(L) - 1 as libc::c_int;
}

unsafe fn luaB_assert(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    if (lua_toboolean(L, 1 as libc::c_int) != 0 as libc::c_int) as libc::c_int as libc::c_long != 0
    {
        return Ok(lua_gettop(L));
    } else {
        luaL_checkany(L, 1 as libc::c_int)?;
        lua_rotate(L, 1 as libc::c_int, -(1 as libc::c_int));
        lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
        lua_pushstring(
            L,
            b"assertion failed!\0" as *const u8 as *const libc::c_char,
        );
        lua_settop(L, 1 as libc::c_int)?;
        return luaB_error(L);
    };
}

unsafe fn luaB_select(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut n: libc::c_int = lua_gettop(L);
    if lua_type(L, 1 as libc::c_int) == 4 as libc::c_int
        && *lua_tolstring(L, 1 as libc::c_int, 0 as *mut usize) as libc::c_int == '#' as i32
    {
        lua_pushinteger(L, (n - 1 as libc::c_int) as i64);
        return Ok(1 as libc::c_int);
    } else {
        let mut i: i64 = luaL_checkinteger(L, 1 as libc::c_int)?;
        if i < 0 as libc::c_int as i64 {
            i = n as i64 + i;
        } else if i > n as i64 {
            i = n as i64;
        }
        (((1 as libc::c_int as i64 <= i) as libc::c_int != 0 as libc::c_int) as libc::c_int
            as libc::c_long
            != 0
            || luaL_argerror(L, 1 as libc::c_int, "index out of range")? != 0)
            as libc::c_int;
        return Ok(n - i as libc::c_int);
    };
}

unsafe fn luaB_pcall(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    luaL_checkany(L, 1 as libc::c_int)?;
    lua_pushboolean(L, 1 as libc::c_int);
    lua_rotate(L, 1 as libc::c_int, 1 as libc::c_int);

    Ok(
        match lua_pcall(L, (lua_gettop(L) - 2).try_into().unwrap(), -1) {
            Ok(_) => lua_gettop(L),
            Err(e) => {
                lua_pushboolean(L, 0 as libc::c_int);
                lua_pushlstring(L, e.to_string());
                2
            }
        },
    )
}

unsafe fn luaB_tostring(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    luaL_checkany(L, 1 as libc::c_int)?;
    luaL_tolstring(L, 1 as libc::c_int, 0 as *mut usize)?;
    return Ok(1 as libc::c_int);
}

static mut base_funcs: [luaL_Reg; 21] = [
    {
        let mut init = luaL_Reg {
            name: b"assert\0" as *const u8 as *const libc::c_char,
            func: Some(luaB_assert),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"error\0" as *const u8 as *const libc::c_char,
            func: Some(luaB_error),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"getmetatable\0" as *const u8 as *const libc::c_char,
            func: Some(luaB_getmetatable),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"ipairs\0" as *const u8 as *const libc::c_char,
            func: Some(luaB_ipairs),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"load\0" as *const u8 as *const libc::c_char,
            func: Some(luaB_load),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"next\0" as *const u8 as *const libc::c_char,
            func: Some(luaB_next),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"pairs\0" as *const u8 as *const libc::c_char,
            func: Some(luaB_pairs),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"pcall\0" as *const u8 as *const libc::c_char,
            func: Some(luaB_pcall),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"print\0" as *const u8 as *const libc::c_char,
            func: Some(luaB_print),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"warn\0" as *const u8 as *const libc::c_char,
            func: Some(luaB_warn),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"rawequal\0" as *const u8 as *const libc::c_char,
            func: Some(luaB_rawequal),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"rawlen\0" as *const u8 as *const libc::c_char,
            func: Some(luaB_rawlen),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"rawget\0" as *const u8 as *const libc::c_char,
            func: Some(luaB_rawget),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"rawset\0" as *const u8 as *const libc::c_char,
            func: Some(luaB_rawset),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"select\0" as *const u8 as *const libc::c_char,
            func: Some(luaB_select),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"setmetatable\0" as *const u8 as *const libc::c_char,
            func: Some(luaB_setmetatable),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"tonumber\0" as *const u8 as *const libc::c_char,
            func: Some(luaB_tonumber),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"tostring\0" as *const u8 as *const libc::c_char,
            func: Some(luaB_tostring),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"type\0" as *const u8 as *const libc::c_char,
            func: Some(luaB_type),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"_G\0" as *const u8 as *const libc::c_char,
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

pub unsafe fn luaopen_base(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    lua_rawgeti(
        L,
        -(1000000 as libc::c_int) - 1000 as libc::c_int,
        2 as libc::c_int as i64,
    );
    luaL_setfuncs(
        L,
        &raw const base_funcs as *const luaL_Reg,
        0 as libc::c_int,
    )?;
    lua_pushvalue(L, -(1 as libc::c_int));
    lua_setfield(
        L,
        -(2 as libc::c_int),
        b"_G\0" as *const u8 as *const libc::c_char,
    )?;

    return Ok(1 as libc::c_int);
}
