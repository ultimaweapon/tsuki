#![allow(non_camel_case_types, non_snake_case, unused_assignments)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::lfunc::luaF_getlocalname;
use crate::lobject::Proto;
use crate::lstate::{CallInfo, lua_Debug};
use crate::ltm::{
    TM_BNOT, TM_CLOSE, TM_CONCAT, TM_EQ, TM_INDEX, TM_LE, TM_LEN, TM_LT, TM_NEWINDEX, TM_UNM, TMS,
    luaT_objtypename,
};
use crate::value::UnsafeValue;
use crate::vm::{F2Ieq, OpCode, luaP_opmodes, luaV_tointegerns};
use crate::{Lua, LuaFn, Object, StackValue, Str, Thread};
use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::format;
use core::ffi::{CStr, c_char};
use core::fmt::Display;
use core::ptr::{null, null_mut};
use libc::strcmp;

type c_int = i32;
type c_uint = u32;
type c_long = i64;

unsafe fn currentpc(ci: *mut CallInfo) -> c_int {
    return (*ci).pc as c_int - 1;
}

unsafe fn getbaseline<D>(f: *const Proto<D>, pc: c_int, basepc: *mut c_int) -> c_int {
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

pub unsafe fn luaG_getfuncline<D>(f: *const Proto<D>, pc: c_int) -> c_int {
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

unsafe fn upvalname<D>(p: *const Proto<D>, uv: usize) -> *const c_char {
    let s = (*((*p).upvalues).add(uv)).name;

    if s.is_null() {
        return b"?\0" as *const u8 as *const c_char;
    } else {
        return ((*s).contents).as_ptr();
    };
}

pub unsafe fn luaG_findlocal<D>(
    L: *const Thread<D>,
    ci: *mut CallInfo,
    n: c_int,
    pos: *mut *mut StackValue<D>,
) -> *const c_char {
    let base = (*L).stack.get().add((*ci).func + 1);
    let mut name: *const c_char = 0 as *const c_char;

    if (*ci).callstatus as c_int & (1 as c_int) << 1 as c_int == 0 {
        let f = (*L).stack.get().add((*ci).func);

        if n < 0 as c_int {
            if (*(*(*f).value_.gc.cast::<LuaFn<D>>()).p.get()).is_vararg != 0 {
                let nextra: c_int = (*ci).nextraargs;
                if n >= -nextra {
                    *pos = f.offset(-(nextra as isize)).offset(-((n + 1) as isize));

                    return b"(vararg)\0" as *const u8 as *const c_char;
                }
            }

            return 0 as *const c_char;
        } else {
            name = luaF_getlocalname(
                (*(*f).value_.gc.cast::<LuaFn<D>>()).p.get(),
                n,
                currentpc(ci),
            );
        }
    }

    if name.is_null() {
        let limit = if ci == (*L).ci.get() {
            (*L).top.get()
        } else {
            (*L).stack.get().add((*(*ci).next).func)
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

pub unsafe fn lua_getlocal<D>(
    L: *const Thread<D>,
    ar: *const lua_Debug,
    n: c_int,
) -> *const c_char {
    let mut name: *const c_char = 0 as *const c_char;
    if ar.is_null() {
        if !((*((*L).top.get()).offset(-(1 as c_int as isize))).tt_ as c_int
            == 6 as c_int | (0 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int)
        {
            name = 0 as *const c_char;
        } else {
            name = luaF_getlocalname(
                (*(*((*L).top.get()).offset(-1)).value_.gc.cast::<LuaFn<D>>())
                    .p
                    .get(),
                n,
                0 as c_int,
            );
        }
    } else {
        let mut pos = null_mut();
        name = luaG_findlocal(L, (*ar).i_ci, n, &mut pos);
        if !name.is_null() {
            let io1 = (*L).top.get();
            let io2 = pos;

            (*io1).value_ = (*io2).value_;
            (*io1).tt_ = (*io2).tt_;

            unsafe { (*L).top.add(1) };
        }
    }
    return name;
}

pub unsafe fn lua_setlocal<D>(
    L: *const Thread<D>,
    ar: *const lua_Debug,
    n: c_int,
) -> *const c_char {
    let mut pos = null_mut();
    let mut name: *const c_char = 0 as *const c_char;
    name = luaG_findlocal(L, (*ar).i_ci, n, &mut pos);
    if !name.is_null() {
        let io1 = pos;
        let io2 = ((*L).top.get()).offset(-1);

        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        (*L).top.sub(1);
    }
    return name;
}

pub unsafe fn funcinfo<A>(ar: &mut lua_Debug, cl: *const Object<A>) {
    if !(!cl.is_null() && (*cl).tt as c_int == 6 as c_int | (0 as c_int) << 4) {
        (*ar).source = None;
        (*ar).linedefined = -(1 as c_int);
        (*ar).lastlinedefined = -(1 as c_int);
        (*ar).what = b"C\0" as *const u8 as *const c_char;
    } else {
        let p = (*cl.cast::<LuaFn<A>>()).p.get();

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

pub unsafe fn getfuncname<A>(
    L: *const Thread<A>,
    ci: *mut CallInfo,
    name: *mut *const c_char,
) -> *const c_char {
    if !ci.is_null() && (*ci).callstatus as c_int & (1 as c_int) << 5 as c_int == 0 {
        return funcnamefromcall(L, (*ci).previous, name);
    } else {
        return 0 as *const c_char;
    };
}

unsafe fn filterpc(pc: c_int, jmptarget: c_int) -> c_int {
    if pc < jmptarget {
        return -(1 as c_int);
    } else {
        return pc;
    };
}

unsafe fn findsetreg<D>(p: *const Proto<D>, mut lastpc: c_int, reg: c_int) -> c_int {
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

unsafe fn kname<D>(p: *const Proto<D>, index: c_int, name: *mut *const c_char) -> *const c_char {
    let kvalue = ((*p).k).offset(index as isize) as *mut UnsafeValue<D>;

    if (*kvalue).tt_ as c_int & 0xf as c_int == 4 as c_int {
        *name = ((*((*kvalue).value_.gc as *mut Str<D>)).contents).as_mut_ptr();
        return b"constant\0" as *const u8 as *const c_char;
    } else {
        *name = b"?\0" as *const u8 as *const c_char;
        return 0 as *const c_char;
    };
}

unsafe fn basicgetobjname<D>(
    p: *const Proto<D>,
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

unsafe fn rname<D>(p: *const Proto<D>, mut pc: c_int, c: c_int, name: *mut *const c_char) {
    let what: *const c_char = basicgetobjname(p, &mut pc, c, name);
    if !(!what.is_null() && *what as c_int == 'c' as i32) {
        *name = b"?\0" as *const u8 as *const c_char;
    }
}

unsafe fn rkname<D>(p: *const Proto<D>, pc: c_int, i: u32, name: *mut *const c_char) {
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

unsafe fn isEnv<D>(p: *const Proto<D>, mut pc: c_int, i: u32, isup: c_int) -> *const c_char {
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
unsafe fn getobjname<D>(
    p: *const Proto<D>,
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

unsafe fn funcnamefromcode<A>(
    L: *const Lua<A>,
    p: *const Proto<A>,
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
        20 | 11 | 12 | 13 | 14 => tm = TM_INDEX,
        15 | 16 | 17 | 18 => tm = TM_NEWINDEX,
        46 | 47 | 48 => tm = (i >> 0 + 7 + 8 + 1 + 8 & !(!(0u32) << 8) << 0) as c_int as TMS,
        49 => tm = TM_UNM,
        50 => tm = TM_BNOT,
        52 => tm = TM_LEN,
        53 => tm = TM_CONCAT,
        57 => tm = TM_EQ,
        58 | 62 | 64 => tm = TM_LT,
        59 | 63 | 65 => tm = TM_LE,
        54 | 70 => tm = TM_CLOSE,
        _ => return 0 as *const c_char,
    }

    *name = (*(*L)
        .events()
        .get_raw_int_key(tm.into())
        .value_
        .gc
        .cast::<Str<A>>())
    .contents
    .as_ptr()
    .add(2);

    return b"metamethod\0" as *const u8 as *const c_char;
}

unsafe fn funcnamefromcall<A>(
    L: *const Thread<A>,
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
        let f = (*L).stack.get().add((*ci).func);

        return funcnamefromcode(
            (*L).hdr.global,
            (*(*f).value_.gc.cast::<LuaFn<A>>()).p.get(),
            currentpc(ci),
            name,
        );
    } else {
        return 0 as *const c_char;
    };
}

unsafe fn instack<A>(th: &Thread<A>, ci: *mut CallInfo, o: *const UnsafeValue<A>) -> c_int {
    let mut pos: c_int = 0;
    let stack = th.stack.get();
    let base = stack.add((*ci).func).offset(1);
    let end = stack.add((*ci).top.get());

    pos = 0 as c_int;
    while base.offset(pos as isize) < end {
        if o == base.offset(pos as isize).cast() {
            return pos;
        }
        pos += 1;
    }
    return -(1 as c_int);
}

unsafe fn getupvalname<A>(
    th: &Thread<A>,
    ci: *mut CallInfo,
    o: *const UnsafeValue<A>,
    name: *mut *const c_char,
) -> *const c_char {
    let f = th.stack.get().add((*ci).func);
    let c = (*f).value_.gc.cast::<LuaFn<A>>();

    for (i, uv) in (*c).upvals.iter().map(|v| v.get()).enumerate() {
        if (*uv).v.get() == o.cast_mut() {
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

unsafe fn varinfo<D>(L: *const Thread<D>, o: *const UnsafeValue<D>) -> Cow<'static, str> {
    let ci = (*L).ci.get();
    let mut name: *const c_char = 0 as *const c_char;
    let mut kind: *const c_char = 0 as *const c_char;

    if (*ci).callstatus as c_int & (1 as c_int) << 1 as c_int == 0 {
        kind = getupvalname(&*L, ci, o, &mut name);

        if kind.is_null() {
            let reg = instack(&*L, ci, o);

            if reg >= 0 as c_int {
                let f = (*L).stack.get().add((*ci).func);

                kind = getobjname(
                    (*(*f).value_.gc.cast::<LuaFn<D>>()).p.get(),
                    currentpc(ci),
                    reg,
                    &mut name,
                );
            }
        }
    }

    formatvarinfo(kind, name)
}

unsafe fn typeerror<D>(
    L: *const Thread<D>,
    o: *const UnsafeValue<D>,
    op: impl Display,
    extra: impl Display,
) -> Box<dyn core::error::Error> {
    let t = luaT_objtypename((*L).hdr.global, o);

    format!("attempt to {op} a {t} value{extra}").into()
}

pub unsafe fn luaG_typeerror<D>(
    L: *const Thread<D>,
    o: *const UnsafeValue<D>,
    op: impl Display,
) -> Box<dyn core::error::Error> {
    typeerror(L, o, op, varinfo(L, o))
}

pub unsafe fn luaG_callerror<D>(
    L: *const Thread<D>,
    o: *const UnsafeValue<D>,
) -> Box<dyn core::error::Error> {
    let ci = (*L).ci.get();
    let mut name: *const c_char = 0 as *const c_char;
    let kind: *const c_char = funcnamefromcall(L, ci, &mut name);
    let extra = if !kind.is_null() {
        formatvarinfo(kind, name)
    } else {
        varinfo(L, o)
    };

    typeerror(L, o, "call", extra)
}

pub unsafe fn luaG_forerror<D>(
    L: *const Thread<D>,
    o: *const UnsafeValue<D>,
    what: impl Display,
) -> Result<(), Box<dyn core::error::Error>> {
    Err(format!(
        "bad 'for' {} (number expected, got {})",
        what,
        luaT_objtypename((*L).hdr.global, o)
    )
    .into())
}

pub unsafe fn luaG_concaterror<D>(
    L: *const Thread<D>,
    mut p1: *const UnsafeValue<D>,
    p2: *const UnsafeValue<D>,
) -> Box<dyn core::error::Error> {
    if (*p1).tt_ as c_int & 0xf as c_int == 4 as c_int
        || (*p1).tt_ as c_int & 0xf as c_int == 3 as c_int
    {
        p1 = p2;
    }

    luaG_typeerror(L, p1, "concatenate")
}

pub unsafe fn luaG_opinterror<D>(
    L: *const Thread<D>,
    p1: *const UnsafeValue<D>,
    mut p2: *const UnsafeValue<D>,
    msg: impl Display,
) -> Box<dyn core::error::Error> {
    if !((*p1).tt_ as c_int & 0xf as c_int == 3 as c_int) {
        p2 = p1;
    }

    luaG_typeerror(L, p2, msg)
}

pub unsafe fn luaG_tointerror<D>(
    L: *const Thread<D>,
    p1: *const UnsafeValue<D>,
    mut p2: *const UnsafeValue<D>,
) -> Result<(), Box<dyn core::error::Error>> {
    if luaV_tointegerns(p1, F2Ieq).is_none() {
        p2 = p1;
    }

    Err(format!("number{} has no integer representation", varinfo(L, p2)).into())
}

pub unsafe fn luaG_ordererror<D>(
    L: *const Thread<D>,
    p1: *const UnsafeValue<D>,
    p2: *const UnsafeValue<D>,
) -> Box<dyn core::error::Error> {
    let t1 = luaT_objtypename((*L).hdr.global, p1);
    let t2 = luaT_objtypename((*L).hdr.global, p2);

    if t1 == t2 {
        format!("attempt to compare two {t1} values").into()
    } else {
        format!("attempt to compare {t1} with {t2}").into()
    }
}
