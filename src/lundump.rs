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
#![allow(path_statements)]

use crate::ldo::luaD_inctop;
use crate::lfunc::{luaF_newLclosure, luaF_newproto};
use crate::lgc::luaC_barrier_;
use crate::lmem::{luaM_malloc_, luaM_toobig};
use crate::lobject::{AbsLineInfo, GCObject, LClosure, LocVar, Proto, TString, TValue, Upvaldesc};
use crate::lstate::lua_State;
use crate::lstring::{luaS_createlngstrobj, luaS_newlstr};
use crate::lzio::{ZIO, luaZ_fill, luaZ_read};
use libc::{memcmp, strlen};
use std::ffi::CStr;
use std::fmt::Display;

#[repr(C)]
struct LoadState {
    pub L: *mut lua_State,
    pub Z: *mut ZIO,
    pub name: *const libc::c_char,
}

unsafe fn error(S: *mut LoadState, why: impl Display) -> Result<(), Box<dyn std::error::Error>> {
    Err(format!(
        "{}: bad binary format ({})",
        CStr::from_ptr((*S).name).to_string_lossy(),
        why
    )
    .into())
}

unsafe fn loadBlock(
    mut S: *mut LoadState,
    mut b: *mut libc::c_void,
    mut size: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    if luaZ_read((*S).Z, b, size)? != 0 as libc::c_int as usize {
        error(S, "truncated chunk")?;
    }
    Ok(())
}

unsafe fn loadByte(mut S: *mut LoadState) -> Result<u8, Box<dyn std::error::Error>> {
    let fresh0 = (*(*S).Z).n;
    (*(*S).Z).n = ((*(*S).Z).n).wrapping_sub(1);
    let mut b: libc::c_int = if fresh0 > 0 as libc::c_int as usize {
        let fresh1 = (*(*S).Z).p;
        (*(*S).Z).p = ((*(*S).Z).p).offset(1);
        *fresh1 as libc::c_uchar as libc::c_int
    } else {
        luaZ_fill((*S).Z)?
    };
    if b == -(1 as libc::c_int) {
        error(S, "truncated chunk")?;
    }
    return Ok(b as u8);
}

unsafe fn loadUnsigned(
    mut S: *mut LoadState,
    mut limit: usize,
) -> Result<usize, Box<dyn std::error::Error>> {
    let mut x: usize = 0 as libc::c_int as usize;
    let mut b: libc::c_int = 0;
    limit >>= 7 as libc::c_int;
    loop {
        b = loadByte(S)? as libc::c_int;
        if x >= limit {
            error(S, "integer overflow")?;
        }
        x = x << 7 as libc::c_int | (b & 0x7f as libc::c_int) as usize;
        if !(b & 0x80 as libc::c_int == 0 as libc::c_int) {
            break;
        }
    }
    return Ok(x);
}

unsafe fn loadSize(mut S: *mut LoadState) -> Result<usize, Box<dyn std::error::Error>> {
    return loadUnsigned(S, !(0 as libc::c_int as usize));
}

unsafe fn loadInt(mut S: *mut LoadState) -> Result<libc::c_int, Box<dyn std::error::Error>> {
    return loadUnsigned(S, 2147483647 as libc::c_int as usize).map(|v| v as libc::c_int);
}

unsafe fn loadNumber(mut S: *mut LoadState) -> Result<f64, Box<dyn std::error::Error>> {
    let mut x: f64 = 0.;
    loadBlock(
        S,
        &mut x as *mut f64 as *mut libc::c_void,
        1usize.wrapping_mul(::core::mem::size_of::<f64>()),
    )?;
    return Ok(x);
}

unsafe fn loadInteger(mut S: *mut LoadState) -> Result<i64, Box<dyn std::error::Error>> {
    let mut x: i64 = 0;
    loadBlock(
        S,
        &mut x as *mut i64 as *mut libc::c_void,
        1usize.wrapping_mul(::core::mem::size_of::<i64>()),
    )?;
    return Ok(x);
}

unsafe fn loadStringN(
    mut S: *mut LoadState,
    mut p: *mut Proto,
) -> Result<*mut TString, Box<dyn std::error::Error>> {
    let mut L: *mut lua_State = (*S).L;
    let mut ts: *mut TString = 0 as *mut TString;
    let mut size: usize = loadSize(S)?;
    if size == 0 as libc::c_int as usize {
        return Ok(0 as *mut TString);
    } else {
        size = size.wrapping_sub(1);
        if size <= 40 as libc::c_int as usize {
            let mut buff: [libc::c_char; 40] = [0; 40];
            loadBlock(
                S,
                buff.as_mut_ptr() as *mut libc::c_void,
                size.wrapping_mul(::core::mem::size_of::<libc::c_char>()),
            )?;
            ts = luaS_newlstr(L, buff.as_mut_ptr(), size)?;
        } else {
            ts = luaS_createlngstrobj(L, size);
            let mut io: *mut TValue = &mut (*(*L).top.p).val;
            let mut x_: *mut TString = ts;
            (*io).value_.gc = x_ as *mut GCObject;
            (*io).tt_ = ((*x_).tt as libc::c_int | (1 as libc::c_int) << 6 as libc::c_int) as u8;
            luaD_inctop(L)?;
            loadBlock(
                S,
                ((*ts).contents).as_mut_ptr() as *mut libc::c_void,
                size.wrapping_mul(::core::mem::size_of::<libc::c_char>()),
            )?;
            (*L).top.p = ((*L).top.p).offset(-1);
            (*L).top.p;
        }
    }
    if (*p).marked as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
        && (*ts).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            != 0
    {
        luaC_barrier_(L, p as *mut GCObject, ts as *mut GCObject);
    } else {
    };
    return Ok(ts);
}

unsafe fn loadString(
    mut S: *mut LoadState,
    mut p: *mut Proto,
) -> Result<*mut TString, Box<dyn std::error::Error>> {
    let mut st: *mut TString = loadStringN(S, p)?;
    if st.is_null() {
        error(S, "bad format for constant string")?;
    }
    return Ok(st);
}

unsafe fn loadCode(
    mut S: *mut LoadState,
    mut f: *mut Proto,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut n: libc::c_int = loadInt(S)?;
    if ::core::mem::size_of::<libc::c_int>() as libc::c_ulong
        >= ::core::mem::size_of::<usize>() as libc::c_ulong
        && (n as usize).wrapping_add(1 as libc::c_int as usize)
            > (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<u32>())
    {
        luaM_toobig((*S).L)?;
    } else {
    };
    (*f).code = luaM_malloc_(
        (*S).L,
        (n as usize).wrapping_mul(::core::mem::size_of::<u32>()),
    ) as *mut u32;
    (*f).sizecode = n;
    loadBlock(
        S,
        (*f).code as *mut libc::c_void,
        (n as usize).wrapping_mul(::core::mem::size_of::<u32>()),
    )
}

unsafe fn loadConstants(
    mut S: *mut LoadState,
    mut f: *mut Proto,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut i: libc::c_int = 0;
    let mut n: libc::c_int = loadInt(S)?;
    if ::core::mem::size_of::<libc::c_int>() as libc::c_ulong
        >= ::core::mem::size_of::<usize>() as libc::c_ulong
        && (n as usize).wrapping_add(1 as libc::c_int as usize)
            > (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<TValue>())
    {
        luaM_toobig((*S).L)?;
    } else {
    };
    (*f).k = luaM_malloc_(
        (*S).L,
        (n as usize).wrapping_mul(::core::mem::size_of::<TValue>()),
    ) as *mut TValue;
    (*f).sizek = n;
    i = 0 as libc::c_int;
    while i < n {
        (*((*f).k).offset(i as isize)).tt_ =
            (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
        i += 1;
        i;
    }
    i = 0 as libc::c_int;
    while i < n {
        let mut o: *mut TValue = &mut *((*f).k).offset(i as isize) as *mut TValue;
        let mut t: libc::c_int = loadByte(S)? as libc::c_int;
        match t {
            0 => {
                (*o).tt_ = (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
            }
            1 => {
                (*o).tt_ = (1 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
            }
            17 => {
                (*o).tt_ = (1 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
            }
            19 => {
                let mut io: *mut TValue = o;
                (*io).value_.n = loadNumber(S)?;
                (*io).tt_ = (3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
            }
            3 => {
                let mut io_0: *mut TValue = o;
                (*io_0).value_.i = loadInteger(S)?;
                (*io_0).tt_ = (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
            }
            4 | 20 => {
                let mut io_1: *mut TValue = o;
                let mut x_: *mut TString = loadString(S, f)?;
                (*io_1).value_.gc = x_ as *mut GCObject;
                (*io_1).tt_ =
                    ((*x_).tt as libc::c_int | (1 as libc::c_int) << 6 as libc::c_int) as u8;
            }
            _ => {}
        }
        i += 1;
        i;
    }
    Ok(())
}

unsafe fn loadProtos(
    mut S: *mut LoadState,
    mut f: *mut Proto,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut i: libc::c_int = 0;
    let mut n: libc::c_int = loadInt(S)?;
    if ::core::mem::size_of::<libc::c_int>() as libc::c_ulong
        >= ::core::mem::size_of::<usize>() as libc::c_ulong
        && (n as usize).wrapping_add(1 as libc::c_int as usize)
            > (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<*mut Proto>())
    {
        luaM_toobig((*S).L)?;
    } else {
    };
    (*f).p = luaM_malloc_(
        (*S).L,
        (n as usize).wrapping_mul(::core::mem::size_of::<*mut Proto>()),
    ) as *mut *mut Proto;
    (*f).sizep = n;
    i = 0 as libc::c_int;
    while i < n {
        let ref mut fresh2 = *((*f).p).offset(i as isize);
        *fresh2 = 0 as *mut Proto;
        i += 1;
        i;
    }
    i = 0 as libc::c_int;
    while i < n {
        let ref mut fresh3 = *((*f).p).offset(i as isize);
        *fresh3 = luaF_newproto((*S).L);
        if (*f).marked as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
            && (**((*f).p).offset(i as isize)).marked as libc::c_int
                & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
                != 0
        {
            luaC_barrier_(
                (*S).L,
                f as *mut GCObject,
                *((*f).p).offset(i as isize) as *mut GCObject,
            );
        } else {
        };
        loadFunction(S, *((*f).p).offset(i as isize), (*f).source)?;
        i += 1;
        i;
    }
    Ok(())
}

unsafe fn loadUpvalues(
    mut S: *mut LoadState,
    mut f: *mut Proto,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut i: libc::c_int = 0;
    let mut n: libc::c_int = 0;
    n = loadInt(S)?;
    if ::core::mem::size_of::<libc::c_int>() as libc::c_ulong
        >= ::core::mem::size_of::<usize>() as libc::c_ulong
        && (n as usize).wrapping_add(1 as libc::c_int as usize)
            > (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<Upvaldesc>())
    {
        luaM_toobig((*S).L)?;
    } else {
    };
    (*f).upvalues = luaM_malloc_(
        (*S).L,
        (n as usize).wrapping_mul(::core::mem::size_of::<Upvaldesc>()),
    ) as *mut Upvaldesc;
    (*f).sizeupvalues = n;
    i = 0 as libc::c_int;
    while i < n {
        let ref mut fresh4 = (*((*f).upvalues).offset(i as isize)).name;
        *fresh4 = 0 as *mut TString;
        i += 1;
        i;
    }
    i = 0 as libc::c_int;
    while i < n {
        (*((*f).upvalues).offset(i as isize)).instack = loadByte(S)?;
        (*((*f).upvalues).offset(i as isize)).idx = loadByte(S)?;
        (*((*f).upvalues).offset(i as isize)).kind = loadByte(S)?;
        i += 1;
        i;
    }
    Ok(())
}

unsafe fn loadDebug(
    mut S: *mut LoadState,
    mut f: *mut Proto,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut i: libc::c_int = 0;
    let mut n: libc::c_int = 0;
    n = loadInt(S)?;
    if ::core::mem::size_of::<libc::c_int>() as libc::c_ulong
        >= ::core::mem::size_of::<usize>() as libc::c_ulong
        && (n as usize).wrapping_add(1 as libc::c_int as usize)
            > (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<i8>())
    {
        luaM_toobig((*S).L)?;
    } else {
    };
    (*f).lineinfo = luaM_malloc_(
        (*S).L,
        (n as usize).wrapping_mul(::core::mem::size_of::<i8>()),
    ) as *mut i8;
    (*f).sizelineinfo = n;
    loadBlock(
        S,
        (*f).lineinfo as *mut libc::c_void,
        (n as usize).wrapping_mul(::core::mem::size_of::<i8>()),
    )?;
    n = loadInt(S)?;
    if ::core::mem::size_of::<libc::c_int>() as libc::c_ulong
        >= ::core::mem::size_of::<usize>() as libc::c_ulong
        && (n as usize).wrapping_add(1 as libc::c_int as usize)
            > (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<AbsLineInfo>())
    {
        luaM_toobig((*S).L)?;
    } else {
    };
    (*f).abslineinfo = luaM_malloc_(
        (*S).L,
        (n as usize).wrapping_mul(::core::mem::size_of::<AbsLineInfo>()),
    ) as *mut AbsLineInfo;
    (*f).sizeabslineinfo = n;
    i = 0 as libc::c_int;
    while i < n {
        (*((*f).abslineinfo).offset(i as isize)).pc = loadInt(S)?;
        (*((*f).abslineinfo).offset(i as isize)).line = loadInt(S)?;
        i += 1;
        i;
    }
    n = loadInt(S)?;
    if ::core::mem::size_of::<libc::c_int>() as libc::c_ulong
        >= ::core::mem::size_of::<usize>() as libc::c_ulong
        && (n as usize).wrapping_add(1 as libc::c_int as usize)
            > (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<LocVar>())
    {
        luaM_toobig((*S).L)?;
    } else {
    };
    (*f).locvars = luaM_malloc_(
        (*S).L,
        (n as usize).wrapping_mul(::core::mem::size_of::<LocVar>()),
    ) as *mut LocVar;
    (*f).sizelocvars = n;
    i = 0 as libc::c_int;
    while i < n {
        let ref mut fresh5 = (*((*f).locvars).offset(i as isize)).varname;
        *fresh5 = 0 as *mut TString;
        i += 1;
        i;
    }
    i = 0 as libc::c_int;
    while i < n {
        let ref mut fresh6 = (*((*f).locvars).offset(i as isize)).varname;
        *fresh6 = loadStringN(S, f)?;
        (*((*f).locvars).offset(i as isize)).startpc = loadInt(S)?;
        (*((*f).locvars).offset(i as isize)).endpc = loadInt(S)?;
        i += 1;
        i;
    }
    n = loadInt(S)?;
    if n != 0 as libc::c_int {
        n = (*f).sizeupvalues;
    }
    i = 0 as libc::c_int;
    while i < n {
        let ref mut fresh7 = (*((*f).upvalues).offset(i as isize)).name;
        *fresh7 = loadStringN(S, f)?;
        i += 1;
        i;
    }
    Ok(())
}

unsafe fn loadFunction(
    mut S: *mut LoadState,
    mut f: *mut Proto,
    mut psource: *mut TString,
) -> Result<(), Box<dyn std::error::Error>> {
    (*f).source = loadStringN(S, f)?;
    if ((*f).source).is_null() {
        (*f).source = psource;
    }
    (*f).linedefined = loadInt(S)?;
    (*f).lastlinedefined = loadInt(S)?;
    (*f).numparams = loadByte(S)?;
    (*f).is_vararg = loadByte(S)?;
    (*f).maxstacksize = loadByte(S)?;
    loadCode(S, f)?;
    loadConstants(S, f)?;
    loadUpvalues(S, f)?;
    loadProtos(S, f)?;
    loadDebug(S, f)?;
    Ok(())
}

unsafe fn checkliteral(
    mut S: *mut LoadState,
    mut s: *const libc::c_char,
    msg: impl Display,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut buff: [libc::c_char; 12] = [0; 12];
    let mut len: usize = strlen(s);
    loadBlock(
        S,
        buff.as_mut_ptr() as *mut libc::c_void,
        len.wrapping_mul(::core::mem::size_of::<libc::c_char>()),
    )?;
    if memcmp(
        s as *const libc::c_void,
        buff.as_mut_ptr() as *const libc::c_void,
        len,
    ) != 0 as libc::c_int
    {
        error(S, msg)?;
    }
    Ok(())
}

unsafe fn fchecksize(
    mut S: *mut LoadState,
    mut size: usize,
    tname: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if loadByte(S)? as usize != size {
        error(S, format_args!("{tname} size mismatch"))?;
    }
    Ok(())
}

unsafe fn checkHeader(mut S: *mut LoadState) -> Result<(), Box<dyn std::error::Error>> {
    checkliteral(
        S,
        &*(b"\x1BLua\0" as *const u8 as *const libc::c_char).offset(1 as libc::c_int as isize),
        "not a binary chunk",
    )?;
    if loadByte(S)? as libc::c_int
        != 504 as libc::c_int / 100 as libc::c_int * 16 as libc::c_int
            + 504 as libc::c_int % 100 as libc::c_int
    {
        error(S, "version mismatch")?;
    }
    if loadByte(S)? as libc::c_int != 0 as libc::c_int {
        error(S, "format mismatch")?;
    }
    checkliteral(
        S,
        b"\x19\x93\r\n\x1A\n\0" as *const u8 as *const libc::c_char,
        "corrupted chunk",
    )?;
    fchecksize(S, ::core::mem::size_of::<u32>(), "Instruction")?;
    fchecksize(S, ::core::mem::size_of::<i64>(), "lua_Integer")?;
    fchecksize(S, ::core::mem::size_of::<f64>(), "lua_Number")?;
    if loadInteger(S)? != 0x5678 as libc::c_int as i64 {
        error(S, "integer format mismatch")?;
    }
    if loadNumber(S)? != 370.5f64 {
        error(S, "float format mismatch")?;
    }
    Ok(())
}

pub unsafe fn luaU_undump(
    mut L: *mut lua_State,
    mut Z: *mut ZIO,
    mut name: *const libc::c_char,
) -> Result<*mut LClosure, Box<dyn std::error::Error>> {
    let mut S: LoadState = LoadState {
        L: 0 as *mut lua_State,
        Z: 0 as *mut ZIO,
        name: 0 as *const libc::c_char,
    };
    let mut cl: *mut LClosure = 0 as *mut LClosure;
    if *name as libc::c_int == '@' as i32 || *name as libc::c_int == '=' as i32 {
        S.name = name.offset(1 as libc::c_int as isize);
    } else if *name as libc::c_int
        == (*::core::mem::transmute::<&[u8; 5], &[libc::c_char; 5]>(b"\x1BLua\0"))
            [0 as libc::c_int as usize] as libc::c_int
    {
        S.name = b"binary string\0" as *const u8 as *const libc::c_char;
    } else {
        S.name = name;
    }
    S.L = L;
    S.Z = Z;
    checkHeader(&mut S)?;
    cl = luaF_newLclosure(L, loadByte(&mut S)? as libc::c_int);
    let mut io: *mut TValue = &mut (*(*L).top.p).val;
    let mut x_: *mut LClosure = cl;
    (*io).value_.gc = x_ as *mut GCObject;
    (*io).tt_ = (6 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 6 as libc::c_int) as u8;
    luaD_inctop(L)?;
    (*cl).p = luaF_newproto(L);
    if (*cl).marked as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
        && (*(*cl).p).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            != 0
    {
        luaC_barrier_(L, cl as *mut GCObject, (*cl).p as *mut GCObject);
    } else {
    };
    loadFunction(&mut S, (*cl).p, 0 as *mut TString)?;
    return Ok(cl);
}
