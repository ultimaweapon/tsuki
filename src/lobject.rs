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
#![allow(unused_parens)]
#![allow(path_statements)]

use crate::api_incr_top;
use crate::lctype::luai_ctype_;
use crate::lstate::{GCUnion, lua_CFunction, lua_State};
use crate::lstring::luaS_newlstr;
use crate::ltm::{TM_ADD, TMS, luaT_trybinTM};
use crate::lvm::{
    F2Ieq, luaV_concat, luaV_idiv, luaV_mod, luaV_modf, luaV_shiftl, luaV_tointegerns,
};
use libc::{localeconv, memcpy, snprintf, strchr, strcpy, strlen, strpbrk, strspn, strtod};
use libm::{floor, pow};

#[derive(Copy, Clone)]
#[repr(C)]
pub union StkIdRel {
    pub p: StkId,
    pub offset: isize,
}

pub type StkId = *mut StackValue;

#[derive(Copy, Clone)]
#[repr(C)]
pub union StackValue {
    pub val: TValue,
    pub tbclist: C2RustUnnamed_4,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct C2RustUnnamed_4 {
    pub value_: Value,
    pub tt_: u8,
    pub delta: libc::c_ushort,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub union Value {
    pub gc: *mut GCObject,
    pub p: *mut libc::c_void,
    pub f: lua_CFunction,
    pub i: i64,
    pub n: f64,
    pub ub: u8,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct GCObject {
    pub next: *mut GCObject,
    pub tt: u8,
    pub marked: u8,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct TValue {
    pub value_: Value,
    pub tt_: u8,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct UpVal {
    pub next: *mut GCObject,
    pub tt: u8,
    pub marked: u8,
    pub v: C2RustUnnamed_7,
    pub u: C2RustUnnamed_5,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub union C2RustUnnamed_5 {
    pub open: C2RustUnnamed_6,
    pub value: TValue,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct C2RustUnnamed_6 {
    pub next: *mut UpVal,
    pub previous: *mut *mut UpVal,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub union C2RustUnnamed_7 {
    pub p: *mut TValue,
    pub offset: isize,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct TString {
    pub next: *mut GCObject,
    pub tt: u8,
    pub marked: u8,
    pub extra: u8,
    pub shrlen: u8,
    pub hash: libc::c_uint,
    pub u: C2RustUnnamed_8,
    pub contents: [libc::c_char; 1],
}

#[derive(Copy, Clone)]
#[repr(C)]
pub union C2RustUnnamed_8 {
    pub lnglen: usize,
    pub hnext: *mut TString,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Table {
    pub next: *mut GCObject,
    pub tt: u8,
    pub marked: u8,
    pub flags: u8,
    pub lsizenode: u8,
    pub alimit: libc::c_uint,
    pub array: *mut TValue,
    pub node: *mut Node,
    pub lastfree: *mut Node,
    pub metatable: *mut Table,
    pub gclist: *mut GCObject,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub union Node {
    pub u: NodeKey,
    pub i_val: TValue,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct NodeKey {
    pub value_: Value,
    pub tt_: u8,
    pub key_tt: u8,
    pub next: libc::c_int,
    pub key_val: Value,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub union UValue {
    pub uv: TValue,
    pub n: f64,
    pub u: libc::c_double,
    pub s: *mut libc::c_void,
    pub i: i64,
    pub l: libc::c_long,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Udata {
    pub next: *mut GCObject,
    pub tt: u8,
    pub marked: u8,
    pub nuvalue: libc::c_ushort,
    pub len: usize,
    pub metatable: *mut Table,
    pub gclist: *mut GCObject,
    pub uv: [UValue; 1],
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Upvaldesc {
    pub name: *mut TString,
    pub instack: u8,
    pub idx: u8,
    pub kind: u8,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct LocVar {
    pub varname: *mut TString,
    pub startpc: libc::c_int,
    pub endpc: libc::c_int,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct AbsLineInfo {
    pub pc: libc::c_int,
    pub line: libc::c_int,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Proto {
    pub next: *mut GCObject,
    pub tt: u8,
    pub marked: u8,
    pub numparams: u8,
    pub is_vararg: u8,
    pub maxstacksize: u8,
    pub sizeupvalues: libc::c_int,
    pub sizek: libc::c_int,
    pub sizecode: libc::c_int,
    pub sizelineinfo: libc::c_int,
    pub sizep: libc::c_int,
    pub sizelocvars: libc::c_int,
    pub sizeabslineinfo: libc::c_int,
    pub linedefined: libc::c_int,
    pub lastlinedefined: libc::c_int,
    pub k: *mut TValue,
    pub code: *mut u32,
    pub p: *mut *mut Proto,
    pub upvalues: *mut Upvaldesc,
    pub lineinfo: *mut i8,
    pub abslineinfo: *mut AbsLineInfo,
    pub locvars: *mut LocVar,
    pub source: *mut TString,
    pub gclist: *mut GCObject,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct CClosure {
    pub next: *mut GCObject,
    pub tt: u8,
    pub marked: u8,
    pub nupvalues: u8,
    pub gclist: *mut GCObject,
    pub f: lua_CFunction,
    pub upvalue: [TValue; 1],
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct LClosure {
    pub next: *mut GCObject,
    pub tt: u8,
    pub marked: u8,
    pub nupvalues: u8,
    pub gclist: *mut GCObject,
    pub p: *mut Proto,
    pub upvals: [*mut UpVal; 1],
}

#[derive(Copy, Clone)]
#[repr(C)]
pub union Closure {
    pub c: CClosure,
    pub l: LClosure,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct BuffFS {
    pub L: *mut lua_State,
    pub pushed: libc::c_int,
    pub blen: libc::c_int,
    pub space: [libc::c_char; 199],
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaO_ceillog2(mut x: libc::c_uint) -> libc::c_int {
    static mut log_2: [u8; 256] = [
        0 as libc::c_int as u8,
        1 as libc::c_int as u8,
        2 as libc::c_int as u8,
        2 as libc::c_int as u8,
        3 as libc::c_int as u8,
        3 as libc::c_int as u8,
        3 as libc::c_int as u8,
        3 as libc::c_int as u8,
        4 as libc::c_int as u8,
        4 as libc::c_int as u8,
        4 as libc::c_int as u8,
        4 as libc::c_int as u8,
        4 as libc::c_int as u8,
        4 as libc::c_int as u8,
        4 as libc::c_int as u8,
        4 as libc::c_int as u8,
        5 as libc::c_int as u8,
        5 as libc::c_int as u8,
        5 as libc::c_int as u8,
        5 as libc::c_int as u8,
        5 as libc::c_int as u8,
        5 as libc::c_int as u8,
        5 as libc::c_int as u8,
        5 as libc::c_int as u8,
        5 as libc::c_int as u8,
        5 as libc::c_int as u8,
        5 as libc::c_int as u8,
        5 as libc::c_int as u8,
        5 as libc::c_int as u8,
        5 as libc::c_int as u8,
        5 as libc::c_int as u8,
        5 as libc::c_int as u8,
        6 as libc::c_int as u8,
        6 as libc::c_int as u8,
        6 as libc::c_int as u8,
        6 as libc::c_int as u8,
        6 as libc::c_int as u8,
        6 as libc::c_int as u8,
        6 as libc::c_int as u8,
        6 as libc::c_int as u8,
        6 as libc::c_int as u8,
        6 as libc::c_int as u8,
        6 as libc::c_int as u8,
        6 as libc::c_int as u8,
        6 as libc::c_int as u8,
        6 as libc::c_int as u8,
        6 as libc::c_int as u8,
        6 as libc::c_int as u8,
        6 as libc::c_int as u8,
        6 as libc::c_int as u8,
        6 as libc::c_int as u8,
        6 as libc::c_int as u8,
        6 as libc::c_int as u8,
        6 as libc::c_int as u8,
        6 as libc::c_int as u8,
        6 as libc::c_int as u8,
        6 as libc::c_int as u8,
        6 as libc::c_int as u8,
        6 as libc::c_int as u8,
        6 as libc::c_int as u8,
        6 as libc::c_int as u8,
        6 as libc::c_int as u8,
        6 as libc::c_int as u8,
        6 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        7 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
        8 as libc::c_int as u8,
    ];
    let mut l: libc::c_int = 0 as libc::c_int;
    x = x.wrapping_sub(1);
    x;
    while x >= 256 as libc::c_int as libc::c_uint {
        l += 8 as libc::c_int;
        x >>= 8 as libc::c_int;
    }
    return l + log_2[x as usize] as libc::c_int;
}

unsafe extern "C" fn intarith(
    mut L: *mut lua_State,
    mut op: libc::c_int,
    mut v1: i64,
    mut v2: i64,
) -> i64 {
    match op {
        0 => return (v1 as u64).wrapping_add(v2 as u64) as i64,
        1 => return (v1 as u64).wrapping_sub(v2 as u64) as i64,
        2 => return (v1 as u64 * v2 as u64) as i64,
        3 => return luaV_mod(L, v1, v2),
        6 => return luaV_idiv(L, v1, v2),
        7 => return (v1 as u64 & v2 as u64) as i64,
        8 => return (v1 as u64 | v2 as u64) as i64,
        9 => return (v1 as u64 ^ v2 as u64) as i64,
        10 => return luaV_shiftl(v1, v2),
        11 => {
            return luaV_shiftl(v1, (0 as libc::c_int as u64).wrapping_sub(v2 as u64) as i64);
        }
        12 => {
            return (0 as libc::c_int as u64).wrapping_sub(v1 as u64) as i64;
        }
        13 => {
            return (!(0 as libc::c_int as u64) ^ v1 as u64) as i64;
        }
        _ => return 0 as libc::c_int as i64,
    };
}

unsafe extern "C" fn numarith(
    mut L: *mut lua_State,
    mut op: libc::c_int,
    mut v1: f64,
    mut v2: f64,
) -> f64 {
    match op {
        0 => return v1 + v2,
        1 => return v1 - v2,
        2 => return v1 * v2,
        5 => return v1 / v2,
        4 => {
            return (if v2 == 2 as libc::c_int as f64 {
                v1 * v1
            } else {
                pow(v1, v2)
            });
        }
        6 => return floor(v1 / v2),
        12 => return -v1,
        3 => return luaV_modf(L, v1, v2),
        _ => return 0 as libc::c_int as f64,
    };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaO_rawarith(
    mut L: *mut lua_State,
    mut op: libc::c_int,
    mut p1: *const TValue,
    mut p2: *const TValue,
    mut res: *mut TValue,
) -> libc::c_int {
    match op {
        7 | 8 | 9 | 10 | 11 | 13 => {
            let mut i1: i64 = 0;
            let mut i2: i64 = 0;
            if (if (((*p1).tt_ as libc::c_int
                == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int)
                as libc::c_int
                != 0 as libc::c_int) as libc::c_int as libc::c_long
                != 0
            {
                i1 = (*p1).value_.i;
                1 as libc::c_int
            } else {
                luaV_tointegerns(p1, &mut i1, F2Ieq)
            }) != 0
                && (if (((*p2).tt_ as libc::c_int
                    == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int)
                    as libc::c_int
                    != 0 as libc::c_int) as libc::c_int as libc::c_long
                    != 0
                {
                    i2 = (*p2).value_.i;
                    1 as libc::c_int
                } else {
                    luaV_tointegerns(p2, &mut i2, F2Ieq)
                }) != 0
            {
                let mut io: *mut TValue = res;
                (*io).value_.i = intarith(L, op, i1, i2);
                (*io).tt_ = (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                return 1 as libc::c_int;
            } else {
                return 0 as libc::c_int;
            }
        }
        5 | 4 => {
            let mut n1: f64 = 0.;
            let mut n2: f64 = 0.;
            if (if (*p1).tt_ as libc::c_int
                == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
            {
                n1 = (*p1).value_.n;
                1 as libc::c_int
            } else {
                (if (*p1).tt_ as libc::c_int
                    == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                {
                    n1 = (*p1).value_.i as f64;
                    1 as libc::c_int
                } else {
                    0 as libc::c_int
                })
            }) != 0
                && (if (*p2).tt_ as libc::c_int
                    == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                {
                    n2 = (*p2).value_.n;
                    1 as libc::c_int
                } else {
                    (if (*p2).tt_ as libc::c_int
                        == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                    {
                        n2 = (*p2).value_.i as f64;
                        1 as libc::c_int
                    } else {
                        0 as libc::c_int
                    })
                }) != 0
            {
                let mut io_0: *mut TValue = res;
                (*io_0).value_.n = numarith(L, op, n1, n2);
                (*io_0).tt_ = (3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
                return 1 as libc::c_int;
            } else {
                return 0 as libc::c_int;
            }
        }
        _ => {
            let mut n1_0: f64 = 0.;
            let mut n2_0: f64 = 0.;
            if (*p1).tt_ as libc::c_int == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                && (*p2).tt_ as libc::c_int
                    == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
            {
                let mut io_1: *mut TValue = res;
                (*io_1).value_.i = intarith(L, op, (*p1).value_.i, (*p2).value_.i);
                (*io_1).tt_ = (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                return 1 as libc::c_int;
            } else if (if (*p1).tt_ as libc::c_int
                == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
            {
                n1_0 = (*p1).value_.n;
                1 as libc::c_int
            } else {
                (if (*p1).tt_ as libc::c_int
                    == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                {
                    n1_0 = (*p1).value_.i as f64;
                    1 as libc::c_int
                } else {
                    0 as libc::c_int
                })
            }) != 0
                && (if (*p2).tt_ as libc::c_int
                    == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                {
                    n2_0 = (*p2).value_.n;
                    1 as libc::c_int
                } else {
                    (if (*p2).tt_ as libc::c_int
                        == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                    {
                        n2_0 = (*p2).value_.i as f64;
                        1 as libc::c_int
                    } else {
                        0 as libc::c_int
                    })
                }) != 0
            {
                let mut io_2: *mut TValue = res;
                (*io_2).value_.n = numarith(L, op, n1_0, n2_0);
                (*io_2).tt_ = (3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
                return 1 as libc::c_int;
            } else {
                return 0 as libc::c_int;
            }
        }
    };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaO_arith(
    mut L: *mut lua_State,
    mut op: libc::c_int,
    mut p1: *const TValue,
    mut p2: *const TValue,
    mut res: StkId,
) {
    if luaO_rawarith(L, op, p1, p2, &mut (*res).val) == 0 {
        luaT_trybinTM(
            L,
            p1,
            p2,
            res,
            (op - 0 as libc::c_int + TM_ADD as libc::c_int) as TMS,
        );
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaO_hexavalue(mut c: libc::c_int) -> libc::c_int {
    if luai_ctype_[(c + 1 as libc::c_int) as usize] as libc::c_int
        & (1 as libc::c_int) << 1 as libc::c_int
        != 0
    {
        return c - '0' as i32;
    } else {
        return (c | 'A' as i32 ^ 'a' as i32) - 'a' as i32 + 10 as libc::c_int;
    };
}

unsafe extern "C" fn isneg(mut s: *mut *const libc::c_char) -> libc::c_int {
    if **s as libc::c_int == '-' as i32 {
        *s = (*s).offset(1);
        return 1 as libc::c_int;
    } else if **s as libc::c_int == '+' as i32 {
        *s = (*s).offset(1);
    }
    return 0 as libc::c_int;
}

unsafe extern "C" fn l_str2dloc(
    mut s: *const libc::c_char,
    mut result: *mut f64,
    mut mode: libc::c_int,
) -> *const libc::c_char {
    let mut endptr: *mut libc::c_char = 0 as *mut libc::c_char;
    *result = if mode == 'x' as i32 {
        strtod(s, &mut endptr)
    } else {
        strtod(s, &mut endptr)
    };
    if endptr == s as *mut libc::c_char {
        return 0 as *const libc::c_char;
    }
    while luai_ctype_[(*endptr as libc::c_uchar as libc::c_int + 1 as libc::c_int) as usize]
        as libc::c_int
        & (1 as libc::c_int) << 3 as libc::c_int
        != 0
    {
        endptr = endptr.offset(1);
        endptr;
    }
    return if *endptr as libc::c_int == '\0' as i32 {
        endptr
    } else {
        0 as *mut libc::c_char
    };
}

unsafe extern "C" fn l_str2d(
    mut s: *const libc::c_char,
    mut result: *mut f64,
) -> *const libc::c_char {
    let mut endptr: *const libc::c_char = 0 as *const libc::c_char;
    let mut pmode: *const libc::c_char = strpbrk(s, b".xXnN\0" as *const u8 as *const libc::c_char);
    let mut mode: libc::c_int = if !pmode.is_null() {
        *pmode as libc::c_uchar as libc::c_int | 'A' as i32 ^ 'a' as i32
    } else {
        0 as libc::c_int
    };
    if mode == 'n' as i32 {
        return 0 as *const libc::c_char;
    }
    endptr = l_str2dloc(s, result, mode);
    if endptr.is_null() {
        let mut buff: [libc::c_char; 201] = [0; 201];
        let mut pdot: *const libc::c_char = strchr(s, '.' as i32);
        if pdot.is_null() || strlen(s) > 200 {
            return 0 as *const libc::c_char;
        }
        strcpy(buff.as_mut_ptr(), s);
        buff[pdot.offset_from(s) as libc::c_long as usize] =
            *((*localeconv()).decimal_point).offset(0 as libc::c_int as isize);
        endptr = l_str2dloc(buff.as_mut_ptr(), result, mode);
        if !endptr.is_null() {
            endptr = s.offset(endptr.offset_from(buff.as_mut_ptr()) as libc::c_long as isize);
        }
    }
    return endptr;
}

unsafe extern "C" fn l_str2int(
    mut s: *const libc::c_char,
    mut result: *mut i64,
) -> *const libc::c_char {
    let mut a: u64 = 0 as libc::c_int as u64;
    let mut empty: libc::c_int = 1 as libc::c_int;
    let mut neg: libc::c_int = 0;
    while luai_ctype_[(*s as libc::c_uchar as libc::c_int + 1 as libc::c_int) as usize]
        as libc::c_int
        & (1 as libc::c_int) << 3 as libc::c_int
        != 0
    {
        s = s.offset(1);
        s;
    }
    neg = isneg(&mut s);
    if *s.offset(0 as libc::c_int as isize) as libc::c_int == '0' as i32
        && (*s.offset(1 as libc::c_int as isize) as libc::c_int == 'x' as i32
            || *s.offset(1 as libc::c_int as isize) as libc::c_int == 'X' as i32)
    {
        s = s.offset(2 as libc::c_int as isize);
        while luai_ctype_[(*s as libc::c_uchar as libc::c_int + 1 as libc::c_int) as usize]
            as libc::c_int
            & (1 as libc::c_int) << 4 as libc::c_int
            != 0
        {
            a = (a * 16 as libc::c_int as u64)
                .wrapping_add(luaO_hexavalue(*s as libc::c_int) as u64);
            empty = 0 as libc::c_int;
            s = s.offset(1);
            s;
        }
    } else {
        while luai_ctype_[(*s as libc::c_uchar as libc::c_int + 1 as libc::c_int) as usize]
            as libc::c_int
            & (1 as libc::c_int) << 1 as libc::c_int
            != 0
        {
            let mut d: libc::c_int = *s as libc::c_int - '0' as i32;
            if a >= (0x7fffffffffffffff as libc::c_longlong / 10 as libc::c_int as libc::c_longlong)
                as u64
                && (a
                    > (0x7fffffffffffffff as libc::c_longlong
                        / 10 as libc::c_int as libc::c_longlong) as u64
                    || d > (0x7fffffffffffffff as libc::c_longlong
                        % 10 as libc::c_int as libc::c_longlong)
                        as libc::c_int
                        + neg)
            {
                return 0 as *const libc::c_char;
            }
            a = (a * 10 as libc::c_int as u64).wrapping_add(d as u64);
            empty = 0 as libc::c_int;
            s = s.offset(1);
            s;
        }
    }
    while luai_ctype_[(*s as libc::c_uchar as libc::c_int + 1 as libc::c_int) as usize]
        as libc::c_int
        & (1 as libc::c_int) << 3 as libc::c_int
        != 0
    {
        s = s.offset(1);
        s;
    }
    if empty != 0 || *s as libc::c_int != '\0' as i32 {
        return 0 as *const libc::c_char;
    } else {
        *result = (if neg != 0 {
            (0 as libc::c_uint as u64).wrapping_sub(a)
        } else {
            a
        }) as i64;
        return s;
    };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaO_str2num(mut s: *const libc::c_char, mut o: *mut TValue) -> usize {
    let mut i: i64 = 0;
    let mut n: f64 = 0.;
    let mut e: *const libc::c_char = 0 as *const libc::c_char;
    e = l_str2int(s, &mut i);
    if !e.is_null() {
        let mut io: *mut TValue = o;
        (*io).value_.i = i;
        (*io).tt_ = (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    } else {
        e = l_str2d(s, &mut n);
        if !e.is_null() {
            let mut io_0: *mut TValue = o;
            (*io_0).value_.n = n;
            (*io_0).tt_ = (3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
        } else {
            return 0 as libc::c_int as usize;
        }
    }
    return (e.offset_from(s) as libc::c_long + 1 as libc::c_int as libc::c_long) as usize;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaO_utf8esc(
    mut buff: *mut libc::c_char,
    mut x: libc::c_ulong,
) -> libc::c_int {
    let mut n: libc::c_int = 1 as libc::c_int;
    if x < 0x80 as libc::c_int as libc::c_ulong {
        *buff.offset((8 as libc::c_int - 1 as libc::c_int) as isize) = x as libc::c_char;
    } else {
        let mut mfb: libc::c_uint = 0x3f as libc::c_int as libc::c_uint;
        loop {
            let fresh1 = n;
            n = n + 1;
            *buff.offset((8 as libc::c_int - fresh1) as isize) =
                (0x80 as libc::c_int as libc::c_ulong | x & 0x3f as libc::c_int as libc::c_ulong)
                    as libc::c_char;
            x >>= 6 as libc::c_int;
            mfb >>= 1 as libc::c_int;
            if !(x > mfb as libc::c_ulong) {
                break;
            }
        }
        *buff.offset((8 as libc::c_int - n) as isize) =
            ((!mfb << 1 as libc::c_int) as libc::c_ulong | x) as libc::c_char;
    }
    return n;
}

unsafe extern "C" fn tostringbuff(
    mut obj: *mut TValue,
    mut buff: *mut libc::c_char,
) -> libc::c_int {
    let mut len: libc::c_int = 0;
    if (*obj).tt_ as libc::c_int == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int {
        len = snprintf(
            buff,
            44,
            b"%lld\0" as *const u8 as *const libc::c_char,
            (*obj).value_.i,
        );
    } else {
        len = snprintf(
            buff,
            44,
            b"%.14g\0" as *const u8 as *const libc::c_char,
            (*obj).value_.n,
        );
        if *buff.offset(strspn(buff, b"-0123456789\0" as *const u8 as *const libc::c_char) as isize)
            as libc::c_int
            == '\0' as i32
        {
            let fresh2 = len;
            len = len + 1;
            *buff.offset(fresh2 as isize) =
                *((*localeconv()).decimal_point).offset(0 as libc::c_int as isize);
            let fresh3 = len;
            len = len + 1;
            *buff.offset(fresh3 as isize) = '0' as i32 as libc::c_char;
        }
    }
    return len;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaO_tostring(mut L: *mut lua_State, mut obj: *mut TValue) {
    let mut buff: [libc::c_char; 44] = [0; 44];
    let mut len: libc::c_int = tostringbuff(obj, buff.as_mut_ptr());
    let mut io: *mut TValue = obj;
    let mut x_: *mut TString = luaS_newlstr(L, buff.as_mut_ptr(), len as usize);
    (*io).value_.gc = &mut (*(x_ as *mut GCUnion)).gc;
    (*io).tt_ = ((*x_).tt as libc::c_int | (1 as libc::c_int) << 6 as libc::c_int) as u8;
}

unsafe extern "C" fn pushstr(mut buff: *mut BuffFS, mut str: *const libc::c_char, mut lstr: usize) {
    let mut L: *mut lua_State = (*buff).L;
    let mut io: *mut TValue = &mut (*(*L).top.p).val;
    let mut x_: *mut TString = luaS_newlstr(L, str, lstr);
    (*io).value_.gc = &mut (*(x_ as *mut GCUnion)).gc;
    (*io).tt_ = ((*x_).tt as libc::c_int | (1 as libc::c_int) << 6 as libc::c_int) as u8;
    if (*buff).pushed == 0 {
        (*buff).pushed = 1 as libc::c_int;
        api_incr_top(L);
    } else {
        (*L).top.p = ((*L).top.p).offset(1);
        (*L).top.p;
        luaV_concat(L, 2 as libc::c_int);
    };
}

unsafe extern "C" fn clearbuff(mut buff: *mut BuffFS) {
    pushstr(buff, ((*buff).space).as_mut_ptr(), (*buff).blen as usize);
    (*buff).blen = 0 as libc::c_int;
}

unsafe extern "C" fn getbuff(mut buff: *mut BuffFS, mut sz: libc::c_int) -> *mut libc::c_char {
    if sz > 60 as libc::c_int + 44 as libc::c_int + 95 as libc::c_int - (*buff).blen {
        clearbuff(buff);
    }
    return ((*buff).space).as_mut_ptr().offset((*buff).blen as isize);
}

unsafe extern "C" fn addstr2buff(
    mut buff: *mut BuffFS,
    mut str: *const libc::c_char,
    mut slen: usize,
) {
    if slen <= (60 as libc::c_int + 44 as libc::c_int + 95 as libc::c_int) as usize {
        let mut bf: *mut libc::c_char = getbuff(buff, slen as libc::c_int);
        memcpy(bf as *mut libc::c_void, str as *const libc::c_void, slen);
        (*buff).blen += slen as libc::c_int;
    } else {
        clearbuff(buff);
        pushstr(buff, str, slen);
    };
}

unsafe extern "C" fn addnum2buff(mut buff: *mut BuffFS, mut num: *mut TValue) {
    let mut numbuff: *mut libc::c_char = getbuff(buff, 44 as libc::c_int);
    let mut len: libc::c_int = tostringbuff(num, numbuff);
    (*buff).blen += len;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaO_chunkid(
    mut out: *mut libc::c_char,
    mut source: *const libc::c_char,
    mut srclen: usize,
) {
    let mut bufflen: usize = 60 as libc::c_int as usize;
    if *source as libc::c_int == '=' as i32 {
        if srclen <= bufflen {
            memcpy(
                out as *mut libc::c_void,
                source.offset(1 as libc::c_int as isize) as *const libc::c_void,
                srclen.wrapping_mul(::core::mem::size_of::<libc::c_char>()),
            );
        } else {
            memcpy(
                out as *mut libc::c_void,
                source.offset(1 as libc::c_int as isize) as *const libc::c_void,
                bufflen
                    .wrapping_sub(1 as libc::c_int as usize)
                    .wrapping_mul(::core::mem::size_of::<libc::c_char>()),
            );
            out = out.offset(bufflen.wrapping_sub(1 as libc::c_int as usize) as isize);
            *out = '\0' as i32 as libc::c_char;
        }
    } else if *source as libc::c_int == '@' as i32 {
        if srclen <= bufflen {
            memcpy(
                out as *mut libc::c_void,
                source.offset(1 as libc::c_int as isize) as *const libc::c_void,
                srclen.wrapping_mul(::core::mem::size_of::<libc::c_char>()),
            );
        } else {
            memcpy(
                out as *mut libc::c_void,
                b"...\0" as *const u8 as *const libc::c_char as *const libc::c_void,
                ::core::mem::size_of::<[libc::c_char; 4]>()
                    .wrapping_div(::core::mem::size_of::<libc::c_char>())
                    .wrapping_sub(1)
                    .wrapping_mul(::core::mem::size_of::<libc::c_char>()),
            );
            out = out.offset(
                (::core::mem::size_of::<[libc::c_char; 4]>() as libc::c_ulong)
                    .wrapping_div(::core::mem::size_of::<libc::c_char>() as libc::c_ulong)
                    .wrapping_sub(1 as libc::c_int as libc::c_ulong) as isize,
            );
            bufflen = (bufflen as libc::c_ulong).wrapping_sub(
                (::core::mem::size_of::<[libc::c_char; 4]>() as libc::c_ulong)
                    .wrapping_div(::core::mem::size_of::<libc::c_char>() as libc::c_ulong)
                    .wrapping_sub(1 as libc::c_int as libc::c_ulong),
            ) as usize as usize;
            memcpy(
                out as *mut libc::c_void,
                source
                    .offset(1 as libc::c_int as isize)
                    .offset(srclen as isize)
                    .offset(-(bufflen as isize)) as *const libc::c_void,
                bufflen.wrapping_mul(::core::mem::size_of::<libc::c_char>()),
            );
        }
    } else {
        let mut nl: *const libc::c_char = strchr(source, '\n' as i32);
        memcpy(
            out as *mut libc::c_void,
            b"[string \"\0" as *const u8 as *const libc::c_char as *const libc::c_void,
            ::core::mem::size_of::<[libc::c_char; 10]>()
                .wrapping_div(::core::mem::size_of::<libc::c_char>())
                .wrapping_sub(1)
                .wrapping_mul(::core::mem::size_of::<libc::c_char>()),
        );
        out = out.offset(
            (::core::mem::size_of::<[libc::c_char; 10]>() as libc::c_ulong)
                .wrapping_div(::core::mem::size_of::<libc::c_char>() as libc::c_ulong)
                .wrapping_sub(1 as libc::c_int as libc::c_ulong) as isize,
        );
        bufflen = (bufflen as libc::c_ulong).wrapping_sub(
            (::core::mem::size_of::<[libc::c_char; 15]>() as libc::c_ulong)
                .wrapping_div(::core::mem::size_of::<libc::c_char>() as libc::c_ulong)
                .wrapping_sub(1 as libc::c_int as libc::c_ulong)
                .wrapping_add(1 as libc::c_int as libc::c_ulong),
        ) as usize as usize;
        if srclen < bufflen && nl.is_null() {
            memcpy(
                out as *mut libc::c_void,
                source as *const libc::c_void,
                srclen.wrapping_mul(::core::mem::size_of::<libc::c_char>()),
            );
            out = out.offset(srclen as isize);
        } else {
            if !nl.is_null() {
                srclen = nl.offset_from(source) as libc::c_long as usize;
            }
            if srclen > bufflen {
                srclen = bufflen;
            }
            memcpy(
                out as *mut libc::c_void,
                source as *const libc::c_void,
                srclen.wrapping_mul(::core::mem::size_of::<libc::c_char>()),
            );
            out = out.offset(srclen as isize);
            memcpy(
                out as *mut libc::c_void,
                b"...\0" as *const u8 as *const libc::c_char as *const libc::c_void,
                ::core::mem::size_of::<[libc::c_char; 4]>()
                    .wrapping_div(::core::mem::size_of::<libc::c_char>())
                    .wrapping_sub(1)
                    .wrapping_mul(::core::mem::size_of::<libc::c_char>()),
            );
            out = out.offset(
                (::core::mem::size_of::<[libc::c_char; 4]>() as libc::c_ulong)
                    .wrapping_div(::core::mem::size_of::<libc::c_char>() as libc::c_ulong)
                    .wrapping_sub(1 as libc::c_int as libc::c_ulong) as isize,
            );
        }
        memcpy(
            out as *mut libc::c_void,
            b"\"]\0" as *const u8 as *const libc::c_char as *const libc::c_void,
            ::core::mem::size_of::<[libc::c_char; 3]>()
                .wrapping_div(::core::mem::size_of::<libc::c_char>())
                .wrapping_sub(1)
                .wrapping_add(1)
                .wrapping_mul(::core::mem::size_of::<libc::c_char>()),
        );
    };
}
