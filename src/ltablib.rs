#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments
)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::lapi::{
    lua_call, lua_checkstack, lua_compare, lua_createtable, lua_geti, lua_getmetatable, lua_gettop,
    lua_isstring, lua_pushinteger, lua_pushlstring, lua_pushnil, lua_pushstring, lua_pushvalue,
    lua_rawget, lua_rotate, lua_setfield, lua_seti, lua_toboolean, lua_tolstring, lua_type,
    lua_typename,
};
use crate::lauxlib::{
    luaL_Reg, luaL_argerror, luaL_checkinteger, luaL_checktype, luaL_error, luaL_len,
    luaL_optinteger, luaL_optlstring, luaL_setfuncs,
};
use crate::{Thread, lua_pop, lua_settop};
use std::boxed::Box;
use std::ffi::c_int;
use std::vec::Vec;

type IdxT = libc::c_uint;

unsafe fn checkfield(L: *const Thread, key: *const libc::c_char, n: libc::c_int) -> c_int {
    lua_pushstring(L, key);
    (lua_rawget(L, -n) != 0 as libc::c_int) as libc::c_int
}

unsafe fn checktab(
    L: *const Thread,
    arg: libc::c_int,
    what: libc::c_int,
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

unsafe fn tinsert(L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
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

unsafe fn tremove(L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    checktab(
        L,
        1 as libc::c_int,
        1 as libc::c_int | 2 as libc::c_int | 4 as libc::c_int,
    )?;
    let size: i64 = luaL_len(L, 1 as libc::c_int)?;
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

unsafe fn tmove(L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let f: i64 = luaL_checkinteger(L, 2 as libc::c_int)?;
    let e: i64 = luaL_checkinteger(L, 3 as libc::c_int)?;
    let t: i64 = luaL_checkinteger(L, 4 as libc::c_int)?;
    let tt: libc::c_int = if !(lua_type(L, 5 as libc::c_int) <= 0 as libc::c_int) {
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
    L: *const Thread,
    b: &mut Vec<u8>,
    i: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    lua_geti(L, 1 as libc::c_int, i)?;
    if ((lua_isstring(L, -(1 as libc::c_int)) == 0) as libc::c_int != 0 as libc::c_int)
        as libc::c_int as libc::c_long
        != 0
    {
        luaL_error(
            L,
            format_args!(
                "invalid value ({}) at index {} in table for 'concat'",
                lua_typename(lua_type(L, -1)),
                i
            ),
        )?;
    }

    let mut l = 0;
    let s = lua_tolstring(L, -1, &mut l);
    b.extend_from_slice(std::slice::from_raw_parts(s.cast(), l));
    lua_pop(L, 1)?;
    Ok(())
}

unsafe fn tconcat(L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    checktab(L, 1 as libc::c_int, 1 as libc::c_int | 4 as libc::c_int)?;
    let mut last: i64 = luaL_len(L, 1 as libc::c_int)?;
    let mut lsep: usize = 0;
    let sep: *const libc::c_char = luaL_optlstring(
        L,
        2 as libc::c_int,
        b"\0" as *const u8 as *const libc::c_char,
        &mut lsep,
    )?;
    let sep = std::slice::from_raw_parts(sep.cast(), lsep);
    let mut i: i64 = luaL_optinteger(L, 3 as libc::c_int, 1 as libc::c_int as i64)?;
    let mut b = Vec::new();
    last = luaL_optinteger(L, 4 as libc::c_int, last)?;

    while i < last {
        addfield(L, &mut b, i)?;
        b.extend_from_slice(sep);
        i += 1;
    }

    if i == last {
        addfield(L, &mut b, i)?;
    }

    lua_pushlstring(L, b);

    return Ok(1 as libc::c_int);
}

unsafe fn tpack(L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut i: libc::c_int = 0;
    let n: libc::c_int = lua_gettop(L);
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

unsafe fn tunpack(L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut n: u64 = 0;
    let mut i = luaL_optinteger(L, 2, 1)?;
    let e = if lua_type(L, 3) <= 0 {
        luaL_len(L, 1)?
    } else {
        luaL_checkinteger(L, 3)?
    };

    if i > e {
        return Ok(0);
    }

    n = (e as u64).wrapping_sub(i as u64);

    if n >= 2147483647 || {
        n = n.wrapping_add(1);
        lua_checkstack(L, n.try_into().unwrap()).is_err()
    } {
        return luaL_error(L, "too many results to unpack");
    }

    while i < e {
        lua_geti(L, 1 as libc::c_int, i)?;
        i += 1;
    }

    lua_geti(L, 1, e)?;

    Ok(n as libc::c_int)
}

unsafe fn set2(L: *const Thread, i: IdxT, j: IdxT) -> Result<(), Box<dyn std::error::Error>> {
    lua_seti(L, 1 as libc::c_int, i as i64)?;
    lua_seti(L, 1 as libc::c_int, j as i64)
}

unsafe fn sort_comp(
    L: *const Thread,
    a: libc::c_int,
    b: libc::c_int,
) -> Result<c_int, Box<dyn std::error::Error>> {
    if lua_type(L, 2 as libc::c_int) == 0 as libc::c_int {
        return lua_compare(L, a, b, 1 as libc::c_int);
    } else {
        let mut res: libc::c_int = 0;
        lua_pushvalue(L, 2 as libc::c_int);
        lua_pushvalue(L, a - 1 as libc::c_int);
        lua_pushvalue(L, b - 2 as libc::c_int);
        lua_call(L, 2 as libc::c_int, 1 as libc::c_int)?;
        res = lua_toboolean(L, -(1 as libc::c_int));
        lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int)?;
        return Ok(res);
    };
}

unsafe fn partition(
    L: *const Thread,
    lo: IdxT,
    up: IdxT,
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

unsafe fn choosePivot(lo: IdxT, up: IdxT, rnd: libc::c_uint) -> IdxT {
    let r4: IdxT = up.wrapping_sub(lo) / 4 as libc::c_int as IdxT;
    let p: IdxT = rnd
        .wrapping_rem(r4 * 2 as libc::c_int as IdxT)
        .wrapping_add(lo.wrapping_add(r4));
    return p;
}

unsafe fn auxsort(
    L: *const Thread,
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

unsafe fn sort(L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    checktab(
        L,
        1 as libc::c_int,
        1 as libc::c_int | 2 as libc::c_int | 4 as libc::c_int,
    )?;
    let n: i64 = luaL_len(L, 1 as libc::c_int)?;
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
        let init = luaL_Reg {
            name: b"concat\0" as *const u8 as *const libc::c_char,
            func: Some(tconcat),
        };
        init
    },
    {
        let init = luaL_Reg {
            name: b"insert\0" as *const u8 as *const libc::c_char,
            func: Some(tinsert),
        };
        init
    },
    {
        let init = luaL_Reg {
            name: b"pack\0" as *const u8 as *const libc::c_char,
            func: Some(tpack),
        };
        init
    },
    {
        let init = luaL_Reg {
            name: b"unpack\0" as *const u8 as *const libc::c_char,
            func: Some(tunpack),
        };
        init
    },
    {
        let init = luaL_Reg {
            name: b"remove\0" as *const u8 as *const libc::c_char,
            func: Some(tremove),
        };
        init
    },
    {
        let init = luaL_Reg {
            name: b"move\0" as *const u8 as *const libc::c_char,
            func: Some(tmove),
        };
        init
    },
    {
        let init = luaL_Reg {
            name: b"sort\0" as *const u8 as *const libc::c_char,
            func: Some(sort),
        };
        init
    },
    {
        let init = luaL_Reg {
            name: 0 as *const libc::c_char,
            func: None,
        };
        init
    },
];

pub unsafe fn luaopen_table(L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
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
