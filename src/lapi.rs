#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments
)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::gc::{luaC_barrier_, luaC_barrierback_, luaC_step};
use crate::ldo::{luaD_call, luaD_growstack, luaD_pcall, luaD_protectedparser};
use crate::ldump::luaU_dump;
use crate::lfunc::{luaF_close, luaF_newCclosure, luaF_newtbcupval};
use crate::lobject::{
    CClosure, Proto, StackValue, StkId, TString, TValue, Table, UValue, Udata, UpVal, Value,
    luaO_arith, luaO_str2num, luaO_tostring,
};
use crate::lstate::{CallInfo, lua_CFunction, lua_Writer};
use crate::lstring::{luaS_new, luaS_newlstr, luaS_newudata};
use crate::ltable::{
    luaH_get, luaH_getint, luaH_getn, luaH_getstr, luaH_new, luaH_next, luaH_resize, luaH_set,
    luaH_setint,
};
use crate::ltm::{TM_EQ, TM_GC, luaT_gettm, luaT_typenames_};
use crate::lvm::{
    F2Ieq, luaV_concat, luaV_equalobj, luaV_finishget, luaV_finishset, luaV_lessequal,
    luaV_lessthan, luaV_objlen, luaV_tointeger, luaV_tonumber_,
};
use crate::lzio::Zio;
use crate::{LuaClosure, Object, Thread, api_incr_top};
use std::ffi::{c_int, c_void};
use std::mem::offset_of;
use std::ptr::null_mut;

unsafe fn index2value(L: *const Thread, mut idx: libc::c_int) -> *mut TValue {
    let ci: *mut CallInfo = (*L).ci.get();
    if idx > 0 as libc::c_int {
        let o: StkId = ((*ci).func).offset(idx as isize);
        if o >= (*L).top.get() {
            return (*(*L).global).nilvalue.get();
        } else {
            return &mut (*o).val;
        }
    } else if !(idx <= -(1000000 as libc::c_int) - 1000 as libc::c_int) {
        return &raw mut (*((*L).top.get()).offset(idx as isize)).val;
    } else if idx == -(1000000 as libc::c_int) - 1000 as libc::c_int {
        return (*(*L).global).l_registry.get();
    } else {
        idx = -(1000000 as libc::c_int) - 1000 as libc::c_int - idx;
        if (*(*ci).func).val.tt_ as libc::c_int
            == 6 as libc::c_int
                | (2 as libc::c_int) << 4 as libc::c_int
                | (1 as libc::c_int) << 6 as libc::c_int
        {
            let func: *mut CClosure = (*(*ci).func).val.value_.gc as *mut CClosure;
            return if idx <= (*func).nupvalues as libc::c_int {
                &mut *((*func).upvalue)
                    .as_mut_ptr()
                    .offset((idx - 1 as libc::c_int) as isize) as *mut TValue
            } else {
                (*(*L).global).nilvalue.get()
            };
        } else {
            return (*(*L).global).nilvalue.get();
        }
    };
}

unsafe fn index2stack(L: *const Thread, idx: libc::c_int) -> StkId {
    let ci: *mut CallInfo = (*L).ci.get();
    if idx > 0 as libc::c_int {
        let o: StkId = ((*ci).func).offset(idx as isize);
        return o;
    } else {
        return ((*L).top.get()).offset(idx as isize);
    };
}

pub unsafe fn lua_checkstack(L: *const Thread, n: usize) -> Result<(), Box<dyn std::error::Error>> {
    let ci = (*L).ci.get();

    if ((*L).stack_last.get()).offset_from_unsigned((*L).top.get()) > n {
    } else {
        luaD_growstack(L, n)?;
    }

    if (*ci).top < ((*L).top.get()).add(n) {
        (*ci).top = ((*L).top.get()).add(n);
    }

    Ok(())
}

pub unsafe fn lua_xmove(from: *mut Thread, to: *mut Thread, n: libc::c_int) {
    let mut i: libc::c_int = 0;
    if from == to {
        return;
    }
    (*from).top.sub(n.try_into().unwrap());
    i = 0 as libc::c_int;
    while i < n {
        let io1: *mut TValue = &raw mut (*(*to).top.get()).val;
        let io2: *const TValue = &raw mut (*((*from).top.get()).offset(i as isize)).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        (*to).top.add(1);

        i += 1;
    }
}

pub unsafe fn lua_absindex(L: *const Thread, idx: libc::c_int) -> libc::c_int {
    return if idx > 0 as libc::c_int || idx <= -(1000000 as libc::c_int) - 1000 as libc::c_int {
        idx
    } else {
        ((*L).top.get()).offset_from((*(*L).ci.get()).func) as libc::c_long as libc::c_int + idx
    };
}

pub unsafe fn lua_gettop(L: *const Thread) -> libc::c_int {
    return ((*L).top.get()).offset_from(((*(*L).ci.get()).func).offset(1 as libc::c_int as isize))
        as libc::c_long as libc::c_int;
}

pub unsafe fn lua_settop(
    L: *const Thread,
    idx: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut ci: *mut CallInfo = 0 as *mut CallInfo;
    let mut func: StkId = 0 as *mut StackValue;
    let mut newtop: StkId = 0 as *mut StackValue;
    let mut diff: isize = 0;
    ci = (*L).ci.get();
    func = (*ci).func;
    if idx >= 0 as libc::c_int {
        diff = func
            .offset(1 as libc::c_int as isize)
            .offset(idx as isize)
            .offset_from((*L).top.get());
        while diff > 0 as libc::c_int as isize {
            let fresh1 = (*L).top.get();
            (*L).top.add(1);
            (*fresh1).val.tt_ = (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
            diff -= 1;
        }
    } else {
        diff = (idx + 1 as libc::c_int) as isize;
    }

    newtop = ((*L).top.get()).offset(diff as isize);

    if diff < 0 as libc::c_int as isize && (*L).tbclist.get() >= newtop {
        newtop = luaF_close(L, newtop)?;
    }

    (*L).top.set(newtop);

    Ok(())
}

pub unsafe fn lua_closeslot(
    L: *mut Thread,
    idx: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut level: StkId = 0 as *mut StackValue;
    level = index2stack(L, idx);
    level = luaF_close(L, level)?;
    (*level).val.tt_ = (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    Ok(())
}

unsafe fn reverse(mut from: StkId, mut to: StkId) {
    while from < to {
        let mut temp: TValue = TValue {
            value_: Value {
                gc: 0 as *mut Object,
            },
            tt_: 0,
        };
        let io1: *mut TValue = &mut temp;
        let io2: *const TValue = &mut (*from).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        let io1_0: *mut TValue = &mut (*from).val;
        let io2_0: *const TValue = &mut (*to).val;
        (*io1_0).value_ = (*io2_0).value_;
        (*io1_0).tt_ = (*io2_0).tt_;
        let io1_1: *mut TValue = &mut (*to).val;
        let io2_1: *const TValue = &mut temp;
        (*io1_1).value_ = (*io2_1).value_;
        (*io1_1).tt_ = (*io2_1).tt_;
        from = from.offset(1);
        to = to.offset(-1);
    }
}

pub unsafe fn lua_rotate(L: *const Thread, idx: libc::c_int, n: libc::c_int) {
    let mut p: StkId = 0 as *mut StackValue;
    let mut t: StkId = 0 as *mut StackValue;
    let mut m: StkId = 0 as *mut StackValue;
    t = ((*L).top.get()).offset(-(1 as libc::c_int as isize));
    p = index2stack(L, idx);
    m = if n >= 0 as libc::c_int {
        t.offset(-(n as isize))
    } else {
        p.offset(-(n as isize)).offset(-(1 as libc::c_int as isize))
    };
    reverse(p, m);
    reverse(m.offset(1 as libc::c_int as isize), t);
    reverse(p, t);
}

pub unsafe fn lua_copy(L: *const Thread, fromidx: libc::c_int, toidx: libc::c_int) {
    let mut fr: *mut TValue = 0 as *mut TValue;
    let mut to: *mut TValue = 0 as *mut TValue;
    fr = index2value(L, fromidx);
    to = index2value(L, toidx);
    let io1: *mut TValue = to;
    let io2: *const TValue = fr;
    (*io1).value_ = (*io2).value_;
    (*io1).tt_ = (*io2).tt_;
    if toidx < -(1000000 as libc::c_int) - 1000 as libc::c_int {
        if (*fr).tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0 {
            if (*((*(*(*L).ci.get()).func).val.value_.gc as *mut CClosure))
                .hdr
                .marked
                .get() as libc::c_int
                & (1 as libc::c_int) << 5 as libc::c_int
                != 0
                && (*(*fr).value_.gc).marked.get() as libc::c_int
                    & ((1 as libc::c_int) << 3 as libc::c_int
                        | (1 as libc::c_int) << 4 as libc::c_int)
                    != 0
            {
                luaC_barrier_(
                    L,
                    ((*(*(*L).ci.get()).func).val.value_.gc as *mut CClosure) as *mut CClosure
                        as *mut Object,
                    (*fr).value_.gc as *mut Object,
                );
            } else {
            };
        } else {
        };
    }
}

pub unsafe fn lua_pushvalue(L: *const Thread, idx: libc::c_int) {
    let io1: *mut TValue = &raw mut (*(*L).top.get()).val;
    let io2: *const TValue = index2value(L, idx);
    (*io1).value_ = (*io2).value_;
    (*io1).tt_ = (*io2).tt_;
    api_incr_top(L);
}

pub unsafe fn lua_type(L: *const Thread, idx: libc::c_int) -> libc::c_int {
    let o: *const TValue = index2value(L, idx);
    return if !((*o).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int)
        || o != (*(*L).global).nilvalue.get() as *mut TValue as *const TValue
    {
        (*o).tt_ as libc::c_int & 0xf as libc::c_int
    } else {
        -(1 as libc::c_int)
    };
}

#[inline(always)]
pub fn lua_typename(t: c_int) -> &'static str {
    luaT_typenames_[(t + 1) as usize]
}

pub unsafe fn lua_iscfunction(L: *mut Thread, idx: libc::c_int) -> libc::c_int {
    let o: *const TValue = index2value(L, idx);
    return ((*o).tt_ as libc::c_int == 6 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
        || (*o).tt_ as libc::c_int
            == 6 as libc::c_int
                | (2 as libc::c_int) << 4 as libc::c_int
                | (1 as libc::c_int) << 6 as libc::c_int) as libc::c_int;
}

pub unsafe fn lua_isinteger(L: *const Thread, idx: libc::c_int) -> libc::c_int {
    let o: *const TValue = index2value(L, idx);
    return ((*o).tt_ as libc::c_int == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int)
        as libc::c_int;
}

pub unsafe fn lua_isnumber(L: *const Thread, idx: libc::c_int) -> libc::c_int {
    let mut n: f64 = 0.;
    let o: *const TValue = index2value(L, idx);
    return if (*o).tt_ as libc::c_int == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int {
        n = (*o).value_.n;
        1 as libc::c_int
    } else {
        luaV_tonumber_(o, &mut n)
    };
}

pub unsafe fn lua_isstring(L: *const Thread, idx: libc::c_int) -> libc::c_int {
    let o: *const TValue = index2value(L, idx);
    return ((*o).tt_ as libc::c_int & 0xf as libc::c_int == 4 as libc::c_int
        || (*o).tt_ as libc::c_int & 0xf as libc::c_int == 3 as libc::c_int)
        as libc::c_int;
}

pub unsafe fn lua_isuserdata(L: *mut Thread, idx: libc::c_int) -> libc::c_int {
    let o: *const TValue = index2value(L, idx);
    return ((*o).tt_ as libc::c_int
        == 7 as libc::c_int
            | (0 as libc::c_int) << 4 as libc::c_int
            | (1 as libc::c_int) << 6 as libc::c_int
        || (*o).tt_ as libc::c_int == 2 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int)
        as libc::c_int;
}

pub unsafe fn lua_rawequal(
    L: *const Thread,
    index1: libc::c_int,
    index2: libc::c_int,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let o1: *const TValue = index2value(L, index1);
    let o2: *const TValue = index2value(L, index2);
    return if (!((*o1).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int)
        || o1 != (*(*L).global).nilvalue.get() as *mut TValue as *const TValue)
        && (!((*o2).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int)
            || o2 != (*(*L).global).nilvalue.get() as *mut TValue as *const TValue)
    {
        luaV_equalobj(0 as *mut Thread, o1, o2)
    } else {
        Ok(0 as libc::c_int)
    };
}

pub unsafe fn lua_arith(
    L: *const Thread,
    op: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    if !(op != 12 as libc::c_int && op != 13 as libc::c_int) {
        let io1: *mut TValue = &raw mut (*(*L).top.get()).val;
        let io2: *const TValue =
            &raw mut (*((*L).top.get()).offset(-(1 as libc::c_int as isize))).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        api_incr_top(L);
    }
    luaO_arith(
        L,
        op,
        &raw mut (*((*L).top.get()).offset(-(2 as libc::c_int as isize))).val,
        &raw mut (*((*L).top.get()).offset(-(1 as libc::c_int as isize))).val,
        ((*L).top.get()).offset(-(2 as libc::c_int as isize)),
    )?;
    (*L).top.sub(1);

    Ok(())
}

pub unsafe fn lua_compare(
    L: *const Thread,
    index1: libc::c_int,
    index2: libc::c_int,
    op: libc::c_int,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut o1: *const TValue = 0 as *const TValue;
    let mut o2: *const TValue = 0 as *const TValue;
    let mut i: libc::c_int = 0 as libc::c_int;
    o1 = index2value(L, index1);
    o2 = index2value(L, index2);
    if (!((*o1).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int)
        || o1 != (*(*L).global).nilvalue.get() as *mut TValue as *const TValue)
        && (!((*o2).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int)
            || o2 != (*(*L).global).nilvalue.get() as *mut TValue as *const TValue)
    {
        match op {
            0 => {
                i = luaV_equalobj(L, o1, o2)?;
            }
            1 => {
                i = luaV_lessthan(L, o1, o2)?;
            }
            2 => {
                i = luaV_lessequal(L, o1, o2)?;
            }
            _ => {}
        }
    }
    return Ok(i);
}

pub unsafe fn lua_stringtonumber(L: *const Thread, s: *const libc::c_char) -> usize {
    let sz: usize = luaO_str2num(s, &raw mut (*(*L).top.get()).val);
    if sz != 0 as libc::c_int as usize {
        api_incr_top(L);
    }
    return sz;
}

pub unsafe fn lua_tonumberx(L: *const Thread, idx: libc::c_int, pisnum: *mut libc::c_int) -> f64 {
    let mut n: f64 = 0 as libc::c_int as f64;
    let o: *const TValue = index2value(L, idx);
    let isnum: libc::c_int =
        if (*o).tt_ as libc::c_int == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int {
            n = (*o).value_.n;
            1 as libc::c_int
        } else {
            luaV_tonumber_(o, &mut n)
        };
    if !pisnum.is_null() {
        *pisnum = isnum;
    }
    return n;
}

pub unsafe fn lua_tointegerx(L: *const Thread, idx: libc::c_int, pisnum: *mut libc::c_int) -> i64 {
    let mut res: i64 = 0 as libc::c_int as i64;
    let o: *const TValue = index2value(L, idx);
    let isnum: libc::c_int = if (((*o).tt_ as libc::c_int
        == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int)
        as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        res = (*o).value_.i;
        1 as libc::c_int
    } else {
        luaV_tointeger(o, &mut res, F2Ieq)
    };
    if !pisnum.is_null() {
        *pisnum = isnum;
    }
    return res;
}

pub unsafe fn lua_toboolean(L: *const Thread, idx: libc::c_int) -> libc::c_int {
    let o: *const TValue = index2value(L, idx);
    return !((*o).tt_ as libc::c_int == 1 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
        || (*o).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int)
        as libc::c_int;
}

pub unsafe fn lua_tolstring(
    L: *const Thread,
    idx: libc::c_int,
    len: *mut usize,
) -> *const libc::c_char {
    let mut o = index2value(L, idx);

    if !((*o).tt_ as libc::c_int & 0xf as libc::c_int == 4 as libc::c_int) {
        if !((*o).tt_ as libc::c_int & 0xf as libc::c_int == 3 as libc::c_int) {
            if !len.is_null() {
                *len = 0 as libc::c_int as usize;
            }
            return 0 as *const libc::c_char;
        }
        luaO_tostring((*L).global, o);
        if (*(*L).global).gc.debt() > 0 as libc::c_int as isize {
            luaC_step(L);
        }
        o = index2value(L, idx);
    }

    if !len.is_null() {
        *len = if (*((*o).value_.gc as *mut TString)).shrlen.get() as libc::c_int != 0xff {
            (*((*o).value_.gc as *mut TString)).shrlen.get() as usize
        } else {
            (*(*((*o).value_.gc as *mut TString)).u.get()).lnglen
        };
    }

    ((*((*o).value_.gc as *mut TString)).contents).as_mut_ptr()
}

pub unsafe fn lua_rawlen(L: *const Thread, idx: libc::c_int) -> u64 {
    let o: *const TValue = index2value(L, idx);
    match (*o).tt_ as libc::c_int & 0x3f as libc::c_int {
        4 => return (*((*o).value_.gc as *mut TString)).shrlen.get() as u64,
        20 => return (*(*((*o).value_.gc as *mut TString)).u.get()).lnglen as u64,
        7 => return (*((*o).value_.gc as *mut Udata)).len as u64,
        5 => return luaH_getn((*o).value_.gc as *mut Table),
        _ => return 0 as libc::c_int as u64,
    };
}

pub unsafe fn lua_tocfunction(L: *mut Thread, idx: c_int) -> Option<lua_CFunction> {
    let o: *const TValue = index2value(L, idx);
    if (*o).tt_ as libc::c_int == 6 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int {
        return Some((*o).value_.f);
    } else if (*o).tt_ as libc::c_int
        == 6 as libc::c_int
            | (2 as libc::c_int) << 4 as libc::c_int
            | (1 as libc::c_int) << 6 as libc::c_int
    {
        return Some((*((*o).value_.gc as *mut CClosure)).f);
    } else {
        return None;
    };
}

unsafe fn touserdata(o: *const TValue) -> *mut libc::c_void {
    match (*o).tt_ as libc::c_int & 0xf as libc::c_int {
        7 => (*o)
            .value_
            .gc
            .byte_add(
                offset_of!(Udata, uv)
                    + size_of::<UValue>() * usize::from((*((*o).value_.gc as *mut Udata)).nuvalue),
            )
            .cast_mut()
            .cast(),
        _ => null_mut(),
    }
}

pub unsafe fn lua_touserdata(L: *const Thread, idx: libc::c_int) -> *mut libc::c_void {
    let o: *const TValue = index2value(L, idx);
    return touserdata(o);
}

pub unsafe fn lua_tothread(L: *mut Thread, idx: libc::c_int) -> *mut Thread {
    let o: *const TValue = index2value(L, idx);
    return if !((*o).tt_ as libc::c_int
        == 8 as libc::c_int
            | (0 as libc::c_int) << 4 as libc::c_int
            | (1 as libc::c_int) << 6 as libc::c_int)
    {
        0 as *mut Thread
    } else {
        (*o).value_.gc as *mut Thread
    };
}

pub unsafe fn lua_topointer(L: *const Thread, idx: libc::c_int) -> *const libc::c_void {
    let o: *const TValue = index2value(L, idx);
    match (*o).tt_ as libc::c_int & 0x3f as libc::c_int {
        22 => {
            return ::core::mem::transmute::<lua_CFunction, usize>((*o).value_.f)
                as *mut libc::c_void;
        }
        7 | 2 => return touserdata(o),
        _ => {
            if (*o).tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0 {
                return (*o).value_.gc as *const libc::c_void;
            } else {
                return 0 as *const libc::c_void;
            }
        }
    };
}

pub unsafe fn lua_pushnil(L: *const Thread) {
    (*(*L).top.get()).val.tt_ = (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    api_incr_top(L);
}

pub unsafe fn lua_pushnumber(L: *const Thread, n: f64) {
    let io: *mut TValue = &raw mut (*(*L).top.get()).val;
    (*io).value_.n = n;
    (*io).tt_ = (3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
    api_incr_top(L);
}

pub unsafe fn lua_pushinteger(L: *const Thread, n: i64) {
    let io: *mut TValue = &raw mut (*(*L).top.get()).val;
    (*io).value_.i = n;
    (*io).tt_ = (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    api_incr_top(L);
}

pub unsafe fn lua_pushlstring(L: *const Thread, s: impl AsRef<[u8]>) -> *const libc::c_char {
    let s = s.as_ref();
    let ts = if s.is_empty() {
        luaS_new((*L).global, b"\0" as *const u8 as *const libc::c_char)
    } else {
        luaS_newlstr((*L).global, s.as_ptr().cast(), s.len())
    };
    let io: *mut TValue = &raw mut (*(*L).top.get()).val;
    let x_: *mut TString = ts;
    (*io).value_.gc = x_ as *mut Object;
    (*io).tt_ = ((*x_).hdr.tt as libc::c_int | (1 as libc::c_int) << 6 as libc::c_int) as u8;
    api_incr_top(L);

    if (*(*L).global).gc.debt() > 0 as libc::c_int as isize {
        luaC_step(L);
    }

    ((*ts).contents).as_mut_ptr()
}

pub unsafe fn lua_pushstring(L: *const Thread, mut s: *const libc::c_char) -> *const libc::c_char {
    if s.is_null() {
        (*(*L).top.get()).val.tt_ =
            (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    } else {
        let ts = luaS_new((*L).global, s);
        let io: *mut TValue = &raw mut (*(*L).top.get()).val;
        let x_: *mut TString = ts;
        (*io).value_.gc = x_ as *mut Object;
        (*io).tt_ = ((*x_).hdr.tt as libc::c_int | (1 as libc::c_int) << 6 as libc::c_int) as u8;
        s = ((*ts).contents).as_mut_ptr();
    }

    api_incr_top(L);

    if (*(*L).global).gc.debt() > 0 as libc::c_int as isize {
        luaC_step(L);
    }

    s
}

pub unsafe fn lua_pushcclosure(L: *const Thread, fn_0: lua_CFunction, mut n: libc::c_int) {
    if n == 0 as libc::c_int {
        let io: *mut TValue = &raw mut (*(*L).top.get()).val;
        (*io).value_.f = fn_0;
        (*io).tt_ = (6 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
        api_incr_top(L);
    } else {
        let mut cl: *mut CClosure = 0 as *mut CClosure;
        cl = luaF_newCclosure(L, n);
        (*cl).f = fn_0;
        (*L).top.sub(n.try_into().unwrap());
        loop {
            let fresh2 = n;
            n = n - 1;
            if !(fresh2 != 0) {
                break;
            }
            let io1: *mut TValue =
                &raw mut *((*cl).upvalue).as_mut_ptr().offset(n as isize) as *mut TValue;
            let io2: *const TValue = &raw mut (*((*L).top.get()).offset(n as isize)).val;
            (*io1).value_ = (*io2).value_;
            (*io1).tt_ = (*io2).tt_;
        }
        let io_0: *mut TValue = &raw mut (*(*L).top.get()).val;
        let x_: *mut CClosure = cl;
        (*io_0).value_.gc = x_ as *mut Object;
        (*io_0).tt_ = (6 as libc::c_int
            | (2 as libc::c_int) << 4 as libc::c_int
            | (1 as libc::c_int) << 6 as libc::c_int) as u8;
        api_incr_top(L);
        if (*(*L).global).gc.debt() > 0 as libc::c_int as isize {
            luaC_step(L);
        }
    };
}

pub unsafe fn lua_pushboolean(L: *const Thread, b: libc::c_int) {
    if b != 0 {
        (*(*L).top.get()).val.tt_ =
            (1 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
    } else {
        (*(*L).top.get()).val.tt_ =
            (1 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    }
    api_incr_top(L);
}

pub unsafe fn lua_pushthread(L: *mut Thread) {
    let io: *mut TValue = &raw mut (*(*L).top.get()).val;
    let x_: *mut Thread = L;
    (*io).value_.gc = x_ as *mut Object;
    (*io).tt_ = (8 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 6 as libc::c_int) as u8;
    api_incr_top(L);
}

unsafe fn auxgetstr(
    L: *const Thread,
    t: *const TValue,
    k: &[u8],
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut slot: *const TValue = 0 as *const TValue;
    let str: *mut TString = luaS_newlstr((*L).global, k.as_ptr().cast(), k.len());
    if if !((*t).tt_ as libc::c_int
        == 5 as libc::c_int
            | (0 as libc::c_int) << 4 as libc::c_int
            | (1 as libc::c_int) << 6 as libc::c_int)
    {
        slot = 0 as *const TValue;
        0 as libc::c_int
    } else {
        slot = luaH_getstr((*t).value_.gc as *mut Table, str);
        !((*slot).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int) as libc::c_int
    } != 0
    {
        let io1: *mut TValue = &raw mut (*(*L).top.get()).val;
        let io2: *const TValue = slot;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        api_incr_top(L);
    } else {
        let io: *mut TValue = &raw mut (*(*L).top.get()).val;
        let x_: *mut TString = str;
        (*io).value_.gc = x_ as *mut Object;
        (*io).tt_ = ((*x_).hdr.tt as libc::c_int | (1 as libc::c_int) << 6 as libc::c_int) as u8;
        api_incr_top(L);
        luaV_finishget(
            L,
            t,
            &raw mut (*((*L).top.get()).offset(-(1 as libc::c_int as isize))).val,
            ((*L).top.get()).offset(-(1 as libc::c_int as isize)),
            slot,
        )?;
    }

    return Ok((*((*L).top.get()).offset(-(1 as libc::c_int as isize)))
        .val
        .tt_ as libc::c_int
        & 0xf as libc::c_int);
}

pub unsafe fn lua_getglobal(
    L: *mut Thread,
    name: impl AsRef<[u8]>,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut G: *const TValue = 0 as *const TValue;
    G = &mut *((*((*(*(*L).global).l_registry.get()).value_.gc as *mut Table)).array)
        .offset((2 as libc::c_int - 1 as libc::c_int) as isize) as *mut TValue;
    return auxgetstr(L, G, name.as_ref());
}

pub unsafe fn lua_gettable(
    L: *const Thread,
    idx: libc::c_int,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut slot: *const TValue = 0 as *const TValue;
    let mut t: *mut TValue = 0 as *mut TValue;
    t = index2value(L, idx);
    if if !((*t).tt_ as libc::c_int
        == 5 as libc::c_int
            | (0 as libc::c_int) << 4 as libc::c_int
            | (1 as libc::c_int) << 6 as libc::c_int)
    {
        slot = 0 as *const TValue;
        0 as libc::c_int
    } else {
        slot = luaH_get(
            (*t).value_.gc as *mut Table,
            &raw mut (*((*L).top.get()).offset(-(1 as libc::c_int as isize))).val,
        );
        !((*slot).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int) as libc::c_int
    } != 0
    {
        let io1: *mut TValue =
            &raw mut (*((*L).top.get()).offset(-(1 as libc::c_int as isize))).val;
        let io2: *const TValue = slot;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
    } else {
        luaV_finishget(
            L,
            t,
            &raw mut (*((*L).top.get()).offset(-(1 as libc::c_int as isize))).val,
            ((*L).top.get()).offset(-(1 as libc::c_int as isize)),
            slot,
        )?;
    }
    return Ok((*((*L).top.get()).offset(-(1 as libc::c_int as isize)))
        .val
        .tt_ as libc::c_int
        & 0xf as libc::c_int);
}

pub unsafe fn lua_getfield(
    L: *const Thread,
    idx: libc::c_int,
    k: impl AsRef<[u8]>,
) -> Result<c_int, Box<dyn std::error::Error>> {
    return auxgetstr(L, index2value(L, idx), k.as_ref());
}

pub unsafe fn lua_geti(
    L: *const Thread,
    idx: libc::c_int,
    n: i64,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut t: *mut TValue = 0 as *mut TValue;
    let mut slot: *const TValue = 0 as *const TValue;
    t = index2value(L, idx);
    if if !((*t).tt_ as libc::c_int
        == 5 as libc::c_int
            | (0 as libc::c_int) << 4 as libc::c_int
            | (1 as libc::c_int) << 6 as libc::c_int)
    {
        slot = 0 as *const TValue;
        0 as libc::c_int
    } else {
        slot = if (n as u64).wrapping_sub(1 as libc::c_uint as u64)
            < (*((*t).value_.gc as *mut Table)).alimit as u64
        {
            &mut *((*((*t).value_.gc as *mut Table)).array)
                .offset((n - 1 as libc::c_int as i64) as isize) as *mut TValue
                as *const TValue
        } else {
            luaH_getint((*t).value_.gc as *mut Table, n)
        };
        !((*slot).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int) as libc::c_int
    } != 0
    {
        let io1: *mut TValue = &raw mut (*(*L).top.get()).val;
        let io2: *const TValue = slot;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
    } else {
        let mut aux: TValue = TValue {
            value_: Value {
                gc: 0 as *mut Object,
            },
            tt_: 0,
        };
        let io: *mut TValue = &mut aux;
        (*io).value_.i = n;
        (*io).tt_ = (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
        luaV_finishget(L, t, &mut aux, (*L).top.get(), slot)?;
    }
    api_incr_top(L);

    return Ok((*((*L).top.get()).offset(-(1 as libc::c_int as isize)))
        .val
        .tt_ as libc::c_int
        & 0xf as libc::c_int);
}

unsafe fn finishrawget(L: *const Thread, val: *const TValue) -> libc::c_int {
    if (*val).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int {
        (*(*L).top.get()).val.tt_ =
            (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    } else {
        let io1: *mut TValue = &raw mut (*(*L).top.get()).val;
        let io2: *const TValue = val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
    }
    api_incr_top(L);
    return (*((*L).top.get()).offset(-(1 as libc::c_int as isize)))
        .val
        .tt_ as libc::c_int
        & 0xf as libc::c_int;
}

unsafe fn gettable(L: *const Thread, idx: libc::c_int) -> *mut Table {
    let t: *mut TValue = index2value(L, idx);
    return (*t).value_.gc as *mut Table;
}

pub unsafe fn lua_rawget(L: *const Thread, idx: libc::c_int) -> libc::c_int {
    let mut t: *mut Table = 0 as *mut Table;
    let mut val: *const TValue = 0 as *const TValue;
    t = gettable(L, idx);
    val = luaH_get(
        t,
        &raw mut (*((*L).top.get()).offset(-(1 as libc::c_int as isize))).val,
    );
    (*L).top.sub(1);

    return finishrawget(L, val);
}

pub unsafe fn lua_rawgeti(L: *const Thread, idx: libc::c_int, n: i64) -> libc::c_int {
    let mut t: *mut Table = 0 as *mut Table;
    t = gettable(L, idx);
    return finishrawget(L, luaH_getint(t, n));
}

pub unsafe fn lua_createtable(
    L: *const Thread,
    narray: libc::c_int,
    nrec: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut t: *mut Table = 0 as *mut Table;
    t = luaH_new(L)?;
    let io: *mut TValue = &raw mut (*(*L).top.get()).val;
    let x_: *mut Table = t;
    (*io).value_.gc = x_ as *mut Object;
    (*io).tt_ = (5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 6 as libc::c_int) as u8;
    api_incr_top(L);
    if narray > 0 as libc::c_int || nrec > 0 as libc::c_int {
        luaH_resize(L, t, narray as libc::c_uint, nrec as libc::c_uint)?;
    }
    if (*(*L).global).gc.debt() > 0 as libc::c_int as isize {
        luaC_step(L);
    }
    Ok(())
}

pub unsafe fn lua_getmetatable(L: *const Thread, objindex: libc::c_int) -> libc::c_int {
    let mut obj: *const TValue = 0 as *const TValue;
    let mut mt: *mut Table = 0 as *mut Table;
    let mut res: libc::c_int = 0 as libc::c_int;
    obj = index2value(L, objindex);
    match (*obj).tt_ as libc::c_int & 0xf as libc::c_int {
        5 => {
            mt = (*((*obj).value_.gc as *mut Table)).metatable;
        }
        7 => {
            mt = (*((*obj).value_.gc as *mut Udata)).metatable;
        }
        _ => {
            mt = (*(*L).global).mt[((*obj).tt_ & 0xf) as usize].get();
        }
    }
    if !mt.is_null() {
        let io: *mut TValue = &raw mut (*(*L).top.get()).val;
        let x_: *mut Table = mt;
        (*io).value_.gc = x_ as *mut Object;
        (*io).tt_ = (5 as libc::c_int
            | (0 as libc::c_int) << 4 as libc::c_int
            | (1 as libc::c_int) << 6 as libc::c_int) as u8;
        api_incr_top(L);
        res = 1 as libc::c_int;
    }
    return res;
}

pub unsafe fn lua_getiuservalue(L: *mut Thread, idx: libc::c_int, n: libc::c_int) -> libc::c_int {
    let mut o: *mut TValue = 0 as *mut TValue;
    let mut t: libc::c_int = 0;
    o = index2value(L, idx);
    if n <= 0 as libc::c_int || n > (*((*o).value_.gc as *mut Udata)).nuvalue as libc::c_int {
        (*(*L).top.get()).val.tt_ =
            (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
        t = -(1 as libc::c_int);
    } else {
        let io1: *mut TValue = &raw mut (*(*L).top.get()).val;
        let io2: *const TValue = &mut (*((*((*o).value_.gc as *mut Udata)).uv)
            .as_mut_ptr()
            .offset((n - 1 as libc::c_int) as isize))
        .uv;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        t = (*(*L).top.get()).val.tt_ as libc::c_int & 0xf as libc::c_int;
    }
    api_incr_top(L);
    return t;
}

unsafe fn auxsetstr(
    L: *const Thread,
    t: *const TValue,
    k: *const libc::c_char,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut slot: *const TValue = 0 as *const TValue;
    let str: *mut TString = luaS_new((*L).global, k);
    if if !((*t).tt_ as libc::c_int
        == 5 as libc::c_int
            | (0 as libc::c_int) << 4 as libc::c_int
            | (1 as libc::c_int) << 6 as libc::c_int)
    {
        slot = 0 as *const TValue;
        0 as libc::c_int
    } else {
        slot = luaH_getstr((*t).value_.gc as *mut Table, str);
        !((*slot).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int) as libc::c_int
    } != 0
    {
        let io1: *mut TValue = slot as *mut TValue;
        let io2: *const TValue =
            &raw mut (*((*L).top.get()).offset(-(1 as libc::c_int as isize))).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        if (*((*L).top.get()).offset(-(1 as libc::c_int as isize)))
            .val
            .tt_ as libc::c_int
            & (1 as libc::c_int) << 6 as libc::c_int
            != 0
        {
            if (*(*t).value_.gc).marked.get() as libc::c_int
                & (1 as libc::c_int) << 5 as libc::c_int
                != 0
                && (*(*((*L).top.get()).offset(-(1 as libc::c_int as isize)))
                    .val
                    .value_
                    .gc)
                    .marked
                    .get() as libc::c_int
                    & ((1 as libc::c_int) << 3 as libc::c_int
                        | (1 as libc::c_int) << 4 as libc::c_int)
                    != 0
            {
                luaC_barrierback_(L, (*t).value_.gc);
            } else {
            };
        } else {
        };

        (*L).top.sub(1);
    } else {
        let io: *mut TValue = &raw mut (*(*L).top.get()).val;
        let x_: *mut TString = str;
        (*io).value_.gc = x_ as *mut Object;
        (*io).tt_ = ((*x_).hdr.tt as libc::c_int | (1 as libc::c_int) << 6 as libc::c_int) as u8;
        api_incr_top(L);
        luaV_finishset(
            L,
            t,
            &raw mut (*((*L).top.get()).offset(-(1 as libc::c_int as isize))).val,
            &raw mut (*((*L).top.get()).offset(-(2 as libc::c_int as isize))).val,
            slot,
        )?;
        (*L).top.sub(2);
    };
    Ok(())
}

pub unsafe fn lua_setglobal(
    L: *const Thread,
    name: *const libc::c_char,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut G: *const TValue = 0 as *const TValue;
    G = &mut *((*((*(*(*L).global).l_registry.get()).value_.gc as *mut Table)).array)
        .offset((2 as libc::c_int - 1 as libc::c_int) as isize) as *mut TValue;
    auxsetstr(L, G, name)
}

pub unsafe fn lua_settable(
    L: *mut Thread,
    idx: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut t: *mut TValue = 0 as *mut TValue;
    let mut slot: *const TValue = 0 as *const TValue;
    t = index2value(L, idx);
    if if !((*t).tt_ as libc::c_int
        == 5 as libc::c_int
            | (0 as libc::c_int) << 4 as libc::c_int
            | (1 as libc::c_int) << 6 as libc::c_int)
    {
        slot = 0 as *const TValue;
        0 as libc::c_int
    } else {
        slot = luaH_get(
            (*t).value_.gc as *mut Table,
            &raw mut (*((*L).top.get()).offset(-(2 as libc::c_int as isize))).val,
        );
        !((*slot).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int) as libc::c_int
    } != 0
    {
        let io1: *mut TValue = slot as *mut TValue;
        let io2: *const TValue =
            &raw mut (*((*L).top.get()).offset(-(1 as libc::c_int as isize))).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        if (*((*L).top.get()).offset(-(1 as libc::c_int as isize)))
            .val
            .tt_ as libc::c_int
            & (1 as libc::c_int) << 6 as libc::c_int
            != 0
        {
            if (*(*t).value_.gc).marked.get() as libc::c_int
                & (1 as libc::c_int) << 5 as libc::c_int
                != 0
                && (*(*((*L).top.get()).offset(-(1 as libc::c_int as isize)))
                    .val
                    .value_
                    .gc)
                    .marked
                    .get() as libc::c_int
                    & ((1 as libc::c_int) << 3 as libc::c_int
                        | (1 as libc::c_int) << 4 as libc::c_int)
                    != 0
            {
                luaC_barrierback_(L, (*t).value_.gc);
            } else {
            };
        } else {
        };
    } else {
        luaV_finishset(
            L,
            t,
            &raw mut (*((*L).top.get()).offset(-(2 as libc::c_int as isize))).val,
            &raw mut (*((*L).top.get()).offset(-(1 as libc::c_int as isize))).val,
            slot,
        )?;
    }
    (*L).top.sub(2);
    Ok(())
}

pub unsafe fn lua_setfield(
    L: *const Thread,
    idx: libc::c_int,
    k: *const libc::c_char,
) -> Result<(), Box<dyn std::error::Error>> {
    auxsetstr(L, index2value(L, idx), k)
}

pub unsafe fn lua_seti(
    L: *const Thread,
    idx: libc::c_int,
    n: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut t: *mut TValue = 0 as *mut TValue;
    let mut slot: *const TValue = 0 as *const TValue;
    t = index2value(L, idx);
    if if !((*t).tt_ as libc::c_int
        == 5 as libc::c_int
            | (0 as libc::c_int) << 4 as libc::c_int
            | (1 as libc::c_int) << 6 as libc::c_int)
    {
        slot = 0 as *const TValue;
        0 as libc::c_int
    } else {
        slot = if (n as u64).wrapping_sub(1 as libc::c_uint as u64)
            < (*((*t).value_.gc as *mut Table)).alimit as u64
        {
            &mut *((*((*t).value_.gc as *mut Table)).array)
                .offset((n - 1 as libc::c_int as i64) as isize) as *mut TValue
                as *const TValue
        } else {
            luaH_getint((*t).value_.gc as *mut Table, n)
        };
        !((*slot).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int) as libc::c_int
    } != 0
    {
        let io1: *mut TValue = slot as *mut TValue;
        let io2: *const TValue =
            &raw mut (*((*L).top.get()).offset(-(1 as libc::c_int as isize))).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        if (*((*L).top.get()).offset(-(1 as libc::c_int as isize)))
            .val
            .tt_ as libc::c_int
            & (1 as libc::c_int) << 6 as libc::c_int
            != 0
        {
            if (*(*t).value_.gc).marked.get() as libc::c_int
                & (1 as libc::c_int) << 5 as libc::c_int
                != 0
                && (*(*((*L).top.get()).offset(-(1 as libc::c_int as isize)))
                    .val
                    .value_
                    .gc)
                    .marked
                    .get() as libc::c_int
                    & ((1 as libc::c_int) << 3 as libc::c_int
                        | (1 as libc::c_int) << 4 as libc::c_int)
                    != 0
            {
                luaC_barrierback_(L, (*t).value_.gc);
            } else {
            };
        } else {
        };
    } else {
        let mut aux: TValue = TValue {
            value_: Value {
                gc: 0 as *mut Object,
            },
            tt_: 0,
        };
        let io: *mut TValue = &mut aux;
        (*io).value_.i = n;
        (*io).tt_ = (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
        luaV_finishset(
            L,
            t,
            &mut aux,
            &raw mut (*((*L).top.get()).offset(-(1 as libc::c_int as isize))).val,
            slot,
        )?;
    }

    (*L).top.sub(1);

    Ok(())
}

unsafe fn aux_rawset(
    L: *const Thread,
    idx: libc::c_int,
    key: *mut TValue,
    n: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut t: *mut Table = 0 as *mut Table;
    t = gettable(L, idx);
    luaH_set(
        L,
        t,
        key,
        &raw mut (*((*L).top.get()).offset(-(1 as libc::c_int as isize))).val,
    )?;

    (*t).flags
        .set(((*t).flags.get() as libc::c_uint & !!(!(0 as libc::c_uint) << TM_EQ + 1)) as u8);

    if (*((*L).top.get()).offset(-(1 as libc::c_int as isize)))
        .val
        .tt_ as libc::c_int
        & (1 as libc::c_int) << 6 as libc::c_int
        != 0
    {
        if (*t).hdr.marked.get() as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
            && (*(*((*L).top.get()).offset(-(1 as libc::c_int as isize)))
                .val
                .value_
                .gc)
                .marked
                .get() as libc::c_int
                & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
                != 0
        {
            luaC_barrierback_(L, t as *mut Object);
        } else {
        };
    } else {
    };
    (*L).top.sub(n.try_into().unwrap());
    Ok(())
}

pub unsafe fn lua_rawset(
    L: *const Thread,
    idx: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    aux_rawset(
        L,
        idx,
        &raw mut (*((*L).top.get()).offset(-(2 as libc::c_int as isize))).val,
        2 as libc::c_int,
    )
}

pub unsafe fn lua_rawseti(
    L: *mut Thread,
    idx: libc::c_int,
    n: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut t: *mut Table = 0 as *mut Table;
    t = gettable(L, idx);
    luaH_setint(
        L,
        t,
        n,
        &raw mut (*((*L).top.get()).offset(-(1 as libc::c_int as isize))).val,
    )?;
    if (*((*L).top.get()).offset(-(1 as libc::c_int as isize)))
        .val
        .tt_ as libc::c_int
        & (1 as libc::c_int) << 6 as libc::c_int
        != 0
    {
        if (*t).hdr.marked.get() as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
            && (*(*((*L).top.get()).offset(-(1 as libc::c_int as isize)))
                .val
                .value_
                .gc)
                .marked
                .get() as libc::c_int
                & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
                != 0
        {
            luaC_barrierback_(L, t as *mut Object);
        } else {
        };
    } else {
    };

    (*L).top.sub(1);

    Ok(())
}

pub unsafe fn lua_setmetatable(
    L: *const Thread,
    objindex: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let obj = index2value(L, objindex);
    let mt = if (*((*L).top.get()).offset(-1)).val.tt_ & 0xf == 0 {
        0 as *mut Table
    } else {
        (*((*L).top.get()).offset(-1)).val.value_.gc as *mut Table
    };

    // Prevent __gc metamethod.
    let g = (*L).global;

    if !mt.is_null()
        && (*mt).flags.get() & 1 << TM_GC == 0
        && !luaT_gettm(mt, TM_GC, (*g).tmname[TM_GC as usize].get()).is_null()
    {
        return Err("__gc metamethod is not supported".into());
    }

    match (*obj).tt_ & 0xf {
        5 => {
            let ref mut fresh3 = (*((*obj).value_.gc as *mut Table)).metatable;
            *fresh3 = mt;
            if !mt.is_null() {
                if (*(*obj).value_.gc).marked.get() as libc::c_int
                    & (1 as libc::c_int) << 5 as libc::c_int
                    != 0
                    && (*mt).hdr.marked.get() as libc::c_int
                        & ((1 as libc::c_int) << 3 as libc::c_int
                            | (1 as libc::c_int) << 4 as libc::c_int)
                        != 0
                {
                    luaC_barrier_(L, (*obj).value_.gc as *mut Object, mt as *mut Object);
                };
            }
        }
        7 => {
            let ref mut fresh4 = (*((*obj).value_.gc as *mut Udata)).metatable;
            *fresh4 = mt;
            if !mt.is_null() {
                if (*((*obj).value_.gc as *mut Udata)).hdr.marked.get() as libc::c_int
                    & (1 as libc::c_int) << 5 as libc::c_int
                    != 0
                    && (*mt).hdr.marked.get() as libc::c_int
                        & ((1 as libc::c_int) << 3 as libc::c_int
                            | (1 as libc::c_int) << 4 as libc::c_int)
                        != 0
                {
                    luaC_barrier_(
                        L,
                        ((*obj).value_.gc as *mut Udata) as *mut Udata as *mut Object,
                        mt as *mut Object,
                    );
                };
            }
        }
        _ => {
            (*(*L).global).mt[((*obj).tt_ & 0xf) as usize].set(mt);
        }
    }

    (*L).top.sub(1);

    Ok(())
}

pub unsafe fn lua_setiuservalue(L: *mut Thread, idx: libc::c_int, n: libc::c_int) -> libc::c_int {
    let mut o: *mut TValue = 0 as *mut TValue;
    let mut res: libc::c_int = 0;
    o = index2value(L, idx);
    if !((n as libc::c_uint).wrapping_sub(1 as libc::c_uint)
        < (*((*o).value_.gc as *mut Udata)).nuvalue as libc::c_uint)
    {
        res = 0 as libc::c_int;
    } else {
        let io1: *mut TValue = &mut (*((*((*o).value_.gc as *mut Udata)).uv)
            .as_mut_ptr()
            .offset((n - 1 as libc::c_int) as isize))
        .uv;
        let io2: *const TValue =
            &raw mut (*((*L).top.get()).offset(-(1 as libc::c_int as isize))).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        if (*((*L).top.get()).offset(-(1 as libc::c_int as isize)))
            .val
            .tt_ as libc::c_int
            & (1 as libc::c_int) << 6 as libc::c_int
            != 0
        {
            if (*(*o).value_.gc).marked.get() as libc::c_int
                & (1 as libc::c_int) << 5 as libc::c_int
                != 0
                && (*(*((*L).top.get()).offset(-(1 as libc::c_int as isize)))
                    .val
                    .value_
                    .gc)
                    .marked
                    .get() as libc::c_int
                    & ((1 as libc::c_int) << 3 as libc::c_int
                        | (1 as libc::c_int) << 4 as libc::c_int)
                    != 0
            {
                luaC_barrierback_(L, (*o).value_.gc);
            } else {
            };
        } else {
        };
        res = 1 as libc::c_int;
    }

    (*L).top.sub(1);

    return res;
}

pub unsafe fn lua_call(
    L: *const Thread,
    nargs: libc::c_int,
    nresults: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let func = ((*L).top.get()).offset(-((nargs + 1) as isize));

    luaD_call(L, func, nresults)?;

    if nresults <= -1 && (*(*L).ci.get()).top < (*L).top.get() {
        (*(*L).ci.get()).top = (*L).top.get();
    }

    Ok(())
}

pub unsafe fn lua_pcall(
    L: *const Thread,
    nargs: libc::c_int,
    nresults: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let func = ((*L).top.get()).offset(-((nargs + 1) as isize));
    let status = luaD_pcall(
        L,
        func.byte_offset_from_unsigned((*L).stack.get()),
        move |L| luaD_call(L, func, nresults),
    );

    // Adjust current CI.
    if nresults <= -1 && (*(*L).ci.get()).top < (*L).top.get() {
        (*(*L).ci.get()).top = (*L).top.get();
    }

    status
}

pub unsafe fn lua_load(
    L: *const Thread,
    mut name: *const libc::c_char,
    mode: *const libc::c_char,
    chunk: impl AsRef<[u8]>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Load.
    let chunk = chunk.as_ref();
    let z = Zio {
        n: chunk.len(),
        p: chunk.as_ptr().cast(),
    };

    if name.is_null() {
        name = b"?\0" as *const u8 as *const libc::c_char;
    }

    luaD_protectedparser(L, z, name, mode)?;

    let f = (*((*L).top.get()).offset(-(1 as libc::c_int as isize)))
        .val
        .value_
        .gc as *mut LuaClosure;

    if !(*f).upvals.is_empty() {
        let gt: *const TValue =
            ((*((*(*(*L).global).l_registry.get()).value_.gc as *mut Table)).array).offset(2 - 1);
        let io1: *mut TValue = (*(*f).upvals[0].get()).v.get();
        let io2: *const TValue = gt;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;

        if (*gt).tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0 {
            if (*(*f).upvals[0].get()).hdr.marked.get() & 1 << 5 != 0
                && (*(*gt).value_.gc).marked.get() & (1 << 3 | 1 << 4) != 0
            {
                luaC_barrier_(L, (*f).upvals[0].get().cast(), (*gt).value_.gc);
            } else {
            };
        } else {
        };
    }

    Ok(())
}

pub unsafe fn lua_dump(
    L: *const Thread,
    writer: lua_Writer,
    data: *mut c_void,
    strip: c_int,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let o = &raw mut (*((*L).top.get()).offset(-1)).val;

    if (*o).tt_ as libc::c_int
        == 6 as libc::c_int
            | (0 as libc::c_int) << 4 as libc::c_int
            | (1 as libc::c_int) << 6 as libc::c_int
    {
        luaU_dump(
            L,
            (*(*o).value_.gc.cast::<LuaClosure>()).p.get(),
            writer,
            data,
            strip,
        )
    } else {
        Ok(1)
    }
}

pub unsafe fn lua_next(
    L: *const Thread,
    idx: libc::c_int,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut t: *mut Table = 0 as *mut Table;
    let mut more: libc::c_int = 0;
    t = gettable(L, idx);
    more = luaH_next(L, t, ((*L).top.get()).offset(-(1 as libc::c_int as isize)))?;
    if more != 0 {
        api_incr_top(L);
    } else {
        (*L).top.sub(1);
    }
    return Ok(more);
}

pub unsafe fn lua_toclose(
    L: *mut Thread,
    idx: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut nresults: libc::c_int = 0;
    let mut o: StkId = 0 as *mut StackValue;
    o = index2stack(L, idx);
    nresults = (*(*L).ci.get()).nresults as libc::c_int;
    luaF_newtbcupval(L, o)?;
    if !(nresults < -(1 as libc::c_int)) {
        (*(*L).ci.get()).nresults = (-nresults - 3 as libc::c_int) as libc::c_short;
    }
    Ok(())
}

pub unsafe fn lua_concat(L: *const Thread, n: c_int) -> Result<(), Box<dyn std::error::Error>> {
    if n > 0 as libc::c_int {
        luaV_concat(L, n)?;
    } else {
        let io: *mut TValue = &raw mut (*(*L).top.get()).val;
        let x_: *mut TString = luaS_newlstr(
            (*L).global,
            b"\0" as *const u8 as *const libc::c_char,
            0 as libc::c_int as usize,
        );
        (*io).value_.gc = x_ as *mut Object;
        (*io).tt_ = ((*x_).hdr.tt as libc::c_int | (1 as libc::c_int) << 6 as libc::c_int) as u8;
        api_incr_top(L);
    }
    if (*(*L).global).gc.debt() > 0 as libc::c_int as isize {
        luaC_step(L);
    }
    Ok(())
}

pub unsafe fn lua_len(L: *const Thread, idx: c_int) -> Result<(), Box<dyn std::error::Error>> {
    let mut t: *mut TValue = 0 as *mut TValue;
    t = index2value(L, idx);
    luaV_objlen(L, (*L).top.get(), t)?;
    api_incr_top(L);
    Ok(())
}

pub unsafe fn lua_newuserdatauv(
    L: *const Thread,
    size: usize,
    nuvalue: libc::c_int,
) -> Result<*mut c_void, Box<dyn std::error::Error>> {
    let mut u: *mut Udata = 0 as *mut Udata;
    u = luaS_newudata(L, size, nuvalue)?;
    let io: *mut TValue = &raw mut (*(*L).top.get()).val;
    let x_: *mut Udata = u;
    (*io).value_.gc = x_ as *mut Object;
    (*io).tt_ = (7 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 6 as libc::c_int) as u8;
    api_incr_top(L);
    if (*(*L).global).gc.debt() > 0 as libc::c_int as isize {
        luaC_step(L);
    }

    Ok(
        u.byte_add(offset_of!(Udata, uv) + size_of::<UValue>() * usize::from((*u).nuvalue))
            .cast(),
    )
}

unsafe fn aux_upvalue(
    fi: *mut TValue,
    n: libc::c_int,
    val: *mut *mut TValue,
    owner: *mut *mut Object,
) -> *const libc::c_char {
    match (*fi).tt_ as libc::c_int & 0x3f as libc::c_int {
        38 => {
            let f: *mut CClosure = (*fi).value_.gc as *mut CClosure;
            if !((n as libc::c_uint).wrapping_sub(1 as libc::c_uint)
                < (*f).nupvalues as libc::c_uint)
            {
                return 0 as *const libc::c_char;
            }
            *val = &mut *((*f).upvalue)
                .as_mut_ptr()
                .offset((n - 1 as libc::c_int) as isize) as *mut TValue;
            if !owner.is_null() {
                *owner = f as *mut Object;
            }
            return b"\0" as *const u8 as *const libc::c_char;
        }
        6 => {
            let f_0: *mut LuaClosure = (*fi).value_.gc as *mut LuaClosure;
            let mut name: *mut TString = 0 as *mut TString;
            let p: *mut Proto = (*f_0).p.get();

            if !((n as libc::c_uint).wrapping_sub(1 as libc::c_uint)
                < (*p).sizeupvalues as libc::c_uint)
            {
                return 0 as *const libc::c_char;
            }

            *val = (*(*f_0).upvals[(n - 1) as usize].get()).v.get();

            if !owner.is_null() {
                *owner = (*f_0).upvals[(n - 1) as usize].get().cast();
            }

            name = (*((*p).upvalues).offset((n - 1 as libc::c_int) as isize)).name;
            return if name.is_null() {
                b"(no name)\0" as *const u8 as *const libc::c_char
            } else {
                ((*name).contents).as_mut_ptr() as *const libc::c_char
            };
        }
        _ => return 0 as *const libc::c_char,
    };
}

pub unsafe fn lua_getupvalue(
    L: *mut Thread,
    funcindex: libc::c_int,
    n: libc::c_int,
) -> *const libc::c_char {
    let mut name: *const libc::c_char = 0 as *const libc::c_char;
    let mut val: *mut TValue = 0 as *mut TValue;
    name = aux_upvalue(
        index2value(L, funcindex),
        n,
        &mut val,
        0 as *mut *mut Object,
    );
    if !name.is_null() {
        let io1: *mut TValue = &raw mut (*(*L).top.get()).val;
        let io2: *const TValue = val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        api_incr_top(L);
    }
    return name;
}

pub unsafe fn lua_setupvalue(
    L: *const Thread,
    funcindex: libc::c_int,
    n: libc::c_int,
) -> *const libc::c_char {
    let mut name: *const libc::c_char = 0 as *const libc::c_char;
    let mut val: *mut TValue = 0 as *mut TValue;
    let mut owner: *mut Object = 0 as *mut Object;
    let mut fi: *mut TValue = 0 as *mut TValue;
    fi = index2value(L, funcindex);
    name = aux_upvalue(fi, n, &mut val, &mut owner);
    if !name.is_null() {
        (*L).top.sub(1);

        let io1: *mut TValue = val;
        let io2: *const TValue = &raw mut (*(*L).top.get()).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        if (*val).tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0 {
            if (*owner).marked.get() as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
                && (*(*val).value_.gc).marked.get() as libc::c_int
                    & ((1 as libc::c_int) << 3 as libc::c_int
                        | (1 as libc::c_int) << 4 as libc::c_int)
                    != 0
            {
                luaC_barrier_(L, owner as *mut Object, (*val).value_.gc as *mut Object);
            } else {
            };
        } else {
        };
    }
    return name;
}

unsafe fn getupvalref(
    L: *mut Thread,
    fidx: libc::c_int,
    n: libc::c_int,
    pf: *mut *const LuaClosure,
) -> *mut *mut UpVal {
    static mut nullup: *const UpVal = 0 as *const UpVal;
    let fi: *mut TValue = index2value(L, fidx);
    let f = (*fi).value_.gc.cast::<LuaClosure>();

    if !pf.is_null() {
        *pf = f;
    }

    if 1 as libc::c_int <= n && n <= (*(*f).p.get()).sizeupvalues {
        return (*f).upvals[(n - 1) as usize].as_ptr();
    } else {
        return &raw const nullup as *const *const UpVal as *mut *mut UpVal;
    };
}

pub unsafe fn lua_upvalueid(
    L: *mut Thread,
    fidx: libc::c_int,
    n: libc::c_int,
) -> *mut libc::c_void {
    let fi: *mut TValue = index2value(L, fidx);

    match (*fi).tt_ as libc::c_int & 0x3f as libc::c_int {
        6 => return *getupvalref(L, fidx, n, null_mut()).cast(),
        38 => {
            let f: *mut CClosure = (*fi).value_.gc as *mut CClosure;
            if 1 as libc::c_int <= n && n <= (*f).nupvalues as libc::c_int {
                return &mut *((*f).upvalue)
                    .as_mut_ptr()
                    .offset((n - 1 as libc::c_int) as isize) as *mut TValue
                    as *mut libc::c_void;
            }
        }
        22 => {}
        _ => return 0 as *mut libc::c_void,
    }

    return 0 as *mut libc::c_void;
}

pub unsafe fn lua_upvaluejoin(
    L: *mut Thread,
    fidx1: libc::c_int,
    n1: libc::c_int,
    fidx2: libc::c_int,
    n2: libc::c_int,
) {
    let mut f1 = 0 as *const LuaClosure;
    let up1: *mut *mut UpVal = getupvalref(L, fidx1, n1, &mut f1);
    let up2: *mut *mut UpVal = getupvalref(L, fidx2, n2, null_mut());

    *up1 = *up2;

    if (*f1).hdr.marked.get() as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
        && (**up1).hdr.marked.get() as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            != 0
    {
        luaC_barrier_(L, f1 as *mut Object, *up1 as *mut Object);
    } else {
    };
}
