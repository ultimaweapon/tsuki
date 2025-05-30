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
    CClosure, GCObject, LClosure, Proto, StackValue, StkId, TString, TValue, Table, UValue, Udata,
    UpVal, Value, luaO_arith, luaO_str2num, luaO_tostring,
};
use crate::lstate::{CallInfo, lua_CFunction, lua_Reader, lua_Writer};
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
use crate::{Thread, api_incr_top};
use std::ffi::{c_int, c_void};
use std::mem::offset_of;

#[derive(Copy, Clone)]
#[repr(C)]
pub struct CallS {
    pub func: StkId,
    pub nresults: libc::c_int,
}

unsafe fn index2value(L: *mut Thread, mut idx: libc::c_int) -> *mut TValue {
    let ci: *mut CallInfo = (*L).ci.get();
    if idx > 0 as libc::c_int {
        let o: StkId = ((*ci).func).offset(idx as isize);
        if o >= (*L).top {
            return (*(*L).global).nilvalue.get();
        } else {
            return &mut (*o).val;
        }
    } else if !(idx <= -(1000000 as libc::c_int) - 1000 as libc::c_int) {
        return &mut (*((*L).top).offset(idx as isize)).val;
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

unsafe fn index2stack(L: *mut Thread, idx: libc::c_int) -> StkId {
    let ci: *mut CallInfo = (*L).ci.get();
    if idx > 0 as libc::c_int {
        let o: StkId = ((*ci).func).offset(idx as isize);
        return o;
    } else {
        return ((*L).top).offset(idx as isize);
    };
}

pub unsafe fn lua_checkstack(L: *mut Thread, n: usize) -> Result<(), Box<dyn std::error::Error>> {
    let ci = (*L).ci.get();

    if ((*L).stack_last.get()).offset_from_unsigned((*L).top) > n {
    } else {
        luaD_growstack(L, n)?;
    }

    if (*ci).top < ((*L).top).add(n) {
        (*ci).top = ((*L).top).add(n);
    }

    Ok(())
}

pub unsafe fn lua_xmove(from: *mut Thread, to: *mut Thread, n: libc::c_int) {
    let mut i: libc::c_int = 0;
    if from == to {
        return;
    }
    (*from).top = ((*from).top).offset(-(n as isize));
    i = 0 as libc::c_int;
    while i < n {
        let io1: *mut TValue = &raw mut (*(*to).top).val;
        let io2: *const TValue = &raw mut (*((*from).top).offset(i as isize)).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        (*to).top = ((*to).top).offset(1);

        i += 1;
    }
}

pub unsafe fn lua_absindex(L: *mut Thread, idx: libc::c_int) -> libc::c_int {
    return if idx > 0 as libc::c_int || idx <= -(1000000 as libc::c_int) - 1000 as libc::c_int {
        idx
    } else {
        ((*L).top).offset_from((*(*L).ci.get()).func) as libc::c_long as libc::c_int + idx
    };
}

pub unsafe fn lua_gettop(L: *mut Thread) -> libc::c_int {
    return ((*L).top).offset_from(((*(*L).ci.get()).func).offset(1 as libc::c_int as isize))
        as libc::c_long as libc::c_int;
}

pub unsafe fn lua_settop(
    L: *mut Thread,
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
            .offset_from((*L).top);
        while diff > 0 as libc::c_int as isize {
            let fresh1 = (*L).top;
            (*L).top = ((*L).top).offset(1);
            (*fresh1).val.tt_ = (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
            diff -= 1;
        }
    } else {
        diff = (idx + 1 as libc::c_int) as isize;
    }

    newtop = ((*L).top).offset(diff as isize);

    if diff < 0 as libc::c_int as isize && (*L).tbclist.get() >= newtop {
        newtop = luaF_close(L, newtop)?;
    }

    (*L).top = newtop;

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
                gc: 0 as *mut GCObject,
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

pub unsafe fn lua_rotate(L: *mut Thread, idx: libc::c_int, n: libc::c_int) {
    let mut p: StkId = 0 as *mut StackValue;
    let mut t: StkId = 0 as *mut StackValue;
    let mut m: StkId = 0 as *mut StackValue;
    t = ((*L).top).offset(-(1 as libc::c_int as isize));
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

pub unsafe fn lua_copy(L: *mut Thread, fromidx: libc::c_int, toidx: libc::c_int) {
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
            if (*((*(*(*L).ci.get()).func).val.value_.gc as *mut CClosure)).marked as libc::c_int
                & (1 as libc::c_int) << 5 as libc::c_int
                != 0
                && (*(*fr).value_.gc).marked as libc::c_int
                    & ((1 as libc::c_int) << 3 as libc::c_int
                        | (1 as libc::c_int) << 4 as libc::c_int)
                    != 0
            {
                luaC_barrier_(
                    L,
                    ((*(*(*L).ci.get()).func).val.value_.gc as *mut CClosure) as *mut CClosure
                        as *mut GCObject,
                    (*fr).value_.gc as *mut GCObject,
                );
            } else {
            };
        } else {
        };
    }
}

pub unsafe fn lua_pushvalue(L: *mut Thread, idx: libc::c_int) {
    let io1: *mut TValue = &raw mut (*(*L).top).val;
    let io2: *const TValue = index2value(L, idx);
    (*io1).value_ = (*io2).value_;
    (*io1).tt_ = (*io2).tt_;
    api_incr_top(L);
}

pub unsafe fn lua_type(L: *mut Thread, idx: libc::c_int) -> libc::c_int {
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

pub unsafe fn lua_isinteger(L: *mut Thread, idx: libc::c_int) -> libc::c_int {
    let o: *const TValue = index2value(L, idx);
    return ((*o).tt_ as libc::c_int == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int)
        as libc::c_int;
}

pub unsafe fn lua_isnumber(L: *mut Thread, idx: libc::c_int) -> libc::c_int {
    let mut n: f64 = 0.;
    let o: *const TValue = index2value(L, idx);
    return if (*o).tt_ as libc::c_int == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int {
        n = (*o).value_.n;
        1 as libc::c_int
    } else {
        luaV_tonumber_(o, &mut n)
    };
}

pub unsafe fn lua_isstring(L: *mut Thread, idx: libc::c_int) -> libc::c_int {
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
    L: *mut Thread,
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

pub unsafe fn lua_arith(L: *mut Thread, op: libc::c_int) -> Result<(), Box<dyn std::error::Error>> {
    if !(op != 12 as libc::c_int && op != 13 as libc::c_int) {
        let io1: *mut TValue = &raw mut (*(*L).top).val;
        let io2: *const TValue = &raw mut (*((*L).top).offset(-(1 as libc::c_int as isize))).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        api_incr_top(L);
    }
    luaO_arith(
        L,
        op,
        &raw mut (*((*L).top).offset(-(2 as libc::c_int as isize))).val,
        &raw mut (*((*L).top).offset(-(1 as libc::c_int as isize))).val,
        ((*L).top).offset(-(2 as libc::c_int as isize)),
    )?;
    (*L).top = ((*L).top).offset(-1);

    Ok(())
}

pub unsafe fn lua_compare(
    L: *mut Thread,
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

pub unsafe fn lua_stringtonumber(L: *mut Thread, s: *const libc::c_char) -> usize {
    let sz: usize = luaO_str2num(s, &raw mut (*(*L).top).val);
    if sz != 0 as libc::c_int as usize {
        api_incr_top(L);
    }
    return sz;
}

pub unsafe fn lua_tonumberx(L: *mut Thread, idx: libc::c_int, pisnum: *mut libc::c_int) -> f64 {
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

pub unsafe fn lua_tointegerx(L: *mut Thread, idx: libc::c_int, pisnum: *mut libc::c_int) -> i64 {
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

pub unsafe fn lua_toboolean(L: *mut Thread, idx: libc::c_int) -> libc::c_int {
    let o: *const TValue = index2value(L, idx);
    return !((*o).tt_ as libc::c_int == 1 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
        || (*o).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int)
        as libc::c_int;
}

pub unsafe fn lua_tolstring(
    L: *mut Thread,
    idx: libc::c_int,
    len: *mut usize,
) -> Result<*const libc::c_char, Box<dyn std::error::Error>> {
    let mut o: *mut TValue = 0 as *mut TValue;
    o = index2value(L, idx);
    if !((*o).tt_ as libc::c_int & 0xf as libc::c_int == 4 as libc::c_int) {
        if !((*o).tt_ as libc::c_int & 0xf as libc::c_int == 3 as libc::c_int) {
            if !len.is_null() {
                *len = 0 as libc::c_int as usize;
            }
            return Ok(0 as *const libc::c_char);
        }
        luaO_tostring(L, o)?;
        if (*(*L).global).gc.debt() > 0 as libc::c_int as isize {
            luaC_step(L);
        }
        o = index2value(L, idx);
    }
    if !len.is_null() {
        *len = if (*((*o).value_.gc as *mut TString)).shrlen as libc::c_int != 0xff as libc::c_int {
            (*((*o).value_.gc as *mut TString)).shrlen as usize
        } else {
            (*((*o).value_.gc as *mut TString)).u.lnglen
        };
    }
    return Ok(((*((*o).value_.gc as *mut TString)).contents).as_mut_ptr());
}

pub unsafe fn lua_rawlen(L: *mut Thread, idx: libc::c_int) -> u64 {
    let o: *const TValue = index2value(L, idx);
    match (*o).tt_ as libc::c_int & 0x3f as libc::c_int {
        4 => return (*((*o).value_.gc as *mut TString)).shrlen as u64,
        20 => return (*((*o).value_.gc as *mut TString)).u.lnglen as u64,
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
        7 => {
            return ((*o).value_.gc as *mut libc::c_char).offset(
                (if (*((*o).value_.gc as *mut Udata)).nuvalue == 0 {
                    offset_of!(Udata, gclist)
                } else {
                    offset_of!(UValue, uv)
                        + size_of::<UValue>()
                            .wrapping_mul((*((*o).value_.gc as *mut Udata)).nuvalue.into())
                }) as isize,
            ) as *mut libc::c_void;
        }
        2 => return (*o).value_.p,
        _ => return 0 as *mut libc::c_void,
    };
}

pub unsafe fn lua_touserdata(L: *mut Thread, idx: libc::c_int) -> *mut libc::c_void {
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

pub unsafe fn lua_topointer(L: *mut Thread, idx: libc::c_int) -> *const libc::c_void {
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

pub unsafe fn lua_pushnil(L: *mut Thread) {
    (*(*L).top).val.tt_ = (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    api_incr_top(L);
}

pub unsafe fn lua_pushnumber(L: *mut Thread, n: f64) {
    let io: *mut TValue = &raw mut (*(*L).top).val;
    (*io).value_.n = n;
    (*io).tt_ = (3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
    api_incr_top(L);
}

pub unsafe fn lua_pushinteger(L: *mut Thread, n: i64) {
    let io: *mut TValue = &raw mut (*(*L).top).val;
    (*io).value_.i = n;
    (*io).tt_ = (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    api_incr_top(L);
}

pub unsafe fn lua_pushlstring(
    L: *mut Thread,
    s: impl AsRef<[u8]>,
) -> Result<*const libc::c_char, Box<dyn std::error::Error>> {
    let s = s.as_ref();
    let mut ts: *mut TString = 0 as *mut TString;
    ts = if s.is_empty() {
        luaS_new(L, b"\0" as *const u8 as *const libc::c_char)?
    } else {
        luaS_newlstr(L, s.as_ptr().cast(), s.len() as _)?
    };
    let io: *mut TValue = &raw mut (*(*L).top).val;
    let x_: *mut TString = ts;
    (*io).value_.gc = x_ as *mut GCObject;
    (*io).tt_ = ((*x_).tt as libc::c_int | (1 as libc::c_int) << 6 as libc::c_int) as u8;
    api_incr_top(L);
    if (*(*L).global).gc.debt() > 0 as libc::c_int as isize {
        luaC_step(L);
    }
    return Ok(((*ts).contents).as_mut_ptr());
}

pub unsafe fn lua_pushstring(
    L: *mut Thread,
    mut s: *const libc::c_char,
) -> Result<*const libc::c_char, Box<dyn std::error::Error>> {
    if s.is_null() {
        (*(*L).top).val.tt_ = (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    } else {
        let mut ts: *mut TString = 0 as *mut TString;
        ts = luaS_new(L, s)?;
        let io: *mut TValue = &raw mut (*(*L).top).val;
        let x_: *mut TString = ts;
        (*io).value_.gc = x_ as *mut GCObject;
        (*io).tt_ = ((*x_).tt as libc::c_int | (1 as libc::c_int) << 6 as libc::c_int) as u8;
        s = ((*ts).contents).as_mut_ptr();
    }
    api_incr_top(L);
    if (*(*L).global).gc.debt() > 0 as libc::c_int as isize {
        luaC_step(L);
    }
    return Ok(s);
}

pub unsafe fn lua_pushcclosure(L: *mut Thread, fn_0: lua_CFunction, mut n: libc::c_int) {
    if n == 0 as libc::c_int {
        let io: *mut TValue = &raw mut (*(*L).top).val;
        (*io).value_.f = fn_0;
        (*io).tt_ = (6 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
        api_incr_top(L);
    } else {
        let mut cl: *mut CClosure = 0 as *mut CClosure;
        cl = luaF_newCclosure(L, n);
        (*cl).f = fn_0;
        (*L).top = ((*L).top).offset(-(n as isize));
        loop {
            let fresh2 = n;
            n = n - 1;
            if !(fresh2 != 0) {
                break;
            }
            let io1: *mut TValue =
                &raw mut *((*cl).upvalue).as_mut_ptr().offset(n as isize) as *mut TValue;
            let io2: *const TValue = &raw mut (*((*L).top).offset(n as isize)).val;
            (*io1).value_ = (*io2).value_;
            (*io1).tt_ = (*io2).tt_;
        }
        let io_0: *mut TValue = &raw mut (*(*L).top).val;
        let x_: *mut CClosure = cl;
        (*io_0).value_.gc = x_ as *mut GCObject;
        (*io_0).tt_ = (6 as libc::c_int
            | (2 as libc::c_int) << 4 as libc::c_int
            | (1 as libc::c_int) << 6 as libc::c_int) as u8;
        api_incr_top(L);
        if (*(*L).global).gc.debt() > 0 as libc::c_int as isize {
            luaC_step(L);
        }
    };
}

pub unsafe fn lua_pushboolean(L: *mut Thread, b: libc::c_int) {
    if b != 0 {
        (*(*L).top).val.tt_ = (1 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
    } else {
        (*(*L).top).val.tt_ = (1 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    }
    api_incr_top(L);
}

pub unsafe fn lua_pushlightuserdata(L: *mut Thread, p: *mut libc::c_void) {
    let io: *mut TValue = &raw mut (*(*L).top).val;
    (*io).value_.p = p;
    (*io).tt_ = (2 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    api_incr_top(L);
}

pub unsafe fn lua_pushthread(L: *mut Thread) {
    let io: *mut TValue = &raw mut (*(*L).top).val;
    let x_: *mut Thread = L;
    (*io).value_.gc = x_ as *mut GCObject;
    (*io).tt_ = (8 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 6 as libc::c_int) as u8;
    api_incr_top(L);
}

unsafe fn auxgetstr(
    L: *mut Thread,
    t: *const TValue,
    k: &[u8],
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut slot: *const TValue = 0 as *const TValue;
    let str: *mut TString = luaS_newlstr(L, k.as_ptr().cast(), k.len())?;
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
        let io1: *mut TValue = &raw mut (*(*L).top).val;
        let io2: *const TValue = slot;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        api_incr_top(L);
    } else {
        let io: *mut TValue = &raw mut (*(*L).top).val;
        let x_: *mut TString = str;
        (*io).value_.gc = x_ as *mut GCObject;
        (*io).tt_ = ((*x_).tt as libc::c_int | (1 as libc::c_int) << 6 as libc::c_int) as u8;
        api_incr_top(L);
        luaV_finishget(
            L,
            t,
            &raw mut (*((*L).top).offset(-(1 as libc::c_int as isize))).val,
            ((*L).top).offset(-(1 as libc::c_int as isize)),
            slot,
        )?;
    }

    return Ok(
        (*((*L).top).offset(-(1 as libc::c_int as isize))).val.tt_ as libc::c_int
            & 0xf as libc::c_int,
    );
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
    L: *mut Thread,
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
            &raw mut (*((*L).top).offset(-(1 as libc::c_int as isize))).val,
        );
        !((*slot).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int) as libc::c_int
    } != 0
    {
        let io1: *mut TValue = &raw mut (*((*L).top).offset(-(1 as libc::c_int as isize))).val;
        let io2: *const TValue = slot;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
    } else {
        luaV_finishget(
            L,
            t,
            &raw mut (*((*L).top).offset(-(1 as libc::c_int as isize))).val,
            ((*L).top).offset(-(1 as libc::c_int as isize)),
            slot,
        )?;
    }
    return Ok(
        (*((*L).top).offset(-(1 as libc::c_int as isize))).val.tt_ as libc::c_int
            & 0xf as libc::c_int,
    );
}

pub unsafe fn lua_getfield(
    L: *mut Thread,
    idx: libc::c_int,
    k: impl AsRef<[u8]>,
) -> Result<c_int, Box<dyn std::error::Error>> {
    return auxgetstr(L, index2value(L, idx), k.as_ref());
}

pub unsafe fn lua_geti(
    L: *mut Thread,
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
        let io1: *mut TValue = &raw mut (*(*L).top).val;
        let io2: *const TValue = slot;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
    } else {
        let mut aux: TValue = TValue {
            value_: Value {
                gc: 0 as *mut GCObject,
            },
            tt_: 0,
        };
        let io: *mut TValue = &mut aux;
        (*io).value_.i = n;
        (*io).tt_ = (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
        luaV_finishget(L, t, &mut aux, (*L).top, slot)?;
    }
    api_incr_top(L);

    return Ok(
        (*((*L).top).offset(-(1 as libc::c_int as isize))).val.tt_ as libc::c_int
            & 0xf as libc::c_int,
    );
}

unsafe fn finishrawget(L: *mut Thread, val: *const TValue) -> libc::c_int {
    if (*val).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int {
        (*(*L).top).val.tt_ = (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    } else {
        let io1: *mut TValue = &raw mut (*(*L).top).val;
        let io2: *const TValue = val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
    }
    api_incr_top(L);
    return (*((*L).top).offset(-(1 as libc::c_int as isize))).val.tt_ as libc::c_int
        & 0xf as libc::c_int;
}

unsafe fn gettable(L: *mut Thread, idx: libc::c_int) -> *mut Table {
    let t: *mut TValue = index2value(L, idx);
    return (*t).value_.gc as *mut Table;
}

pub unsafe fn lua_rawget(L: *mut Thread, idx: libc::c_int) -> libc::c_int {
    let mut t: *mut Table = 0 as *mut Table;
    let mut val: *const TValue = 0 as *const TValue;
    t = gettable(L, idx);
    val = luaH_get(
        t,
        &raw mut (*((*L).top).offset(-(1 as libc::c_int as isize))).val,
    );
    (*L).top = ((*L).top).offset(-1);

    return finishrawget(L, val);
}

pub unsafe fn lua_rawgeti(L: *mut Thread, idx: libc::c_int, n: i64) -> libc::c_int {
    let mut t: *mut Table = 0 as *mut Table;
    t = gettable(L, idx);
    return finishrawget(L, luaH_getint(t, n));
}

pub unsafe fn lua_rawgetp(L: *mut Thread, idx: libc::c_int, p: *const libc::c_void) -> libc::c_int {
    let mut t: *mut Table = 0 as *mut Table;
    let mut k: TValue = TValue {
        value_: Value {
            gc: 0 as *mut GCObject,
        },
        tt_: 0,
    };
    t = gettable(L, idx);
    let io: *mut TValue = &mut k;
    (*io).value_.p = p as *mut libc::c_void;
    (*io).tt_ = (2 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    return finishrawget(L, luaH_get(t, &mut k));
}

pub unsafe fn lua_createtable(
    L: *mut Thread,
    narray: libc::c_int,
    nrec: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut t: *mut Table = 0 as *mut Table;
    t = luaH_new(L)?;
    let io: *mut TValue = &raw mut (*(*L).top).val;
    let x_: *mut Table = t;
    (*io).value_.gc = x_ as *mut GCObject;
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

pub unsafe fn lua_getmetatable(L: *mut Thread, objindex: libc::c_int) -> libc::c_int {
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
        let io: *mut TValue = &raw mut (*(*L).top).val;
        let x_: *mut Table = mt;
        (*io).value_.gc = x_ as *mut GCObject;
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
        (*(*L).top).val.tt_ = (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
        t = -(1 as libc::c_int);
    } else {
        let io1: *mut TValue = &raw mut (*(*L).top).val;
        let io2: *const TValue = &mut (*((*((*o).value_.gc as *mut Udata)).uv)
            .as_mut_ptr()
            .offset((n - 1 as libc::c_int) as isize))
        .uv;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        t = (*(*L).top).val.tt_ as libc::c_int & 0xf as libc::c_int;
    }
    api_incr_top(L);
    return t;
}

unsafe fn auxsetstr(
    L: *mut Thread,
    t: *const TValue,
    k: *const libc::c_char,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut slot: *const TValue = 0 as *const TValue;
    let str: *mut TString = luaS_new(L, k)?;
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
        let io2: *const TValue = &raw mut (*((*L).top).offset(-(1 as libc::c_int as isize))).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        if (*((*L).top).offset(-(1 as libc::c_int as isize))).val.tt_ as libc::c_int
            & (1 as libc::c_int) << 6 as libc::c_int
            != 0
        {
            if (*(*t).value_.gc).marked as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
                && (*(*((*L).top).offset(-(1 as libc::c_int as isize)))
                    .val
                    .value_
                    .gc)
                    .marked as libc::c_int
                    & ((1 as libc::c_int) << 3 as libc::c_int
                        | (1 as libc::c_int) << 4 as libc::c_int)
                    != 0
            {
                luaC_barrierback_(L, (*t).value_.gc);
            } else {
            };
        } else {
        };

        (*L).top = ((*L).top).offset(-1);
    } else {
        let io: *mut TValue = &raw mut (*(*L).top).val;
        let x_: *mut TString = str;
        (*io).value_.gc = x_ as *mut GCObject;
        (*io).tt_ = ((*x_).tt as libc::c_int | (1 as libc::c_int) << 6 as libc::c_int) as u8;
        api_incr_top(L);
        luaV_finishset(
            L,
            t,
            &raw mut (*((*L).top).offset(-(1 as libc::c_int as isize))).val,
            &raw mut (*((*L).top).offset(-(2 as libc::c_int as isize))).val,
            slot,
        )?;
        (*L).top = ((*L).top).offset(-(2 as libc::c_int as isize));
    };
    Ok(())
}

pub unsafe fn lua_setglobal(
    L: *mut Thread,
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
            &raw mut (*((*L).top).offset(-(2 as libc::c_int as isize))).val,
        );
        !((*slot).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int) as libc::c_int
    } != 0
    {
        let io1: *mut TValue = slot as *mut TValue;
        let io2: *const TValue = &raw mut (*((*L).top).offset(-(1 as libc::c_int as isize))).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        if (*((*L).top).offset(-(1 as libc::c_int as isize))).val.tt_ as libc::c_int
            & (1 as libc::c_int) << 6 as libc::c_int
            != 0
        {
            if (*(*t).value_.gc).marked as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
                && (*(*((*L).top).offset(-(1 as libc::c_int as isize)))
                    .val
                    .value_
                    .gc)
                    .marked as libc::c_int
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
            &raw mut (*((*L).top).offset(-(2 as libc::c_int as isize))).val,
            &raw mut (*((*L).top).offset(-(1 as libc::c_int as isize))).val,
            slot,
        )?;
    }
    (*L).top = ((*L).top).offset(-(2 as libc::c_int as isize));
    Ok(())
}

pub unsafe fn lua_setfield(
    L: *mut Thread,
    idx: libc::c_int,
    k: *const libc::c_char,
) -> Result<(), Box<dyn std::error::Error>> {
    auxsetstr(L, index2value(L, idx), k)
}

pub unsafe fn lua_seti(
    L: *mut Thread,
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
        let io2: *const TValue = &raw mut (*((*L).top).offset(-(1 as libc::c_int as isize))).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        if (*((*L).top).offset(-(1 as libc::c_int as isize))).val.tt_ as libc::c_int
            & (1 as libc::c_int) << 6 as libc::c_int
            != 0
        {
            if (*(*t).value_.gc).marked as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
                && (*(*((*L).top).offset(-(1 as libc::c_int as isize)))
                    .val
                    .value_
                    .gc)
                    .marked as libc::c_int
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
                gc: 0 as *mut GCObject,
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
            &raw mut (*((*L).top).offset(-(1 as libc::c_int as isize))).val,
            slot,
        )?;
    }

    (*L).top = ((*L).top).offset(-1);

    Ok(())
}

unsafe fn aux_rawset(
    L: *mut Thread,
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
        &raw mut (*((*L).top).offset(-(1 as libc::c_int as isize))).val,
    )?;
    (*t).flags = ((*t).flags as libc::c_uint
        & !!(!(0 as libc::c_uint) << TM_EQ as libc::c_int + 1 as libc::c_int))
        as u8;
    if (*((*L).top).offset(-(1 as libc::c_int as isize))).val.tt_ as libc::c_int
        & (1 as libc::c_int) << 6 as libc::c_int
        != 0
    {
        if (*t).marked as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
            && (*(*((*L).top).offset(-(1 as libc::c_int as isize)))
                .val
                .value_
                .gc)
                .marked as libc::c_int
                & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
                != 0
        {
            luaC_barrierback_(L, t as *mut GCObject);
        } else {
        };
    } else {
    };
    (*L).top = ((*L).top).offset(-(n as isize));
    Ok(())
}

pub unsafe fn lua_rawset(
    L: *mut Thread,
    idx: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    aux_rawset(
        L,
        idx,
        &raw mut (*((*L).top).offset(-(2 as libc::c_int as isize))).val,
        2 as libc::c_int,
    )
}

pub unsafe fn lua_rawsetp(
    L: *mut Thread,
    idx: libc::c_int,
    p: *const libc::c_void,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut k: TValue = TValue {
        value_: Value {
            gc: 0 as *mut GCObject,
        },
        tt_: 0,
    };
    let io: *mut TValue = &mut k;
    (*io).value_.p = p as *mut libc::c_void;
    (*io).tt_ = (2 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    aux_rawset(L, idx, &mut k, 1 as libc::c_int)
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
        &raw mut (*((*L).top).offset(-(1 as libc::c_int as isize))).val,
    )?;
    if (*((*L).top).offset(-(1 as libc::c_int as isize))).val.tt_ as libc::c_int
        & (1 as libc::c_int) << 6 as libc::c_int
        != 0
    {
        if (*t).marked as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
            && (*(*((*L).top).offset(-(1 as libc::c_int as isize)))
                .val
                .value_
                .gc)
                .marked as libc::c_int
                & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
                != 0
        {
            luaC_barrierback_(L, t as *mut GCObject);
        } else {
        };
    } else {
    };

    (*L).top = ((*L).top).offset(-1);

    Ok(())
}

pub unsafe fn lua_setmetatable(
    L: *mut Thread,
    objindex: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let obj = index2value(L, objindex);
    let mt = if (*((*L).top).offset(-1)).val.tt_ & 0xf == 0 {
        0 as *mut Table
    } else {
        (*((*L).top).offset(-1)).val.value_.gc as *mut Table
    };

    // Prevent __gc metamethod.
    let g = (*L).global;

    if !mt.is_null()
        && (*mt).flags & 1 << TM_GC == 0
        && !luaT_gettm(mt, TM_GC, (*g).tmname[TM_GC as usize].get()).is_null()
    {
        return Err("__gc metamethod is not supported".into());
    }

    match (*obj).tt_ & 0xf {
        5 => {
            let ref mut fresh3 = (*((*obj).value_.gc as *mut Table)).metatable;
            *fresh3 = mt;
            if !mt.is_null() {
                if (*(*obj).value_.gc).marked as libc::c_int
                    & (1 as libc::c_int) << 5 as libc::c_int
                    != 0
                    && (*mt).marked as libc::c_int
                        & ((1 as libc::c_int) << 3 as libc::c_int
                            | (1 as libc::c_int) << 4 as libc::c_int)
                        != 0
                {
                    luaC_barrier_(L, (*obj).value_.gc as *mut GCObject, mt as *mut GCObject);
                };
            }
        }
        7 => {
            let ref mut fresh4 = (*((*obj).value_.gc as *mut Udata)).metatable;
            *fresh4 = mt;
            if !mt.is_null() {
                if (*((*obj).value_.gc as *mut Udata)).marked as libc::c_int
                    & (1 as libc::c_int) << 5 as libc::c_int
                    != 0
                    && (*mt).marked as libc::c_int
                        & ((1 as libc::c_int) << 3 as libc::c_int
                            | (1 as libc::c_int) << 4 as libc::c_int)
                        != 0
                {
                    luaC_barrier_(
                        L,
                        ((*obj).value_.gc as *mut Udata) as *mut Udata as *mut GCObject,
                        mt as *mut GCObject,
                    );
                };
            }
        }
        _ => {
            (*(*L).global).mt[((*obj).tt_ & 0xf) as usize].set(mt);
        }
    }

    (*L).top = ((*L).top).offset(-1);

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
        let io2: *const TValue = &raw mut (*((*L).top).offset(-(1 as libc::c_int as isize))).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        if (*((*L).top).offset(-(1 as libc::c_int as isize))).val.tt_ as libc::c_int
            & (1 as libc::c_int) << 6 as libc::c_int
            != 0
        {
            if (*(*o).value_.gc).marked as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
                && (*(*((*L).top).offset(-(1 as libc::c_int as isize)))
                    .val
                    .value_
                    .gc)
                    .marked as libc::c_int
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

    (*L).top = ((*L).top).offset(-1);

    return res;
}

pub unsafe fn lua_call(
    L: *mut Thread,
    nargs: libc::c_int,
    nresults: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let func = ((*L).top).offset(-((nargs + 1) as isize));

    luaD_call(L, func, nresults)?;

    if nresults <= -1 && (*(*L).ci.get()).top < (*L).top {
        (*(*L).ci.get()).top = (*L).top;
    }

    Ok(())
}

pub unsafe fn lua_pcall(
    L: *mut Thread,
    nargs: libc::c_int,
    nresults: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let func = ((*L).top).offset(-((nargs + 1) as isize));
    let c: CallS = CallS { func, nresults };
    let status = luaD_pcall(
        L,
        (c.func as *mut libc::c_char).offset_from((*L).stack.get() as *mut libc::c_char),
        |L| luaD_call(L, c.func, c.nresults),
    );

    if nresults <= -1 && (*(*L).ci.get()).top < (*L).top {
        (*(*L).ci.get()).top = (*L).top;
    }

    status
}

pub unsafe fn lua_load(
    L: *mut Thread,
    reader: lua_Reader,
    data: *mut libc::c_void,
    mut chunkname: *const libc::c_char,
    mode: *const libc::c_char,
) -> Result<(), Box<dyn std::error::Error>> {
    if chunkname.is_null() {
        chunkname = b"?\0" as *const u8 as *const libc::c_char;
    }

    let mut z = Zio::new(reader, data);
    let status = luaD_protectedparser(L, &mut z, chunkname, mode);

    if status.is_ok() {
        let f: *mut LClosure = (*((*L).top).offset(-(1 as libc::c_int as isize)))
            .val
            .value_
            .gc as *mut LClosure;
        if (*f).nupvalues as libc::c_int >= 1 as libc::c_int {
            let gt: *const TValue =
                &mut *((*((*(*(*L).global).l_registry.get()).value_.gc as *mut Table)).array)
                    .offset((2 as libc::c_int - 1 as libc::c_int) as isize)
                    as *mut TValue;
            let io1: *mut TValue =
                (**((*f).upvals).as_mut_ptr().offset(0 as libc::c_int as isize)).v;
            let io2: *const TValue = gt;
            (*io1).value_ = (*io2).value_;
            (*io1).tt_ = (*io2).tt_;
            if (*gt).tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0 {
                if (**((*f).upvals).as_mut_ptr().offset(0 as libc::c_int as isize)).marked
                    as libc::c_int
                    & (1 as libc::c_int) << 5 as libc::c_int
                    != 0
                    && (*(*gt).value_.gc).marked as libc::c_int
                        & ((1 as libc::c_int) << 3 as libc::c_int
                            | (1 as libc::c_int) << 4 as libc::c_int)
                        != 0
                {
                    luaC_barrier_(
                        L,
                        *((*f).upvals).as_mut_ptr().offset(0 as libc::c_int as isize)
                            as *mut GCObject,
                        (*gt).value_.gc as *mut GCObject,
                    );
                } else {
                };
            } else {
            };
        }
    }

    status
}

pub unsafe fn lua_dump(
    L: *mut Thread,
    writer: lua_Writer,
    data: *mut c_void,
    strip: c_int,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let o = &raw mut (*((*L).top).offset(-1)).val;

    if (*o).tt_ as libc::c_int
        == 6 as libc::c_int
            | (0 as libc::c_int) << 4 as libc::c_int
            | (1 as libc::c_int) << 6 as libc::c_int
    {
        luaU_dump(
            L,
            (*((*o).value_.gc as *mut LClosure)).p,
            writer,
            data,
            strip,
        )
    } else {
        Ok(1)
    }
}

pub unsafe fn lua_next(
    L: *mut Thread,
    idx: libc::c_int,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut t: *mut Table = 0 as *mut Table;
    let mut more: libc::c_int = 0;
    t = gettable(L, idx);
    more = luaH_next(L, t, ((*L).top).offset(-(1 as libc::c_int as isize)))?;
    if more != 0 {
        api_incr_top(L);
    } else {
        (*L).top = ((*L).top).offset(-(1 as libc::c_int as isize));
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

pub unsafe fn lua_concat(L: *mut Thread, n: c_int) -> Result<(), Box<dyn std::error::Error>> {
    if n > 0 as libc::c_int {
        luaV_concat(L, n)?;
    } else {
        let io: *mut TValue = &raw mut (*(*L).top).val;
        let x_: *mut TString = luaS_newlstr(
            L,
            b"\0" as *const u8 as *const libc::c_char,
            0 as libc::c_int as usize,
        )?;
        (*io).value_.gc = x_ as *mut GCObject;
        (*io).tt_ = ((*x_).tt as libc::c_int | (1 as libc::c_int) << 6 as libc::c_int) as u8;
        api_incr_top(L);
    }
    if (*(*L).global).gc.debt() > 0 as libc::c_int as isize {
        luaC_step(L);
    }
    Ok(())
}

pub unsafe fn lua_len(L: *mut Thread, idx: c_int) -> Result<(), Box<dyn std::error::Error>> {
    let mut t: *mut TValue = 0 as *mut TValue;
    t = index2value(L, idx);
    luaV_objlen(L, (*L).top, t)?;
    api_incr_top(L);
    Ok(())
}

pub unsafe fn lua_newuserdatauv(
    L: *mut Thread,
    size: usize,
    nuvalue: libc::c_int,
) -> Result<*mut c_void, Box<dyn std::error::Error>> {
    let mut u: *mut Udata = 0 as *mut Udata;
    u = luaS_newudata(L, size, nuvalue)?;
    let io: *mut TValue = &raw mut (*(*L).top).val;
    let x_: *mut Udata = u;
    (*io).value_.gc = x_ as *mut GCObject;
    (*io).tt_ = (7 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 6 as libc::c_int) as u8;
    api_incr_top(L);
    if (*(*L).global).gc.debt() > 0 as libc::c_int as isize {
        luaC_step(L);
    }

    return Ok((u as *mut libc::c_char).offset(
        (if (*u).nuvalue == 0 {
            offset_of!(Udata, gclist)
        } else {
            offset_of!(Udata, uv) + size_of::<UValue>().wrapping_mul((*u).nuvalue.into())
        }) as isize,
    ) as *mut libc::c_void);
}

unsafe fn aux_upvalue(
    fi: *mut TValue,
    n: libc::c_int,
    val: *mut *mut TValue,
    owner: *mut *mut GCObject,
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
                *owner = f as *mut GCObject;
            }
            return b"\0" as *const u8 as *const libc::c_char;
        }
        6 => {
            let f_0: *mut LClosure = (*fi).value_.gc as *mut LClosure;
            let mut name: *mut TString = 0 as *mut TString;
            let p: *mut Proto = (*f_0).p;
            if !((n as libc::c_uint).wrapping_sub(1 as libc::c_uint)
                < (*p).sizeupvalues as libc::c_uint)
            {
                return 0 as *const libc::c_char;
            }
            *val = (**((*f_0).upvals)
                .as_mut_ptr()
                .offset((n - 1 as libc::c_int) as isize))
            .v;
            if !owner.is_null() {
                *owner = *((*f_0).upvals)
                    .as_mut_ptr()
                    .offset((n - 1 as libc::c_int) as isize)
                    as *mut GCObject;
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
        0 as *mut *mut GCObject,
    );
    if !name.is_null() {
        let io1: *mut TValue = &raw mut (*(*L).top).val;
        let io2: *const TValue = val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        api_incr_top(L);
    }
    return name;
}

pub unsafe fn lua_setupvalue(
    L: *mut Thread,
    funcindex: libc::c_int,
    n: libc::c_int,
) -> *const libc::c_char {
    let mut name: *const libc::c_char = 0 as *const libc::c_char;
    let mut val: *mut TValue = 0 as *mut TValue;
    let mut owner: *mut GCObject = 0 as *mut GCObject;
    let mut fi: *mut TValue = 0 as *mut TValue;
    fi = index2value(L, funcindex);
    name = aux_upvalue(fi, n, &mut val, &mut owner);
    if !name.is_null() {
        (*L).top = ((*L).top).offset(-1);

        let io1: *mut TValue = val;
        let io2: *const TValue = &raw mut (*(*L).top).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        if (*val).tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0 {
            if (*owner).marked as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
                && (*(*val).value_.gc).marked as libc::c_int
                    & ((1 as libc::c_int) << 3 as libc::c_int
                        | (1 as libc::c_int) << 4 as libc::c_int)
                    != 0
            {
                luaC_barrier_(L, owner as *mut GCObject, (*val).value_.gc as *mut GCObject);
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
    pf: *mut *mut LClosure,
) -> *mut *mut UpVal {
    static mut nullup: *const UpVal = 0 as *const UpVal;
    let mut f: *mut LClosure = 0 as *mut LClosure;
    let fi: *mut TValue = index2value(L, fidx);
    f = (*fi).value_.gc as *mut LClosure;
    if !pf.is_null() {
        *pf = f;
    }
    if 1 as libc::c_int <= n && n <= (*(*f).p).sizeupvalues {
        return &mut *((*f).upvals)
            .as_mut_ptr()
            .offset((n - 1 as libc::c_int) as isize) as *mut *mut UpVal;
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
        6 => {
            return *getupvalref(L, fidx, n, 0 as *mut *mut LClosure) as *mut libc::c_void;
        }
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
    let mut f1: *mut LClosure = 0 as *mut LClosure;
    let up1: *mut *mut UpVal = getupvalref(L, fidx1, n1, &mut f1);
    let up2: *mut *mut UpVal = getupvalref(L, fidx2, n2, 0 as *mut *mut LClosure);
    *up1 = *up2;
    if (*f1).marked as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
        && (**up1).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            != 0
    {
        luaC_barrier_(L, f1 as *mut GCObject, *up1 as *mut GCObject);
    } else {
    };
}
