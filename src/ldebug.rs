#![allow(
    dead_code,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments
)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::ldo::{luaD_hook, luaD_hookcall};
use crate::lfunc::luaF_getlocalname;
use crate::lobject::{CClosure, Proto, StkId};
use crate::lstate::{CallInfo, lua_Debug, lua_Hook};
use crate::ltm::{
    TM_BNOT, TM_CLOSE, TM_CONCAT, TM_EQ, TM_INDEX, TM_LE, TM_LEN, TM_LT, TM_NEWINDEX, TM_UNM, TMS,
    luaT_objtypename,
};
use crate::table::luaH_setint;
use crate::value::{UnsafeValue, UntaggedValue};
use crate::vm::{F2Ieq, OpCode, luaP_opmodes, luaV_tointegerns};
use crate::{ChunkInfo, LuaFn, Object, Str, Table, Thread, api_incr_top};
use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use core::ffi::{CStr, c_char};
use core::fmt::Display;
use core::ptr::null;
use libc::{strchr, strcmp};

type c_int = i32;
type c_uint = u32;
type c_long = i64;

unsafe fn currentpc(ci: *mut CallInfo) -> c_int {
    return ((*ci).u.savedpc)
        .offset_from((*(*(*(*ci).func).val.value_.gc.cast::<LuaFn>()).p.get()).code)
        as c_int
        - 1;
}

unsafe fn getbaseline(f: *const Proto, pc: c_int, basepc: *mut c_int) -> c_int {
    if (*f).sizeabslineinfo == 0 as c_int
        || pc < (*((*f).abslineinfo).offset(0 as c_int as isize)).pc
    {
        *basepc = -(1 as c_int);
        return (*f).linedefined;
    } else {
        let mut i: c_int = (pc as c_uint)
            .wrapping_div(128 as c_int as c_uint)
            .wrapping_sub(1 as c_int as c_uint) as c_int;
        while (i + 1 as c_int) < (*f).sizeabslineinfo
            && pc >= (*((*f).abslineinfo).offset((i + 1 as c_int) as isize)).pc
        {
            i += 1;
        }
        *basepc = (*((*f).abslineinfo).offset(i as isize)).pc;
        return (*((*f).abslineinfo).offset(i as isize)).line;
    };
}

pub unsafe fn luaG_getfuncline(f: *const Proto, pc: c_int) -> c_int {
    if ((*f).lineinfo).is_null() {
        return -(1 as c_int);
    } else {
        let mut basepc: c_int = 0;
        let mut baseline: c_int = getbaseline(f, pc, &mut basepc);
        loop {
            let fresh1 = basepc;
            basepc = basepc + 1;
            if !(fresh1 < pc) {
                break;
            }
            baseline += *((*f).lineinfo).offset(basepc as isize) as c_int;
        }
        return baseline;
    };
}

unsafe fn getcurrentline(ci: *mut CallInfo) -> c_int {
    luaG_getfuncline(
        (*(*(*ci).func).val.value_.gc.cast::<LuaFn>()).p.get(),
        currentpc(ci),
    )
}

unsafe fn settraps(mut ci: *mut CallInfo) {
    while !ci.is_null() {
        if (*ci).callstatus as c_int & (1 as c_int) << 1 as c_int == 0 {
            ::core::ptr::write_volatile(&mut (*ci).u.trap as *mut c_int, 1 as c_int);
        }
        ci = (*ci).previous;
    }
}

pub unsafe fn lua_sethook(L: *mut Thread, mut func: lua_Hook, mut mask: c_int, count: c_int) {
    if func.is_none() || mask == 0 as c_int {
        mask = 0 as c_int;
        func = None;
    }

    (*L).hook.set(func);
    (*L).basehookcount.set(count);
    (*L).hookcount.set((*L).basehookcount.get());
    (*L).hookmask.set(mask);

    if mask != 0 {
        settraps((*L).ci.get());
    }
}

pub unsafe fn lua_gethook(L: *mut Thread) -> lua_Hook {
    return (*L).hook.get();
}

pub unsafe fn lua_gethookmask(L: *mut Thread) -> c_int {
    return (*L).hookmask.get();
}

pub unsafe fn lua_gethookcount(L: *mut Thread) -> c_int {
    return (*L).basehookcount.get();
}

pub unsafe fn lua_getstack(L: *const Thread, mut level: c_int, ar: &mut lua_Debug) -> c_int {
    let mut status: c_int = 0;
    let mut ci: *mut CallInfo = 0 as *mut CallInfo;
    if level < 0 as c_int {
        return 0 as c_int;
    }
    ci = (*L).ci.get();

    while level > 0 && ci != (*L).base_ci.get() {
        level -= 1;
        ci = (*ci).previous;
    }

    if level == 0 && ci != (*L).base_ci.get() {
        status = 1 as c_int;
        (*ar).i_ci = ci;
    } else {
        status = 0 as c_int;
    }

    return status;
}

unsafe fn upvalname(p: *const Proto, uv: usize) -> *const c_char {
    let s = (*((*p).upvalues).add(uv)).name;

    if s.is_null() {
        return b"?\0" as *const u8 as *const c_char;
    } else {
        return ((*s).contents).as_ptr();
    };
}

unsafe fn findvararg(ci: *mut CallInfo, n: c_int, pos: *mut StkId) -> *const c_char {
    if (*(*(*(*ci).func).val.value_.gc.cast::<LuaFn>()).p.get()).is_vararg != 0 {
        let nextra: c_int = (*ci).u.nextraargs;
        if n >= -nextra {
            *pos = ((*ci).func)
                .offset(-(nextra as isize))
                .offset(-((n + 1 as c_int) as isize));
            return b"(vararg)\0" as *const u8 as *const c_char;
        }
    }
    return 0 as *const c_char;
}

pub unsafe fn luaG_findlocal(
    L: *const Thread,
    ci: *mut CallInfo,
    n: c_int,
    pos: *mut StkId,
) -> *const c_char {
    let base: StkId = ((*ci).func).offset(1 as c_int as isize);
    let mut name: *const c_char = 0 as *const c_char;
    if (*ci).callstatus as c_int & (1 as c_int) << 1 as c_int == 0 {
        if n < 0 as c_int {
            return findvararg(ci, n, pos);
        } else {
            name = luaF_getlocalname(
                (*(*(*ci).func).val.value_.gc.cast::<LuaFn>()).p.get(),
                n,
                currentpc(ci),
            );
        }
    }
    if name.is_null() {
        let limit: StkId = if ci == (*L).ci.get() {
            (*L).top.get()
        } else {
            (*(*ci).next).func
        };
        if limit.offset_from(base) as c_long >= n as c_long && n > 0 as c_int {
            name = if (*ci).callstatus as c_int & (1 as c_int) << 1 as c_int == 0 {
                b"(temporary)\0" as *const u8 as *const c_char
            } else {
                b"(C temporary)\0" as *const u8 as *const c_char
            };
        } else {
            return 0 as *const c_char;
        }
    }
    if !pos.is_null() {
        *pos = base.offset((n - 1 as c_int) as isize);
    }
    return name;
}

pub unsafe fn lua_getlocal(L: *mut Thread, ar: *const lua_Debug, n: c_int) -> *const c_char {
    let mut name: *const c_char = 0 as *const c_char;
    if ar.is_null() {
        if !((*((*L).top.get()).offset(-(1 as c_int as isize))).val.tt_ as c_int
            == 6 as c_int | (0 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int)
        {
            name = 0 as *const c_char;
        } else {
            name = luaF_getlocalname(
                (*(*((*L).top.get()).offset(-1)).val.value_.gc.cast::<LuaFn>())
                    .p
                    .get(),
                n,
                0 as c_int,
            );
        }
    } else {
        let mut pos: StkId = 0 as StkId;
        name = luaG_findlocal(L, (*ar).i_ci, n, &mut pos);
        if !name.is_null() {
            let io1: *mut UnsafeValue = &raw mut (*(*L).top.get()).val;
            let io2: *const UnsafeValue = &raw mut (*pos).val;
            (*io1).value_ = (*io2).value_;
            (*io1).tt_ = (*io2).tt_;
            api_incr_top(L);
        }
    }
    return name;
}

pub unsafe fn lua_setlocal(L: *mut Thread, ar: *const lua_Debug, n: c_int) -> *const c_char {
    let mut pos: StkId = 0 as StkId;
    let mut name: *const c_char = 0 as *const c_char;
    name = luaG_findlocal(L, (*ar).i_ci, n, &mut pos);
    if !name.is_null() {
        let io1: *mut UnsafeValue = &raw mut (*pos).val;
        let io2: *const UnsafeValue = &raw mut (*((*L).top.get()).offset(-1)).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        (*L).top.sub(1);
    }
    return name;
}

unsafe fn funcinfo(ar: &mut lua_Debug, cl: *const Object) {
    if !(!cl.is_null() && (*cl).tt as c_int == 6 as c_int | (0 as c_int) << 4) {
        (*ar).source = None;
        (*ar).linedefined = -(1 as c_int);
        (*ar).lastlinedefined = -(1 as c_int);
        (*ar).what = b"C\0" as *const u8 as *const c_char;
    } else {
        let p: *const Proto = (*cl.cast::<LuaFn>()).p.get();

        (*ar).source = Some((*p).chunk.clone());
        (*ar).linedefined = (*p).linedefined;
        (*ar).lastlinedefined = (*p).lastlinedefined;
        (*ar).what = if (*ar).linedefined == 0 as c_int {
            b"main\0" as *const u8 as *const c_char
        } else {
            b"Lua\0" as *const u8 as *const c_char
        };
    }
}

unsafe fn nextline(p: *const Proto, currentline: c_int, pc: c_int) -> c_int {
    if *((*p).lineinfo).offset(pc as isize) as c_int != -(0x80 as c_int) {
        return currentline + *((*p).lineinfo).offset(pc as isize) as c_int;
    } else {
        return luaG_getfuncline(p, pc);
    };
}

unsafe fn collectvalidlines(L: *const Thread, f: *const Object) {
    if !(!f.is_null() && (*f).tt as c_int == 6 as c_int | (0 as c_int) << 4) {
        (*(*L).top.get()).val.tt_ = (0 as c_int | (0 as c_int) << 4) as u8;
        api_incr_top(L);
    } else {
        let p: *const Proto = (*f.cast::<LuaFn>()).p.get();
        let mut currentline: c_int = (*p).linedefined;
        let t = Table::new((*L).hdr.global);
        let io: *mut UnsafeValue = &raw mut (*(*L).top.get()).val;

        (*io).value_.gc = t.cast();
        (*io).tt_ = (5 as c_int | (0 as c_int) << 4 as c_int | 1 << 6) as u8;

        api_incr_top(L);

        if !((*p).lineinfo).is_null() {
            let mut i: c_int = 0;
            let mut v: UnsafeValue = UnsafeValue {
                value_: UntaggedValue {
                    gc: 0 as *mut Object,
                },
                tt_: 0,
            };
            v.tt_ = (1 as c_int | (1 as c_int) << 4 as c_int) as u8;
            if (*p).is_vararg == 0 {
                i = 0 as c_int;
            } else {
                currentline = nextline(p, currentline, 0 as c_int);
                i = 1 as c_int;
            }
            while i < (*p).sizelineinfo {
                currentline = nextline(p, currentline, i);
                luaH_setint(t, currentline as i64, &raw const v);
                i += 1;
            }
        }
    }
}

unsafe fn getfuncname(
    L: *const Thread,
    ci: *mut CallInfo,
    name: *mut *const c_char,
) -> *const c_char {
    if !ci.is_null() && (*ci).callstatus as c_int & (1 as c_int) << 5 as c_int == 0 {
        return funcnamefromcall(L, (*ci).previous, name);
    } else {
        return 0 as *const c_char;
    };
}

unsafe fn auxgetinfo(
    L: *const Thread,
    mut what: *const c_char,
    ar: &mut lua_Debug,
    f: *const Object,
    ci: *mut CallInfo,
) -> c_int {
    let mut status: c_int = 1 as c_int;
    while *what != 0 {
        match *what as u8 {
            b'S' => funcinfo(ar, f),
            b'l' => {
                (*ar).currentline = if !ci.is_null()
                    && (*ci).callstatus as c_int & (1 as c_int) << 1 as c_int == 0
                {
                    getcurrentline(ci)
                } else {
                    -(1 as c_int)
                };
            }
            117 => {
                (*ar).nups = (if f.is_null() {
                    0 as c_int
                } else {
                    (*(f as *mut CClosure)).nupvalues as c_int
                }) as libc::c_uchar;
                if !(!f.is_null() && (*f).tt as c_int == 6 as c_int | (0 as c_int) << 4 as c_int) {
                    (*ar).isvararg = 1 as c_int as c_char;
                    (*ar).nparams = 0 as c_int as libc::c_uchar;
                } else {
                    (*ar).isvararg = (*(*f.cast::<LuaFn>()).p.get()).is_vararg as c_char;
                    (*ar).nparams = (*(*f.cast::<LuaFn>()).p.get()).numparams;
                }
            }
            116 => {
                (*ar).istailcall = (if !ci.is_null() {
                    (*ci).callstatus as c_int & (1 as c_int) << 5 as c_int
                } else {
                    0 as c_int
                }) as c_char;
            }
            110 => {
                (*ar).namewhat = getfuncname(L, ci, &mut (*ar).name);
                if ((*ar).namewhat).is_null() {
                    (*ar).namewhat = b"\0" as *const u8 as *const c_char;
                    (*ar).name = 0 as *const c_char;
                }
            }
            114 => {
                if ci.is_null() || (*ci).callstatus as c_int & (1 as c_int) << 8 == 0 {
                    (*ar).ntransfer = 0;
                    (*ar).ftransfer = (*ar).ntransfer;
                } else {
                    (*ar).ftransfer = (*ci).u2.transferinfo.ftransfer.into();
                    (*ar).ntransfer = (*ci).u2.transferinfo.ntransfer;
                }
            }
            76 | 102 => {}
            _ => {
                status = 0 as c_int;
            }
        }
        what = what.offset(1);
    }
    return status;
}

pub unsafe fn lua_getinfo(L: *const Thread, mut what: *const c_char, ar: &mut lua_Debug) -> c_int {
    let mut status: c_int = 0;
    let mut ci: *mut CallInfo = 0 as *mut CallInfo;
    let mut func: *mut UnsafeValue = 0 as *mut UnsafeValue;

    if *what as c_int == '>' as i32 {
        ci = 0 as *mut CallInfo;
        func = &raw mut (*((*L).top.get()).offset(-(1 as c_int as isize))).val;
        what = what.offset(1);
        (*L).top.sub(1);
    } else {
        ci = (*ar).i_ci;
        func = &raw mut (*(*ci).func).val;
    }

    let cl = if (*func).tt_ as c_int
        == 6 as c_int | (0 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int
        || (*func).tt_ as c_int
            == 6 as c_int | (2 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int
    {
        (*func).value_.gc
    } else {
        null()
    };
    status = auxgetinfo(L, what, ar, cl, ci);
    if !(strchr(what, 'f' as i32)).is_null() {
        let io1: *mut UnsafeValue = &raw mut (*(*L).top.get()).val;
        let io2: *const UnsafeValue = func;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        api_incr_top(L);
    }
    if !(strchr(what, 'L' as i32)).is_null() {
        collectvalidlines(L, cl);
    }

    status
}

unsafe fn filterpc(pc: c_int, jmptarget: c_int) -> c_int {
    if pc < jmptarget {
        return -(1 as c_int);
    } else {
        return pc;
    };
}

unsafe fn findsetreg(p: *const Proto, mut lastpc: c_int, reg: c_int) -> c_int {
    let mut pc: c_int = 0;
    let mut setreg: c_int = -(1 as c_int);
    let mut jmptarget: c_int = 0 as c_int;
    if luaP_opmodes[(*((*p).code).offset(lastpc as isize) >> 0 as c_int
        & !(!(0 as c_int as u32) << 7 as c_int) << 0 as c_int) as OpCode
        as usize] as c_int
        & (1 as c_int) << 7 as c_int
        != 0
    {
        lastpc -= 1;
    }
    pc = 0 as c_int;
    while pc < lastpc {
        let i: u32 = *((*p).code).offset(pc as isize);
        let op: OpCode =
            (i >> 0 as c_int & !(!(0 as c_int as u32) << 7 as c_int) << 0 as c_int) as OpCode;
        let a: c_int = (i >> 0 as c_int + 7 as c_int
            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int) as c_int;
        let mut change: c_int = 0;
        match op as c_uint {
            8 => {
                let b: c_int = (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                    as c_int;
                change = (a <= reg && reg <= a + b) as c_int;
            }
            76 => {
                change = (reg >= a + 2 as c_int) as c_int;
            }
            68 | 69 => {
                change = (reg >= a) as c_int;
            }
            56 => {
                let b_0: c_int = (i >> 0 as c_int + 7 as c_int
                    & !(!(0 as c_int as u32) << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                        << 0 as c_int) as c_int
                    - (((1 as c_int) << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                        - 1 as c_int
                        >> 1 as c_int);
                let dest: c_int = pc + 1 as c_int + b_0;
                if dest <= lastpc && dest > jmptarget {
                    jmptarget = dest;
                }
                change = 0 as c_int;
            }
            _ => {
                change = (luaP_opmodes[op as usize] as c_int & (1 as c_int) << 3 as c_int != 0
                    && reg == a) as c_int;
            }
        }
        if change != 0 {
            setreg = filterpc(pc, jmptarget);
        }
        pc += 1;
    }
    return setreg;
}

unsafe fn kname(p: *const Proto, index: c_int, name: *mut *const c_char) -> *const c_char {
    let kvalue: *mut UnsafeValue = &mut *((*p).k).offset(index as isize) as *mut UnsafeValue;
    if (*kvalue).tt_ as c_int & 0xf as c_int == 4 as c_int {
        *name = ((*((*kvalue).value_.gc as *mut Str)).contents).as_mut_ptr();
        return b"constant\0" as *const u8 as *const c_char;
    } else {
        *name = b"?\0" as *const u8 as *const c_char;
        return 0 as *const c_char;
    };
}

unsafe fn basicgetobjname(
    p: *const Proto,
    ppc: *mut c_int,
    reg: c_int,
    name: *mut *const c_char,
) -> *const c_char {
    let mut pc: c_int = *ppc;
    *name = luaF_getlocalname(p, reg + 1 as c_int, pc);
    if !(*name).is_null() {
        return b"local\0" as *const u8 as *const c_char;
    }
    pc = findsetreg(p, pc, reg);
    *ppc = pc;
    if pc != -(1 as c_int) {
        let i: u32 = *((*p).code).offset(pc as isize);
        let op: OpCode =
            (i >> 0 as c_int & !(!(0 as c_int as u32) << 7 as c_int) << 0 as c_int) as OpCode;
        match op as c_uint {
            0 => {
                let b: c_int = (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                    as c_int;
                if b < (i >> 0 as c_int + 7 as c_int
                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                    as c_int
                {
                    return basicgetobjname(p, ppc, b, name);
                }
            }
            9 => {
                *name = upvalname(
                    p,
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        .try_into()
                        .unwrap(),
                );
                return b"upvalue\0" as *const u8 as *const c_char;
            }
            3 => {
                return kname(
                    p,
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int + 8 as c_int + 1 as c_int)
                            << 0 as c_int) as c_int,
                    name,
                );
            }
            4 => {
                return kname(
                    p,
                    (*((*p).code).offset((pc + 1 as c_int) as isize) >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32)
                            << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                            << 0 as c_int) as c_int,
                    name,
                );
            }
            _ => {}
        }
    }
    return 0 as *const c_char;
}

unsafe fn rname(p: *const Proto, mut pc: c_int, c: c_int, name: *mut *const c_char) {
    let what: *const c_char = basicgetobjname(p, &mut pc, c, name);
    if !(!what.is_null() && *what as c_int == 'c' as i32) {
        *name = b"?\0" as *const u8 as *const c_char;
    }
}

unsafe fn rkname(p: *const Proto, pc: c_int, i: u32, name: *mut *const c_char) {
    let c: c_int = (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int) as c_int;
    if (i >> 0 as c_int + 7 as c_int + 8 as c_int
        & !(!(0 as c_int as u32) << 1 as c_int) << 0 as c_int) as c_int
        != 0
    {
        kname(p, c, name);
    } else {
        rname(p, pc, c, name);
    };
}

unsafe fn isEnv(p: *const Proto, mut pc: c_int, i: u32, isup: c_int) -> *const c_char {
    let t: c_int = (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int) as c_int;
    let mut name: *const c_char = 0 as *const c_char;

    if isup != 0 {
        name = upvalname(p, t.try_into().unwrap());
    } else {
        let what = basicgetobjname(p, &raw mut pc, t, &raw mut name);

        if !what.is_null()
            && strcmp(what, c"local".as_ptr()) != 0
            && strcmp(what, c"upvalue".as_ptr()) != 0
        {
            name = null();
        }
    }

    return if !name.is_null() && strcmp(name, b"_ENV\0" as *const u8 as *const c_char) == 0 as c_int
    {
        b"global\0" as *const u8 as *const c_char
    } else {
        b"field\0" as *const u8 as *const c_char
    };
}
unsafe fn getobjname(
    p: *const Proto,
    mut lastpc: c_int,
    reg: c_int,
    name: *mut *const c_char,
) -> *const c_char {
    let kind: *const c_char = basicgetobjname(p, &mut lastpc, reg, name);
    if !kind.is_null() {
        return kind;
    } else if lastpc != -(1 as c_int) {
        let i: u32 = *((*p).code).offset(lastpc as isize);
        let op: OpCode =
            (i >> 0 as c_int & !(!(0 as c_int as u32) << 7 as c_int) << 0 as c_int) as OpCode;
        match op as c_uint {
            11 => {
                let k: c_int = (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                    as c_int;
                kname(p, k, name);
                return isEnv(p, lastpc, i, 1 as c_int);
            }
            12 => {
                let k_0: c_int = (i
                    >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                    as c_int;
                rname(p, lastpc, k_0, name);
                return isEnv(p, lastpc, i, 0 as c_int);
            }
            13 => {
                *name = b"integer index\0" as *const u8 as *const c_char;
                return b"field\0" as *const u8 as *const c_char;
            }
            14 => {
                let k_1: c_int = (i
                    >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                    as c_int;
                kname(p, k_1, name);
                return isEnv(p, lastpc, i, 0 as c_int);
            }
            20 => {
                rkname(p, lastpc, i, name);
                return b"method\0" as *const u8 as *const c_char;
            }
            _ => {}
        }
    }
    return 0 as *const c_char;
}

unsafe fn funcnamefromcode(
    L: *const Thread,
    p: *const Proto,
    pc: c_int,
    name: *mut *const c_char,
) -> *const c_char {
    let mut tm: TMS = TM_INDEX;
    let i: u32 = *((*p).code).offset(pc as isize);
    match (i >> 0 as c_int & !(!(0 as c_int as u32) << 7 as c_int) << 0 as c_int) as OpCode
        as c_uint
    {
        68 | 69 => {
            return getobjname(
                p,
                pc,
                (i >> 0 as c_int + 7 as c_int & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                    as c_int,
                name,
            );
        }
        76 => {
            *name = b"for iterator\0" as *const u8 as *const c_char;
            return b"for iterator\0" as *const u8 as *const c_char;
        }
        20 | 11 | 12 | 13 | 14 => {
            tm = TM_INDEX;
        }
        15 | 16 | 17 | 18 => {
            tm = TM_NEWINDEX;
        }
        46 | 47 | 48 => {
            tm = (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int) as c_int
                as TMS;
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
        _ => return 0 as *const c_char,
    }

    *name = ((*(*(*L).hdr.global).tmname[tm as usize].get()).contents)
        .as_ptr()
        .offset(2 as c_int as isize);

    return b"metamethod\0" as *const u8 as *const c_char;
}

unsafe fn funcnamefromcall(
    L: *const Thread,
    ci: *mut CallInfo,
    name: *mut *const c_char,
) -> *const c_char {
    if (*ci).callstatus as c_int & (1 as c_int) << 3 as c_int != 0 {
        *name = b"?\0" as *const u8 as *const c_char;
        return b"hook\0" as *const u8 as *const c_char;
    } else if (*ci).callstatus as c_int & (1 as c_int) << 7 as c_int != 0 {
        *name = b"__gc\0" as *const u8 as *const c_char;
        return b"metamethod\0" as *const u8 as *const c_char;
    } else if (*ci).callstatus as c_int & (1 as c_int) << 1 as c_int == 0 {
        return funcnamefromcode(
            L,
            (*(*(*ci).func).val.value_.gc.cast::<LuaFn>()).p.get(),
            currentpc(ci),
            name,
        );
    } else {
        return 0 as *const c_char;
    };
}

unsafe fn instack(ci: *mut CallInfo, o: *const UnsafeValue) -> c_int {
    let mut pos: c_int = 0;
    let base: StkId = ((*ci).func).offset(1 as c_int as isize);
    pos = 0 as c_int;
    while base.offset(pos as isize) < (*ci).top {
        if o == &mut (*base.offset(pos as isize)).val as *mut UnsafeValue as *const UnsafeValue {
            return pos;
        }
        pos += 1;
    }
    return -(1 as c_int);
}

unsafe fn getupvalname(
    ci: *mut CallInfo,
    o: *const UnsafeValue,
    name: *mut *const c_char,
) -> *const c_char {
    let c = (*(*ci).func).val.value_.gc.cast::<LuaFn>();

    for (i, uv) in (*c).upvals.iter().map(|v| v.get()).enumerate() {
        if (*uv).v.get() == o as *mut UnsafeValue {
            *name = upvalname((*c).p.get(), i);
            return b"upvalue\0" as *const u8 as *const c_char;
        }
    }

    return 0 as *const c_char;
}

unsafe fn formatvarinfo(kind: *const c_char, name: *const c_char) -> Cow<'static, str> {
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

unsafe fn varinfo(L: *const Thread, o: *const UnsafeValue) -> Cow<'static, str> {
    let ci: *mut CallInfo = (*L).ci.get();
    let mut name: *const c_char = 0 as *const c_char;
    let mut kind: *const c_char = 0 as *const c_char;
    if (*ci).callstatus as c_int & (1 as c_int) << 1 as c_int == 0 {
        kind = getupvalname(ci, o, &mut name);
        if kind.is_null() {
            let reg: c_int = instack(ci, o);
            if reg >= 0 as c_int {
                kind = getobjname(
                    (*(*(*ci).func).val.value_.gc.cast::<LuaFn>()).p.get(),
                    currentpc(ci),
                    reg,
                    &mut name,
                );
            }
        }
    }

    formatvarinfo(kind, name)
}

unsafe fn typeerror(
    L: *const Thread,
    o: *const UnsafeValue,
    op: impl Display,
    extra: impl Display,
) -> Box<dyn core::error::Error> {
    let t = luaT_objtypename((*L).hdr.global, o);

    format!("attempt to {op} a {t} value{extra}").into()
}

pub unsafe fn luaG_typeerror(
    L: *const Thread,
    o: *const UnsafeValue,
    op: impl Display,
) -> Box<dyn core::error::Error> {
    typeerror(L, o, op, varinfo(L, o))
}

pub unsafe fn luaG_callerror(
    L: *const Thread,
    o: *const UnsafeValue,
) -> Box<dyn core::error::Error> {
    let ci: *mut CallInfo = (*L).ci.get();
    let mut name: *const c_char = 0 as *const c_char;
    let kind: *const c_char = funcnamefromcall(L, ci, &mut name);
    let extra = if !kind.is_null() {
        formatvarinfo(kind, name)
    } else {
        varinfo(L, o)
    };

    typeerror(L, o, "call", extra)
}

pub unsafe fn luaG_forerror(
    L: *const Thread,
    o: *const UnsafeValue,
    what: impl Display,
) -> Result<(), Box<dyn core::error::Error>> {
    luaG_runerror(
        L,
        format_args!(
            "bad 'for' {} (number expected, got {})",
            what,
            luaT_objtypename((*L).hdr.global, o)
        ),
    )
}

pub unsafe fn luaG_concaterror(
    L: *const Thread,
    mut p1: *const UnsafeValue,
    p2: *const UnsafeValue,
) -> Box<dyn core::error::Error> {
    if (*p1).tt_ as c_int & 0xf as c_int == 4 as c_int
        || (*p1).tt_ as c_int & 0xf as c_int == 3 as c_int
    {
        p1 = p2;
    }

    luaG_typeerror(L, p1, "concatenate")
}

pub unsafe fn luaG_opinterror(
    L: *const Thread,
    p1: *const UnsafeValue,
    mut p2: *const UnsafeValue,
    msg: impl Display,
) -> Box<dyn core::error::Error> {
    if !((*p1).tt_ as c_int & 0xf as c_int == 3 as c_int) {
        p2 = p1;
    }

    luaG_typeerror(L, p2, msg)
}

pub unsafe fn luaG_tointerror(
    L: *const Thread,
    p1: *const UnsafeValue,
    mut p2: *const UnsafeValue,
) -> Result<(), Box<dyn core::error::Error>> {
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
    L: *const Thread,
    p1: *const UnsafeValue,
    p2: *const UnsafeValue,
) -> Result<(), Box<dyn core::error::Error>> {
    let t1 = luaT_objtypename((*L).hdr.global, p1);
    let t2 = luaT_objtypename((*L).hdr.global, p2);

    if t1 == t2 {
        luaG_runerror(L, format_args!("attempt to compare two {t1} values"))
    } else {
        luaG_runerror(L, format_args!("attempt to compare {t1} with {t2}"))
    }
}

pub unsafe fn luaG_addinfo(msg: impl Display, src: &ChunkInfo, line: c_int) -> String {
    format!("{}:{}: {}", src.name(), line, msg)
}

pub unsafe fn luaG_runerror(
    L: *const Thread,
    fmt: impl Display,
) -> Result<(), Box<dyn core::error::Error>> {
    let ci = (*L).ci.get();
    let msg = if (*ci).callstatus as c_int & (1 as c_int) << 1 as c_int == 0 {
        luaG_addinfo(
            fmt,
            &(*(*(*(*ci).func).val.value_.gc.cast::<LuaFn>()).p.get()).chunk,
            getcurrentline(ci),
        )
    } else {
        fmt.to_string()
    };

    Err(msg.into())
}

unsafe fn changedline(p: *const Proto, oldpc: c_int, newpc: c_int) -> c_int {
    if ((*p).lineinfo).is_null() {
        return 0 as c_int;
    }
    if newpc - oldpc < 128 as c_int / 2 as c_int {
        let mut delta: c_int = 0 as c_int;
        let mut pc: c_int = oldpc;
        loop {
            pc += 1;
            let lineinfo: c_int = *((*p).lineinfo).offset(pc as isize) as c_int;
            if lineinfo == -(0x80 as c_int) {
                break;
            }
            delta += lineinfo;
            if pc == newpc {
                return (delta != 0 as c_int) as c_int;
            }
        }
    }
    return (luaG_getfuncline(p, oldpc) != luaG_getfuncline(p, newpc)) as c_int;
}

pub unsafe fn luaG_tracecall(L: *const Thread) -> Result<c_int, Box<dyn core::error::Error>> {
    let ci: *mut CallInfo = (*L).ci.get();
    let p: *mut Proto = (*(*(*ci).func).val.value_.gc.cast::<LuaFn>()).p.get();
    ::core::ptr::write_volatile(&mut (*ci).u.trap as *mut c_int, 1 as c_int);
    if (*ci).u.savedpc == (*p).code as *const u32 {
        if (*p).is_vararg != 0 {
            return Ok(0 as c_int);
        } else if (*ci).callstatus as c_int & (1 as c_int) << 6 as c_int == 0 {
            luaD_hookcall(L, ci)?;
        }
    }
    return Ok(1 as c_int);
}

pub unsafe fn luaG_traceexec(
    L: *const Thread,
    mut pc: *const u32,
) -> Result<c_int, Box<dyn core::error::Error>> {
    let ci: *mut CallInfo = (*L).ci.get();
    let mask: u8 = (*L).hookmask.get() as u8;
    let p: *const Proto = (*(*(*ci).func).val.value_.gc.cast::<LuaFn>()).p.get();
    let mut counthook: c_int = 0;
    if mask as c_int & ((1 as c_int) << 2 as c_int | (1 as c_int) << 3 as c_int) == 0 {
        ::core::ptr::write_volatile(&mut (*ci).u.trap as *mut c_int, 0 as c_int);
        return Ok(0 as c_int);
    }
    pc = pc.offset(1);
    (*ci).u.savedpc = pc;
    counthook = (mask as c_int & (1 as c_int) << 3 as c_int != 0 && {
        (*L).hookcount.set((*L).hookcount.get() - 1);
        (*L).hookcount.get() == 0
    }) as c_int;
    if counthook != 0 {
        (*L).hookcount.set((*L).basehookcount.get());
    } else if mask as c_int & (1 as c_int) << 2 as c_int == 0 {
        return Ok(1 as c_int);
    }
    if (*ci).callstatus as c_int & (1 as c_int) << 6 as c_int != 0 {
        (*ci).callstatus =
            ((*ci).callstatus as c_int & !((1 as c_int) << 6 as c_int)) as libc::c_ushort;
        return Ok(1 as c_int);
    }
    if !(luaP_opmodes[(*((*ci).u.savedpc).offset(-(1 as c_int as isize)) >> 0 as c_int
        & !(!(0 as c_int as u32) << 7 as c_int) << 0 as c_int) as OpCode
        as usize] as c_int
        & (1 as c_int) << 5 as c_int
        != 0
        && (*((*ci).u.savedpc).offset(-(1 as c_int as isize))
            >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int) as c_int
            == 0 as c_int)
    {
        (*L).top.set((*ci).top);
    }

    if counthook != 0 {
        luaD_hook(L, 3 as c_int, -(1 as c_int), 0, 0)?;
    }

    if mask as c_int & (1 as c_int) << 2 as c_int != 0 {
        let oldpc: c_int = if (*L).oldpc.get() < (*p).sizecode {
            (*L).oldpc.get()
        } else {
            0 as c_int
        };
        let npci: c_int = pc.offset_from((*p).code) as c_long as c_int - 1 as c_int;

        if npci <= oldpc || changedline(p, oldpc, npci) != 0 {
            let newline: c_int = luaG_getfuncline(p, npci);

            luaD_hook(L, 2 as c_int, newline, 0 as c_int, 0)?;
        }

        (*L).oldpc.set(npci);
    }

    Ok(1)
}
