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
#![allow(path_statements)]

use crate::api_incr_top;
use crate::ldo::{luaD_hook, luaD_hookcall};
use crate::lfunc::luaF_getlocalname;
use crate::lobject::{
    Closure, GCObject, LClosure, Proto, StkId, TString, TValue, Table, Value, luaO_chunkid,
};
use crate::lopcodes::{OpCode, luaP_opmodes};
use crate::lstate::{CallInfo, GCUnion, lua_Debug, lua_Hook, lua_State};
use crate::ltable::{luaH_new, luaH_setint};
use crate::ltm::{
    TM_BNOT, TM_CLOSE, TM_CONCAT, TM_EQ, TM_INDEX, TM_LE, TM_LEN, TM_LT, TM_NEWINDEX, TM_UNM, TMS,
    luaT_objtypename,
};
use crate::lvm::{F2Ieq, luaV_tointegerns};
use libc::{strchr, strcmp};
use std::borrow::Cow;
use std::ffi::{CStr, c_int};
use std::fmt::Display;

unsafe extern "C" fn currentpc(mut ci: *mut CallInfo) -> libc::c_int {
    return ((*ci).u.savedpc)
        .offset_from((*(*((*(*ci).func.p).val.value_.gc as *mut GCUnion)).cl.l.p).code)
        as libc::c_long as libc::c_int
        - 1 as libc::c_int;
}

unsafe extern "C" fn getbaseline(
    mut f: *const Proto,
    mut pc: libc::c_int,
    mut basepc: *mut libc::c_int,
) -> libc::c_int {
    if (*f).sizeabslineinfo == 0 as libc::c_int
        || pc < (*((*f).abslineinfo).offset(0 as libc::c_int as isize)).pc
    {
        *basepc = -(1 as libc::c_int);
        return (*f).linedefined;
    } else {
        let mut i: libc::c_int = (pc as libc::c_uint)
            .wrapping_div(128 as libc::c_int as libc::c_uint)
            .wrapping_sub(1 as libc::c_int as libc::c_uint)
            as libc::c_int;
        while (i + 1 as libc::c_int) < (*f).sizeabslineinfo
            && pc >= (*((*f).abslineinfo).offset((i + 1 as libc::c_int) as isize)).pc
        {
            i += 1;
            i;
        }
        *basepc = (*((*f).abslineinfo).offset(i as isize)).pc;
        return (*((*f).abslineinfo).offset(i as isize)).line;
    };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaG_getfuncline(mut f: *const Proto, mut pc: libc::c_int) -> libc::c_int {
    if ((*f).lineinfo).is_null() {
        return -(1 as libc::c_int);
    } else {
        let mut basepc: libc::c_int = 0;
        let mut baseline: libc::c_int = getbaseline(f, pc, &mut basepc);
        loop {
            let fresh1 = basepc;
            basepc = basepc + 1;
            if !(fresh1 < pc) {
                break;
            }
            baseline += *((*f).lineinfo).offset(basepc as isize) as libc::c_int;
        }
        return baseline;
    };
}

unsafe extern "C" fn getcurrentline(mut ci: *mut CallInfo) -> libc::c_int {
    return luaG_getfuncline(
        (*((*(*ci).func.p).val.value_.gc as *mut GCUnion)).cl.l.p,
        currentpc(ci),
    );
}

unsafe extern "C" fn settraps(mut ci: *mut CallInfo) {
    while !ci.is_null() {
        if (*ci).callstatus as libc::c_int & (1 as libc::c_int) << 1 as libc::c_int == 0 {
            ::core::ptr::write_volatile(&mut (*ci).u.trap as *mut libc::c_int, 1 as libc::c_int);
        }
        ci = (*ci).previous;
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lua_sethook(
    mut L: *mut lua_State,
    mut func: lua_Hook,
    mut mask: libc::c_int,
    mut count: libc::c_int,
) {
    if func.is_none() || mask == 0 as libc::c_int {
        mask = 0 as libc::c_int;
        func = None;
    }
    ::core::ptr::write_volatile(&mut (*L).hook as *mut lua_Hook, func);
    (*L).basehookcount = count;
    (*L).hookcount = (*L).basehookcount;
    ::core::ptr::write_volatile(
        &mut (*L).hookmask as *mut libc::c_int,
        mask as u8 as libc::c_int,
    );
    if mask != 0 {
        settraps((*L).ci);
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lua_gethook(mut L: *mut lua_State) -> lua_Hook {
    return (*L).hook;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lua_gethookmask(mut L: *mut lua_State) -> libc::c_int {
    return (*L).hookmask;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lua_gethookcount(mut L: *mut lua_State) -> libc::c_int {
    return (*L).basehookcount;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lua_getstack(
    mut L: *mut lua_State,
    mut level: libc::c_int,
    mut ar: *mut lua_Debug,
) -> libc::c_int {
    let mut status: libc::c_int = 0;
    let mut ci: *mut CallInfo = 0 as *mut CallInfo;
    if level < 0 as libc::c_int {
        return 0 as libc::c_int;
    }
    ci = (*L).ci;
    while level > 0 as libc::c_int && ci != &mut (*L).base_ci as *mut CallInfo {
        level -= 1;
        level;
        ci = (*ci).previous;
    }
    if level == 0 as libc::c_int && ci != &mut (*L).base_ci as *mut CallInfo {
        status = 1 as libc::c_int;
        (*ar).i_ci = ci;
    } else {
        status = 0 as libc::c_int;
    }
    return status;
}

unsafe extern "C" fn upvalname(mut p: *const Proto, mut uv: libc::c_int) -> *const libc::c_char {
    let mut s: *mut TString = (*((*p).upvalues).offset(uv as isize)).name;
    if s.is_null() {
        return b"?\0" as *const u8 as *const libc::c_char;
    } else {
        return ((*s).contents).as_mut_ptr();
    };
}

unsafe extern "C" fn findvararg(
    mut ci: *mut CallInfo,
    mut n: libc::c_int,
    mut pos: *mut StkId,
) -> *const libc::c_char {
    if (*(*((*(*ci).func.p).val.value_.gc as *mut GCUnion)).cl.l.p).is_vararg != 0 {
        let mut nextra: libc::c_int = (*ci).u.nextraargs;
        if n >= -nextra {
            *pos = ((*ci).func.p)
                .offset(-(nextra as isize))
                .offset(-((n + 1 as libc::c_int) as isize));
            return b"(vararg)\0" as *const u8 as *const libc::c_char;
        }
    }
    return 0 as *const libc::c_char;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaG_findlocal(
    mut L: *mut lua_State,
    mut ci: *mut CallInfo,
    mut n: libc::c_int,
    mut pos: *mut StkId,
) -> *const libc::c_char {
    let mut base: StkId = ((*ci).func.p).offset(1 as libc::c_int as isize);
    let mut name: *const libc::c_char = 0 as *const libc::c_char;
    if (*ci).callstatus as libc::c_int & (1 as libc::c_int) << 1 as libc::c_int == 0 {
        if n < 0 as libc::c_int {
            return findvararg(ci, n, pos);
        } else {
            name = luaF_getlocalname(
                (*((*(*ci).func.p).val.value_.gc as *mut GCUnion)).cl.l.p,
                n,
                currentpc(ci),
            );
        }
    }
    if name.is_null() {
        let mut limit: StkId = if ci == (*L).ci {
            (*L).top.p
        } else {
            (*(*ci).next).func.p
        };
        if limit.offset_from(base) as libc::c_long >= n as libc::c_long && n > 0 as libc::c_int {
            name = if (*ci).callstatus as libc::c_int & (1 as libc::c_int) << 1 as libc::c_int == 0
            {
                b"(temporary)\0" as *const u8 as *const libc::c_char
            } else {
                b"(C temporary)\0" as *const u8 as *const libc::c_char
            };
        } else {
            return 0 as *const libc::c_char;
        }
    }
    if !pos.is_null() {
        *pos = base.offset((n - 1 as libc::c_int) as isize);
    }
    return name;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lua_getlocal(
    mut L: *mut lua_State,
    mut ar: *const lua_Debug,
    mut n: libc::c_int,
) -> *const libc::c_char {
    let mut name: *const libc::c_char = 0 as *const libc::c_char;
    if ar.is_null() {
        if !((*((*L).top.p).offset(-(1 as libc::c_int as isize))).val.tt_ as libc::c_int
            == 6 as libc::c_int
                | (0 as libc::c_int) << 4 as libc::c_int
                | (1 as libc::c_int) << 6 as libc::c_int)
        {
            name = 0 as *const libc::c_char;
        } else {
            name = luaF_getlocalname(
                (*((*((*L).top.p).offset(-(1 as libc::c_int as isize)))
                    .val
                    .value_
                    .gc as *mut GCUnion))
                    .cl
                    .l
                    .p,
                n,
                0 as libc::c_int,
            );
        }
    } else {
        let mut pos: StkId = 0 as StkId;
        name = luaG_findlocal(L, (*ar).i_ci, n, &mut pos);
        if !name.is_null() {
            let mut io1: *mut TValue = &mut (*(*L).top.p).val;
            let mut io2: *const TValue = &mut (*pos).val;
            (*io1).value_ = (*io2).value_;
            (*io1).tt_ = (*io2).tt_;
            api_incr_top(L);
        }
    }
    return name;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lua_setlocal(
    mut L: *mut lua_State,
    mut ar: *const lua_Debug,
    mut n: libc::c_int,
) -> *const libc::c_char {
    let mut pos: StkId = 0 as StkId;
    let mut name: *const libc::c_char = 0 as *const libc::c_char;
    name = luaG_findlocal(L, (*ar).i_ci, n, &mut pos);
    if !name.is_null() {
        let mut io1: *mut TValue = &mut (*pos).val;
        let mut io2: *const TValue = &mut (*((*L).top.p).offset(-(1 as libc::c_int as isize))).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        (*L).top.p = ((*L).top.p).offset(-1);
        (*L).top.p;
    }
    return name;
}
unsafe extern "C" fn funcinfo(mut ar: *mut lua_Debug, mut cl: *mut Closure) {
    if !(!cl.is_null()
        && (*cl).c.tt as libc::c_int == 6 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int)
    {
        (*ar).source = b"=[C]\0" as *const u8 as *const libc::c_char;
        (*ar).srclen = ::core::mem::size_of::<[libc::c_char; 5]>()
            .wrapping_div(::core::mem::size_of::<libc::c_char>())
            .wrapping_sub(1);
        (*ar).linedefined = -(1 as libc::c_int);
        (*ar).lastlinedefined = -(1 as libc::c_int);
        (*ar).what = b"C\0" as *const u8 as *const libc::c_char;
    } else {
        let mut p: *const Proto = (*cl).l.p;
        if !((*p).source).is_null() {
            (*ar).source = ((*(*p).source).contents).as_mut_ptr();
            (*ar).srclen = if (*(*p).source).shrlen as libc::c_int != 0xff as libc::c_int {
                (*(*p).source).shrlen as usize
            } else {
                (*(*p).source).u.lnglen
            };
        } else {
            (*ar).source = b"=?\0" as *const u8 as *const libc::c_char;
            (*ar).srclen = ::core::mem::size_of::<[libc::c_char; 3]>()
                .wrapping_div(::core::mem::size_of::<libc::c_char>())
                .wrapping_sub(1);
        }
        (*ar).linedefined = (*p).linedefined;
        (*ar).lastlinedefined = (*p).lastlinedefined;
        (*ar).what = if (*ar).linedefined == 0 as libc::c_int {
            b"main\0" as *const u8 as *const libc::c_char
        } else {
            b"Lua\0" as *const u8 as *const libc::c_char
        };
    }
    luaO_chunkid(((*ar).short_src).as_mut_ptr(), (*ar).source, (*ar).srclen);
}
unsafe extern "C" fn nextline(
    mut p: *const Proto,
    mut currentline: libc::c_int,
    mut pc: libc::c_int,
) -> libc::c_int {
    if *((*p).lineinfo).offset(pc as isize) as libc::c_int != -(0x80 as libc::c_int) {
        return currentline + *((*p).lineinfo).offset(pc as isize) as libc::c_int;
    } else {
        return luaG_getfuncline(p, pc);
    };
}

unsafe fn collectvalidlines(
    mut L: *mut lua_State,
    mut f: *mut Closure,
) -> Result<(), Box<dyn std::error::Error>> {
    if !(!f.is_null()
        && (*f).c.tt as libc::c_int == 6 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int)
    {
        (*(*L).top.p).val.tt_ = (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
        api_incr_top(L);
    } else {
        let mut p: *const Proto = (*f).l.p;
        let mut currentline: libc::c_int = (*p).linedefined;
        let mut t: *mut Table = luaH_new(L)?;
        let mut io: *mut TValue = &mut (*(*L).top.p).val;
        let mut x_: *mut Table = t;
        (*io).value_.gc = &mut (*(x_ as *mut GCUnion)).gc;
        (*io).tt_ = (5 as libc::c_int
            | (0 as libc::c_int) << 4 as libc::c_int
            | (1 as libc::c_int) << 6 as libc::c_int) as u8;
        api_incr_top(L);
        if !((*p).lineinfo).is_null() {
            let mut i: libc::c_int = 0;
            let mut v: TValue = TValue {
                value_: Value {
                    gc: 0 as *mut GCObject,
                },
                tt_: 0,
            };
            v.tt_ = (1 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
            if (*p).is_vararg == 0 {
                i = 0 as libc::c_int;
            } else {
                currentline = nextline(p, currentline, 0 as libc::c_int);
                i = 1 as libc::c_int;
            }
            while i < (*p).sizelineinfo {
                currentline = nextline(p, currentline, i);
                luaH_setint(L, t, currentline as i64, &mut v)?;
                i += 1;
                i;
            }
        }
    };
    Ok(())
}

unsafe extern "C" fn getfuncname(
    mut L: *mut lua_State,
    mut ci: *mut CallInfo,
    mut name: *mut *const libc::c_char,
) -> *const libc::c_char {
    if !ci.is_null()
        && (*ci).callstatus as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int == 0
    {
        return funcnamefromcall(L, (*ci).previous, name);
    } else {
        return 0 as *const libc::c_char;
    };
}
unsafe extern "C" fn auxgetinfo(
    mut L: *mut lua_State,
    mut what: *const libc::c_char,
    mut ar: *mut lua_Debug,
    mut f: *mut Closure,
    mut ci: *mut CallInfo,
) -> libc::c_int {
    let mut status: libc::c_int = 1 as libc::c_int;
    while *what != 0 {
        match *what as libc::c_int {
            83 => {
                funcinfo(ar, f);
            }
            108 => {
                (*ar).currentline = if !ci.is_null()
                    && (*ci).callstatus as libc::c_int & (1 as libc::c_int) << 1 as libc::c_int == 0
                {
                    getcurrentline(ci)
                } else {
                    -(1 as libc::c_int)
                };
            }
            117 => {
                (*ar).nups = (if f.is_null() {
                    0 as libc::c_int
                } else {
                    (*f).c.nupvalues as libc::c_int
                }) as libc::c_uchar;
                if !(!f.is_null()
                    && (*f).c.tt as libc::c_int
                        == 6 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int)
                {
                    (*ar).isvararg = 1 as libc::c_int as libc::c_char;
                    (*ar).nparams = 0 as libc::c_int as libc::c_uchar;
                } else {
                    (*ar).isvararg = (*(*f).l.p).is_vararg as libc::c_char;
                    (*ar).nparams = (*(*f).l.p).numparams;
                }
            }
            116 => {
                (*ar).istailcall = (if !ci.is_null() {
                    (*ci).callstatus as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int
                } else {
                    0 as libc::c_int
                }) as libc::c_char;
            }
            110 => {
                (*ar).namewhat = getfuncname(L, ci, &mut (*ar).name);
                if ((*ar).namewhat).is_null() {
                    (*ar).namewhat = b"\0" as *const u8 as *const libc::c_char;
                    (*ar).name = 0 as *const libc::c_char;
                }
            }
            114 => {
                if ci.is_null()
                    || (*ci).callstatus as libc::c_int & (1 as libc::c_int) << 8 as libc::c_int == 0
                {
                    (*ar).ntransfer = 0 as libc::c_int as libc::c_ushort;
                    (*ar).ftransfer = (*ar).ntransfer;
                } else {
                    (*ar).ftransfer = (*ci).u2.transferinfo.ftransfer;
                    (*ar).ntransfer = (*ci).u2.transferinfo.ntransfer;
                }
            }
            76 | 102 => {}
            _ => {
                status = 0 as libc::c_int;
            }
        }
        what = what.offset(1);
        what;
    }
    return status;
}

pub unsafe fn lua_getinfo(
    mut L: *mut lua_State,
    mut what: *const libc::c_char,
    mut ar: *mut lua_Debug,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut status: libc::c_int = 0;
    let mut cl: *mut Closure = 0 as *mut Closure;
    let mut ci: *mut CallInfo = 0 as *mut CallInfo;
    let mut func: *mut TValue = 0 as *mut TValue;
    if *what as libc::c_int == '>' as i32 {
        ci = 0 as *mut CallInfo;
        func = &mut (*((*L).top.p).offset(-(1 as libc::c_int as isize))).val;
        what = what.offset(1);
        what;
        (*L).top.p = ((*L).top.p).offset(-1);
        (*L).top.p;
    } else {
        ci = (*ar).i_ci;
        func = &mut (*(*ci).func.p).val;
    }
    cl = if (*func).tt_ as libc::c_int
        == 6 as libc::c_int
            | (0 as libc::c_int) << 4 as libc::c_int
            | (1 as libc::c_int) << 6 as libc::c_int
        || (*func).tt_ as libc::c_int
            == 6 as libc::c_int
                | (2 as libc::c_int) << 4 as libc::c_int
                | (1 as libc::c_int) << 6 as libc::c_int
    {
        &mut (*((*func).value_.gc as *mut GCUnion)).cl
    } else {
        0 as *mut Closure
    };
    status = auxgetinfo(L, what, ar, cl, ci);
    if !(strchr(what, 'f' as i32)).is_null() {
        let mut io1: *mut TValue = &mut (*(*L).top.p).val;
        let mut io2: *const TValue = func;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        api_incr_top(L);
    }
    if !(strchr(what, 'L' as i32)).is_null() {
        collectvalidlines(L, cl)?;
    }
    return Ok(status);
}

unsafe extern "C" fn filterpc(mut pc: libc::c_int, mut jmptarget: libc::c_int) -> libc::c_int {
    if pc < jmptarget {
        return -(1 as libc::c_int);
    } else {
        return pc;
    };
}
unsafe extern "C" fn findsetreg(
    mut p: *const Proto,
    mut lastpc: libc::c_int,
    mut reg: libc::c_int,
) -> libc::c_int {
    let mut pc: libc::c_int = 0;
    let mut setreg: libc::c_int = -(1 as libc::c_int);
    let mut jmptarget: libc::c_int = 0 as libc::c_int;
    if luaP_opmodes[(*((*p).code).offset(lastpc as isize) >> 0 as libc::c_int
        & !(!(0 as libc::c_int as u32) << 7 as libc::c_int) << 0 as libc::c_int)
        as OpCode as usize] as libc::c_int
        & (1 as libc::c_int) << 7 as libc::c_int
        != 0
    {
        lastpc -= 1;
        lastpc;
    }
    pc = 0 as libc::c_int;
    while pc < lastpc {
        let mut i: u32 = *((*p).code).offset(pc as isize);
        let mut op: OpCode = (i >> 0 as libc::c_int
            & !(!(0 as libc::c_int as u32) << 7 as libc::c_int) << 0 as libc::c_int)
            as OpCode;
        let mut a: libc::c_int = (i >> 0 as libc::c_int + 7 as libc::c_int
            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
            as libc::c_int;
        let mut change: libc::c_int = 0;
        match op as libc::c_uint {
            8 => {
                let mut b: libc::c_int = (i
                    >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int
                    & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                    as libc::c_int;
                change = (a <= reg && reg <= a + b) as libc::c_int;
            }
            76 => {
                change = (reg >= a + 2 as libc::c_int) as libc::c_int;
            }
            68 | 69 => {
                change = (reg >= a) as libc::c_int;
            }
            56 => {
                let mut b_0: libc::c_int = (i >> 0 as libc::c_int + 7 as libc::c_int
                    & !(!(0 as libc::c_int as u32)
                        << 8 as libc::c_int
                            + 8 as libc::c_int
                            + 1 as libc::c_int
                            + 8 as libc::c_int)
                        << 0 as libc::c_int)
                    as libc::c_int
                    - (((1 as libc::c_int)
                        << 8 as libc::c_int
                            + 8 as libc::c_int
                            + 1 as libc::c_int
                            + 8 as libc::c_int)
                        - 1 as libc::c_int
                        >> 1 as libc::c_int);
                let mut dest: libc::c_int = pc + 1 as libc::c_int + b_0;
                if dest <= lastpc && dest > jmptarget {
                    jmptarget = dest;
                }
                change = 0 as libc::c_int;
            }
            _ => {
                change = (luaP_opmodes[op as usize] as libc::c_int
                    & (1 as libc::c_int) << 3 as libc::c_int
                    != 0
                    && reg == a) as libc::c_int;
            }
        }
        if change != 0 {
            setreg = filterpc(pc, jmptarget);
        }
        pc += 1;
        pc;
    }
    return setreg;
}
unsafe extern "C" fn kname(
    mut p: *const Proto,
    mut index: libc::c_int,
    mut name: *mut *const libc::c_char,
) -> *const libc::c_char {
    let mut kvalue: *mut TValue = &mut *((*p).k).offset(index as isize) as *mut TValue;
    if (*kvalue).tt_ as libc::c_int & 0xf as libc::c_int == 4 as libc::c_int {
        *name = ((*((*kvalue).value_.gc as *mut GCUnion)).ts.contents).as_mut_ptr();
        return b"constant\0" as *const u8 as *const libc::c_char;
    } else {
        *name = b"?\0" as *const u8 as *const libc::c_char;
        return 0 as *const libc::c_char;
    };
}
unsafe extern "C" fn basicgetobjname(
    mut p: *const Proto,
    mut ppc: *mut libc::c_int,
    mut reg: libc::c_int,
    mut name: *mut *const libc::c_char,
) -> *const libc::c_char {
    let mut pc: libc::c_int = *ppc;
    *name = luaF_getlocalname(p, reg + 1 as libc::c_int, pc);
    if !(*name).is_null() {
        return b"local\0" as *const u8 as *const libc::c_char;
    }
    pc = findsetreg(p, pc, reg);
    *ppc = pc;
    if pc != -(1 as libc::c_int) {
        let mut i: u32 = *((*p).code).offset(pc as isize);
        let mut op: OpCode = (i >> 0 as libc::c_int
            & !(!(0 as libc::c_int as u32) << 7 as libc::c_int) << 0 as libc::c_int)
            as OpCode;
        match op as libc::c_uint {
            0 => {
                let mut b: libc::c_int = (i
                    >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int
                    & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                    as libc::c_int;
                if b < (i >> 0 as libc::c_int + 7 as libc::c_int
                    & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                    as libc::c_int
                {
                    return basicgetobjname(p, ppc, b, name);
                }
            }
            9 => {
                *name = upvalname(
                    p,
                    (i >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int
                        & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                        as libc::c_int,
                );
                return b"upvalue\0" as *const u8 as *const libc::c_char;
            }
            3 => {
                return kname(
                    p,
                    (i >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int
                        & !(!(0 as libc::c_int as u32)
                            << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int)
                            << 0 as libc::c_int) as libc::c_int,
                    name,
                );
            }
            4 => {
                return kname(
                    p,
                    (*((*p).code).offset((pc + 1 as libc::c_int) as isize)
                        >> 0 as libc::c_int + 7 as libc::c_int
                        & !(!(0 as libc::c_int as u32)
                            << 8 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int)
                            << 0 as libc::c_int) as libc::c_int,
                    name,
                );
            }
            _ => {}
        }
    }
    return 0 as *const libc::c_char;
}
unsafe extern "C" fn rname(
    mut p: *const Proto,
    mut pc: libc::c_int,
    mut c: libc::c_int,
    mut name: *mut *const libc::c_char,
) {
    let mut what: *const libc::c_char = basicgetobjname(p, &mut pc, c, name);
    if !(!what.is_null() && *what as libc::c_int == 'c' as i32) {
        *name = b"?\0" as *const u8 as *const libc::c_char;
    }
}
unsafe extern "C" fn rkname(
    mut p: *const Proto,
    mut pc: libc::c_int,
    mut i: u32,
    mut name: *mut *const libc::c_char,
) {
    let mut c: libc::c_int = (i
        >> 0 as libc::c_int
            + 7 as libc::c_int
            + 8 as libc::c_int
            + 1 as libc::c_int
            + 8 as libc::c_int
        & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
        as libc::c_int;
    if (i >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int
        & !(!(0 as libc::c_int as u32) << 1 as libc::c_int) << 0 as libc::c_int)
        as libc::c_int
        != 0
    {
        kname(p, c, name);
    } else {
        rname(p, pc, c, name);
    };
}
unsafe extern "C" fn isEnv(
    mut p: *const Proto,
    mut pc: libc::c_int,
    mut i: u32,
    mut isup: libc::c_int,
) -> *const libc::c_char {
    let mut t: libc::c_int = (i
        >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int
        & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
        as libc::c_int;
    let mut name: *const libc::c_char = 0 as *const libc::c_char;
    if isup != 0 {
        name = upvalname(p, t);
    } else {
        basicgetobjname(p, &mut pc, t, &mut name);
    }
    return if !name.is_null()
        && strcmp(name, b"_ENV\0" as *const u8 as *const libc::c_char) == 0 as libc::c_int
    {
        b"global\0" as *const u8 as *const libc::c_char
    } else {
        b"field\0" as *const u8 as *const libc::c_char
    };
}
unsafe extern "C" fn getobjname(
    mut p: *const Proto,
    mut lastpc: libc::c_int,
    mut reg: libc::c_int,
    mut name: *mut *const libc::c_char,
) -> *const libc::c_char {
    let mut kind: *const libc::c_char = basicgetobjname(p, &mut lastpc, reg, name);
    if !kind.is_null() {
        return kind;
    } else if lastpc != -(1 as libc::c_int) {
        let mut i: u32 = *((*p).code).offset(lastpc as isize);
        let mut op: OpCode = (i >> 0 as libc::c_int
            & !(!(0 as libc::c_int as u32) << 7 as libc::c_int) << 0 as libc::c_int)
            as OpCode;
        match op as libc::c_uint {
            11 => {
                let mut k: libc::c_int = (i
                    >> 0 as libc::c_int
                        + 7 as libc::c_int
                        + 8 as libc::c_int
                        + 1 as libc::c_int
                        + 8 as libc::c_int
                    & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                    as libc::c_int;
                kname(p, k, name);
                return isEnv(p, lastpc, i, 1 as libc::c_int);
            }
            12 => {
                let mut k_0: libc::c_int = (i
                    >> 0 as libc::c_int
                        + 7 as libc::c_int
                        + 8 as libc::c_int
                        + 1 as libc::c_int
                        + 8 as libc::c_int
                    & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                    as libc::c_int;
                rname(p, lastpc, k_0, name);
                return isEnv(p, lastpc, i, 0 as libc::c_int);
            }
            13 => {
                *name = b"integer index\0" as *const u8 as *const libc::c_char;
                return b"field\0" as *const u8 as *const libc::c_char;
            }
            14 => {
                let mut k_1: libc::c_int = (i
                    >> 0 as libc::c_int
                        + 7 as libc::c_int
                        + 8 as libc::c_int
                        + 1 as libc::c_int
                        + 8 as libc::c_int
                    & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                    as libc::c_int;
                kname(p, k_1, name);
                return isEnv(p, lastpc, i, 0 as libc::c_int);
            }
            20 => {
                rkname(p, lastpc, i, name);
                return b"method\0" as *const u8 as *const libc::c_char;
            }
            _ => {}
        }
    }
    return 0 as *const libc::c_char;
}
unsafe extern "C" fn funcnamefromcode(
    mut L: *mut lua_State,
    mut p: *const Proto,
    mut pc: libc::c_int,
    mut name: *mut *const libc::c_char,
) -> *const libc::c_char {
    let mut tm: TMS = TM_INDEX;
    let mut i: u32 = *((*p).code).offset(pc as isize);
    match (i >> 0 as libc::c_int
        & !(!(0 as libc::c_int as u32) << 7 as libc::c_int) << 0 as libc::c_int) as OpCode
        as libc::c_uint
    {
        68 | 69 => {
            return getobjname(
                p,
                pc,
                (i >> 0 as libc::c_int + 7 as libc::c_int
                    & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                    as libc::c_int,
                name,
            );
        }
        76 => {
            *name = b"for iterator\0" as *const u8 as *const libc::c_char;
            return b"for iterator\0" as *const u8 as *const libc::c_char;
        }
        20 | 11 | 12 | 13 | 14 => {
            tm = TM_INDEX;
        }
        15 | 16 | 17 | 18 => {
            tm = TM_NEWINDEX;
        }
        46 | 47 | 48 => {
            tm = (i
                >> 0 as libc::c_int
                    + 7 as libc::c_int
                    + 8 as libc::c_int
                    + 1 as libc::c_int
                    + 8 as libc::c_int
                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                as libc::c_int as TMS;
        }
        49 => {
            tm = TM_UNM;
        }
        50 => {
            tm = TM_BNOT;
        }
        52 => {
            tm = TM_LEN;
        }
        53 => {
            tm = TM_CONCAT;
        }
        57 => {
            tm = TM_EQ;
        }
        58 | 62 | 64 => {
            tm = TM_LT;
        }
        59 | 63 | 65 => {
            tm = TM_LE;
        }
        54 | 70 => {
            tm = TM_CLOSE;
        }
        _ => return 0 as *const libc::c_char,
    }
    *name = ((*(*(*L).l_G).tmname[tm as usize]).contents)
        .as_mut_ptr()
        .offset(2 as libc::c_int as isize);
    return b"metamethod\0" as *const u8 as *const libc::c_char;
}
unsafe extern "C" fn funcnamefromcall(
    mut L: *mut lua_State,
    mut ci: *mut CallInfo,
    mut name: *mut *const libc::c_char,
) -> *const libc::c_char {
    if (*ci).callstatus as libc::c_int & (1 as libc::c_int) << 3 as libc::c_int != 0 {
        *name = b"?\0" as *const u8 as *const libc::c_char;
        return b"hook\0" as *const u8 as *const libc::c_char;
    } else if (*ci).callstatus as libc::c_int & (1 as libc::c_int) << 7 as libc::c_int != 0 {
        *name = b"__gc\0" as *const u8 as *const libc::c_char;
        return b"metamethod\0" as *const u8 as *const libc::c_char;
    } else if (*ci).callstatus as libc::c_int & (1 as libc::c_int) << 1 as libc::c_int == 0 {
        return funcnamefromcode(
            L,
            (*((*(*ci).func.p).val.value_.gc as *mut GCUnion)).cl.l.p,
            currentpc(ci),
            name,
        );
    } else {
        return 0 as *const libc::c_char;
    };
}
unsafe extern "C" fn instack(mut ci: *mut CallInfo, mut o: *const TValue) -> libc::c_int {
    let mut pos: libc::c_int = 0;
    let mut base: StkId = ((*ci).func.p).offset(1 as libc::c_int as isize);
    pos = 0 as libc::c_int;
    while base.offset(pos as isize) < (*ci).top.p {
        if o == &mut (*base.offset(pos as isize)).val as *mut TValue as *const TValue {
            return pos;
        }
        pos += 1;
        pos;
    }
    return -(1 as libc::c_int);
}
unsafe extern "C" fn getupvalname(
    mut ci: *mut CallInfo,
    mut o: *const TValue,
    mut name: *mut *const libc::c_char,
) -> *const libc::c_char {
    let mut c: *mut LClosure = &mut (*((*(*ci).func.p).val.value_.gc as *mut GCUnion)).cl.l;
    let mut i: libc::c_int = 0;
    i = 0 as libc::c_int;
    while i < (*c).nupvalues as libc::c_int {
        if (**((*c).upvals).as_mut_ptr().offset(i as isize)).v.p == o as *mut TValue {
            *name = upvalname((*c).p, i);
            return b"upvalue\0" as *const u8 as *const libc::c_char;
        }
        i += 1;
        i;
    }
    return 0 as *const libc::c_char;
}

unsafe fn formatvarinfo(
    mut L: *mut lua_State,
    mut kind: *const libc::c_char,
    mut name: *const libc::c_char,
) -> Cow<'static, str> {
    if kind.is_null() {
        "".into()
    } else {
        format!(
            " ({} '{}')",
            CStr::from_ptr(kind).to_string_lossy(),
            CStr::from_ptr(name).to_string_lossy()
        )
        .into()
    }
}

unsafe fn varinfo(mut L: *mut lua_State, mut o: *const TValue) -> Cow<'static, str> {
    let mut ci: *mut CallInfo = (*L).ci;
    let mut name: *const libc::c_char = 0 as *const libc::c_char;
    let mut kind: *const libc::c_char = 0 as *const libc::c_char;
    if (*ci).callstatus as libc::c_int & (1 as libc::c_int) << 1 as libc::c_int == 0 {
        kind = getupvalname(ci, o, &mut name);
        if kind.is_null() {
            let mut reg: libc::c_int = instack(ci, o);
            if reg >= 0 as libc::c_int {
                kind = getobjname(
                    (*((*(*ci).func.p).val.value_.gc as *mut GCUnion)).cl.l.p,
                    currentpc(ci),
                    reg,
                    &mut name,
                );
            }
        }
    }

    formatvarinfo(L, kind, name)
}

unsafe fn typeerror(
    L: *mut lua_State,
    o: *const TValue,
    op: impl Display,
    extra: impl Display,
) -> Result<(), Box<dyn std::error::Error>> {
    let t = luaT_objtypename(L, o)?;

    luaG_runerror(L, format_args!("attempt to {op} a {t} value{extra}"))
}

pub unsafe fn luaG_typeerror(
    L: *mut lua_State,
    o: *const TValue,
    op: impl Display,
) -> Result<(), Box<dyn std::error::Error>> {
    typeerror(L, o, op, varinfo(L, o))
}

pub unsafe fn luaG_callerror(
    mut L: *mut lua_State,
    mut o: *const TValue,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut ci: *mut CallInfo = (*L).ci;
    let mut name: *const libc::c_char = 0 as *const libc::c_char;
    let mut kind: *const libc::c_char = funcnamefromcall(L, ci, &mut name);
    let extra = if !kind.is_null() {
        formatvarinfo(L, kind, name)
    } else {
        varinfo(L, o)
    };

    typeerror(L, o, "call", extra)
}

pub unsafe fn luaG_forerror(
    L: *mut lua_State,
    o: *const TValue,
    what: impl Display,
) -> Result<(), Box<dyn std::error::Error>> {
    luaG_runerror(
        L,
        format_args!(
            "bad 'for' {} (number expected, got {})",
            what,
            luaT_objtypename(L, o)?
        ),
    )
}

pub unsafe fn luaG_concaterror(
    L: *mut lua_State,
    mut p1: *const TValue,
    p2: *const TValue,
) -> Result<(), Box<dyn std::error::Error>> {
    if (*p1).tt_ as libc::c_int & 0xf as libc::c_int == 4 as libc::c_int
        || (*p1).tt_ as libc::c_int & 0xf as libc::c_int == 3 as libc::c_int
    {
        p1 = p2;
    }

    luaG_typeerror(L, p1, "concatenate")
}

pub unsafe fn luaG_opinterror(
    mut L: *mut lua_State,
    mut p1: *const TValue,
    mut p2: *const TValue,
    msg: impl Display,
) -> Result<(), Box<dyn std::error::Error>> {
    if !((*p1).tt_ as libc::c_int & 0xf as libc::c_int == 3 as libc::c_int) {
        p2 = p1;
    }

    luaG_typeerror(L, p2, msg)
}

pub unsafe fn luaG_tointerror(
    L: *mut lua_State,
    p1: *const TValue,
    mut p2: *const TValue,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut temp: i64 = 0;

    if luaV_tointegerns(p1, &mut temp, F2Ieq) == 0 {
        p2 = p1;
    }

    luaG_runerror(
        L,
        format_args!("number{} has no integer representation", varinfo(L, p2)),
    )
}

pub unsafe fn luaG_ordererror(
    L: *mut lua_State,
    p1: *const TValue,
    p2: *const TValue,
) -> Result<(), Box<dyn std::error::Error>> {
    let t1 = luaT_objtypename(L, p1)?;
    let t2 = luaT_objtypename(L, p2)?;

    if t1 == t2 {
        luaG_runerror(L, format_args!("attempt to compare two {t1} values"))
    } else {
        luaG_runerror(L, format_args!("attempt to compare {t1} with {t2}"))
    }
}

pub unsafe fn luaG_addinfo(
    mut L: *mut lua_State,
    msg: impl Display,
    mut src: *mut TString,
    line: libc::c_int,
) -> String {
    let mut buff: [libc::c_char; 60] = [0; 60];

    if !src.is_null() {
        luaO_chunkid(
            buff.as_mut_ptr(),
            ((*src).contents).as_mut_ptr(),
            if (*src).shrlen as libc::c_int != 0xff as libc::c_int {
                (*src).shrlen as usize
            } else {
                (*src).u.lnglen
            },
        );
    } else {
        buff[0 as libc::c_int as usize] = '?' as i32 as libc::c_char;
        buff[1 as libc::c_int as usize] = '\0' as i32 as libc::c_char;
    }

    format!(
        "{}:{}: {}",
        CStr::from_ptr(buff.as_ptr()).to_string_lossy(),
        line,
        msg,
    )
}

pub unsafe fn luaG_runerror(
    mut L: *mut lua_State,
    fmt: impl Display,
) -> Result<(), Box<dyn std::error::Error>> {
    let ci = (*L).ci;
    let msg = if (*ci).callstatus as libc::c_int & (1 as libc::c_int) << 1 as libc::c_int == 0 {
        luaG_addinfo(
            L,
            fmt,
            (*(*((*(*ci).func.p).val.value_.gc as *mut GCUnion)).cl.l.p).source,
            getcurrentline(ci),
        )
    } else {
        fmt.to_string()
    };

    Err(msg.into())
}

unsafe extern "C" fn changedline(
    mut p: *const Proto,
    mut oldpc: libc::c_int,
    mut newpc: libc::c_int,
) -> libc::c_int {
    if ((*p).lineinfo).is_null() {
        return 0 as libc::c_int;
    }
    if newpc - oldpc < 128 as libc::c_int / 2 as libc::c_int {
        let mut delta: libc::c_int = 0 as libc::c_int;
        let mut pc: libc::c_int = oldpc;
        loop {
            pc += 1;
            let mut lineinfo: libc::c_int = *((*p).lineinfo).offset(pc as isize) as libc::c_int;
            if lineinfo == -(0x80 as libc::c_int) {
                break;
            }
            delta += lineinfo;
            if pc == newpc {
                return (delta != 0 as libc::c_int) as libc::c_int;
            }
        }
    }
    return (luaG_getfuncline(p, oldpc) != luaG_getfuncline(p, newpc)) as libc::c_int;
}

pub unsafe fn luaG_tracecall(mut L: *mut lua_State) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut ci: *mut CallInfo = (*L).ci;
    let mut p: *mut Proto = (*((*(*ci).func.p).val.value_.gc as *mut GCUnion)).cl.l.p;
    ::core::ptr::write_volatile(&mut (*ci).u.trap as *mut libc::c_int, 1 as libc::c_int);
    if (*ci).u.savedpc == (*p).code as *const u32 {
        if (*p).is_vararg != 0 {
            return Ok(0 as libc::c_int);
        } else if (*ci).callstatus as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int == 0 {
            luaD_hookcall(L, ci)?;
        }
    }
    return Ok(1 as libc::c_int);
}

pub unsafe fn luaG_traceexec(
    mut L: *mut lua_State,
    mut pc: *const u32,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut ci: *mut CallInfo = (*L).ci;
    let mut mask: u8 = (*L).hookmask as u8;
    let mut p: *const Proto = (*((*(*ci).func.p).val.value_.gc as *mut GCUnion)).cl.l.p;
    let mut counthook: libc::c_int = 0;
    if mask as libc::c_int
        & ((1 as libc::c_int) << 2 as libc::c_int | (1 as libc::c_int) << 3 as libc::c_int)
        == 0
    {
        ::core::ptr::write_volatile(&mut (*ci).u.trap as *mut libc::c_int, 0 as libc::c_int);
        return Ok(0 as libc::c_int);
    }
    pc = pc.offset(1);
    pc;
    (*ci).u.savedpc = pc;
    counthook = (mask as libc::c_int & (1 as libc::c_int) << 3 as libc::c_int != 0 && {
        (*L).hookcount -= 1;
        (*L).hookcount == 0 as libc::c_int
    }) as libc::c_int;
    if counthook != 0 {
        (*L).hookcount = (*L).basehookcount;
    } else if mask as libc::c_int & (1 as libc::c_int) << 2 as libc::c_int == 0 {
        return Ok(1 as libc::c_int);
    }
    if (*ci).callstatus as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0 {
        (*ci).callstatus = ((*ci).callstatus as libc::c_int
            & !((1 as libc::c_int) << 6 as libc::c_int))
            as libc::c_ushort;
        return Ok(1 as libc::c_int);
    }
    if !(luaP_opmodes[(*((*ci).u.savedpc).offset(-(1 as libc::c_int as isize)) >> 0 as libc::c_int
        & !(!(0 as libc::c_int as u32) << 7 as libc::c_int) << 0 as libc::c_int)
        as OpCode as usize] as libc::c_int
        & (1 as libc::c_int) << 5 as libc::c_int
        != 0
        && (*((*ci).u.savedpc).offset(-(1 as libc::c_int as isize))
            >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int
            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
            as libc::c_int
            == 0 as libc::c_int)
    {
        (*L).top.p = (*ci).top.p;
    }
    if counthook != 0 {
        luaD_hook(
            L,
            3 as libc::c_int,
            -(1 as libc::c_int),
            0 as libc::c_int,
            0 as libc::c_int,
        )?;
    }
    if mask as libc::c_int & (1 as libc::c_int) << 2 as libc::c_int != 0 {
        let mut oldpc: libc::c_int = if (*L).oldpc < (*p).sizecode {
            (*L).oldpc
        } else {
            0 as libc::c_int
        };
        let mut npci: libc::c_int =
            pc.offset_from((*p).code) as libc::c_long as libc::c_int - 1 as libc::c_int;
        if npci <= oldpc || changedline(p, oldpc, npci) != 0 {
            let mut newline: libc::c_int = luaG_getfuncline(p, npci);
            luaD_hook(
                L,
                2 as libc::c_int,
                newline,
                0 as libc::c_int,
                0 as libc::c_int,
            )?;
        }
        (*L).oldpc = npci;
    }

    Ok(1)
}
