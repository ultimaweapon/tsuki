use crate::lapi::{
    lua_arith, lua_call, lua_createtable, lua_gettable, lua_gettop, lua_isinteger, lua_isstring,
    lua_newuserdatauv, lua_pushcclosure, lua_pushinteger, lua_pushlstring, lua_pushnil,
    lua_pushnumber, lua_pushstring, lua_pushvalue, lua_rotate, lua_setfield, lua_setmetatable,
    lua_stringtonumber, lua_toboolean, lua_tointegerx, lua_tolstring, lua_tonumberx, lua_topointer,
    lua_touserdata, lua_type, lua_typename,
};
use crate::lauxlib::{
    luaL_Reg, luaL_argerror, luaL_checkinteger, luaL_checklstring, luaL_checknumber,
    luaL_checkstack, luaL_error, luaL_getmetafield, luaL_optinteger, luaL_optlstring,
    luaL_setfuncs, luaL_tolstring, luaL_typeerror,
};
use crate::{Thread, lua_pop, lua_settop};
use libc::{
    isalnum, isalpha, iscntrl, isdigit, isgraph, islower, ispunct, isspace, isupper, isxdigit,
    memchr, memcmp, memcpy, snprintf, strchr, strcpy, strlen, strpbrk, strspn, tolower,
};
use std::boxed::Box;
use std::ffi::{CStr, c_int};
use std::format;
use std::vec::Vec;

pub const Knop: KOption = 10;
pub const Kpadding: KOption = 8;
pub const Kpaddalign: KOption = 9;
pub const Kzstr: KOption = 7;

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Header {
    pub L: *const Thread,
    pub islittle: libc::c_int,
    pub maxalign: libc::c_int,
}

pub const Kstring: KOption = 6;
pub const Kchar: KOption = 5;

#[derive(Copy, Clone)]
#[repr(C)]
pub union C2RustUnnamed_0 {
    pub dummy: libc::c_int,
    pub little: libc::c_char,
}

pub const Kdouble: KOption = 4;
pub const Knumber: KOption = 3;
pub const Kfloat: KOption = 2;
pub const Kint: KOption = 0;
pub type KOption = libc::c_uint;
pub const Kuint: KOption = 1;

#[derive(Copy, Clone)]
#[repr(C)]
pub struct GMatchState {
    pub src: *const libc::c_char,
    pub p: *const libc::c_char,
    pub lastmatch: *const libc::c_char,
    pub ms: MatchState,
}

unsafe fn posrelatI(mut pos: i64, mut len: usize) -> usize {
    if pos > 0 as libc::c_int as i64 {
        return pos as usize;
    } else if pos == 0 as libc::c_int as i64 {
        return 1 as libc::c_int as usize;
    } else if pos < -(len as i64) {
        return 1 as libc::c_int as usize;
    } else {
        return len
            .wrapping_add(pos as usize)
            .wrapping_add(1 as libc::c_int as usize);
    };
}

unsafe fn str_reverse(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut l: usize = 0;
    let mut s: *const libc::c_char = luaL_checklstring(L, 1 as libc::c_int, &mut l)?;
    let mut s = std::slice::from_raw_parts(s.cast(), l).to_vec();

    s.reverse();
    lua_pushlstring(L, s);

    return Ok(1 as libc::c_int);
}

unsafe fn str_lower(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut l: usize = 0;
    let mut s: *const libc::c_char = luaL_checklstring(L, 1 as libc::c_int, &mut l)?;
    let mut s = std::slice::from_raw_parts(s.cast::<u8>(), l).to_vec();

    for b in &mut s {
        b.make_ascii_lowercase();
    }

    lua_pushlstring(L, s);

    return Ok(1 as libc::c_int);
}

unsafe fn str_upper(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut l: usize = 0;
    let mut s: *const libc::c_char = luaL_checklstring(L, 1 as libc::c_int, &mut l)?;
    let mut s = std::slice::from_raw_parts(s.cast::<u8>(), l).to_vec();

    for b in &mut s {
        b.make_ascii_uppercase();
    }

    lua_pushlstring(L, s);

    return Ok(1 as libc::c_int);
}

unsafe fn str_char(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut n: libc::c_int = lua_gettop(L);
    let mut b = Vec::with_capacity(n.try_into().unwrap());
    let mut i = 1 as libc::c_int;

    while i <= n {
        let mut c: u64 = luaL_checkinteger(L, i)? as u64;
        ((c <= 255 as libc::c_int as u64) || luaL_argerror(L, i, "value out of range")? != 0)
            as libc::c_int;
        b.push(c as u8);
        i += 1;
    }

    lua_pushlstring(L, b);

    return Ok(1 as libc::c_int);
}

unsafe fn arith_mul(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    return arith(
        L,
        2 as libc::c_int,
        b"__mul\0" as *const u8 as *const libc::c_char,
    );
}

unsafe fn arith_div(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    return arith(
        L,
        5 as libc::c_int,
        b"__div\0" as *const u8 as *const libc::c_char,
    );
}

unsafe fn arith_idiv(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    return arith(
        L,
        6 as libc::c_int,
        b"__idiv\0" as *const u8 as *const libc::c_char,
    );
}

static mut stringmetamethods: [luaL_Reg; 10] = [
    {
        let mut init = luaL_Reg {
            name: b"__mul\0" as *const u8 as *const libc::c_char,
            func: Some(arith_mul),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"__div\0" as *const u8 as *const libc::c_char,
            func: Some(arith_div),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"__idiv\0" as *const u8 as *const libc::c_char,
            func: Some(arith_idiv),
        };
        init
    },
];

unsafe fn str_match(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    return str_find_aux(L, 0 as libc::c_int);
}

unsafe fn gmatch_aux(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut gm: *mut GMatchState = lua_touserdata(
        L,
        -(1000000 as libc::c_int) - 1000 as libc::c_int - 3 as libc::c_int,
    ) as *mut GMatchState;
    let mut src: *const libc::c_char = 0 as *const libc::c_char;
    (*gm).ms.L = L;
    src = (*gm).src;
    while src <= (*gm).ms.src_end {
        let mut e: *const libc::c_char = 0 as *const libc::c_char;
        reprepstate(&mut (*gm).ms);
        e = match_0(&mut (*gm).ms, src, (*gm).p)?;
        if !e.is_null() && e != (*gm).lastmatch {
            (*gm).lastmatch = e;
            (*gm).src = (*gm).lastmatch;
            return push_captures(&mut (*gm).ms, src, e);
        }
        src = src.offset(1);
    }
    return Ok(0 as libc::c_int);
}

unsafe fn gmatch(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut ls: usize = 0;
    let mut lp: usize = 0;
    let mut s: *const libc::c_char = luaL_checklstring(L, 1 as libc::c_int, &mut ls)?;
    let mut p: *const libc::c_char = luaL_checklstring(L, 2 as libc::c_int, &mut lp)?;
    let mut init: usize = (posrelatI(
        luaL_optinteger(L, 3 as libc::c_int, 1 as libc::c_int as i64)?,
        ls,
    ))
    .wrapping_sub(1 as libc::c_int as usize);
    let mut gm: *mut GMatchState = 0 as *mut GMatchState;
    lua_settop(L, 2 as libc::c_int)?;
    gm = lua_newuserdatauv(L, ::core::mem::size_of::<GMatchState>(), 0) as *mut GMatchState;
    if init > ls {
        init = ls.wrapping_add(1 as libc::c_int as usize);
    }
    prepstate(&mut (*gm).ms, L, s, ls, p, lp);
    (*gm).src = s.offset(init as isize);
    (*gm).p = p;
    (*gm).lastmatch = 0 as *const libc::c_char;
    lua_pushcclosure(L, gmatch_aux, 3 as libc::c_int);
    return Ok(1 as libc::c_int);
}

static mut nativeendian: C2RustUnnamed_0 = C2RustUnnamed_0 {
    dummy: 1 as libc::c_int,
};

unsafe extern "C" fn digit(mut c: libc::c_int) -> libc::c_int {
    return ('0' as i32 <= c && c <= '9' as i32) as libc::c_int;
}

unsafe extern "C" fn getnum(mut fmt: *mut *const libc::c_char, mut df: libc::c_int) -> libc::c_int {
    if digit(**fmt as libc::c_int) == 0 {
        return df;
    } else {
        let mut a: libc::c_int = 0 as libc::c_int;
        loop {
            let fresh20 = *fmt;
            *fmt = (*fmt).offset(1);
            a = a * 10 as libc::c_int + (*fresh20 as libc::c_int - '0' as i32);
            if !(digit(**fmt as libc::c_int) != 0
                && a <= ((if (::core::mem::size_of::<usize>() as libc::c_ulong)
                    < ::core::mem::size_of::<libc::c_int>() as libc::c_ulong
                {
                    !(0 as libc::c_int as usize)
                } else {
                    2147483647 as libc::c_int as usize
                }) as libc::c_int
                    - 9 as libc::c_int)
                    / 10 as libc::c_int)
            {
                break;
            }
        }
        return a;
    };
}

unsafe fn getnumlimit(
    mut h: *mut Header,
    mut fmt: *mut *const libc::c_char,
    mut df: libc::c_int,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut sz: libc::c_int = getnum(fmt, df);
    if ((sz > 16 as libc::c_int || sz <= 0 as libc::c_int) as libc::c_int != 0 as libc::c_int)
        as libc::c_int as libc::c_long
        != 0
    {
        return luaL_error(
            (*h).L,
            format!("integral size ({}) out of limits [1,16]", sz),
        );
    }
    return Ok(sz);
}

unsafe fn initheader(mut L: *const Thread, mut h: *mut Header) {
    (*h).L = L;
    (*h).islittle = nativeendian.little as libc::c_int;
    (*h).maxalign = 1 as libc::c_int;
}

unsafe fn getoption(
    mut h: *mut Header,
    mut fmt: *mut *const libc::c_char,
    mut size: *mut libc::c_int,
) -> Result<KOption, Box<dyn std::error::Error>> {
    let fresh21 = *fmt;
    *fmt = (*fmt).offset(1);
    let mut opt: libc::c_int = *fresh21 as libc::c_int;
    *size = 0 as libc::c_int;
    match opt {
        98 => {
            *size = ::core::mem::size_of::<libc::c_char>() as libc::c_ulong as libc::c_int;
            return Ok(Kint);
        }
        66 => {
            *size = ::core::mem::size_of::<libc::c_char>() as libc::c_ulong as libc::c_int;
            return Ok(Kuint);
        }
        104 => {
            *size = ::core::mem::size_of::<libc::c_short>() as libc::c_ulong as libc::c_int;
            return Ok(Kint);
        }
        72 => {
            *size = ::core::mem::size_of::<libc::c_short>() as libc::c_ulong as libc::c_int;
            return Ok(Kuint);
        }
        108 => {
            *size = ::core::mem::size_of::<libc::c_long>() as libc::c_ulong as libc::c_int;
            return Ok(Kint);
        }
        76 => {
            *size = ::core::mem::size_of::<libc::c_long>() as libc::c_ulong as libc::c_int;
            return Ok(Kuint);
        }
        106 => {
            *size = ::core::mem::size_of::<i64>() as libc::c_ulong as libc::c_int;
            return Ok(Kint);
        }
        74 => {
            *size = ::core::mem::size_of::<i64>() as libc::c_ulong as libc::c_int;
            return Ok(Kuint);
        }
        84 => {
            *size = ::core::mem::size_of::<usize>() as libc::c_ulong as libc::c_int;
            return Ok(Kuint);
        }
        102 => {
            *size = ::core::mem::size_of::<libc::c_float>() as libc::c_ulong as libc::c_int;
            return Ok(Kfloat);
        }
        110 => {
            *size = ::core::mem::size_of::<f64>() as libc::c_ulong as libc::c_int;
            return Ok(Knumber);
        }
        100 => {
            *size = ::core::mem::size_of::<libc::c_double>() as libc::c_ulong as libc::c_int;
            return Ok(Kdouble);
        }
        105 => {
            *size = getnumlimit(
                h,
                fmt,
                ::core::mem::size_of::<libc::c_int>() as libc::c_ulong as libc::c_int,
            )?;
            return Ok(Kint);
        }
        73 => {
            *size = getnumlimit(
                h,
                fmt,
                ::core::mem::size_of::<libc::c_int>() as libc::c_ulong as libc::c_int,
            )?;
            return Ok(Kuint);
        }
        115 => {
            *size = getnumlimit(
                h,
                fmt,
                ::core::mem::size_of::<usize>() as libc::c_ulong as libc::c_int,
            )?;
            return Ok(Kstring);
        }
        99 => {
            *size = getnum(fmt, -(1 as libc::c_int));
            if ((*size == -(1 as libc::c_int)) as libc::c_int != 0 as libc::c_int) as libc::c_int
                as libc::c_long
                != 0
            {
                luaL_error((*h).L, "missing size for format option 'c'")?;
            }
            return Ok(Kchar);
        }
        122 => return Ok(Kzstr),
        120 => {
            *size = 1 as libc::c_int;
            return Ok(Kpadding);
        }
        88 => return Ok(Kpaddalign),
        32 => {}
        60 => {
            (*h).islittle = 1 as libc::c_int;
        }
        62 => {
            (*h).islittle = 0 as libc::c_int;
        }
        61 => {
            (*h).islittle = nativeendian.little as libc::c_int;
        }
        33 => {
            let maxalign: libc::c_int = 8 as libc::c_ulong as libc::c_int;
            (*h).maxalign = getnumlimit(h, fmt, maxalign)?;
        }
        _ => {
            luaL_error(
                (*h).L,
                format!(
                    "invalid format option '{}'",
                    char::from_u32(opt as _).unwrap()
                ),
            )?;
        }
    }
    return Ok(Knop);
}

unsafe fn getdetails(
    mut h: *mut Header,
    mut totalsize: usize,
    mut fmt: *mut *const libc::c_char,
    mut psize: *mut libc::c_int,
    mut ntoalign: *mut libc::c_int,
) -> Result<KOption, Box<dyn std::error::Error>> {
    let mut opt: KOption = getoption(h, fmt, psize)?;
    let mut align: libc::c_int = *psize;
    if opt as libc::c_uint == Kpaddalign as libc::c_int as libc::c_uint {
        if **fmt as libc::c_int == '\0' as i32
            || getoption(h, fmt, &mut align)? as libc::c_uint
                == Kchar as libc::c_int as libc::c_uint
            || align == 0 as libc::c_int
        {
            luaL_argerror(
                (*h).L,
                1 as libc::c_int,
                "invalid next option for option 'X'",
            )?;
        }
    }
    if align <= 1 as libc::c_int || opt as libc::c_uint == Kchar as libc::c_int as libc::c_uint {
        *ntoalign = 0 as libc::c_int;
    } else {
        if align > (*h).maxalign {
            align = (*h).maxalign;
        }
        if ((align & align - 1 as libc::c_int != 0 as libc::c_int) as libc::c_int
            != 0 as libc::c_int) as libc::c_int as libc::c_long
            != 0
        {
            luaL_argerror(
                (*h).L,
                1 as libc::c_int,
                "format asks for alignment not power of 2",
            )?;
        }
        *ntoalign = align - (totalsize & (align - 1 as libc::c_int) as usize) as libc::c_int
            & align - 1 as libc::c_int;
    }
    return Ok(opt);
}

fn packint(
    mut b: &mut Vec<u8>,
    mut n: u64,
    mut islittle: libc::c_int,
    mut size: libc::c_int,
    mut neg: libc::c_int,
) {
    let mut i: libc::c_int = 0;
    let o = b.len();

    b.push((n & 0xFF) as u8);

    i = 1 as libc::c_int;

    while i < size {
        n >>= 8;
        b.push((n & 0xFF) as u8);
        i += 1;
    }

    if neg != 0 && size > ::core::mem::size_of::<i64>() as libc::c_ulong as libc::c_int {
        b[size_of::<i64>()..].fill(0xFF);
    }

    if islittle == 0 {
        b[o..].reverse();
    }
}

unsafe fn copywithendian(
    mut dest: *mut libc::c_char,
    mut src: *const libc::c_char,
    mut size: libc::c_int,
    mut islittle: libc::c_int,
) {
    if islittle == nativeendian.little as libc::c_int {
        memcpy(
            dest as *mut libc::c_void,
            src as *const libc::c_void,
            size as usize,
        );
    } else {
        dest = dest.offset((size - 1 as libc::c_int) as isize);
        loop {
            let fresh22 = size;
            size = size - 1;
            if !(fresh22 != 0 as libc::c_int) {
                break;
            }
            let fresh23 = src;
            src = src.offset(1);
            let fresh24 = dest;
            dest = dest.offset(-1);
            *fresh24 = *fresh23;
        }
    };
}

unsafe fn str_pack(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut h: Header = Header {
        L: 0 as *mut Thread,
        islittle: 0,
        maxalign: 0,
    };
    let mut fmt: *const libc::c_char = luaL_checklstring(L, 1 as libc::c_int, 0 as *mut usize)?;
    let mut arg: libc::c_int = 1 as libc::c_int;
    let mut totalsize: usize = 0 as libc::c_int as usize;
    let mut b = Vec::new();

    initheader(L, &mut h);
    lua_pushnil(L);

    while *fmt as libc::c_int != '\0' as i32 {
        let mut size: libc::c_int = 0;
        let mut ntoalign: libc::c_int = 0;
        let mut opt: KOption = getdetails(&mut h, totalsize, &mut fmt, &mut size, &mut ntoalign)?;
        totalsize = totalsize.wrapping_add((ntoalign + size) as usize);
        loop {
            let fresh25 = ntoalign;
            ntoalign = ntoalign - 1;
            if !(fresh25 > 0 as libc::c_int) {
                break;
            }

            b.push(0);
        }
        arg += 1;
        let mut current_block_33: u64;
        match opt as libc::c_uint {
            0 => {
                let mut n: i64 = luaL_checkinteger(L, arg)?;
                if size < ::core::mem::size_of::<i64>() as libc::c_ulong as libc::c_int {
                    let mut lim: i64 =
                        (1 as libc::c_int as i64) << size * 8 as libc::c_int - 1 as libc::c_int;
                    (((-lim <= n && n < lim) as libc::c_int != 0 as libc::c_int) as libc::c_int
                        as libc::c_long
                        != 0
                        || luaL_argerror(L, arg, "integer overflow")? != 0)
                        as libc::c_int;
                }
                packint(
                    &mut b,
                    n as u64,
                    h.islittle,
                    size,
                    (n < 0 as libc::c_int as i64) as libc::c_int,
                );
                current_block_33 = 3222590281903869779;
            }
            1 => {
                let mut n_0: i64 = luaL_checkinteger(L, arg)?;
                if size < ::core::mem::size_of::<i64>() as libc::c_ulong as libc::c_int {
                    ((((n_0 as u64) < (1 as libc::c_int as u64) << size * 8 as libc::c_int)
                        as libc::c_int
                        != 0 as libc::c_int) as libc::c_int as libc::c_long
                        != 0
                        || luaL_argerror(L, arg, "unsigned overflow")? != 0)
                        as libc::c_int;
                }
                packint(&mut b, n_0 as u64, h.islittle, size, 0 as libc::c_int);
                current_block_33 = 3222590281903869779;
            }
            2 => {
                let mut f: libc::c_float = luaL_checknumber(L, arg)? as libc::c_float;
                let mut buff = [0; size_of::<f32>()];

                copywithendian(
                    buff.as_mut_ptr().cast(),
                    &mut f as *mut libc::c_float as *mut libc::c_char,
                    ::core::mem::size_of::<libc::c_float>() as libc::c_ulong as libc::c_int,
                    h.islittle,
                );

                b.extend_from_slice(&buff[..size as usize]);
                current_block_33 = 3222590281903869779;
            }
            3 | 4 => {
                let mut f_0: f64 = luaL_checknumber(L, arg)?;
                let mut buff_0 = [0; size_of::<f64>()];

                copywithendian(
                    buff_0.as_mut_ptr().cast(),
                    &mut f_0 as *mut f64 as *mut libc::c_char,
                    ::core::mem::size_of::<f64>() as libc::c_ulong as libc::c_int,
                    h.islittle,
                );

                b.extend_from_slice(&buff_0[..size as usize]);
                current_block_33 = 3222590281903869779;
            }
            5 => {
                let mut len: usize = 0;
                let mut s: *const libc::c_char = luaL_checklstring(L, arg, &mut len)?;
                (((len <= size as usize) as libc::c_int != 0 as libc::c_int) as libc::c_int
                    as libc::c_long
                    != 0
                    || luaL_argerror(L, arg, "string longer than given size")? != 0)
                    as libc::c_int;
                b.extend_from_slice(std::slice::from_raw_parts(s.cast(), len));
                loop {
                    let fresh27 = len;
                    len = len.wrapping_add(1);
                    if !(fresh27 < size as usize) {
                        break;
                    }

                    b.push(0);
                }
                current_block_33 = 3222590281903869779;
            }
            6 => {
                let mut len_0: usize = 0;
                let mut s_0: *const libc::c_char = luaL_checklstring(L, arg, &mut len_0)?;
                (((size >= ::core::mem::size_of::<usize>() as libc::c_ulong as libc::c_int
                    || len_0 < (1 as libc::c_int as usize) << size * 8 as libc::c_int)
                    as libc::c_int
                    != 0 as libc::c_int) as libc::c_int as libc::c_long
                    != 0
                    || luaL_argerror(L, arg, "string length does not fit in given size")? != 0)
                    as libc::c_int;
                packint(&mut b, len_0 as u64, h.islittle, size, 0 as libc::c_int);
                b.extend_from_slice(std::slice::from_raw_parts(s_0.cast(), len_0));
                totalsize = totalsize.wrapping_add(len_0);
                current_block_33 = 3222590281903869779;
            }
            7 => {
                let mut len_1: usize = 0;
                let mut s_1: *const libc::c_char = luaL_checklstring(L, arg, &mut len_1)?;
                (((strlen(s_1) == len_1) as libc::c_int != 0 as libc::c_int) as libc::c_int
                    as libc::c_long
                    != 0
                    || luaL_argerror(L, arg, "string contains zeros")? != 0)
                    as libc::c_int;
                b.extend_from_slice(std::slice::from_raw_parts(s_1.cast(), len_1));
                b.push(0);
                totalsize = totalsize.wrapping_add(len_1.wrapping_add(1 as libc::c_int as usize));
                current_block_33 = 3222590281903869779;
            }
            8 => {
                b.push(0);
                current_block_33 = 12790994980371924011;
            }
            9 | 10 => {
                current_block_33 = 12790994980371924011;
            }
            _ => {
                current_block_33 = 3222590281903869779;
            }
        }
        match current_block_33 {
            12790994980371924011 => {
                arg -= 1;
            }
            _ => {}
        }
    }

    lua_pushlstring(L, b);

    return Ok(1 as libc::c_int);
}

unsafe fn str_packsize(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut h: Header = Header {
        L: 0 as *mut Thread,
        islittle: 0,
        maxalign: 0,
    };
    let mut fmt: *const libc::c_char = luaL_checklstring(L, 1 as libc::c_int, 0 as *mut usize)?;
    let mut totalsize: usize = 0 as libc::c_int as usize;
    initheader(L, &mut h);
    while *fmt as libc::c_int != '\0' as i32 {
        let mut size: libc::c_int = 0;
        let mut ntoalign: libc::c_int = 0;
        let mut opt: KOption = getdetails(&mut h, totalsize, &mut fmt, &mut size, &mut ntoalign)?;
        (((opt as libc::c_uint != Kstring as libc::c_int as libc::c_uint
            && opt as libc::c_uint != Kzstr as libc::c_int as libc::c_uint)
            as libc::c_int
            != 0 as libc::c_int) as libc::c_int as libc::c_long
            != 0
            || luaL_argerror(L, 1 as libc::c_int, "variable-length format")? != 0)
            as libc::c_int;
        size += ntoalign;
        (((totalsize
            <= (if (::core::mem::size_of::<usize>() as libc::c_ulong)
                < ::core::mem::size_of::<libc::c_int>() as libc::c_ulong
            {
                !(0 as libc::c_int as usize)
            } else {
                2147483647 as libc::c_int as usize
            })
            .wrapping_sub(size as usize)) as libc::c_int
            != 0 as libc::c_int) as libc::c_int as libc::c_long
            != 0
            || luaL_argerror(L, 1 as libc::c_int, "format result too large")? != 0)
            as libc::c_int;
        totalsize = totalsize.wrapping_add(size as usize);
    }
    lua_pushinteger(L, totalsize as i64);
    return Ok(1 as libc::c_int);
}

unsafe fn unpackint(
    mut L: *const Thread,
    mut str: *const libc::c_char,
    mut islittle: libc::c_int,
    mut size: libc::c_int,
    mut issigned: libc::c_int,
) -> Result<i64, Box<dyn std::error::Error>> {
    let mut res: u64 = 0 as libc::c_int as u64;
    let mut i: libc::c_int = 0;
    let mut limit: libc::c_int =
        if size <= ::core::mem::size_of::<i64>() as libc::c_ulong as libc::c_int {
            size
        } else {
            ::core::mem::size_of::<i64>() as libc::c_ulong as libc::c_int
        };
    i = limit - 1 as libc::c_int;
    while i >= 0 as libc::c_int {
        res <<= 8 as libc::c_int;
        res |= *str.offset(
            (if islittle != 0 {
                i
            } else {
                size - 1 as libc::c_int - i
            }) as isize,
        ) as libc::c_uchar as u64;
        i -= 1;
    }
    if size < ::core::mem::size_of::<i64>() as libc::c_ulong as libc::c_int {
        if issigned != 0 {
            let mut mask: u64 =
                (1 as libc::c_int as u64) << size * 8 as libc::c_int - 1 as libc::c_int;
            res = (res ^ mask).wrapping_sub(mask);
        }
    } else if size > ::core::mem::size_of::<i64>() as libc::c_ulong as libc::c_int {
        let mut mask_0: libc::c_int = if issigned == 0 || res as i64 >= 0 as libc::c_int as i64 {
            0 as libc::c_int
        } else {
            ((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int
        };
        i = limit;
        while i < size {
            if ((*str.offset(
                (if islittle != 0 {
                    i
                } else {
                    size - 1 as libc::c_int - i
                }) as isize,
            ) as libc::c_uchar as libc::c_int
                != mask_0) as libc::c_int
                != 0 as libc::c_int) as libc::c_int as libc::c_long
                != 0
            {
                luaL_error(
                    L,
                    format!("{size}-byte integer does not fit into Lua Integer"),
                )?;
            }
            i += 1;
        }
    }
    return Ok(res as i64);
}

unsafe fn str_unpack(mut L: *const Thread) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut h: Header = Header {
        L: 0 as *mut Thread,
        islittle: 0,
        maxalign: 0,
    };
    let mut fmt: *const libc::c_char = luaL_checklstring(L, 1 as libc::c_int, 0 as *mut usize)?;
    let mut ld: usize = 0;
    let mut data: *const libc::c_char = luaL_checklstring(L, 2 as libc::c_int, &mut ld)?;
    let mut pos: usize = (posrelatI(
        luaL_optinteger(L, 3 as libc::c_int, 1 as libc::c_int as i64)?,
        ld,
    ))
    .wrapping_sub(1 as libc::c_int as usize);
    let mut n: libc::c_int = 0 as libc::c_int;
    (((pos <= ld) as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long != 0
        || luaL_argerror(L, 3 as libc::c_int, "initial position out of string")? != 0)
        as libc::c_int;
    initheader(L, &mut h);
    while *fmt as libc::c_int != '\0' as i32 {
        let mut size: libc::c_int = 0;
        let mut ntoalign: libc::c_int = 0;
        let mut opt: KOption = getdetails(&mut h, pos, &mut fmt, &mut size, &mut ntoalign)?;
        ((((ntoalign as usize).wrapping_add(size as usize) <= ld.wrapping_sub(pos)) as libc::c_int
            != 0 as libc::c_int) as libc::c_int as libc::c_long
            != 0
            || luaL_argerror(L, 2 as libc::c_int, "data string too short")? != 0)
            as libc::c_int;
        pos = pos.wrapping_add(ntoalign as usize);
        luaL_checkstack(
            L,
            2,
            b"too many results\0" as *const u8 as *const libc::c_char,
        )?;
        n += 1;
        match opt as libc::c_uint {
            0 | 1 => {
                let mut res: i64 = unpackint(
                    L,
                    data.offset(pos as isize),
                    h.islittle,
                    size,
                    (opt as libc::c_uint == Kint as libc::c_int as libc::c_uint) as libc::c_int,
                )?;
                lua_pushinteger(L, res);
            }
            2 => {
                let mut f: libc::c_float = 0.;
                copywithendian(
                    &mut f as *mut libc::c_float as *mut libc::c_char,
                    data.offset(pos as isize),
                    ::core::mem::size_of::<libc::c_float>() as libc::c_ulong as libc::c_int,
                    h.islittle,
                );
                lua_pushnumber(L, f as f64);
            }
            3 => {
                let mut f_0: f64 = 0.;
                copywithendian(
                    &mut f_0 as *mut f64 as *mut libc::c_char,
                    data.offset(pos as isize),
                    ::core::mem::size_of::<f64>() as libc::c_ulong as libc::c_int,
                    h.islittle,
                );
                lua_pushnumber(L, f_0);
            }
            4 => {
                let mut f_1: libc::c_double = 0.;
                copywithendian(
                    &mut f_1 as *mut libc::c_double as *mut libc::c_char,
                    data.offset(pos as isize),
                    ::core::mem::size_of::<libc::c_double>() as libc::c_ulong as libc::c_int,
                    h.islittle,
                );
                lua_pushnumber(L, f_1);
            }
            5 => {
                lua_pushlstring(
                    L,
                    std::slice::from_raw_parts(data.offset(pos as isize).cast(), size as usize),
                );
            }
            6 => {
                let mut len: usize = unpackint(
                    L,
                    data.offset(pos as isize),
                    h.islittle,
                    size,
                    0 as libc::c_int,
                )? as usize;
                (((len <= ld.wrapping_sub(pos).wrapping_sub(size as usize)) as libc::c_int
                    != 0 as libc::c_int) as libc::c_int as libc::c_long
                    != 0
                    || luaL_argerror(L, 2 as libc::c_int, "data string too short")? != 0)
                    as libc::c_int;
                lua_pushlstring(
                    L,
                    std::slice::from_raw_parts(
                        data.offset(pos as isize).offset(size as isize).cast(),
                        len,
                    ),
                );
                pos = pos.wrapping_add(len);
            }
            7 => {
                let mut len_0: usize = strlen(data.offset(pos as isize));
                (((pos.wrapping_add(len_0) < ld) as libc::c_int != 0 as libc::c_int) as libc::c_int
                    as libc::c_long
                    != 0
                    || luaL_argerror(L, 2 as libc::c_int, "unfinished string for format 'z'")? != 0)
                    as libc::c_int;
                lua_pushlstring(
                    L,
                    std::slice::from_raw_parts(data.offset(pos as isize).cast(), len_0),
                );
                pos = pos.wrapping_add(len_0.wrapping_add(1 as libc::c_int as usize));
            }
            9 | 8 | 10 => {
                n -= 1;
            }
            _ => {}
        }
        pos = pos.wrapping_add(size as usize);
    }
    lua_pushinteger(L, pos.wrapping_add(1 as libc::c_int as usize) as i64);
    return Ok(n + 1 as libc::c_int);
}

static mut strlib: [luaL_Reg; 17] = [
    {
        let mut init = luaL_Reg {
            name: b"char\0" as *const u8 as *const libc::c_char,
            func: Some(str_char),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"gmatch\0" as *const u8 as *const libc::c_char,
            func: Some(gmatch),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"lower\0" as *const u8 as *const libc::c_char,
            func: Some(str_lower),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"match\0" as *const u8 as *const libc::c_char,
            func: Some(str_match),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"reverse\0" as *const u8 as *const libc::c_char,
            func: Some(str_reverse),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"upper\0" as *const u8 as *const libc::c_char,
            func: Some(str_upper),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"pack\0" as *const u8 as *const libc::c_char,
            func: Some(str_pack),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"packsize\0" as *const u8 as *const libc::c_char,
            func: Some(str_packsize),
        };
        init
    },
    {
        let mut init = luaL_Reg {
            name: b"unpack\0" as *const u8 as *const libc::c_char,
            func: Some(str_unpack),
        };
        init
    },
];
