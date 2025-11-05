#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::context::{Args, Context, Ret};
use crate::gc::Object;
use crate::lctype::luai_ctype_;
use crate::lmem::luaM_free_;
use crate::ltm::{TM_ADD, TMS, luaT_trybinTM};
use crate::value::UnsafeValue;
use crate::vm::{F2Ieq, luaV_idiv, luaV_mod, luaV_modf, luaV_shiftl, luaV_tointegerns};
use crate::{ArithError, ChunkInfo, Float, Number, Ops, Str, Thread};
use alloc::boxed::Box;
use core::cell::{Cell, UnsafeCell};
use core::ffi::{c_char, c_void};

type c_int = i32;
type c_uint = u32;
type c_ulong = u64;

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
    #[cfg(feature = "jit")]
    pub jitted: core::cell::OnceCell<Box<[Jitted<D>]>>,
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
                self.code as *mut c_void,
                (self.sizecode as usize).wrapping_mul(::core::mem::size_of::<u32>()),
            )
        };
        unsafe {
            luaM_free_(
                self.p as *mut c_void,
                (self.sizep as usize).wrapping_mul(::core::mem::size_of::<*mut Self>()),
            )
        };
        unsafe {
            luaM_free_(
                self.k as *mut c_void,
                (self.sizek as usize).wrapping_mul(::core::mem::size_of::<UnsafeValue<D>>()),
            )
        };
        unsafe {
            luaM_free_(
                self.lineinfo as *mut c_void,
                (self.sizelineinfo as usize).wrapping_mul(::core::mem::size_of::<i8>()),
            )
        };
        unsafe {
            luaM_free_(
                self.abslineinfo as *mut c_void,
                (self.sizeabslineinfo as usize).wrapping_mul(::core::mem::size_of::<AbsLineInfo>()),
            )
        };
        unsafe {
            luaM_free_(
                self.locvars as *mut c_void,
                (self.sizelocvars as usize).wrapping_mul(::core::mem::size_of::<LocVar<D>>()),
            )
        };
        unsafe {
            luaM_free_(
                self.upvalues as *mut c_void,
                (self.sizeupvalues as usize).wrapping_mul(::core::mem::size_of::<Upvaldesc<D>>()),
            )
        };
    }
}

#[cfg(feature = "jit")]
pub enum Jitted<A> {
    Inst(u32),
    Func(unsafe extern "C" fn(*const Thread<A>, *const *mut UpVal<A>)),
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

pub unsafe fn luaO_ceillog2(mut x: c_uint) -> c_int {
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

    while x >= 256 as c_int as c_uint {
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

fn numarith(op: Ops, v1: Float, v2: Float) -> Float {
    match op {
        Ops::Add => v1 + v2,
        Ops::Sub => v1 - v2,
        Ops::Mul => v1 * v2,
        Ops::NumDiv => v1 / v2,
        Ops::Pow => {
            if v2 == 2.0 {
                v1 * v1
            } else {
                v1.pow(v2)
            }
        }
        Ops::IntDiv => (v1 / v2).floor(),
        Ops::Neg => -v1,
        Ops::Mod => luaV_modf(v1, v2),
        _ => Float::default(),
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
            } else if let Some(v) = luaV_tointegerns(p1, F2Ieq) {
                i1 = v;
                1
            } else {
                0
            }) != 0
                && (if (*p2).tt_ == 3 | 0 << 4 {
                    i2 = (*p2).value_.i;
                    1 as c_int
                } else if let Some(v) = luaV_tointegerns(p2, F2Ieq) {
                    i2 = v;
                    1
                } else {
                    0
                }) != 0
            {
                intarith(op, i1, i2).map(|v| Some(v.into()))
            } else {
                Ok(None)
            }
        }
        Ops::NumDiv | Ops::Pow => {
            let mut n1 = Float::default();
            let mut n2 = Float::default();

            if (if (*p1).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                n1 = (*p1).value_.n;
                1 as c_int
            } else {
                if (*p1).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                    n1 = ((*p1).value_.i as f64).into();
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
                        n2 = ((*p2).value_.i as f64).into();
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
            let mut n1_0 = Float::default();
            let mut n2_0 = Float::default();

            if (*p1).tt_ == 3 | 0 << 4 && (*p2).tt_ == 3 | 0 << 4 {
                intarith(op, (*p1).value_.i, (*p2).value_.i).map(|v| Some(v.into()))
            } else if (if (*p1).tt_ == 3 | 1 << 4 {
                n1_0 = (*p1).value_.n;
                1 as c_int
            } else {
                if (*p1).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                    n1_0 = ((*p1).value_.i as f64).into();
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
                        n2_0 = ((*p2).value_.i as f64).into();
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

pub unsafe fn luaO_arith<A>(
    L: &Thread<A>,
    op: Ops,
    p1: *const UnsafeValue<A>,
    p2: *const UnsafeValue<A>,
) -> Result<UnsafeValue<A>, Box<dyn core::error::Error>> {
    match luaO_rawarith(op, p1, p2)? {
        Some(v) => Ok(v),
        None => luaT_trybinTM(L, p1, p2, (op as i32 + TM_ADD as c_int) as TMS),
    }
}

pub fn luaO_hexavalue(c: c_int) -> c_int {
    if luai_ctype_[(c + 1 as c_int) as usize] as c_int & (1 as c_int) << 1 as c_int != 0 {
        return c - '0' as i32;
    } else {
        return (c | 'A' as i32 ^ 'a' as i32) - 'a' as i32 + 10 as c_int;
    };
}

fn l_str2d(s: &[u8]) -> Option<f64> {
    let s = s.trim_ascii();

    core::str::from_utf8(s)
        .ok()?
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
}

fn l_str2int(s: &[u8]) -> Option<i64> {
    // Skip leading whitespace.
    let mut s = s.iter();
    let mut b = s.next().copied()?;

    while luai_ctype_[usize::from(b) + 1] & 1 << 3 != 0 {
        b = s.next().copied()?;
    }

    // Check if negative.
    let neg = match b {
        b'-' => {
            b = s.next().copied()?;
            true
        }
        b'+' => {
            b = s.next().copied()?;
            false
        }
        _ => false,
    };

    // Parse.
    let mut a = 0u64;
    let mut empty = true;
    let mut b = if b == b'0' && matches!(s.as_slice().first(), Some(b'x') | Some(b'X')) {
        let mut b = Some(s.by_ref().skip(1).next().copied()?);

        loop {
            let v = match b {
                Some(v) => v,
                None => break,
            };

            if luai_ctype_[usize::from(v) + 1] & 1 << 4 == 0 {
                break;
            }

            a = a
                .wrapping_mul(16)
                .wrapping_add(luaO_hexavalue(v.into()) as u64);
            empty = false;
            b = s.next().copied();
        }

        b
    } else {
        let mut b = Some(b);

        loop {
            let v = match b {
                Some(v) => v,
                None => break,
            };

            if luai_ctype_[usize::from(v) + 1] & 1 << 1 == 0 {
                break;
            }

            // TODO: Refactor this.
            let d = u64::from(v - b'0');

            if a >= (0x7fffffffffffffffu64 / 10)
                && (a > (0x7fffffffffffffffu64 / 10)
                    || d > (0x7fffffffffffffffu64 % 10) + u64::from(neg))
            {
                return None;
            }

            a = a * 10 + d;
            empty = false;
            b = s.next().copied();
        }

        b
    };

    if empty {
        return None;
    }

    // Skip trailing whitespace.
    while let Some(v) = b {
        match luai_ctype_[usize::from(v) + 1] & 1 << 3 != 0 {
            true => b = s.next().copied(),
            false => break,
        }
    }

    if b.is_some() {
        None
    } else {
        let v = if neg { 0u64.wrapping_sub(a) } else { a };

        Some(v as i64)
    }
}

pub fn luaO_str2num(s: &[u8]) -> Option<Number> {
    match l_str2int(s) {
        Some(i) => Some(i.into()),
        None => match l_str2d(s) {
            Some(n) => Some(n.into()),
            None => None,
        },
    }
}

pub unsafe fn luaO_utf8esc(buff: *mut c_char, mut x: c_ulong) -> c_int {
    let mut n: c_int = 1 as c_int;
    if x < 0x80 as c_int as c_ulong {
        *buff.offset((8 as c_int - 1 as c_int) as isize) = x as c_char;
    } else {
        let mut mfb: c_uint = 0x3f as c_int as c_uint;
        loop {
            let fresh1 = n;
            n = n + 1;
            *buff.offset((8 as c_int - fresh1) as isize) =
                (0x80 as c_int as c_ulong | x & 0x3f as c_int as c_ulong) as c_char;
            x >>= 6 as c_int;
            mfb >>= 1 as c_int;
            if !(x > mfb as c_ulong) {
                break;
            }
        }
        *buff.offset((8 as c_int - n) as isize) = ((!mfb << 1 as c_int) as c_ulong | x) as c_char;
    }
    return n;
}
