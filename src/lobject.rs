#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments
)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::gc::Object;
use crate::lctype::luai_ctype_;
use crate::ldebug::luaG_runerror;
use crate::ltm::{TM_ADD, TMS, luaT_trybinTM};
use crate::lvm::{F2Ieq, luaV_idiv, luaV_mod, luaV_modf, luaV_shiftl, luaV_tointegerns};
use crate::value::{UnsafeValue, UntaggedValue};
use crate::{ArithError, ChunkInfo, Context, Lua, Str, Table, Thread};
use alloc::boxed::Box;
use core::cell::{Cell, UnsafeCell};
use libc::{c_char, c_int, sprintf, strpbrk, strspn, strtod};
use libm::{floor, pow};

pub type StkId = *mut StackValue;

#[derive(Copy, Clone)]
#[repr(C)]
pub union StackValue {
    pub val: UnsafeValue,
    pub tbclist: TbcList,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct TbcList {
    pub value_: UntaggedValue,
    pub tt_: u8,
    pub delta: libc::c_ushort,
}

#[repr(C)]
pub struct UpVal {
    pub hdr: Object,
    pub v: Cell<*mut UnsafeValue>,
    pub u: UnsafeCell<C2RustUnnamed_5>,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub union C2RustUnnamed_5 {
    pub open: C2RustUnnamed_6,
    pub value: UnsafeValue,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct C2RustUnnamed_6 {
    pub next: *mut UpVal,
    pub previous: *mut *mut UpVal,
}

#[repr(C)]
pub struct Udata {
    pub hdr: Object,
    pub nuvalue: libc::c_ushort,
    pub len: usize,
    pub metatable: *const Table,
    pub uv: [UnsafeValue; 1],
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Upvaldesc {
    pub name: *const Str,
    pub instack: u8,
    pub idx: u8,
    pub kind: u8,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct LocVar {
    pub varname: *const Str,
    pub startpc: c_int,
    pub endpc: c_int,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct AbsLineInfo {
    pub pc: c_int,
    pub line: c_int,
}

#[repr(C)]
pub struct Proto {
    pub hdr: Object,
    pub numparams: u8,
    pub is_vararg: u8,
    pub maxstacksize: u8,
    pub sizeupvalues: c_int,
    pub sizek: c_int,
    pub sizecode: c_int,
    pub sizelineinfo: c_int,
    pub sizep: c_int,
    pub sizelocvars: c_int,
    pub sizeabslineinfo: c_int,
    pub linedefined: c_int,
    pub lastlinedefined: c_int,
    pub k: *mut UnsafeValue,
    pub code: *mut u32,
    pub p: *mut *mut Proto,
    pub upvalues: *mut Upvaldesc,
    pub lineinfo: *mut i8,
    pub abslineinfo: *mut AbsLineInfo,
    pub locvars: *mut LocVar,
    pub chunk: ChunkInfo,
}

#[repr(C)]
pub struct CClosure {
    pub hdr: Object,
    pub nupvalues: u8,
    pub f: fn(&mut Context) -> Result<(), Box<dyn core::error::Error>>,
    pub upvalue: [UnsafeValue; 1],
}

pub unsafe fn luaO_ceillog2(mut x: libc::c_uint) -> c_int {
    static mut log_2: [u8; 256] = [
        0 as c_int as u8,
        1 as c_int as u8,
        2 as c_int as u8,
        2 as c_int as u8,
        3 as c_int as u8,
        3 as c_int as u8,
        3 as c_int as u8,
        3 as c_int as u8,
        4 as c_int as u8,
        4 as c_int as u8,
        4 as c_int as u8,
        4 as c_int as u8,
        4 as c_int as u8,
        4 as c_int as u8,
        4 as c_int as u8,
        4 as c_int as u8,
        5 as c_int as u8,
        5 as c_int as u8,
        5 as c_int as u8,
        5 as c_int as u8,
        5 as c_int as u8,
        5 as c_int as u8,
        5 as c_int as u8,
        5 as c_int as u8,
        5 as c_int as u8,
        5 as c_int as u8,
        5 as c_int as u8,
        5 as c_int as u8,
        5 as c_int as u8,
        5 as c_int as u8,
        5 as c_int as u8,
        5 as c_int as u8,
        6 as c_int as u8,
        6 as c_int as u8,
        6 as c_int as u8,
        6 as c_int as u8,
        6 as c_int as u8,
        6 as c_int as u8,
        6 as c_int as u8,
        6 as c_int as u8,
        6 as c_int as u8,
        6 as c_int as u8,
        6 as c_int as u8,
        6 as c_int as u8,
        6 as c_int as u8,
        6 as c_int as u8,
        6 as c_int as u8,
        6 as c_int as u8,
        6 as c_int as u8,
        6 as c_int as u8,
        6 as c_int as u8,
        6 as c_int as u8,
        6 as c_int as u8,
        6 as c_int as u8,
        6 as c_int as u8,
        6 as c_int as u8,
        6 as c_int as u8,
        6 as c_int as u8,
        6 as c_int as u8,
        6 as c_int as u8,
        6 as c_int as u8,
        6 as c_int as u8,
        6 as c_int as u8,
        6 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        7 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
        8 as c_int as u8,
    ];
    let mut l: c_int = 0 as c_int;
    x = x.wrapping_sub(1);

    while x >= 256 as c_int as libc::c_uint {
        l += 8 as c_int;
        x >>= 8 as c_int;
    }

    return l + log_2[x as usize] as c_int;
}

unsafe fn intarith(op: c_int, v1: i64, v2: i64) -> Result<i64, ArithError> {
    let r = match op {
        0 => (v1 as u64).wrapping_add(v2 as u64) as i64,
        1 => (v1 as u64).wrapping_sub(v2 as u64) as i64,
        2 => (v1 as u64).wrapping_mul(v2 as u64) as i64,
        3 => luaV_mod(v1, v2).ok_or(ArithError::ModZero)?,
        6 => luaV_idiv(v1, v2).ok_or(ArithError::DivZero)?,
        7 => (v1 as u64 & v2 as u64) as i64,
        8 => (v1 as u64 | v2 as u64) as i64,
        9 => (v1 as u64 ^ v2 as u64) as i64,
        10 => luaV_shiftl(v1, v2),
        11 => luaV_shiftl(v1, (0 as c_int as u64).wrapping_sub(v2 as u64) as i64),
        12 => (0 as c_int as u64).wrapping_sub(v1 as u64) as i64,
        13 => (!(0 as c_int as u64) ^ v1 as u64) as i64,
        _ => 0,
    };

    Ok(r)
}

unsafe fn numarith(op: c_int, v1: f64, v2: f64) -> f64 {
    match op {
        0 => return v1 + v2,
        1 => return v1 - v2,
        2 => return v1 * v2,
        5 => return v1 / v2,
        4 => {
            return if v2 == 2 as c_int as f64 {
                v1 * v1
            } else {
                pow(v1, v2)
            };
        }
        6 => return floor(v1 / v2),
        12 => return -v1,
        3 => luaV_modf(v1, v2),
        _ => return 0 as c_int as f64,
    }
}

pub unsafe fn luaO_rawarith(
    op: c_int,
    p1: *const UnsafeValue,
    p2: *const UnsafeValue,
    res: *mut UnsafeValue,
) -> Result<c_int, ArithError> {
    match op {
        7 | 8 | 9 | 10 | 11 | 13 => {
            let mut i1: i64 = 0;
            let mut i2: i64 = 0;
            if (if (((*p1).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int) as c_int
                != 0 as c_int) as c_int as libc::c_long
                != 0
            {
                i1 = (*p1).value_.i;
                1 as c_int
            } else {
                luaV_tointegerns(p1, &mut i1, F2Ieq)
            }) != 0
                && (if (((*p2).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int) as c_int
                    != 0 as c_int) as c_int as libc::c_long
                    != 0
                {
                    i2 = (*p2).value_.i;
                    1 as c_int
                } else {
                    luaV_tointegerns(p2, &mut i2, F2Ieq)
                }) != 0
            {
                let io: *mut UnsafeValue = res;
                (*io).value_.i = intarith(op, i1, i2)?;
                (*io).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                return Ok(1 as c_int);
            } else {
                return Ok(0 as c_int);
            }
        }
        5 | 4 => {
            let mut n1: f64 = 0.;
            let mut n2: f64 = 0.;
            if (if (*p1).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                n1 = (*p1).value_.n;
                1 as c_int
            } else {
                if (*p1).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                    n1 = (*p1).value_.i as f64;
                    1 as c_int
                } else {
                    0 as c_int
                }
            }) != 0
                && (if (*p2).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                    n2 = (*p2).value_.n;
                    1 as c_int
                } else {
                    if (*p2).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                        n2 = (*p2).value_.i as f64;
                        1 as c_int
                    } else {
                        0 as c_int
                    }
                }) != 0
            {
                let io_0: *mut UnsafeValue = res;
                (*io_0).value_.n = numarith(op, n1, n2);
                (*io_0).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                return Ok(1 as c_int);
            } else {
                return Ok(0 as c_int);
            }
        }
        _ => {
            let mut n1_0: f64 = 0.;
            let mut n2_0: f64 = 0.;
            if (*p1).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                && (*p2).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
            {
                let io_1: *mut UnsafeValue = res;
                (*io_1).value_.i = intarith(op, (*p1).value_.i, (*p2).value_.i)?;
                (*io_1).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                return Ok(1 as c_int);
            } else if (if (*p1).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                n1_0 = (*p1).value_.n;
                1 as c_int
            } else {
                if (*p1).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                    n1_0 = (*p1).value_.i as f64;
                    1 as c_int
                } else {
                    0 as c_int
                }
            }) != 0
                && (if (*p2).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                    n2_0 = (*p2).value_.n;
                    1 as c_int
                } else {
                    if (*p2).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                        n2_0 = (*p2).value_.i as f64;
                        1 as c_int
                    } else {
                        0 as c_int
                    }
                }) != 0
            {
                let io_2: *mut UnsafeValue = res;
                (*io_2).value_.n = numarith(op, n1_0, n2_0);
                (*io_2).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                return Ok(1 as c_int);
            } else {
                return Ok(0 as c_int);
            }
        }
    };
}

pub unsafe fn luaO_arith(
    L: *const Thread,
    op: c_int,
    p1: *const UnsafeValue,
    p2: *const UnsafeValue,
    res: StkId,
) -> Result<(), Box<dyn core::error::Error>> {
    let r = match luaO_rawarith(op, p1, p2, &mut (*res).val) {
        Ok(v) => v,
        Err(e) => return luaG_runerror(L, e),
    };

    if r == 0 {
        luaT_trybinTM(L, p1, p2, res, (op - 0 as c_int + TM_ADD as c_int) as TMS)?;
    }

    Ok(())
}

pub unsafe fn luaO_hexavalue(c: c_int) -> c_int {
    if luai_ctype_[(c + 1 as c_int) as usize] as c_int & (1 as c_int) << 1 as c_int != 0 {
        return c - '0' as i32;
    } else {
        return (c | 'A' as i32 ^ 'a' as i32) - 'a' as i32 + 10 as c_int;
    };
}

unsafe fn isneg(s: *mut *const c_char) -> c_int {
    if **s as c_int == '-' as i32 {
        *s = (*s).offset(1);
        return 1 as c_int;
    } else if **s as c_int == '+' as i32 {
        *s = (*s).offset(1);
    }
    return 0 as c_int;
}

unsafe fn l_str2dloc(s: *const c_char, result: *mut f64, mode: c_int) -> *const c_char {
    let mut endptr: *mut c_char = 0 as *mut c_char;
    *result = if mode == 'x' as i32 {
        strtod(s, &mut endptr)
    } else {
        strtod(s, &mut endptr)
    };
    if endptr == s as *mut c_char {
        return 0 as *const c_char;
    }
    while luai_ctype_[(*endptr as libc::c_uchar as c_int + 1 as c_int) as usize] as c_int
        & (1 as c_int) << 3 as c_int
        != 0
    {
        endptr = endptr.offset(1);
    }
    return if *endptr as c_int == '\0' as i32 {
        endptr
    } else {
        0 as *mut c_char
    };
}

unsafe fn l_str2d(s: *const c_char, result: *mut f64) -> *const c_char {
    let pmode: *const c_char = strpbrk(s, b".xXnN\0" as *const u8 as *const c_char);
    let mode: c_int = if !pmode.is_null() {
        *pmode as libc::c_uchar as c_int | 'A' as i32 ^ 'a' as i32
    } else {
        0 as c_int
    };

    if mode == 'n' as i32 {
        return 0 as *const c_char;
    }

    l_str2dloc(s, result, mode)
}

unsafe fn l_str2int(mut s: *const c_char, result: *mut i64) -> *const c_char {
    let mut a: u64 = 0 as c_int as u64;
    let mut empty: c_int = 1 as c_int;
    let mut neg: c_int = 0;
    while luai_ctype_[(*s as libc::c_uchar as c_int + 1 as c_int) as usize] as c_int
        & (1 as c_int) << 3 as c_int
        != 0
    {
        s = s.offset(1);
    }
    neg = isneg(&mut s);
    if *s.offset(0 as c_int as isize) as c_int == '0' as i32
        && (*s.offset(1 as c_int as isize) as c_int == 'x' as i32
            || *s.offset(1 as c_int as isize) as c_int == 'X' as i32)
    {
        s = s.offset(2 as c_int as isize);
        while luai_ctype_[(*s as libc::c_uchar as c_int + 1 as c_int) as usize] as c_int
            & (1 as c_int) << 4 as c_int
            != 0
        {
            a = a
                .wrapping_mul(16)
                .wrapping_add(luaO_hexavalue(*s as c_int) as u64);
            empty = 0 as c_int;
            s = s.offset(1);
        }
    } else {
        while luai_ctype_[(*s as libc::c_uchar as c_int + 1 as c_int) as usize] as c_int
            & (1 as c_int) << 1 as c_int
            != 0
        {
            let d: c_int = *s as c_int - '0' as i32;
            if a >= (0x7fffffffffffffff as libc::c_longlong / 10 as c_int as libc::c_longlong)
                as u64
                && (a
                    > (0x7fffffffffffffff as libc::c_longlong / 10 as c_int as libc::c_longlong)
                        as u64
                    || d > (0x7fffffffffffffff as libc::c_longlong
                        % 10 as c_int as libc::c_longlong) as c_int
                        + neg)
            {
                return 0 as *const c_char;
            }
            a = (a * 10 as c_int as u64).wrapping_add(d as u64);
            empty = 0 as c_int;
            s = s.offset(1);
        }
    }
    while luai_ctype_[(*s as libc::c_uchar as c_int + 1 as c_int) as usize] as c_int
        & (1 as c_int) << 3 as c_int
        != 0
    {
        s = s.offset(1);
    }
    if empty != 0 || *s as c_int != '\0' as i32 {
        return 0 as *const c_char;
    } else {
        *result = (if neg != 0 {
            (0 as libc::c_uint as u64).wrapping_sub(a)
        } else {
            a
        }) as i64;
        return s;
    };
}

pub unsafe fn luaO_str2num(s: *const c_char, o: *mut UnsafeValue) -> usize {
    let mut i: i64 = 0;
    let mut n: f64 = 0.;
    let mut e: *const c_char = 0 as *const c_char;
    e = l_str2int(s, &mut i);
    if !e.is_null() {
        let io: *mut UnsafeValue = o;
        (*io).value_.i = i;
        (*io).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
    } else {
        e = l_str2d(s, &mut n);
        if !e.is_null() {
            let io_0: *mut UnsafeValue = o;
            (*io_0).value_.n = n;
            (*io_0).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
        } else {
            return 0 as c_int as usize;
        }
    }
    return (e.offset_from(s) as libc::c_long + 1 as c_int as libc::c_long) as usize;
}

pub unsafe fn luaO_utf8esc(buff: *mut c_char, mut x: libc::c_ulong) -> c_int {
    let mut n: c_int = 1 as c_int;
    if x < 0x80 as c_int as libc::c_ulong {
        *buff.offset((8 as c_int - 1 as c_int) as isize) = x as c_char;
    } else {
        let mut mfb: libc::c_uint = 0x3f as c_int as libc::c_uint;
        loop {
            let fresh1 = n;
            n = n + 1;
            *buff.offset((8 as c_int - fresh1) as isize) =
                (0x80 as c_int as libc::c_ulong | x & 0x3f as c_int as libc::c_ulong) as c_char;
            x >>= 6 as c_int;
            mfb >>= 1 as c_int;
            if !(x > mfb as libc::c_ulong) {
                break;
            }
        }
        *buff.offset((8 as c_int - n) as isize) =
            ((!mfb << 1 as c_int) as libc::c_ulong | x) as c_char;
    }
    return n;
}

unsafe fn tostringbuff(obj: *mut UnsafeValue, buff: *mut c_char) -> c_int {
    if (*obj).tt_ == 3 | 0 << 4 {
        sprintf(
            buff,
            b"%lld\0" as *const u8 as *const c_char,
            (*obj).value_.i,
        )
    } else {
        let mut len = sprintf(
            buff,
            b"%.14g\0" as *const u8 as *const c_char,
            (*obj).value_.n,
        );

        if *buff.offset(strspn(buff, c"-0123456789".as_ptr()) as isize) as c_int == '\0' as i32 {
            let fresh2 = len;
            len = len + 1;
            *buff.offset(fresh2 as isize) = b'.' as _;
            let fresh3 = len;
            len = len + 1;
            *buff.offset(fresh3 as isize) = '0' as i32 as c_char;
        }

        len
    }
}

pub unsafe fn luaO_tostring(g: *const Lua, obj: *mut UnsafeValue) {
    let mut buff: [c_char; 44] = [0; 44];
    let len: c_int = tostringbuff(obj, buff.as_mut_ptr());
    let io: *mut UnsafeValue = obj;
    let x_ = Str::new(
        g,
        core::slice::from_raw_parts(buff.as_ptr().cast(), len as usize),
    );

    (*io).value_.gc = x_.cast();
    (*io).tt_ = ((*x_).hdr.tt as c_int | (1 as c_int) << 6 as c_int) as u8;
}
