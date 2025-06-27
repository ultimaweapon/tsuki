#![allow(
    dead_code,
    mutable_transmutes,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments,
    unused_mut
)]

pub type utfint = libc::c_uint;

unsafe extern "C" fn u_posrelat(mut pos: i64, mut len: usize) -> i64 {
    if pos >= 0 as libc::c_int as i64 {
        return pos;
    } else if (0 as libc::c_uint as usize).wrapping_sub(pos as usize) > len {
        return 0 as libc::c_int as i64;
    } else {
        return len as i64 + pos + 1 as libc::c_int as i64;
    };
}
unsafe extern "C" fn utf8_decode(
    mut s: *const libc::c_char,
    mut val: *mut utfint,
    mut strict: libc::c_int,
) -> *const libc::c_char {
    static mut limits: [utfint; 6] = [
        !(0 as libc::c_int as utfint),
        0x80 as libc::c_int as utfint,
        0x800 as libc::c_int as utfint,
        0x10000 as libc::c_uint,
        0x200000 as libc::c_uint,
        0x4000000 as libc::c_uint,
    ];
    let mut c: libc::c_uint = *s.offset(0 as libc::c_int as isize) as libc::c_uchar as libc::c_uint;
    let mut res: utfint = 0 as libc::c_int as utfint;
    if c < 0x80 as libc::c_int as libc::c_uint {
        res = c;
    } else {
        let mut count: libc::c_int = 0 as libc::c_int;
        while c & 0x40 as libc::c_int as libc::c_uint != 0 {
            count += 1;
            let mut cc: libc::c_uint = *s.offset(count as isize) as libc::c_uchar as libc::c_uint;
            if !(cc & 0xc0 as libc::c_int as libc::c_uint == 0x80 as libc::c_int as libc::c_uint) {
                return 0 as *const libc::c_char;
            }
            res = res << 6 as libc::c_int | cc & 0x3f as libc::c_int as libc::c_uint;
            c <<= 1 as libc::c_int;
        }
        res |= (c & 0x7f as libc::c_int as libc::c_uint) << count * 5 as libc::c_int;
        if count > 5 as libc::c_int
            || res > 0x7fffffff as libc::c_uint
            || res < limits[count as usize]
        {
            return 0 as *const libc::c_char;
        }
        s = s.offset(count as isize);
    }
    if strict != 0 {
        if res > 0x10ffff as libc::c_uint
            || 0xd800 as libc::c_uint <= res && res <= 0xdfff as libc::c_uint
        {
            return 0 as *const libc::c_char;
        }
    }
    if !val.is_null() {
        *val = res;
    }
    return s.offset(1 as libc::c_int as isize);
}
unsafe extern "C" fn utflen(mut L: *mut lua_State) -> libc::c_int {
    let mut n: i64 = 0 as libc::c_int as i64;
    let mut len: usize = 0;
    let mut s: *const libc::c_char = luaL_checklstring(L, 1 as libc::c_int, &mut len);
    let mut posi: i64 = u_posrelat(
        luaL_optinteger(L, 2 as libc::c_int, 1 as libc::c_int as i64),
        len,
    );
    let mut posj: i64 = u_posrelat(
        luaL_optinteger(L, 3 as libc::c_int, -(1 as libc::c_int) as i64),
        len,
    );
    let mut lax: libc::c_int = lua_toboolean(L, 4 as libc::c_int);
    (((1 as libc::c_int as i64 <= posi && {
        posi -= 1;
        posi <= len as i64
    }) as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
        || luaL_argerror(
            L,
            2 as libc::c_int,
            b"initial position out of bounds\0" as *const u8 as *const libc::c_char,
        ) != 0) as libc::c_int;
    posj -= 1;
    (((posj < len as i64) as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long != 0
        || luaL_argerror(
            L,
            3 as libc::c_int,
            b"final position out of bounds\0" as *const u8 as *const libc::c_char,
        ) != 0) as libc::c_int;
    while posi <= posj {
        let mut s1: *const libc::c_char = utf8_decode(
            s.offset(posi as isize),
            0 as *mut utfint,
            (lax == 0) as libc::c_int,
        );
        if s1.is_null() {
            lua_pushnil(L);
            lua_pushinteger(L, posi + 1 as libc::c_int as i64);
            return 2 as libc::c_int;
        }
        posi = s1.offset_from(s) as libc::c_long as i64;
        n += 1;
        n;
    }
    lua_pushinteger(L, n);
    return 1 as libc::c_int;
}
unsafe extern "C" fn codepoint(mut L: *mut lua_State) -> libc::c_int {
    let mut len: usize = 0;
    let mut s: *const libc::c_char = luaL_checklstring(L, 1 as libc::c_int, &mut len);
    let mut posi: i64 = u_posrelat(
        luaL_optinteger(L, 2 as libc::c_int, 1 as libc::c_int as i64),
        len,
    );
    let mut pose: i64 = u_posrelat(luaL_optinteger(L, 3 as libc::c_int, posi), len);
    let mut lax: libc::c_int = lua_toboolean(L, 4 as libc::c_int);
    let mut n: libc::c_int = 0;
    let mut se: *const libc::c_char = 0 as *const libc::c_char;
    (((posi >= 1 as libc::c_int as i64) as libc::c_int != 0 as libc::c_int) as libc::c_int
        as libc::c_long
        != 0
        || luaL_argerror(
            L,
            2 as libc::c_int,
            b"out of bounds\0" as *const u8 as *const libc::c_char,
        ) != 0) as libc::c_int;
    (((pose <= len as i64) as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long != 0
        || luaL_argerror(
            L,
            3 as libc::c_int,
            b"out of bounds\0" as *const u8 as *const libc::c_char,
        ) != 0) as libc::c_int;
    if posi > pose {
        return 0 as libc::c_int;
    }
    if pose - posi >= 2147483647 as libc::c_int as i64 {
        return luaL_error(
            L,
            b"string slice too long\0" as *const u8 as *const libc::c_char,
        );
    }
    n = (pose - posi) as libc::c_int + 1 as libc::c_int;
    luaL_checkstack(
        L,
        n,
        b"string slice too long\0" as *const u8 as *const libc::c_char,
    );
    n = 0 as libc::c_int;
    se = s.offset(pose as isize);
    s = s.offset((posi - 1 as libc::c_int as i64) as isize);
    while s < se {
        let mut code: utfint = 0;
        s = utf8_decode(s, &mut code, (lax == 0) as libc::c_int);
        if s.is_null() {
            return luaL_error(
                L,
                b"invalid UTF-8 code\0" as *const u8 as *const libc::c_char,
            );
        }
        lua_pushinteger(L, code as i64);
        n += 1;
        n;
    }
    return n;
}
unsafe extern "C" fn pushutfchar(mut L: *mut lua_State, mut arg: libc::c_int) {
    let mut code: u64 = luaL_checkinteger(L, arg) as u64;
    (((code <= 0x7fffffff as libc::c_uint as u64) as libc::c_int != 0 as libc::c_int) as libc::c_int
        as libc::c_long
        != 0
        || luaL_argerror(
            L,
            arg,
            b"value out of range\0" as *const u8 as *const libc::c_char,
        ) != 0) as libc::c_int;
    lua_pushfstring(
        L,
        b"%U\0" as *const u8 as *const libc::c_char,
        code as libc::c_long,
    );
}
unsafe extern "C" fn utfchar(mut L: *mut lua_State) -> libc::c_int {
    let mut n: libc::c_int = lua_gettop(L);
    if n == 1 as libc::c_int {
        pushutfchar(L, 1 as libc::c_int);
    } else {
        let mut i: libc::c_int = 0;
        let mut b: luaL_Buffer = luaL_Buffer {
            b: 0 as *mut libc::c_char,
            size: 0,
            n: 0,
            L: 0 as *mut lua_State,
            init: C2RustUnnamed { n: 0. },
        };
        luaL_buffinit(L, &mut b);
        i = 1 as libc::c_int;
        while i <= n {
            pushutfchar(L, i);
            luaL_addvalue(&mut b);
            i += 1;
            i;
        }
        luaL_pushresult(&mut b);
    }
    return 1 as libc::c_int;
}
unsafe extern "C" fn byteoffset(mut L: *mut lua_State) -> libc::c_int {
    let mut len: usize = 0;
    let mut s: *const libc::c_char = luaL_checklstring(L, 1 as libc::c_int, &mut len);
    let mut n: i64 = luaL_checkinteger(L, 2 as libc::c_int);
    let mut posi: i64 = (if n >= 0 as libc::c_int as i64 {
        1 as libc::c_int as usize
    } else {
        len.wrapping_add(1 as libc::c_int as usize)
    }) as i64;
    posi = u_posrelat(luaL_optinteger(L, 3 as libc::c_int, posi), len);
    (((1 as libc::c_int as i64 <= posi && {
        posi -= 1;
        posi <= len as i64
    }) as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
        || luaL_argerror(
            L,
            3 as libc::c_int,
            b"position out of bounds\0" as *const u8 as *const libc::c_char,
        ) != 0) as libc::c_int;
    if n == 0 as libc::c_int as i64 {
        while posi > 0 as libc::c_int as i64
            && *s.offset(posi as isize) as libc::c_int & 0xc0 as libc::c_int == 0x80 as libc::c_int
        {
            posi -= 1;
            posi;
        }
    } else {
        if *s.offset(posi as isize) as libc::c_int & 0xc0 as libc::c_int == 0x80 as libc::c_int {
            return luaL_error(
                L,
                b"initial position is a continuation byte\0" as *const u8 as *const libc::c_char,
            );
        }
        if n < 0 as libc::c_int as i64 {
            while n < 0 as libc::c_int as i64 && posi > 0 as libc::c_int as i64 {
                loop {
                    posi -= 1;
                    posi;
                    if !(posi > 0 as libc::c_int as i64
                        && *s.offset(posi as isize) as libc::c_int & 0xc0 as libc::c_int
                            == 0x80 as libc::c_int)
                    {
                        break;
                    }
                }
                n += 1;
                n;
            }
        } else {
            n -= 1;
            n;
            while n > 0 as libc::c_int as i64 && posi < len as i64 {
                loop {
                    posi += 1;
                    posi;
                    if !(*s.offset(posi as isize) as libc::c_int & 0xc0 as libc::c_int
                        == 0x80 as libc::c_int)
                    {
                        break;
                    }
                }
                n -= 1;
                n;
            }
        }
    }
    if n == 0 as libc::c_int as i64 {
        lua_pushinteger(L, posi + 1 as libc::c_int as i64);
    } else {
        lua_pushnil(L);
    }
    return 1 as libc::c_int;
}
unsafe extern "C" fn iter_aux(mut L: *mut lua_State, mut strict: libc::c_int) -> libc::c_int {
    let mut len: usize = 0;
    let mut s: *const libc::c_char = luaL_checklstring(L, 1 as libc::c_int, &mut len);
    let mut n: u64 = lua_tointegerx(L, 2 as libc::c_int, 0 as *mut libc::c_int) as u64;
    if n < len as u64 {
        while *s.offset(n as isize) as libc::c_int & 0xc0 as libc::c_int == 0x80 as libc::c_int {
            n = n.wrapping_add(1);
            n;
        }
    }
    if n >= len as u64 {
        return 0 as libc::c_int;
    } else {
        let mut code: utfint = 0;
        let mut next: *const libc::c_char = utf8_decode(s.offset(n as isize), &mut code, strict);
        if next.is_null() || *next as libc::c_int & 0xc0 as libc::c_int == 0x80 as libc::c_int {
            return luaL_error(
                L,
                b"invalid UTF-8 code\0" as *const u8 as *const libc::c_char,
            );
        }
        lua_pushinteger(L, n.wrapping_add(1 as libc::c_int as u64) as i64);
        lua_pushinteger(L, code as i64);
        return 2 as libc::c_int;
    };
}
unsafe extern "C" fn iter_auxstrict(mut L: *mut lua_State) -> libc::c_int {
    return iter_aux(L, 1 as libc::c_int);
}
unsafe extern "C" fn iter_auxlax(mut L: *mut lua_State) -> libc::c_int {
    return iter_aux(L, 0 as libc::c_int);
}
unsafe extern "C" fn iter_codes(mut L: *mut lua_State) -> libc::c_int {
    let mut lax: libc::c_int = lua_toboolean(L, 2 as libc::c_int);
    let mut s: *const libc::c_char = luaL_checklstring(L, 1 as libc::c_int, 0 as *mut usize);
    ((!(*s as libc::c_int & 0xc0 as libc::c_int == 0x80 as libc::c_int) as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
        || luaL_argerror(
            L,
            1 as libc::c_int,
            b"invalid UTF-8 code\0" as *const u8 as *const libc::c_char,
        ) != 0) as libc::c_int;
    lua_pushcclosure(
        L,
        if lax != 0 {
            Some(iter_auxlax as unsafe extern "C" fn(*mut lua_State) -> libc::c_int)
        } else {
            Some(iter_auxstrict as unsafe extern "C" fn(*mut lua_State) -> libc::c_int)
        },
        0 as libc::c_int,
    );
    lua_pushvalue(L, 1 as libc::c_int);
    lua_pushinteger(L, 0 as libc::c_int as i64);
    return 3 as libc::c_int;
}
static mut funcs: [luaL_Reg; 7] = unsafe {
    [
        {
            let mut init = luaL_Reg {
                name: b"offset\0" as *const u8 as *const libc::c_char,
                func: Some(byteoffset as unsafe extern "C" fn(*mut lua_State) -> libc::c_int),
            };
            init
        },
        {
            let mut init = luaL_Reg {
                name: b"codepoint\0" as *const u8 as *const libc::c_char,
                func: Some(codepoint as unsafe extern "C" fn(*mut lua_State) -> libc::c_int),
            };
            init
        },
        {
            let mut init = luaL_Reg {
                name: b"char\0" as *const u8 as *const libc::c_char,
                func: Some(utfchar as unsafe extern "C" fn(*mut lua_State) -> libc::c_int),
            };
            init
        },
        {
            let mut init = luaL_Reg {
                name: b"len\0" as *const u8 as *const libc::c_char,
                func: Some(utflen as unsafe extern "C" fn(*mut lua_State) -> libc::c_int),
            };
            init
        },
        {
            let mut init = luaL_Reg {
                name: b"codes\0" as *const u8 as *const libc::c_char,
                func: Some(iter_codes as unsafe extern "C" fn(*mut lua_State) -> libc::c_int),
            };
            init
        },
        {
            let mut init = luaL_Reg {
                name: b"charpattern\0" as *const u8 as *const libc::c_char,
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
    ]
};
#[no_mangle]
pub unsafe extern "C" fn luaopen_utf8(mut L: *mut lua_State) -> libc::c_int {
    luaL_checkversion_(
        L,
        504 as libc::c_int as f64,
        (::core::mem::size_of::<i64>() as libc::c_ulong)
            .wrapping_mul(16 as libc::c_int as libc::c_ulong)
            .wrapping_add(::core::mem::size_of::<f64>() as libc::c_ulong),
    );
    lua_createtable(
        L,
        0 as libc::c_int,
        (::core::mem::size_of::<[luaL_Reg; 7]>() as libc::c_ulong)
            .wrapping_div(::core::mem::size_of::<luaL_Reg>() as libc::c_ulong)
            .wrapping_sub(1 as libc::c_int as libc::c_ulong) as libc::c_int,
    );
    luaL_setfuncs(L, funcs.as_ptr(), 0 as libc::c_int);
    lua_pushlstring(
        L,
        b"[\0-\x7F\xC2-\xFD][\x80-\xBF]*\0" as *const u8 as *const libc::c_char,
        (::core::mem::size_of::<[libc::c_char; 15]>() as libc::c_ulong)
            .wrapping_div(::core::mem::size_of::<libc::c_char>() as libc::c_ulong)
            .wrapping_sub(1 as libc::c_int as libc::c_ulong),
    );
    lua_setfield(
        L,
        -(2 as libc::c_int),
        b"charpattern\0" as *const u8 as *const libc::c_char,
    );
    return 1 as libc::c_int;
}
