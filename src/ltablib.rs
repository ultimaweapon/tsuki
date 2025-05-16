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

use crate::lapi::{lua_checkstack, lua_compare, lua_geti, lua_getmetatable, lua_rawget, lua_seti};
use crate::lauxlib::luaL_len;
use crate::lstate::lua_KContext;
use crate::{
    C2RustUnnamed, lua_State, lua_callk, lua_createtable, lua_gettop, lua_isstring,
    lua_pushinteger, lua_pushnil, lua_pushstring, lua_pushvalue, lua_rotate, lua_setfield,
    lua_settop, lua_toboolean, lua_type, lua_typename, luaL_Buffer, luaL_Reg, luaL_addlstring,
    luaL_addvalue, luaL_argerror, luaL_buffinit, luaL_checkinteger, luaL_checktype, luaL_error,
    luaL_optinteger, luaL_optlstring, luaL_pushresult, luaL_setfuncs,
};
use std::ffi::c_int;

pub type IdxT = libc::c_uint;

unsafe extern "C" fn checkfield(
    mut L: *mut lua_State,
    mut key: *const libc::c_char,
    mut n: libc::c_int,
) -> libc::c_int {
    lua_pushstring(L, key);
    return (lua_rawget(L, -n) != 0 as libc::c_int) as libc::c_int;
}

unsafe fn checktab(
    mut L: *mut lua_State,
    mut arg: libc::c_int,
    mut what: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    if lua_type(L, arg) != 5 as c_int {
        let mut n: libc::c_int = 1 as libc::c_int;
        if lua_getmetatable(L, arg) != 0
            && (what & 1 as libc::c_int == 0 || {
                n += 1;
                checkfield(L, b"__index\0" as *const u8 as *const libc::c_char, n) != 0
            })
            && (what & 2 as libc::c_int == 0 || {
                n += 1;
                checkfield(L, b"__newindex\0" as *const u8 as *const libc::c_char, n) != 0
            })
            && (what & 4 as libc::c_int == 0 || {
                n += 1;
                checkfield(L, b"__len\0" as *const u8 as *const libc::c_char, n) != 0
            })
        {
            lua_settop(L, -n - 1)?;
        } else {
            luaL_checktype(L, arg, 5)?;
        }
    }

    Ok(())
}

unsafe fn tinsert(mut L: *mut lua_State) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut pos: i64 = 0;
    checktab(
        L,
        1 as libc::c_int,
        1 as libc::c_int | 2 as libc::c_int | 4 as libc::c_int,
    )?;
    let mut e: i64 = luaL_len(L, 1 as libc::c_int)?;
    e = (e as u64).wrapping_add(1 as libc::c_int as u64) as i64;
    match lua_gettop(L) {
        2 => {
            pos = e;
        }
        3 => {
            let mut i: i64 = 0;
            pos = luaL_checkinteger(L, 2 as libc::c_int)?;
            ((((pos as u64).wrapping_sub(1 as libc::c_uint as u64) < e as u64) as libc::c_int
                != 0 as libc::c_int) as libc::c_int as libc::c_long
                != 0
                || luaL_argerror(L, 2 as libc::c_int, "position out of bounds")? != 0)
                as libc::c_int;
            i = e;
            while i > pos {
                lua_geti(L, 1 as libc::c_int, i - 1 as libc::c_int as i64)?;
                lua_seti(L, 1 as libc::c_int, i)?;
                i -= 1;
            }
        }
        _ => {
            return luaL_error(L, "wrong number of arguments to 'insert'");
        }
    }
    lua_seti(L, 1 as libc::c_int, pos)?;
    return Ok(0 as libc::c_int);
}

unsafe fn tremove(mut L: *mut lua_State) -> Result<c_int, Box<dyn std::error::Error>> {
    checktab(
        L,
        1 as libc::c_int,
        1 as libc::c_int | 2 as libc::c_int | 4 as libc::c_int,
    )?;
    let mut size: i64 = luaL_len(L, 1 as libc::c_int)?;
    let mut pos: i64 = luaL_optinteger(L, 2 as libc::c_int, size)?;
    if pos != size {
        ((((pos as u64).wrapping_sub(1 as libc::c_uint as u64) <= size as u64) as libc::c_int
            != 0 as libc::c_int) as libc::c_int as libc::c_long
            != 0
            || luaL_argerror(L, 2 as libc::c_int, "position out of bounds")? != 0)
            as libc::c_int;
    }
    lua_geti(L, 1 as libc::c_int, pos)?;
    while pos < size {
        lua_geti(L, 1 as libc::c_int, pos + 1 as libc::c_int as i64)?;
        lua_seti(L, 1 as libc::c_int, pos)?;
        pos += 1;
    }
    lua_pushnil(L);
    lua_seti(L, 1 as libc::c_int, pos)?;
    return Ok(1 as libc::c_int);
}

unsafe fn tmove(mut L: *mut lua_State) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut f: i64 = luaL_checkinteger(L, 2 as libc::c_int)?;
    let mut e: i64 = luaL_checkinteger(L, 3 as libc::c_int)?;
    let mut t: i64 = luaL_checkinteger(L, 4 as libc::c_int)?;
    let mut tt: libc::c_int = if !(lua_type(L, 5 as libc::c_int) <= 0 as libc::c_int) {
        5 as libc::c_int
    } else {
        1 as libc::c_int
    };
    checktab(L, 1 as libc::c_int, 1 as libc::c_int)?;
    checktab(L, tt, 2 as libc::c_int)?;
    if e >= f {
        let mut n: i64 = 0;
        let mut i: i64 = 0;
        (((f > 0 as libc::c_int as i64 || e < 0x7fffffffffffffff as libc::c_longlong + f)
            as libc::c_int
            != 0 as libc::c_int) as libc::c_int as libc::c_long
            != 0
            || luaL_argerror(L, 3 as libc::c_int, "too many elements to move")? != 0)
            as libc::c_int;
        n = e - f + 1 as libc::c_int as i64;
        (((t <= 0x7fffffffffffffff as libc::c_longlong - n + 1 as libc::c_int as libc::c_longlong)
            as libc::c_int
            != 0 as libc::c_int) as libc::c_int as libc::c_long
            != 0
            || luaL_argerror(L, 4 as libc::c_int, "destination wrap around")? != 0)
            as libc::c_int;
        if t > e || t <= f || tt != 1 && lua_compare(L, 1, tt, 0)? == 0 {
            i = 0 as libc::c_int as i64;
            while i < n {
                lua_geti(L, 1 as libc::c_int, f + i)?;
                lua_seti(L, tt, t + i)?;
                i += 1;
            }
        } else {
            i = n - 1 as libc::c_int as i64;
            while i >= 0 as libc::c_int as i64 {
                lua_geti(L, 1 as libc::c_int, f + i)?;
                lua_seti(L, tt, t + i)?;
                i -= 1;
            }
        }
    }
    lua_pushvalue(L, tt);
    return Ok(1 as libc::c_int);
}

unsafe fn addfield(
    mut L: *mut lua_State,
    mut b: *mut luaL_Buffer,
    mut i: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    lua_geti(L, 1 as libc::c_int, i)?;
    if ((lua_isstring(L, -(1 as libc::c_int)) == 0) as libc::c_int != 0 as libc::c_int)
        as libc::c_int as libc::c_long
        != 0
    {
        luaL_error(
            L,
            format!(
                "invalid value ({}) at index {} in table for 'concat'",
                lua_typename(L, lua_type(L, -1)),
                i
            ),
        )?;
    }
    luaL_addvalue(b)
}

unsafe fn tconcat(mut L: *mut lua_State) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut b: luaL_Buffer = luaL_Buffer {
        b: 0 as *mut libc::c_char,
        size: 0,
        n: 0,
        L: 0 as *mut lua_State,
        init: C2RustUnnamed { n: 0. },
    };
    checktab(L, 1 as libc::c_int, 1 as libc::c_int | 4 as libc::c_int)?;
    let mut last: i64 = luaL_len(L, 1 as libc::c_int)?;
    let mut lsep: usize = 0;
    let mut sep: *const libc::c_char = luaL_optlstring(
        L,
        2 as libc::c_int,
        b"\0" as *const u8 as *const libc::c_char,
        &mut lsep,
    )?;
    let mut i: i64 = luaL_optinteger(L, 3 as libc::c_int, 1 as libc::c_int as i64)?;
    last = luaL_optinteger(L, 4 as libc::c_int, last)?;
    luaL_buffinit(L, &mut b);
    while i < last {
        addfield(L, &mut b, i)?;
        luaL_addlstring(&mut b, sep, lsep)?;
        i += 1;
    }
    if i == last {
        addfield(L, &mut b, i)?;
    }
    luaL_pushresult(&mut b)?;
    return Ok(1 as libc::c_int);
}

unsafe fn tpack(mut L: *mut lua_State) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut i: libc::c_int = 0;
    let mut n: libc::c_int = lua_gettop(L);
    lua_createtable(L, n, 1 as libc::c_int);
    lua_rotate(L, 1 as libc::c_int, 1 as libc::c_int);
    i = n;
    while i >= 1 as libc::c_int {
        lua_seti(L, 1 as libc::c_int, i as i64)?;
        i -= 1;
    }
    lua_pushinteger(L, n as i64);
    lua_setfield(
        L,
        1 as libc::c_int,
        b"n\0" as *const u8 as *const libc::c_char,
    )?;
    return Ok(1 as libc::c_int);
}

unsafe fn tunpack(mut L: *mut lua_State) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut n: u64 = 0;
    let mut i: i64 = luaL_optinteger(L, 2 as libc::c_int, 1 as libc::c_int as i64)?;
    let mut e: i64 = if lua_type(L, 3 as libc::c_int) <= 0 as libc::c_int {
        luaL_len(L, 1 as libc::c_int)?
    } else {
        luaL_checkinteger(L, 3 as libc::c_int)?
    };
    if i > e {
        return Ok(0 as libc::c_int);
    }
    n = (e as u64).wrapping_sub(i as u64);
    if ((n >= 2147483647 as libc::c_int as libc::c_uint as u64 || {
        n = n.wrapping_add(1);
        lua_checkstack(L, n as libc::c_int) == 0
    }) as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        return luaL_error(L, "too many results to unpack");
    }
    while i < e {
        lua_geti(L, 1 as libc::c_int, i)?;
        i += 1;
    }
    lua_geti(L, 1 as libc::c_int, e)?;
    return Ok(n as libc::c_int);
}

unsafe fn set2(
    mut L: *mut lua_State,
    mut i: IdxT,
    mut j: IdxT,
) -> Result<(), Box<dyn std::error::Error>> {
    lua_seti(L, 1 as libc::c_int, i as i64)?;
    lua_seti(L, 1 as libc::c_int, j as i64)
}

unsafe fn sort_comp(
    mut L: *mut lua_State,
    mut a: libc::c_int,
    mut b: libc::c_int,
) -> Result<c_int, Box<dyn std::error::Error>> {
    if lua_type(L, 2 as libc::c_int) == 0 as libc::c_int {
        return lua_compare(L, a, b, 1 as libc::c_int);
    } else {
        let mut res: libc::c_int = 0;
        lua_pushvalue(L, 2 as libc::c_int);
        lua_pushvalue(L, a - 1 as libc::c_int);
        lua_pushvalue(L, b - 2 as libc::c_int);
        lua_callk(
            L,
            2 as libc::c_int,
            1 as libc::c_int,
            0 as libc::c_int as lua_KContext,
            None,
        )?;
        res = lua_toboolean(L, -(1 as libc::c_int));
        lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
        return Ok(res);
    };
}

unsafe fn partition(
    mut L: *mut lua_State,
    mut lo: IdxT,
    mut up: IdxT,
) -> Result<IdxT, Box<dyn std::error::Error>> {
    let mut i: IdxT = lo;
    let mut j: IdxT = up.wrapping_sub(1 as libc::c_int as IdxT);
    loop {
        loop {
            i = i.wrapping_add(1);
            lua_geti(L, 1 as libc::c_int, i as i64)?;
            if !(sort_comp(L, -1, -2)? != 0) {
                break;
            }
            if ((i == up.wrapping_sub(1 as libc::c_int as IdxT)) as libc::c_int != 0 as libc::c_int)
                as libc::c_int as libc::c_long
                != 0
            {
                luaL_error(L, "invalid order function for sorting")?;
            }
            lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
        }
        loop {
            j = j.wrapping_sub(1);
            lua_geti(L, 1 as libc::c_int, j as i64)?;
            if !(sort_comp(L, -3, -1)? != 0) {
                break;
            }
            if ((j < i) as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long != 0 {
                luaL_error(L, "invalid order function for sorting")?;
            }
            lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
        }
        if j < i {
            lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
            set2(L, up.wrapping_sub(1 as libc::c_int as IdxT), i)?;
            return Ok(i);
        }
        set2(L, i, j)?;
    }
}

unsafe extern "C" fn choosePivot(mut lo: IdxT, mut up: IdxT, mut rnd: libc::c_uint) -> IdxT {
    let mut r4: IdxT = up.wrapping_sub(lo) / 4 as libc::c_int as IdxT;
    let mut p: IdxT = rnd
        .wrapping_rem(r4 * 2 as libc::c_int as IdxT)
        .wrapping_add(lo.wrapping_add(r4));
    return p;
}

unsafe fn auxsort(
    mut L: *mut lua_State,
    mut lo: IdxT,
    mut up: IdxT,
    mut rnd: libc::c_uint,
) -> Result<(), Box<dyn std::error::Error>> {
    while lo < up {
        let mut p: IdxT = 0;
        let mut n: IdxT = 0;
        lua_geti(L, 1 as libc::c_int, lo as i64)?;
        lua_geti(L, 1 as libc::c_int, up as i64)?;
        if sort_comp(L, -1, -2)? != 0 {
            set2(L, lo, up)?;
        } else {
            lua_settop(L, -(2 as libc::c_int) - 1 as libc::c_int)?;
        }
        if up.wrapping_sub(lo) == 1 as libc::c_int as IdxT {
            return Ok(());
        }
        if up.wrapping_sub(lo) < 100 as libc::c_uint || rnd == 0 as libc::c_int as libc::c_uint {
            p = lo.wrapping_add(up) / 2 as libc::c_int as IdxT;
        } else {
            p = choosePivot(lo, up, rnd);
        }
        lua_geti(L, 1 as libc::c_int, p as i64)?;
        lua_geti(L, 1 as libc::c_int, lo as i64)?;
        if sort_comp(L, -2, -1)? != 0 {
            set2(L, p, lo)?;
        } else {
            lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
            lua_geti(L, 1 as libc::c_int, up as i64)?;
            if sort_comp(L, -1, -2)? != 0 {
                set2(L, p, up)?;
            } else {
                lua_settop(L, -(2 as libc::c_int) - 1 as libc::c_int)?;
            }
        }
        if up.wrapping_sub(lo) == 2 as libc::c_int as IdxT {
            return Ok(());
        }
        lua_geti(L, 1 as libc::c_int, p as i64)?;
        lua_pushvalue(L, -(1 as libc::c_int));
        lua_geti(
            L,
            1 as libc::c_int,
            up.wrapping_sub(1 as libc::c_int as IdxT) as i64,
        )?;
        set2(L, p, up.wrapping_sub(1 as libc::c_int as IdxT))?;
        p = partition(L, lo, up)?;
        if p.wrapping_sub(lo) < up.wrapping_sub(p) {
            auxsort(L, lo, p.wrapping_sub(1 as libc::c_int as IdxT), rnd)?;
            n = p.wrapping_sub(lo);
            lo = p.wrapping_add(1 as libc::c_int as IdxT);
        } else {
            auxsort(L, p.wrapping_add(1 as libc::c_int as IdxT), up, rnd)?;
            n = up.wrapping_sub(p);
            up = p.wrapping_sub(1 as libc::c_int as IdxT);
        }
        if up.wrapping_sub(lo) / 128 as libc::c_int as IdxT > n {
            rnd = rand::random();
        }
    }

    Ok(())
}

unsafe fn sort(mut L: *mut lua_State) -> Result<c_int, Box<dyn std::error::Error>> {
    checktab(
        L,
        1 as libc::c_int,
        1 as libc::c_int | 2 as libc::c_int | 4 as libc::c_int,
    )?;
    let mut n: i64 = luaL_len(L, 1 as libc::c_int)?;
    if n > 1 as libc::c_int as i64 {
        (((n < 2147483647 as libc::c_int as i64) as libc::c_int != 0 as libc::c_int) as libc::c_int
            as libc::c_long
            != 0
            || luaL_argerror(L, 1 as libc::c_int, "array too big")? != 0) as libc::c_int;
        if !(lua_type(L, 2 as libc::c_int) <= 0 as libc::c_int) {
            luaL_checktype(L, 2 as libc::c_int, 6 as libc::c_int)?;
        }
        lua_settop(L, 2 as libc::c_int)?;
        auxsort(
            L,
            1 as libc::c_int as IdxT,
            n as IdxT,
            0 as libc::c_int as libc::c_uint,
        )?;
    }
    return Ok(0 as libc::c_int);
}

static mut tab_funcs: [luaL_Reg; 8] = [
    {
        let mut init = luaL_Reg {
            name: b"concat\0" as *const u8 as *const libc::c_char,
            func: Some(tconcat),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"insert\0" as *const u8 as *const libc::c_char,
            func: Some(tinsert),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"pack\0" as *const u8 as *const libc::c_char,
            func: Some(tpack),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"unpack\0" as *const u8 as *const libc::c_char,
            func: Some(tunpack),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"remove\0" as *const u8 as *const libc::c_char,
            func: Some(tremove),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"move\0" as *const u8 as *const libc::c_char,
            func: Some(tmove),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"sort\0" as *const u8 as *const libc::c_char,
            func: Some(sort),
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

pub unsafe fn luaopen_table(mut L: *mut lua_State) -> Result<c_int, Box<dyn std::error::Error>> {
    lua_createtable(
        L,
        0 as libc::c_int,
        (::core::mem::size_of::<[luaL_Reg; 8]>() as libc::c_ulong)
            .wrapping_div(::core::mem::size_of::<luaL_Reg>() as libc::c_ulong)
            .wrapping_sub(1 as libc::c_int as libc::c_ulong) as libc::c_int,
    );
    luaL_setfuncs(L, &raw const tab_funcs as *const luaL_Reg, 0 as libc::c_int)?;
    return Ok(1 as libc::c_int);
}
