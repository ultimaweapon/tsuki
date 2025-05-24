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

use crate::Lua;
use crate::gc::{luaC_barrier_, luaC_newobj};
use crate::ldebug::{luaG_findlocal, luaG_runerror};
use crate::ldo::luaD_call;
use crate::lmem::luaM_free_;
use crate::lobject::{
    AbsLineInfo, CClosure, GCObject, LClosure, LocVar, Proto, StackValue, StkId, TString, TValue,
    UpVal, Upvaldesc,
};
use crate::lstate::lua_State;
use crate::ltm::{TM_CLOSE, luaT_gettmbyobj};
use std::ffi::CStr;

pub unsafe fn luaF_newCclosure(mut L: *mut lua_State, mut nupvals: libc::c_int) -> *mut CClosure {
    let mut o: *mut GCObject = luaC_newobj(
        (*L).l_G,
        6 as libc::c_int | (2 as libc::c_int) << 4 as libc::c_int,
        (32 as libc::c_ulong as libc::c_int
            + ::core::mem::size_of::<TValue>() as libc::c_ulong as libc::c_int * nupvals)
            as usize,
    );
    let mut c: *mut CClosure = o as *mut CClosure;
    (*c).nupvalues = nupvals as u8;
    return c;
}

pub unsafe fn luaF_newLclosure(mut L: *mut lua_State, mut nupvals: libc::c_int) -> *mut LClosure {
    let mut o: *mut GCObject = luaC_newobj(
        (*L).l_G,
        6 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int,
        (32 as libc::c_ulong as libc::c_int
            + ::core::mem::size_of::<*mut TValue>() as libc::c_ulong as libc::c_int * nupvals)
            as usize,
    );
    let mut c: *mut LClosure = o as *mut LClosure;
    (*c).p = 0 as *mut Proto;
    (*c).nupvalues = nupvals as u8;
    loop {
        let fresh0 = nupvals;
        nupvals = nupvals - 1;
        if !(fresh0 != 0) {
            break;
        }
        let ref mut fresh1 = *((*c).upvals).as_mut_ptr().offset(nupvals as isize);
        *fresh1 = 0 as *mut UpVal;
    }
    return c;
}

pub unsafe fn luaF_initupvals(mut L: *mut lua_State, mut cl: *mut LClosure) {
    let mut i: libc::c_int = 0;
    i = 0 as libc::c_int;
    while i < (*cl).nupvalues as libc::c_int {
        let mut o: *mut GCObject = luaC_newobj(
            (*L).l_G,
            9 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int,
            ::core::mem::size_of::<UpVal>(),
        );
        let mut uv: *mut UpVal = o as *mut UpVal;
        (*uv).v.p = &mut (*uv).u.value;
        (*(*uv).v.p).tt_ = (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
        let ref mut fresh2 = *((*cl).upvals).as_mut_ptr().offset(i as isize);
        *fresh2 = uv;
        if (*cl).marked as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
            && (*uv).marked as libc::c_int
                & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
                != 0
        {
            luaC_barrier_(L, cl as *mut GCObject, uv as *mut GCObject);
        } else {
        };
        i += 1;
        i;
    }
}

unsafe fn newupval(
    mut L: *mut lua_State,
    mut level: StkId,
    mut prev: *mut *mut UpVal,
) -> *mut UpVal {
    let mut o: *mut GCObject = luaC_newobj(
        (*L).l_G,
        9 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int,
        ::core::mem::size_of::<UpVal>(),
    );
    let mut uv: *mut UpVal = o as *mut UpVal;
    let mut next: *mut UpVal = *prev;
    (*uv).v.p = &mut (*level).val;
    (*uv).u.open.next = next;
    (*uv).u.open.previous = prev;
    if !next.is_null() {
        (*next).u.open.previous = &mut (*uv).u.open.next;
    }
    *prev = uv;

    if !((*L).twups != L) {
        (*L).twups = (*(*L).l_G).twups.get();
        (*(*L).l_G).twups.set(L);
    }

    return uv;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaF_findupval(mut L: *mut lua_State, mut level: StkId) -> *mut UpVal {
    let mut pp: *mut *mut UpVal = &mut (*L).openupval;
    let mut p: *mut UpVal = 0 as *mut UpVal;
    loop {
        p = *pp;
        if !(!p.is_null() && (*p).v.p as StkId >= level) {
            break;
        }
        if (*p).v.p as StkId == level {
            return p;
        }
        pp = &mut (*p).u.open.next;
    }
    return newupval(L, level, pp);
}

unsafe fn callclosemethod(
    mut L: *mut lua_State,
    mut obj: *mut TValue,
    mut err: *mut TValue,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut top: StkId = (*L).top.p;
    let mut tm: *const TValue = luaT_gettmbyobj(L, obj, TM_CLOSE);
    let mut io1: *mut TValue = &mut (*top).val;
    let mut io2: *const TValue = tm;
    (*io1).value_ = (*io2).value_;
    (*io1).tt_ = (*io2).tt_;
    let mut io1_0: *mut TValue = &mut (*top.offset(1 as libc::c_int as isize)).val;
    let mut io2_0: *const TValue = obj;
    (*io1_0).value_ = (*io2_0).value_;
    (*io1_0).tt_ = (*io2_0).tt_;
    let mut io1_1: *mut TValue = &mut (*top.offset(2 as libc::c_int as isize)).val;
    let mut io2_1: *const TValue = err;
    (*io1_1).value_ = (*io2_1).value_;
    (*io1_1).tt_ = (*io2_1).tt_;
    (*L).top.p = top.offset(3 as libc::c_int as isize);

    luaD_call(L, top, 0 as libc::c_int)
}

unsafe fn checkclosemth(
    mut L: *mut lua_State,
    mut level: StkId,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut tm: *const TValue = luaT_gettmbyobj(L, &mut (*level).val, TM_CLOSE);
    if (*tm).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int {
        let mut idx: libc::c_int =
            level.offset_from((*(*L).ci).func.p) as libc::c_long as libc::c_int;
        let mut vname: *const libc::c_char = luaG_findlocal(L, (*L).ci, idx, 0 as *mut StkId);
        if vname.is_null() {
            vname = b"?\0" as *const u8 as *const libc::c_char;
        }

        luaG_runerror(
            L,
            format!(
                "variable '{}' got a non-closable value",
                CStr::from_ptr(vname).to_string_lossy()
            ),
        )?;
    }
    Ok(())
}

unsafe fn prepcallclosemth(
    L: *mut lua_State,
    level: StkId,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut uv: *mut TValue = &mut (*level).val;
    let errobj = (*(*L).l_G).nilvalue.get();

    callclosemethod(L, uv, errobj)
}

pub unsafe fn luaF_newtbcupval(
    mut L: *mut lua_State,
    mut level: StkId,
) -> Result<(), Box<dyn std::error::Error>> {
    if (*level).val.tt_ as libc::c_int == 1 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
        || (*level).val.tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int
    {
        return Ok(());
    }
    checkclosemth(L, level)?;
    while level.offset_from((*L).tbclist.p) as libc::c_long as libc::c_uint as libc::c_ulong
        > ((256 as libc::c_ulong)
            << (::core::mem::size_of::<libc::c_ushort>() as libc::c_ulong)
                .wrapping_sub(1 as libc::c_int as libc::c_ulong)
                .wrapping_mul(8 as libc::c_int as libc::c_ulong))
        .wrapping_sub(1 as libc::c_int as libc::c_ulong)
    {
        (*L).tbclist.p = ((*L).tbclist.p).offset(
            ((256 as libc::c_ulong)
                << (::core::mem::size_of::<libc::c_ushort>() as libc::c_ulong)
                    .wrapping_sub(1 as libc::c_int as libc::c_ulong)
                    .wrapping_mul(8 as libc::c_int as libc::c_ulong))
            .wrapping_sub(1 as libc::c_int as libc::c_ulong) as isize,
        );
        (*(*L).tbclist.p).tbclist.delta = 0 as libc::c_int as libc::c_ushort;
    }
    (*level).tbclist.delta = level.offset_from((*L).tbclist.p) as libc::c_long as libc::c_ushort;
    (*L).tbclist.p = level;
    Ok(())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaF_unlinkupval(mut uv: *mut UpVal) {
    *(*uv).u.open.previous = (*uv).u.open.next;
    if !((*uv).u.open.next).is_null() {
        (*(*uv).u.open.next).u.open.previous = (*uv).u.open.previous;
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaF_closeupval(mut L: *mut lua_State, mut level: StkId) {
    let mut uv: *mut UpVal = 0 as *mut UpVal;
    let mut upl: StkId = 0 as *mut StackValue;
    loop {
        uv = (*L).openupval;
        if !(!uv.is_null() && {
            upl = (*uv).v.p as StkId;
            upl >= level
        }) {
            break;
        }
        let mut slot: *mut TValue = &mut (*uv).u.value;
        luaF_unlinkupval(uv);
        let mut io1: *mut TValue = slot;
        let mut io2: *const TValue = (*uv).v.p;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        (*uv).v.p = slot;
        if (*uv).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            == 0
        {
            (*uv).marked =
                ((*uv).marked as libc::c_int | (1 as libc::c_int) << 5 as libc::c_int) as u8;
            if (*slot).tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0 {
                if (*uv).marked as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
                    && (*(*slot).value_.gc).marked as libc::c_int
                        & ((1 as libc::c_int) << 3 as libc::c_int
                            | (1 as libc::c_int) << 4 as libc::c_int)
                        != 0
                {
                    luaC_barrier_(L, uv as *mut GCObject, (*slot).value_.gc as *mut GCObject);
                } else {
                };
            } else {
            };
        }
    }
}

unsafe extern "C" fn poptbclist(mut L: *mut lua_State) {
    let mut tbc: StkId = (*L).tbclist.p;
    tbc = tbc.offset(-((*tbc).tbclist.delta as libc::c_int as isize));
    while tbc > (*L).stack.p && (*tbc).tbclist.delta as libc::c_int == 0 as libc::c_int {
        tbc = tbc.offset(
            -(((256 as libc::c_ulong)
                << (::core::mem::size_of::<libc::c_ushort>() as libc::c_ulong)
                    .wrapping_sub(1 as libc::c_int as libc::c_ulong)
                    .wrapping_mul(8 as libc::c_int as libc::c_ulong))
            .wrapping_sub(1 as libc::c_int as libc::c_ulong) as isize),
        );
    }
    (*L).tbclist.p = tbc;
}

pub unsafe fn luaF_close(
    mut L: *mut lua_State,
    mut level: StkId,
) -> Result<StkId, Box<dyn std::error::Error>> {
    let mut levelrel = (level as *mut libc::c_char).offset_from((*L).stack.p as *mut libc::c_char);

    luaF_closeupval(L, level);

    while (*L).tbclist.p >= level {
        let mut tbc: StkId = (*L).tbclist.p;
        poptbclist(L);
        prepcallclosemth(L, tbc)?;
        level = ((*L).stack.p as *mut libc::c_char).offset(levelrel as isize) as StkId;
    }

    return Ok(level);
}

pub unsafe fn luaF_newproto(mut L: *mut lua_State) -> *mut Proto {
    let mut o: *mut GCObject = luaC_newobj(
        (*L).l_G,
        9 as libc::c_int + 1 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int,
        ::core::mem::size_of::<Proto>(),
    );
    let mut f: *mut Proto = o as *mut Proto;
    (*f).k = 0 as *mut TValue;
    (*f).sizek = 0 as libc::c_int;
    (*f).p = 0 as *mut *mut Proto;
    (*f).sizep = 0 as libc::c_int;
    (*f).code = 0 as *mut u32;
    (*f).sizecode = 0 as libc::c_int;
    (*f).lineinfo = 0 as *mut i8;
    (*f).sizelineinfo = 0 as libc::c_int;
    (*f).abslineinfo = 0 as *mut AbsLineInfo;
    (*f).sizeabslineinfo = 0 as libc::c_int;
    (*f).upvalues = 0 as *mut Upvaldesc;
    (*f).sizeupvalues = 0 as libc::c_int;
    (*f).numparams = 0 as libc::c_int as u8;
    (*f).is_vararg = 0 as libc::c_int as u8;
    (*f).maxstacksize = 0 as libc::c_int as u8;
    (*f).locvars = 0 as *mut LocVar;
    (*f).sizelocvars = 0 as libc::c_int;
    (*f).linedefined = 0 as libc::c_int;
    (*f).lastlinedefined = 0 as libc::c_int;
    (*f).source = 0 as *mut TString;
    return f;
}

pub unsafe fn luaF_freeproto(g: *const Lua, mut f: *mut Proto) {
    luaM_free_(
        g,
        (*f).code as *mut libc::c_void,
        ((*f).sizecode as usize).wrapping_mul(::core::mem::size_of::<u32>()),
    );
    luaM_free_(
        g,
        (*f).p as *mut libc::c_void,
        ((*f).sizep as usize).wrapping_mul(::core::mem::size_of::<*mut Proto>()),
    );
    luaM_free_(
        g,
        (*f).k as *mut libc::c_void,
        ((*f).sizek as usize).wrapping_mul(::core::mem::size_of::<TValue>()),
    );
    luaM_free_(
        g,
        (*f).lineinfo as *mut libc::c_void,
        ((*f).sizelineinfo as usize).wrapping_mul(::core::mem::size_of::<i8>()),
    );
    luaM_free_(
        g,
        (*f).abslineinfo as *mut libc::c_void,
        ((*f).sizeabslineinfo as usize).wrapping_mul(::core::mem::size_of::<AbsLineInfo>()),
    );
    luaM_free_(
        g,
        (*f).locvars as *mut libc::c_void,
        ((*f).sizelocvars as usize).wrapping_mul(::core::mem::size_of::<LocVar>()),
    );
    luaM_free_(
        g,
        (*f).upvalues as *mut libc::c_void,
        ((*f).sizeupvalues as usize).wrapping_mul(::core::mem::size_of::<Upvaldesc>()),
    );
    luaM_free_(g, f as *mut libc::c_void, size_of::<Proto>());
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaF_getlocalname(
    mut f: *const Proto,
    mut local_number: libc::c_int,
    mut pc: libc::c_int,
) -> *const libc::c_char {
    let mut i: libc::c_int = 0;
    i = 0 as libc::c_int;
    while i < (*f).sizelocvars && (*((*f).locvars).offset(i as isize)).startpc <= pc {
        if pc < (*((*f).locvars).offset(i as isize)).endpc {
            local_number -= 1;
            local_number;
            if local_number == 0 as libc::c_int {
                return ((*(*((*f).locvars).offset(i as isize)).varname).contents).as_mut_ptr();
            }
        }
        i += 1;
        i;
    }
    return 0 as *const libc::c_char;
}
