#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments
)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::gc::Object;
use crate::lctype::luai_ctype_;
use crate::libc::snprintf;
use crate::lmem::luaM_free_;
use crate::ltm::{TM_ADD, TMS, luaT_trybinTM};
use crate::value::UnsafeValue;
use crate::vm::{F2Ieq, luaV_idiv, luaV_mod, luaV_modf, luaV_shiftl, luaV_tointegerns};
use crate::{Args, ArithError, ChunkInfo, Context, Lua, Number, Ops, Ret, Str, Thread};
use alloc::boxed::Box;
use core::cell::{Cell, UnsafeCell};
use libc::{c_char, c_int, strpbrk, strspn, strtod};
use libm::{floor, pow};

#[repr(C)]
pub struct UpVal<D> {
    pub hdr: Object<D>,
    pub v: Cell<*mut UnsafeValue<D>>,
    pub u: UnsafeCell<C2RustUnnamed_5<D>>,
}

#[repr(C)]
pub union C2RustUnnamed_5<D> {
    pub open: C2RustUnnamed_6<D>,
    pub value: UnsafeValue<D>,
}

impl<D> Clone for C2RustUnnamed_5<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for C2RustUnnamed_5<D> {}

#[repr(C)]
pub struct C2RustUnnamed_6<D> {
    pub next: *mut UpVal<D>,
    pub previous: *mut *mut UpVal<D>,
}

impl<D> Clone for C2RustUnnamed_6<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for C2RustUnnamed_6<D> {}

#[repr(C)]
pub struct Upvaldesc<D> {
    pub name: *const Str<D>,
    pub instack: u8,
    pub idx: u8,
    pub kind: u8,
}

impl<D> Clone for Upvaldesc<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for Upvaldesc<D> {}

#[repr(C)]
pub struct LocVar<D> {
    pub varname: *const Str<D>,
    pub startpc: c_int,
    pub endpc: c_int,
}

impl<D> Clone for LocVar<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for LocVar<D> {}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct AbsLineInfo {
    pub pc: c_int,
    pub line: c_int,
}

#[repr(C)]
pub struct Proto<D> {
    pub hdr: Object<D>,
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
    pub k: *mut UnsafeValue<D>,
    pub code: *mut u32,
    pub p: *mut *mut Self,
    pub upvalues: *mut Upvaldesc<D>,
    pub lineinfo: *mut i8,
    pub abslineinfo: *mut AbsLineInfo,
    pub locvars: *mut LocVar<D>,
    pub chunk: ChunkInfo,
}

impl<D> Drop for Proto<D> {
    fn drop(&mut self) {
        unsafe {
            luaM_free_(
                self.code as *mut libc::c_void,
                (self.sizecode as usize).wrapping_mul(::core::mem::size_of::<u32>()),
            )
        };
        unsafe {
            luaM_free_(
                self.p as *mut libc::c_void,
                (self.sizep as usize).wrapping_mul(::core::mem::size_of::<*mut Self>()),
            )
        };
        unsafe {
            luaM_free_(
                self.k as *mut libc::c_void,
                (self.sizek as usize).wrapping_mul(::core::mem::size_of::<UnsafeValue<D>>()),
            )
        };
        unsafe {
            luaM_free_(
                self.lineinfo as *mut libc::c_void,
                (self.sizelineinfo as usize).wrapping_mul(::core::mem::size_of::<i8>()),
            )
        };
        unsafe {
            luaM_free_(
                self.abslineinfo as *mut libc::c_void,
                (self.sizeabslineinfo as usize).wrapping_mul(::core::mem::size_of::<AbsLineInfo>()),
            )
        };
        unsafe {
            luaM_free_(
                self.locvars as *mut libc::c_void,
                (self.sizelocvars as usize).wrapping_mul(::core::mem::size_of::<LocVar<D>>()),
            )
        };
        unsafe {
            luaM_free_(
                self.upvalues as *mut libc::c_void,
                (self.sizeupvalues as usize).wrapping_mul(::core::mem::size_of::<Upvaldesc<D>>()),
            )
        };
    }
}

#[repr(C)]
pub struct CClosure<D> {
    pub hdr: Object<D>,
    pub nupvalues: u8,
    pub f: for<'a> fn(
        Context<'a, D, Args>,
    ) -> Result<Context<'a, D, Ret>, Box<dyn core::error::Error>>,
    pub upvalue: [UnsafeValue<D>; 1],
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

fn intarith(op: Ops, v1: i64, v2: i64) -> Result<i64, ArithError> {
    let r = match op {
        Ops::Add => (v1 as u64).wrapping_add(v2 as u64) as i64,
        Ops::Sub => (v1 as u64).wrapping_sub(v2 as u64) as i64,
        Ops::Mul => (v1 as u64).wrapping_mul(v2 as u64) as i64,
        Ops::Mod => luaV_mod(v1, v2).ok_or(ArithError::ModZero)?,
        Ops::IntDiv => luaV_idiv(v1, v2).ok_or(ArithError::DivZero)?,
        Ops::And => (v1 as u64 & v2 as u64) as i64,
        Ops::Or => (v1 as u64 | v2 as u64) as i64,
        Ops::Xor => (v1 as u64 ^ v2 as u64) as i64,
        Ops::Shl => luaV_shiftl(v1, v2),
        Ops::Shr => luaV_shiftl(v1, (0 as c_int as u64).wrapping_sub(v2 as u64) as i64),
        Ops::Neg => (0 as c_int as u64).wrapping_sub(v1 as u64) as i64,
        Ops::Not => (!(0 as c_int as u64) ^ v1 as u64) as i64,
        _ => 0,
    };

    Ok(r)
}

fn numarith(op: Ops, v1: f64, v2: f64) -> f64 {
    match op {
        Ops::Add => v1 + v2,
        Ops::Sub => v1 - v2,
        Ops::Mul => v1 * v2,
        Ops::NumDiv => v1 / v2,
        Ops::Pow => {
            if v2 == 2.0 {
                v1 * v1
            } else {
                pow(v1, v2)
            }
        }
        Ops::IntDiv => floor(v1 / v2),
        Ops::Neg => -v1,
        Ops::Mod => luaV_modf(v1, v2),
        _ => 0.0,
    }
}

pub unsafe fn luaO_rawarith<D>(
    op: Ops,
    p1: *const UnsafeValue<D>,
    p2: *const UnsafeValue<D>,
) -> Result<Option<UnsafeValue<D>>, ArithError> {
    match op {
        Ops::And | Ops::Or | Ops::Xor | Ops::Shl | Ops::Shr | Ops::Not => {
            let mut i1: i64 = 0;
            let mut i2: i64 = 0;
            if (if (*p1).tt_ == 3 | 0 << 4 {
                i1 = (*p1).value_.i;
                1 as c_int
            } else {
                luaV_tointegerns(p1, &mut i1, F2Ieq)
            }) != 0
                && (if (*p2).tt_ == 3 | 0 << 4 {
                    i2 = (*p2).value_.i;
                    1 as c_int
                } else {
                    luaV_tointegerns(p2, &mut i2, F2Ieq)
                }) != 0
            {
                intarith(op, i1, i2).map(|v| Some(v.into()))
            } else {
                Ok(None)
            }
        }
        Ops::NumDiv | Ops::Pow => {
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
                Ok(Some(numarith(op, n1, n2).into()))
            } else {
                Ok(None)
            }
        }
        op => {
            let mut n1_0: f64 = 0.;
            let mut n2_0: f64 = 0.;

            if (*p1).tt_ == 3 | 0 << 4 && (*p2).tt_ == 3 | 0 << 4 {
                intarith(op, (*p1).value_.i, (*p2).value_.i).map(|v| Some(v.into()))
            } else if (if (*p1).tt_ == 3 | 1 << 4 {
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
                Ok(Some(numarith(op, n1_0, n2_0).into()))
            } else {
                Ok(None)
            }
        }
    }
}

pub unsafe fn luaO_arith<D>(
    L: *const Thread<D>,
    op: Ops,
    p1: *const UnsafeValue<D>,
    p2: *const UnsafeValue<D>,
) -> Result<UnsafeValue<D>, Box<dyn core::error::Error>> {
    match luaO_rawarith(op, p1, p2)? {
        Some(v) => Ok(v),
        None => luaT_trybinTM(L, p1, p2, (op as i32 + TM_ADD as c_int) as TMS),
    }
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

unsafe fn l_str2dloc(s: *const c_char) -> Option<f64> {
    // TODO: How to handle hex floating point in Rust?
    let mut endptr: *mut c_char = 0 as *mut c_char;
    let result = strtod(s, &mut endptr);

    if endptr == s as *mut c_char {
        return None;
    }
    while luai_ctype_[(*endptr as libc::c_uchar as c_int + 1 as c_int) as usize] as c_int
        & (1 as c_int) << 3 as c_int
        != 0
    {
        endptr = endptr.offset(1);
    }
    return if *endptr as c_int == '\0' as i32 {
        Some(result)
    } else {
        None
    };
}

unsafe fn l_str2d(s: *const c_char) -> Option<f64> {
    let pmode: *const c_char = strpbrk(s, b".xXnN\0" as *const u8 as *const c_char);
    let mode: c_int = if !pmode.is_null() {
        *pmode as libc::c_uchar as c_int | 'A' as i32 ^ 'a' as i32
    } else {
        0 as c_int
    };

    if mode == 'n' as i32 {
        return None;
    }

    l_str2dloc(s)
}

unsafe fn l_str2int(mut s: *const c_char) -> Option<i64> {
    let mut a: u64 = 0 as c_int as u64;
    let mut empty: c_int = 1 as c_int;
    let mut neg: c_int = 0;

    // Skip leading whitespace.
    while luai_ctype_[(*s as libc::c_uchar as c_int + 1 as c_int) as usize] as c_int & 1 << 3 != 0 {
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
                return None;
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
        None
    } else {
        let v = if neg != 0 { 0u64.wrapping_sub(a) } else { a };

        Some(v as i64)
    }
}

pub unsafe fn luaO_str2num(s: *const c_char) -> Option<Number> {
    match l_str2int(s) {
        Some(i) => Some(i.into()),
        None => match l_str2d(s) {
            Some(n) => Some(n.into()),
            None => None,
        },
    }
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

unsafe fn tostringbuff<D>(obj: *const UnsafeValue<D>, buff: *mut c_char) -> c_int {
    if (*obj).tt_ == 3 | 0 << 4 {
        snprintf(
            buff,
            44,
            b"%lld\0" as *const u8 as *const c_char,
            (*obj).value_.i,
        )
    } else {
        let mut len = snprintf(
            buff,
            44,
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

#[inline(never)]
pub unsafe fn luaO_tostring<D>(g: *const Lua<D>, obj: *mut UnsafeValue<D>) {
    let mut buff: [c_char; 44] = [0; 44];
    let len: c_int = tostringbuff(obj, buff.as_mut_ptr());
    let s = core::slice::from_raw_parts(buff.as_ptr().cast(), len as usize);
    let io = obj;
    let x_ = Str::from_str(g, core::str::from_utf8(s).unwrap()); // sprintf may effect by locale.

    (*io).value_.gc = x_.cast();
    (*io).tt_ = ((*x_).hdr.tt as c_int | (1 as c_int) << 6 as c_int) as u8;
}
