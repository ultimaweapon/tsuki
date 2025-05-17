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

use crate::ldo::{luaD_callnoyield, luaD_growstack, luaD_pcall, luaD_protectedparser};
use crate::ldump::luaU_dump;
use crate::lfunc::{luaF_close, luaF_newCclosure, luaF_newtbcupval};
use crate::lgc::{
    luaC_barrier_, luaC_barrierback_, luaC_changemode, luaC_checkfinalizer, luaC_fullgc, luaC_step,
};
use crate::lobject::{
    CClosure, GCObject, LClosure, Proto, StackValue, StkId, TString, TValue, Table, UValue, Udata,
    UpVal, Value, luaO_arith, luaO_str2num, luaO_tostring,
};
use crate::lstate::{
    CallInfo, GCUnion, global_State, lua_CFunction, lua_Reader, lua_State, lua_WarnFunction,
    lua_Writer, luaE_setdebt, luaE_warning,
};
use crate::lstring::{luaS_new, luaS_newlstr, luaS_newudata};
use crate::ltable::{
    luaH_get, luaH_getint, luaH_getn, luaH_getstr, luaH_new, luaH_next, luaH_resize, luaH_set,
    luaH_setint,
};
use crate::ltm::{TM_EQ, luaT_typenames_};
use crate::lvm::{
    F2Ieq, luaV_concat, luaV_equalobj, luaV_finishget, luaV_finishset, luaV_lessequal,
    luaV_lessthan, luaV_objlen, luaV_tointeger, luaV_tonumber_,
};
use crate::lzio::Zio;
use crate::{GcCommand, api_incr_top};
use std::ffi::{c_int, c_void};

#[derive(Copy, Clone)]
#[repr(C)]
pub struct CallS {
    pub func: StkId,
    pub nresults: libc::c_int,
}

unsafe fn index2value(mut L: *mut lua_State, mut idx: libc::c_int) -> *mut TValue {
    let mut ci: *mut CallInfo = (*L).ci;
    if idx > 0 as libc::c_int {
        let mut o: StkId = ((*ci).func.p).offset(idx as isize);
        if o >= (*L).top.p {
            return &mut (*(*L).l_G).nilvalue;
        } else {
            return &mut (*o).val;
        }
    } else if !(idx <= -(1000000 as libc::c_int) - 1000 as libc::c_int) {
        return &mut (*((*L).top.p).offset(idx as isize)).val;
    } else if idx == -(1000000 as libc::c_int) - 1000 as libc::c_int {
        return &mut (*(*L).l_G).l_registry;
    } else {
        idx = -(1000000 as libc::c_int) - 1000 as libc::c_int - idx;
        if (*(*ci).func.p).val.tt_ as libc::c_int
            == 6 as libc::c_int
                | (2 as libc::c_int) << 4 as libc::c_int
                | (1 as libc::c_int) << 6 as libc::c_int
        {
            let mut func: *mut CClosure =
                &mut (*((*(*ci).func.p).val.value_.gc as *mut GCUnion)).cl.c;
            return if idx <= (*func).nupvalues as libc::c_int {
                &mut *((*func).upvalue)
                    .as_mut_ptr()
                    .offset((idx - 1 as libc::c_int) as isize) as *mut TValue
            } else {
                &mut (*(*L).l_G).nilvalue
            };
        } else {
            return &mut (*(*L).l_G).nilvalue;
        }
    };
}

unsafe fn index2stack(mut L: *mut lua_State, mut idx: libc::c_int) -> StkId {
    let mut ci: *mut CallInfo = (*L).ci;
    if idx > 0 as libc::c_int {
        let mut o: StkId = ((*ci).func.p).offset(idx as isize);
        return o;
    } else {
        return ((*L).top.p).offset(idx as isize);
    };
}

pub unsafe fn lua_checkstack(
    L: *mut lua_State,
    n: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut ci: *mut CallInfo = 0 as *mut CallInfo;
    ci = (*L).ci;

    if ((*L).stack_last.p).offset_from((*L).top.p) as libc::c_long > n as libc::c_long {
    } else {
        luaD_growstack(L, n)?;
    }

    if (*ci).top.p < ((*L).top.p).offset(n as isize) {
        (*ci).top.p = ((*L).top.p).offset(n as isize);
    }

    Ok(())
}

pub unsafe fn lua_xmove(mut from: *mut lua_State, mut to: *mut lua_State, mut n: libc::c_int) {
    let mut i: libc::c_int = 0;
    if from == to {
        return;
    }
    (*from).top.p = ((*from).top.p).offset(-(n as isize));
    i = 0 as libc::c_int;
    while i < n {
        let mut io1: *mut TValue = &mut (*(*to).top.p).val;
        let mut io2: *const TValue = &mut (*((*from).top.p).offset(i as isize)).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        (*to).top.p = ((*to).top.p).offset(1);
        (*to).top.p;
        i += 1;
    }
}

pub unsafe fn lua_absindex(mut L: *mut lua_State, mut idx: libc::c_int) -> libc::c_int {
    return if idx > 0 as libc::c_int || idx <= -(1000000 as libc::c_int) - 1000 as libc::c_int {
        idx
    } else {
        ((*L).top.p).offset_from((*(*L).ci).func.p) as libc::c_long as libc::c_int + idx
    };
}

pub unsafe fn lua_gettop(mut L: *mut lua_State) -> libc::c_int {
    return ((*L).top.p).offset_from(((*(*L).ci).func.p).offset(1 as libc::c_int as isize))
        as libc::c_long as libc::c_int;
}

pub unsafe fn lua_settop(
    mut L: *mut lua_State,
    mut idx: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut ci: *mut CallInfo = 0 as *mut CallInfo;
    let mut func: StkId = 0 as *mut StackValue;
    let mut newtop: StkId = 0 as *mut StackValue;
    let mut diff: isize = 0;
    ci = (*L).ci;
    func = (*ci).func.p;
    if idx >= 0 as libc::c_int {
        diff = func
            .offset(1 as libc::c_int as isize)
            .offset(idx as isize)
            .offset_from((*L).top.p);
        while diff > 0 as libc::c_int as isize {
            let fresh1 = (*L).top.p;
            (*L).top.p = ((*L).top.p).offset(1);
            (*fresh1).val.tt_ = (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
            diff -= 1;
        }
    } else {
        diff = (idx + 1 as libc::c_int) as isize;
    }
    newtop = ((*L).top.p).offset(diff as isize);
    if diff < 0 as libc::c_int as isize && (*L).tbclist.p >= newtop {
        newtop = luaF_close(L, newtop, 0)?;
    }
    (*L).top.p = newtop;
    Ok(())
}

pub unsafe fn lua_closeslot(
    mut L: *mut lua_State,
    mut idx: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut level: StkId = 0 as *mut StackValue;
    level = index2stack(L, idx);
    level = luaF_close(L, level, 0)?;
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
        let mut io1: *mut TValue = &mut temp;
        let mut io2: *const TValue = &mut (*from).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        let mut io1_0: *mut TValue = &mut (*from).val;
        let mut io2_0: *const TValue = &mut (*to).val;
        (*io1_0).value_ = (*io2_0).value_;
        (*io1_0).tt_ = (*io2_0).tt_;
        let mut io1_1: *mut TValue = &mut (*to).val;
        let mut io2_1: *const TValue = &mut temp;
        (*io1_1).value_ = (*io2_1).value_;
        (*io1_1).tt_ = (*io2_1).tt_;
        from = from.offset(1);
        to = to.offset(-1);
    }
}

pub unsafe fn lua_rotate(mut L: *mut lua_State, mut idx: libc::c_int, mut n: libc::c_int) {
    let mut p: StkId = 0 as *mut StackValue;
    let mut t: StkId = 0 as *mut StackValue;
    let mut m: StkId = 0 as *mut StackValue;
    t = ((*L).top.p).offset(-(1 as libc::c_int as isize));
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

pub unsafe fn lua_copy(mut L: *mut lua_State, mut fromidx: libc::c_int, mut toidx: libc::c_int) {
    let mut fr: *mut TValue = 0 as *mut TValue;
    let mut to: *mut TValue = 0 as *mut TValue;
    fr = index2value(L, fromidx);
    to = index2value(L, toidx);
    let mut io1: *mut TValue = to;
    let mut io2: *const TValue = fr;
    (*io1).value_ = (*io2).value_;
    (*io1).tt_ = (*io2).tt_;
    if toidx < -(1000000 as libc::c_int) - 1000 as libc::c_int {
        if (*fr).tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0 {
            if (*((*(*(*L).ci).func.p).val.value_.gc as *mut GCUnion))
                .cl
                .c
                .marked as libc::c_int
                & (1 as libc::c_int) << 5 as libc::c_int
                != 0
                && (*(*fr).value_.gc).marked as libc::c_int
                    & ((1 as libc::c_int) << 3 as libc::c_int
                        | (1 as libc::c_int) << 4 as libc::c_int)
                    != 0
            {
                luaC_barrier_(
                    L,
                    &mut (*(&mut (*((*(*(*L).ci).func.p).val.value_.gc as *mut GCUnion)).cl.c
                        as *mut CClosure as *mut GCUnion))
                        .gc,
                    &mut (*((*fr).value_.gc as *mut GCUnion)).gc,
                );
            } else {
            };
        } else {
        };
    }
}

pub unsafe fn lua_pushvalue(mut L: *mut lua_State, mut idx: libc::c_int) {
    let mut io1: *mut TValue = &mut (*(*L).top.p).val;
    let mut io2: *const TValue = index2value(L, idx);
    (*io1).value_ = (*io2).value_;
    (*io1).tt_ = (*io2).tt_;
    api_incr_top(L);
}

pub unsafe fn lua_type(mut L: *mut lua_State, mut idx: libc::c_int) -> libc::c_int {
    let mut o: *const TValue = index2value(L, idx);
    return if !((*o).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int)
        || o != &mut (*(*L).l_G).nilvalue as *mut TValue as *const TValue
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

pub unsafe fn lua_iscfunction(mut L: *mut lua_State, mut idx: libc::c_int) -> libc::c_int {
    let mut o: *const TValue = index2value(L, idx);
    return ((*o).tt_ as libc::c_int == 6 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
        || (*o).tt_ as libc::c_int
            == 6 as libc::c_int
                | (2 as libc::c_int) << 4 as libc::c_int
                | (1 as libc::c_int) << 6 as libc::c_int) as libc::c_int;
}

pub unsafe fn lua_isinteger(mut L: *mut lua_State, mut idx: libc::c_int) -> libc::c_int {
    let mut o: *const TValue = index2value(L, idx);
    return ((*o).tt_ as libc::c_int == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int)
        as libc::c_int;
}

pub unsafe fn lua_isnumber(mut L: *mut lua_State, mut idx: libc::c_int) -> libc::c_int {
    let mut n: f64 = 0.;
    let mut o: *const TValue = index2value(L, idx);
    return if (*o).tt_ as libc::c_int == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int {
        n = (*o).value_.n;
        1 as libc::c_int
    } else {
        luaV_tonumber_(o, &mut n)
    };
}

pub unsafe fn lua_isstring(mut L: *mut lua_State, mut idx: libc::c_int) -> libc::c_int {
    let mut o: *const TValue = index2value(L, idx);
    return ((*o).tt_ as libc::c_int & 0xf as libc::c_int == 4 as libc::c_int
        || (*o).tt_ as libc::c_int & 0xf as libc::c_int == 3 as libc::c_int)
        as libc::c_int;
}

pub unsafe fn lua_isuserdata(mut L: *mut lua_State, mut idx: libc::c_int) -> libc::c_int {
    let mut o: *const TValue = index2value(L, idx);
    return ((*o).tt_ as libc::c_int
        == 7 as libc::c_int
            | (0 as libc::c_int) << 4 as libc::c_int
            | (1 as libc::c_int) << 6 as libc::c_int
        || (*o).tt_ as libc::c_int == 2 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int)
        as libc::c_int;
}

pub unsafe fn lua_rawequal(
    mut L: *mut lua_State,
    mut index1: libc::c_int,
    mut index2: libc::c_int,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut o1: *const TValue = index2value(L, index1);
    let mut o2: *const TValue = index2value(L, index2);
    return if (!((*o1).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int)
        || o1 != &mut (*(*L).l_G).nilvalue as *mut TValue as *const TValue)
        && (!((*o2).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int)
            || o2 != &mut (*(*L).l_G).nilvalue as *mut TValue as *const TValue)
    {
        luaV_equalobj(0 as *mut lua_State, o1, o2)
    } else {
        Ok(0 as libc::c_int)
    };
}

pub unsafe fn lua_arith(
    mut L: *mut lua_State,
    mut op: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    if !(op != 12 as libc::c_int && op != 13 as libc::c_int) {
        let mut io1: *mut TValue = &mut (*(*L).top.p).val;
        let mut io2: *const TValue = &mut (*((*L).top.p).offset(-(1 as libc::c_int as isize))).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        api_incr_top(L);
    }
    luaO_arith(
        L,
        op,
        &mut (*((*L).top.p).offset(-(2 as libc::c_int as isize))).val,
        &mut (*((*L).top.p).offset(-(1 as libc::c_int as isize))).val,
        ((*L).top.p).offset(-(2 as libc::c_int as isize)),
    )?;
    (*L).top.p = ((*L).top.p).offset(-1);
    (*L).top.p;
    Ok(())
}

pub unsafe fn lua_compare(
    mut L: *mut lua_State,
    mut index1: libc::c_int,
    mut index2: libc::c_int,
    mut op: libc::c_int,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut o1: *const TValue = 0 as *const TValue;
    let mut o2: *const TValue = 0 as *const TValue;
    let mut i: libc::c_int = 0 as libc::c_int;
    o1 = index2value(L, index1);
    o2 = index2value(L, index2);
    if (!((*o1).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int)
        || o1 != &mut (*(*L).l_G).nilvalue as *mut TValue as *const TValue)
        && (!((*o2).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int)
            || o2 != &mut (*(*L).l_G).nilvalue as *mut TValue as *const TValue)
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

pub unsafe fn lua_stringtonumber(mut L: *mut lua_State, mut s: *const libc::c_char) -> usize {
    let mut sz: usize = luaO_str2num(s, &mut (*(*L).top.p).val);
    if sz != 0 as libc::c_int as usize {
        api_incr_top(L);
    }
    return sz;
}

pub unsafe fn lua_tonumberx(
    mut L: *mut lua_State,
    mut idx: libc::c_int,
    mut pisnum: *mut libc::c_int,
) -> f64 {
    let mut n: f64 = 0 as libc::c_int as f64;
    let mut o: *const TValue = index2value(L, idx);
    let mut isnum: libc::c_int =
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

pub unsafe fn lua_tointegerx(
    mut L: *mut lua_State,
    mut idx: libc::c_int,
    mut pisnum: *mut libc::c_int,
) -> i64 {
    let mut res: i64 = 0 as libc::c_int as i64;
    let mut o: *const TValue = index2value(L, idx);
    let mut isnum: libc::c_int = if (((*o).tt_ as libc::c_int
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

pub unsafe fn lua_toboolean(mut L: *mut lua_State, mut idx: libc::c_int) -> libc::c_int {
    let mut o: *const TValue = index2value(L, idx);
    return !((*o).tt_ as libc::c_int == 1 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
        || (*o).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int)
        as libc::c_int;
}

pub unsafe fn lua_tolstring(
    mut L: *mut lua_State,
    mut idx: libc::c_int,
    mut len: *mut usize,
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
        if (*(*L).l_G).GCdebt > 0 as libc::c_int as isize {
            luaC_step(L);
        }
        o = index2value(L, idx);
    }
    if !len.is_null() {
        *len = if (*((*o).value_.gc as *mut GCUnion)).ts.shrlen as libc::c_int
            != 0xff as libc::c_int
        {
            (*((*o).value_.gc as *mut GCUnion)).ts.shrlen as usize
        } else {
            (*((*o).value_.gc as *mut GCUnion)).ts.u.lnglen
        };
    }
    return Ok(((*((*o).value_.gc as *mut GCUnion)).ts.contents).as_mut_ptr());
}

pub unsafe fn lua_rawlen(mut L: *mut lua_State, mut idx: libc::c_int) -> u64 {
    let mut o: *const TValue = index2value(L, idx);
    match (*o).tt_ as libc::c_int & 0x3f as libc::c_int {
        4 => return (*((*o).value_.gc as *mut GCUnion)).ts.shrlen as u64,
        20 => return (*((*o).value_.gc as *mut GCUnion)).ts.u.lnglen as u64,
        7 => return (*((*o).value_.gc as *mut GCUnion)).u.len as u64,
        5 => return luaH_getn(&mut (*((*o).value_.gc as *mut GCUnion)).h),
        _ => return 0 as libc::c_int as u64,
    };
}

pub unsafe fn lua_tocfunction(L: *mut lua_State, idx: c_int) -> Option<lua_CFunction> {
    let mut o: *const TValue = index2value(L, idx);
    if (*o).tt_ as libc::c_int == 6 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int {
        return Some((*o).value_.f);
    } else if (*o).tt_ as libc::c_int
        == 6 as libc::c_int
            | (2 as libc::c_int) << 4 as libc::c_int
            | (1 as libc::c_int) << 6 as libc::c_int
    {
        return Some((*((*o).value_.gc as *mut GCUnion)).cl.c.f);
    } else {
        return None;
    };
}

unsafe fn touserdata(mut o: *const TValue) -> *mut libc::c_void {
    match (*o).tt_ as libc::c_int & 0xf as libc::c_int {
        7 => {
            return (&mut (*((*o).value_.gc as *mut GCUnion)).u as *mut Udata as *mut libc::c_char)
                .offset(
                    (if (*((*o).value_.gc as *mut GCUnion)).u.nuvalue as libc::c_int
                        == 0 as libc::c_int
                    {
                        32 as libc::c_ulong
                    } else {
                        (40 as libc::c_ulong).wrapping_add(
                            (::core::mem::size_of::<UValue>() as libc::c_ulong).wrapping_mul(
                                (*((*o).value_.gc as *mut GCUnion)).u.nuvalue as libc::c_ulong,
                            ),
                        )
                    }) as isize,
                ) as *mut libc::c_void;
        }
        2 => return (*o).value_.p,
        _ => return 0 as *mut libc::c_void,
    };
}

pub unsafe fn lua_touserdata(mut L: *mut lua_State, mut idx: libc::c_int) -> *mut libc::c_void {
    let mut o: *const TValue = index2value(L, idx);
    return touserdata(o);
}

pub unsafe fn lua_tothread(mut L: *mut lua_State, mut idx: libc::c_int) -> *mut lua_State {
    let mut o: *const TValue = index2value(L, idx);
    return if !((*o).tt_ as libc::c_int
        == 8 as libc::c_int
            | (0 as libc::c_int) << 4 as libc::c_int
            | (1 as libc::c_int) << 6 as libc::c_int)
    {
        0 as *mut lua_State
    } else {
        &mut (*((*o).value_.gc as *mut GCUnion)).th
    };
}

pub unsafe fn lua_topointer(mut L: *mut lua_State, mut idx: libc::c_int) -> *const libc::c_void {
    let mut o: *const TValue = index2value(L, idx);
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

pub unsafe fn lua_pushnil(mut L: *mut lua_State) {
    (*(*L).top.p).val.tt_ = (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    api_incr_top(L);
}

pub unsafe fn lua_pushnumber(mut L: *mut lua_State, mut n: f64) {
    let mut io: *mut TValue = &mut (*(*L).top.p).val;
    (*io).value_.n = n;
    (*io).tt_ = (3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
    api_incr_top(L);
}

pub unsafe fn lua_pushinteger(mut L: *mut lua_State, mut n: i64) {
    let mut io: *mut TValue = &mut (*(*L).top.p).val;
    (*io).value_.i = n;
    (*io).tt_ = (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    api_incr_top(L);
}

pub unsafe fn lua_pushlstring(
    mut L: *mut lua_State,
    s: impl AsRef<[u8]>,
) -> Result<*const libc::c_char, Box<dyn std::error::Error>> {
    let s = s.as_ref();
    let mut ts: *mut TString = 0 as *mut TString;
    ts = if s.is_empty() {
        luaS_new(L, b"\0" as *const u8 as *const libc::c_char)?
    } else {
        luaS_newlstr(L, s.as_ptr().cast(), s.len() as _)?
    };
    let mut io: *mut TValue = &mut (*(*L).top.p).val;
    let mut x_: *mut TString = ts;
    (*io).value_.gc = &mut (*(x_ as *mut GCUnion)).gc;
    (*io).tt_ = ((*x_).tt as libc::c_int | (1 as libc::c_int) << 6 as libc::c_int) as u8;
    api_incr_top(L);
    if (*(*L).l_G).GCdebt > 0 as libc::c_int as isize {
        luaC_step(L);
    }
    return Ok(((*ts).contents).as_mut_ptr());
}

pub unsafe fn lua_pushstring(
    mut L: *mut lua_State,
    mut s: *const libc::c_char,
) -> Result<*const libc::c_char, Box<dyn std::error::Error>> {
    if s.is_null() {
        (*(*L).top.p).val.tt_ = (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    } else {
        let mut ts: *mut TString = 0 as *mut TString;
        ts = luaS_new(L, s)?;
        let mut io: *mut TValue = &mut (*(*L).top.p).val;
        let mut x_: *mut TString = ts;
        (*io).value_.gc = &mut (*(x_ as *mut GCUnion)).gc;
        (*io).tt_ = ((*x_).tt as libc::c_int | (1 as libc::c_int) << 6 as libc::c_int) as u8;
        s = ((*ts).contents).as_mut_ptr();
    }
    api_incr_top(L);
    if (*(*L).l_G).GCdebt > 0 as libc::c_int as isize {
        luaC_step(L);
    }
    return Ok(s);
}

pub unsafe fn lua_pushcclosure(mut L: *mut lua_State, mut fn_0: lua_CFunction, mut n: libc::c_int) {
    if n == 0 as libc::c_int {
        let mut io: *mut TValue = &mut (*(*L).top.p).val;
        (*io).value_.f = fn_0;
        (*io).tt_ = (6 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
        api_incr_top(L);
    } else {
        let mut cl: *mut CClosure = 0 as *mut CClosure;
        cl = luaF_newCclosure(L, n);
        (*cl).f = fn_0;
        (*L).top.p = ((*L).top.p).offset(-(n as isize));
        loop {
            let fresh2 = n;
            n = n - 1;
            if !(fresh2 != 0) {
                break;
            }
            let mut io1: *mut TValue =
                &mut *((*cl).upvalue).as_mut_ptr().offset(n as isize) as *mut TValue;
            let mut io2: *const TValue = &mut (*((*L).top.p).offset(n as isize)).val;
            (*io1).value_ = (*io2).value_;
            (*io1).tt_ = (*io2).tt_;
        }
        let mut io_0: *mut TValue = &mut (*(*L).top.p).val;
        let mut x_: *mut CClosure = cl;
        (*io_0).value_.gc = &mut (*(x_ as *mut GCUnion)).gc;
        (*io_0).tt_ = (6 as libc::c_int
            | (2 as libc::c_int) << 4 as libc::c_int
            | (1 as libc::c_int) << 6 as libc::c_int) as u8;
        api_incr_top(L);
        if (*(*L).l_G).GCdebt > 0 as libc::c_int as isize {
            luaC_step(L);
        }
    };
}

pub unsafe fn lua_pushboolean(mut L: *mut lua_State, mut b: libc::c_int) {
    if b != 0 {
        (*(*L).top.p).val.tt_ = (1 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
    } else {
        (*(*L).top.p).val.tt_ = (1 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    }
    api_incr_top(L);
}

pub unsafe fn lua_pushlightuserdata(mut L: *mut lua_State, mut p: *mut libc::c_void) {
    let mut io: *mut TValue = &mut (*(*L).top.p).val;
    (*io).value_.p = p;
    (*io).tt_ = (2 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    api_incr_top(L);
}

pub unsafe fn lua_pushthread(mut L: *mut lua_State) -> libc::c_int {
    let mut io: *mut TValue = &mut (*(*L).top.p).val;
    let mut x_: *mut lua_State = L;
    (*io).value_.gc = &mut (*(x_ as *mut GCUnion)).gc;
    (*io).tt_ = (8 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 6 as libc::c_int) as u8;
    api_incr_top(L);
    return ((*(*L).l_G).mainthread == L) as libc::c_int;
}

unsafe fn auxgetstr(
    mut L: *mut lua_State,
    mut t: *const TValue,
    mut k: &[u8],
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut slot: *const TValue = 0 as *const TValue;
    let mut str: *mut TString = luaS_newlstr(L, k.as_ptr().cast(), k.len())?;
    if if !((*t).tt_ as libc::c_int
        == 5 as libc::c_int
            | (0 as libc::c_int) << 4 as libc::c_int
            | (1 as libc::c_int) << 6 as libc::c_int)
    {
        slot = 0 as *const TValue;
        0 as libc::c_int
    } else {
        slot = luaH_getstr(&mut (*((*t).value_.gc as *mut GCUnion)).h, str);
        !((*slot).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int) as libc::c_int
    } != 0
    {
        let mut io1: *mut TValue = &mut (*(*L).top.p).val;
        let mut io2: *const TValue = slot;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        api_incr_top(L);
    } else {
        let mut io: *mut TValue = &mut (*(*L).top.p).val;
        let mut x_: *mut TString = str;
        (*io).value_.gc = &mut (*(x_ as *mut GCUnion)).gc;
        (*io).tt_ = ((*x_).tt as libc::c_int | (1 as libc::c_int) << 6 as libc::c_int) as u8;
        api_incr_top(L);
        luaV_finishget(
            L,
            t,
            &mut (*((*L).top.p).offset(-(1 as libc::c_int as isize))).val,
            ((*L).top.p).offset(-(1 as libc::c_int as isize)),
            slot,
        )?;
    }

    return Ok(
        (*((*L).top.p).offset(-(1 as libc::c_int as isize))).val.tt_ as libc::c_int
            & 0xf as libc::c_int,
    );
}

pub unsafe fn lua_getglobal(
    mut L: *mut lua_State,
    name: impl AsRef<[u8]>,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut G: *const TValue = 0 as *const TValue;
    G = &mut *((*((*(*L).l_G).l_registry.value_.gc as *mut GCUnion))
        .h
        .array)
        .offset((2 as libc::c_int - 1 as libc::c_int) as isize) as *mut TValue;
    return auxgetstr(L, G, name.as_ref());
}

pub unsafe fn lua_gettable(
    mut L: *mut lua_State,
    mut idx: libc::c_int,
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
            &mut (*((*t).value_.gc as *mut GCUnion)).h,
            &mut (*((*L).top.p).offset(-(1 as libc::c_int as isize))).val,
        );
        !((*slot).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int) as libc::c_int
    } != 0
    {
        let mut io1: *mut TValue = &mut (*((*L).top.p).offset(-(1 as libc::c_int as isize))).val;
        let mut io2: *const TValue = slot;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
    } else {
        luaV_finishget(
            L,
            t,
            &mut (*((*L).top.p).offset(-(1 as libc::c_int as isize))).val,
            ((*L).top.p).offset(-(1 as libc::c_int as isize)),
            slot,
        )?;
    }
    return Ok(
        (*((*L).top.p).offset(-(1 as libc::c_int as isize))).val.tt_ as libc::c_int
            & 0xf as libc::c_int,
    );
}

pub unsafe fn lua_getfield(
    mut L: *mut lua_State,
    mut idx: libc::c_int,
    k: impl AsRef<[u8]>,
) -> Result<c_int, Box<dyn std::error::Error>> {
    return auxgetstr(L, index2value(L, idx), k.as_ref());
}

pub unsafe fn lua_geti(
    mut L: *mut lua_State,
    mut idx: libc::c_int,
    mut n: i64,
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
        slot = (if (n as u64).wrapping_sub(1 as libc::c_uint as u64)
            < (*((*t).value_.gc as *mut GCUnion)).h.alimit as u64
        {
            &mut *((*((*t).value_.gc as *mut GCUnion)).h.array)
                .offset((n - 1 as libc::c_int as i64) as isize) as *mut TValue
                as *const TValue
        } else {
            luaH_getint(&mut (*((*t).value_.gc as *mut GCUnion)).h, n)
        });
        !((*slot).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int) as libc::c_int
    } != 0
    {
        let mut io1: *mut TValue = &mut (*(*L).top.p).val;
        let mut io2: *const TValue = slot;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
    } else {
        let mut aux: TValue = TValue {
            value_: Value {
                gc: 0 as *mut GCObject,
            },
            tt_: 0,
        };
        let mut io: *mut TValue = &mut aux;
        (*io).value_.i = n;
        (*io).tt_ = (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
        luaV_finishget(L, t, &mut aux, (*L).top.p, slot)?;
    }
    api_incr_top(L);

    return Ok(
        (*((*L).top.p).offset(-(1 as libc::c_int as isize))).val.tt_ as libc::c_int
            & 0xf as libc::c_int,
    );
}

unsafe fn finishrawget(mut L: *mut lua_State, mut val: *const TValue) -> libc::c_int {
    if (*val).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int {
        (*(*L).top.p).val.tt_ = (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    } else {
        let mut io1: *mut TValue = &mut (*(*L).top.p).val;
        let mut io2: *const TValue = val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
    }
    api_incr_top(L);
    return (*((*L).top.p).offset(-(1 as libc::c_int as isize))).val.tt_ as libc::c_int
        & 0xf as libc::c_int;
}

unsafe fn gettable(mut L: *mut lua_State, mut idx: libc::c_int) -> *mut Table {
    let mut t: *mut TValue = index2value(L, idx);
    return &mut (*((*t).value_.gc as *mut GCUnion)).h;
}

pub unsafe fn lua_rawget(mut L: *mut lua_State, mut idx: libc::c_int) -> libc::c_int {
    let mut t: *mut Table = 0 as *mut Table;
    let mut val: *const TValue = 0 as *const TValue;
    t = gettable(L, idx);
    val = luaH_get(
        t,
        &mut (*((*L).top.p).offset(-(1 as libc::c_int as isize))).val,
    );
    (*L).top.p = ((*L).top.p).offset(-1);
    (*L).top.p;
    return finishrawget(L, val);
}

pub unsafe fn lua_rawgeti(mut L: *mut lua_State, mut idx: libc::c_int, mut n: i64) -> libc::c_int {
    let mut t: *mut Table = 0 as *mut Table;
    t = gettable(L, idx);
    return finishrawget(L, luaH_getint(t, n));
}

pub unsafe fn lua_rawgetp(
    mut L: *mut lua_State,
    mut idx: libc::c_int,
    mut p: *const libc::c_void,
) -> libc::c_int {
    let mut t: *mut Table = 0 as *mut Table;
    let mut k: TValue = TValue {
        value_: Value {
            gc: 0 as *mut GCObject,
        },
        tt_: 0,
    };
    t = gettable(L, idx);
    let mut io: *mut TValue = &mut k;
    (*io).value_.p = p as *mut libc::c_void;
    (*io).tt_ = (2 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    return finishrawget(L, luaH_get(t, &mut k));
}

pub unsafe fn lua_createtable(
    mut L: *mut lua_State,
    mut narray: libc::c_int,
    mut nrec: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut t: *mut Table = 0 as *mut Table;
    t = luaH_new(L)?;
    let mut io: *mut TValue = &mut (*(*L).top.p).val;
    let mut x_: *mut Table = t;
    (*io).value_.gc = &mut (*(x_ as *mut GCUnion)).gc;
    (*io).tt_ = (5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 6 as libc::c_int) as u8;
    api_incr_top(L);
    if narray > 0 as libc::c_int || nrec > 0 as libc::c_int {
        luaH_resize(L, t, narray as libc::c_uint, nrec as libc::c_uint)?;
    }
    if (*(*L).l_G).GCdebt > 0 as libc::c_int as isize {
        luaC_step(L);
    }
    Ok(())
}

pub unsafe fn lua_getmetatable(mut L: *mut lua_State, mut objindex: libc::c_int) -> libc::c_int {
    let mut obj: *const TValue = 0 as *const TValue;
    let mut mt: *mut Table = 0 as *mut Table;
    let mut res: libc::c_int = 0 as libc::c_int;
    obj = index2value(L, objindex);
    match (*obj).tt_ as libc::c_int & 0xf as libc::c_int {
        5 => {
            mt = (*((*obj).value_.gc as *mut GCUnion)).h.metatable;
        }
        7 => {
            mt = (*((*obj).value_.gc as *mut GCUnion)).u.metatable;
        }
        _ => {
            mt = (*(*L).l_G).mt[((*obj).tt_ as libc::c_int & 0xf as libc::c_int) as usize];
        }
    }
    if !mt.is_null() {
        let mut io: *mut TValue = &mut (*(*L).top.p).val;
        let mut x_: *mut Table = mt;
        (*io).value_.gc = &mut (*(x_ as *mut GCUnion)).gc;
        (*io).tt_ = (5 as libc::c_int
            | (0 as libc::c_int) << 4 as libc::c_int
            | (1 as libc::c_int) << 6 as libc::c_int) as u8;
        api_incr_top(L);
        res = 1 as libc::c_int;
    }
    return res;
}

pub unsafe fn lua_getiuservalue(
    mut L: *mut lua_State,
    mut idx: libc::c_int,
    mut n: libc::c_int,
) -> libc::c_int {
    let mut o: *mut TValue = 0 as *mut TValue;
    let mut t: libc::c_int = 0;
    o = index2value(L, idx);
    if n <= 0 as libc::c_int || n > (*((*o).value_.gc as *mut GCUnion)).u.nuvalue as libc::c_int {
        (*(*L).top.p).val.tt_ = (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
        t = -(1 as libc::c_int);
    } else {
        let mut io1: *mut TValue = &mut (*(*L).top.p).val;
        let mut io2: *const TValue = &mut (*((*((*o).value_.gc as *mut GCUnion)).u.uv)
            .as_mut_ptr()
            .offset((n - 1 as libc::c_int) as isize))
        .uv;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        t = (*(*L).top.p).val.tt_ as libc::c_int & 0xf as libc::c_int;
    }
    api_incr_top(L);
    return t;
}

unsafe fn auxsetstr(
    mut L: *mut lua_State,
    mut t: *const TValue,
    mut k: *const libc::c_char,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut slot: *const TValue = 0 as *const TValue;
    let mut str: *mut TString = luaS_new(L, k)?;
    if if !((*t).tt_ as libc::c_int
        == 5 as libc::c_int
            | (0 as libc::c_int) << 4 as libc::c_int
            | (1 as libc::c_int) << 6 as libc::c_int)
    {
        slot = 0 as *const TValue;
        0 as libc::c_int
    } else {
        slot = luaH_getstr(&mut (*((*t).value_.gc as *mut GCUnion)).h, str);
        !((*slot).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int) as libc::c_int
    } != 0
    {
        let mut io1: *mut TValue = slot as *mut TValue;
        let mut io2: *const TValue = &mut (*((*L).top.p).offset(-(1 as libc::c_int as isize))).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        if (*((*L).top.p).offset(-(1 as libc::c_int as isize))).val.tt_ as libc::c_int
            & (1 as libc::c_int) << 6 as libc::c_int
            != 0
        {
            if (*(*t).value_.gc).marked as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
                && (*(*((*L).top.p).offset(-(1 as libc::c_int as isize)))
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
        (*L).top.p = ((*L).top.p).offset(-1);
        (*L).top.p;
    } else {
        let mut io: *mut TValue = &mut (*(*L).top.p).val;
        let mut x_: *mut TString = str;
        (*io).value_.gc = &mut (*(x_ as *mut GCUnion)).gc;
        (*io).tt_ = ((*x_).tt as libc::c_int | (1 as libc::c_int) << 6 as libc::c_int) as u8;
        api_incr_top(L);
        luaV_finishset(
            L,
            t,
            &mut (*((*L).top.p).offset(-(1 as libc::c_int as isize))).val,
            &mut (*((*L).top.p).offset(-(2 as libc::c_int as isize))).val,
            slot,
        )?;
        (*L).top.p = ((*L).top.p).offset(-(2 as libc::c_int as isize));
    };
    Ok(())
}

pub unsafe fn lua_setglobal(
    mut L: *mut lua_State,
    mut name: *const libc::c_char,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut G: *const TValue = 0 as *const TValue;
    G = &mut *((*((*(*L).l_G).l_registry.value_.gc as *mut GCUnion))
        .h
        .array)
        .offset((2 as libc::c_int - 1 as libc::c_int) as isize) as *mut TValue;
    auxsetstr(L, G, name)
}

pub unsafe fn lua_settable(
    mut L: *mut lua_State,
    mut idx: libc::c_int,
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
            &mut (*((*t).value_.gc as *mut GCUnion)).h,
            &mut (*((*L).top.p).offset(-(2 as libc::c_int as isize))).val,
        );
        !((*slot).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int) as libc::c_int
    } != 0
    {
        let mut io1: *mut TValue = slot as *mut TValue;
        let mut io2: *const TValue = &mut (*((*L).top.p).offset(-(1 as libc::c_int as isize))).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        if (*((*L).top.p).offset(-(1 as libc::c_int as isize))).val.tt_ as libc::c_int
            & (1 as libc::c_int) << 6 as libc::c_int
            != 0
        {
            if (*(*t).value_.gc).marked as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
                && (*(*((*L).top.p).offset(-(1 as libc::c_int as isize)))
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
            &mut (*((*L).top.p).offset(-(2 as libc::c_int as isize))).val,
            &mut (*((*L).top.p).offset(-(1 as libc::c_int as isize))).val,
            slot,
        )?;
    }
    (*L).top.p = ((*L).top.p).offset(-(2 as libc::c_int as isize));
    Ok(())
}

pub unsafe fn lua_setfield(
    mut L: *mut lua_State,
    mut idx: libc::c_int,
    mut k: *const libc::c_char,
) -> Result<(), Box<dyn std::error::Error>> {
    auxsetstr(L, index2value(L, idx), k)
}

pub unsafe fn lua_seti(
    mut L: *mut lua_State,
    mut idx: libc::c_int,
    mut n: i64,
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
        slot = (if (n as u64).wrapping_sub(1 as libc::c_uint as u64)
            < (*((*t).value_.gc as *mut GCUnion)).h.alimit as u64
        {
            &mut *((*((*t).value_.gc as *mut GCUnion)).h.array)
                .offset((n - 1 as libc::c_int as i64) as isize) as *mut TValue
                as *const TValue
        } else {
            luaH_getint(&mut (*((*t).value_.gc as *mut GCUnion)).h, n)
        });
        !((*slot).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int) as libc::c_int
    } != 0
    {
        let mut io1: *mut TValue = slot as *mut TValue;
        let mut io2: *const TValue = &mut (*((*L).top.p).offset(-(1 as libc::c_int as isize))).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        if (*((*L).top.p).offset(-(1 as libc::c_int as isize))).val.tt_ as libc::c_int
            & (1 as libc::c_int) << 6 as libc::c_int
            != 0
        {
            if (*(*t).value_.gc).marked as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
                && (*(*((*L).top.p).offset(-(1 as libc::c_int as isize)))
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
        let mut io: *mut TValue = &mut aux;
        (*io).value_.i = n;
        (*io).tt_ = (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
        luaV_finishset(
            L,
            t,
            &mut aux,
            &mut (*((*L).top.p).offset(-(1 as libc::c_int as isize))).val,
            slot,
        )?;
    }
    (*L).top.p = ((*L).top.p).offset(-1);
    (*L).top.p;
    Ok(())
}

unsafe fn aux_rawset(
    mut L: *mut lua_State,
    mut idx: libc::c_int,
    mut key: *mut TValue,
    mut n: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut t: *mut Table = 0 as *mut Table;
    t = gettable(L, idx);
    luaH_set(
        L,
        t,
        key,
        &mut (*((*L).top.p).offset(-(1 as libc::c_int as isize))).val,
    )?;
    (*t).flags = ((*t).flags as libc::c_uint
        & !!(!(0 as libc::c_uint) << TM_EQ as libc::c_int + 1 as libc::c_int))
        as u8;
    if (*((*L).top.p).offset(-(1 as libc::c_int as isize))).val.tt_ as libc::c_int
        & (1 as libc::c_int) << 6 as libc::c_int
        != 0
    {
        if (*(t as *mut GCUnion)).gc.marked as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int
            != 0
            && (*(*((*L).top.p).offset(-(1 as libc::c_int as isize)))
                .val
                .value_
                .gc)
                .marked as libc::c_int
                & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
                != 0
        {
            luaC_barrierback_(L, &mut (*(t as *mut GCUnion)).gc);
        } else {
        };
    } else {
    };
    (*L).top.p = ((*L).top.p).offset(-(n as isize));
    Ok(())
}

pub unsafe fn lua_rawset(
    mut L: *mut lua_State,
    mut idx: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    aux_rawset(
        L,
        idx,
        &mut (*((*L).top.p).offset(-(2 as libc::c_int as isize))).val,
        2 as libc::c_int,
    )
}

pub unsafe fn lua_rawsetp(
    mut L: *mut lua_State,
    mut idx: libc::c_int,
    mut p: *const libc::c_void,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut k: TValue = TValue {
        value_: Value {
            gc: 0 as *mut GCObject,
        },
        tt_: 0,
    };
    let mut io: *mut TValue = &mut k;
    (*io).value_.p = p as *mut libc::c_void;
    (*io).tt_ = (2 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    aux_rawset(L, idx, &mut k, 1 as libc::c_int)
}

pub unsafe fn lua_rawseti(
    mut L: *mut lua_State,
    mut idx: libc::c_int,
    mut n: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut t: *mut Table = 0 as *mut Table;
    t = gettable(L, idx);
    luaH_setint(
        L,
        t,
        n,
        &mut (*((*L).top.p).offset(-(1 as libc::c_int as isize))).val,
    )?;
    if (*((*L).top.p).offset(-(1 as libc::c_int as isize))).val.tt_ as libc::c_int
        & (1 as libc::c_int) << 6 as libc::c_int
        != 0
    {
        if (*(t as *mut GCUnion)).gc.marked as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int
            != 0
            && (*(*((*L).top.p).offset(-(1 as libc::c_int as isize)))
                .val
                .value_
                .gc)
                .marked as libc::c_int
                & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
                != 0
        {
            luaC_barrierback_(L, &mut (*(t as *mut GCUnion)).gc);
        } else {
        };
    } else {
    };
    (*L).top.p = ((*L).top.p).offset(-1);
    (*L).top.p;
    Ok(())
}

pub unsafe fn lua_setmetatable(mut L: *mut lua_State, mut objindex: libc::c_int) -> libc::c_int {
    let mut obj: *mut TValue = 0 as *mut TValue;
    let mut mt: *mut Table = 0 as *mut Table;
    obj = index2value(L, objindex);
    if (*((*L).top.p).offset(-(1 as libc::c_int as isize))).val.tt_ as libc::c_int
        & 0xf as libc::c_int
        == 0 as libc::c_int
    {
        mt = 0 as *mut Table;
    } else {
        mt = &mut (*((*((*L).top.p).offset(-(1 as libc::c_int as isize)))
            .val
            .value_
            .gc as *mut GCUnion))
            .h;
    }
    match (*obj).tt_ as libc::c_int & 0xf as libc::c_int {
        5 => {
            let ref mut fresh3 = (*((*obj).value_.gc as *mut GCUnion)).h.metatable;
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
                    luaC_barrier_(
                        L,
                        &mut (*((*obj).value_.gc as *mut GCUnion)).gc,
                        &mut (*(mt as *mut GCUnion)).gc,
                    );
                } else {
                };
                luaC_checkfinalizer(L, (*obj).value_.gc, mt);
            }
        }
        7 => {
            let ref mut fresh4 = (*((*obj).value_.gc as *mut GCUnion)).u.metatable;
            *fresh4 = mt;
            if !mt.is_null() {
                if (*((*obj).value_.gc as *mut GCUnion)).u.marked as libc::c_int
                    & (1 as libc::c_int) << 5 as libc::c_int
                    != 0
                    && (*mt).marked as libc::c_int
                        & ((1 as libc::c_int) << 3 as libc::c_int
                            | (1 as libc::c_int) << 4 as libc::c_int)
                        != 0
                {
                    luaC_barrier_(
                        L,
                        &mut (*(&mut (*((*obj).value_.gc as *mut GCUnion)).u as *mut Udata
                            as *mut GCUnion))
                            .gc,
                        &mut (*(mt as *mut GCUnion)).gc,
                    );
                } else {
                };
                luaC_checkfinalizer(L, (*obj).value_.gc, mt);
            }
        }
        _ => {
            (*(*L).l_G).mt[((*obj).tt_ as libc::c_int & 0xf as libc::c_int) as usize] = mt;
        }
    }
    (*L).top.p = ((*L).top.p).offset(-1);
    (*L).top.p;
    return 1 as libc::c_int;
}

pub unsafe fn lua_setiuservalue(
    mut L: *mut lua_State,
    mut idx: libc::c_int,
    mut n: libc::c_int,
) -> libc::c_int {
    let mut o: *mut TValue = 0 as *mut TValue;
    let mut res: libc::c_int = 0;
    o = index2value(L, idx);
    if !((n as libc::c_uint).wrapping_sub(1 as libc::c_uint)
        < (*((*o).value_.gc as *mut GCUnion)).u.nuvalue as libc::c_uint)
    {
        res = 0 as libc::c_int;
    } else {
        let mut io1: *mut TValue = &mut (*((*((*o).value_.gc as *mut GCUnion)).u.uv)
            .as_mut_ptr()
            .offset((n - 1 as libc::c_int) as isize))
        .uv;
        let mut io2: *const TValue = &mut (*((*L).top.p).offset(-(1 as libc::c_int as isize))).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        if (*((*L).top.p).offset(-(1 as libc::c_int as isize))).val.tt_ as libc::c_int
            & (1 as libc::c_int) << 6 as libc::c_int
            != 0
        {
            if (*(*o).value_.gc).marked as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
                && (*(*((*L).top.p).offset(-(1 as libc::c_int as isize)))
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
    (*L).top.p = ((*L).top.p).offset(-1);
    (*L).top.p;
    return res;
}

pub unsafe fn lua_call(
    L: *mut lua_State,
    nargs: libc::c_int,
    nresults: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let func = ((*L).top.p).offset(-((nargs + 1) as isize));

    luaD_callnoyield(L, func, nresults)?;

    if nresults <= -1 && (*(*L).ci).top.p < (*L).top.p {
        (*(*L).ci).top.p = (*L).top.p;
    }

    Ok(())
}

pub unsafe fn lua_pcall(
    mut L: *mut lua_State,
    mut nargs: libc::c_int,
    mut nresults: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let func = ((*L).top.p).offset(-((nargs + 1) as isize));
    let mut c: CallS = CallS { func, nresults };
    let status = luaD_pcall(
        L,
        (c.func as *mut libc::c_char).offset_from((*L).stack.p as *mut libc::c_char),
        |L| luaD_callnoyield(L, c.func, c.nresults),
    );

    if nresults <= -1 && (*(*L).ci).top.p < (*L).top.p {
        (*(*L).ci).top.p = (*L).top.p;
    }

    status
}

pub unsafe fn lua_load(
    mut L: *mut lua_State,
    mut reader: lua_Reader,
    mut data: *mut libc::c_void,
    mut chunkname: *const libc::c_char,
    mut mode: *const libc::c_char,
) -> Result<(), Box<dyn std::error::Error>> {
    if chunkname.is_null() {
        chunkname = b"?\0" as *const u8 as *const libc::c_char;
    }

    let mut z = Zio::new(reader, data);
    let status = luaD_protectedparser(L, &mut z, chunkname, mode);

    if status.is_ok() {
        let mut f: *mut LClosure = &mut (*((*((*L).top.p).offset(-(1 as libc::c_int as isize)))
            .val
            .value_
            .gc as *mut GCUnion))
            .cl
            .l;
        if (*f).nupvalues as libc::c_int >= 1 as libc::c_int {
            let mut gt: *const TValue = &mut *((*((*(*L).l_G).l_registry.value_.gc
                as *mut GCUnion))
                .h
                .array)
                .offset((2 as libc::c_int - 1 as libc::c_int) as isize)
                as *mut TValue;
            let mut io1: *mut TValue =
                (**((*f).upvals).as_mut_ptr().offset(0 as libc::c_int as isize))
                    .v
                    .p;
            let mut io2: *const TValue = gt;
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
                        &mut (*(*((*f).upvals).as_mut_ptr().offset(0 as libc::c_int as isize)
                            as *mut GCUnion))
                            .gc,
                        &mut (*((*gt).value_.gc as *mut GCUnion)).gc,
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
    mut L: *mut lua_State,
    mut writer: lua_Writer,
    mut data: *mut c_void,
    strip: c_int,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let o = &raw mut (*((*L).top.p).offset(-1)).val;

    if (*o).tt_ as libc::c_int
        == 6 as libc::c_int
            | (0 as libc::c_int) << 4 as libc::c_int
            | (1 as libc::c_int) << 6 as libc::c_int
    {
        luaU_dump(
            L,
            (*((*o).value_.gc as *mut GCUnion)).cl.l.p,
            writer,
            data,
            strip,
        )
    } else {
        Ok(1)
    }
}

pub unsafe fn lua_status(mut L: *mut lua_State) -> libc::c_int {
    return (*L).status as libc::c_int;
}

pub unsafe fn lua_gc(mut L: *mut lua_State, cmd: GcCommand) -> libc::c_int {
    let mut res: libc::c_int = 0 as libc::c_int;
    let mut g: *mut global_State = (*L).l_G;

    if (*g).gcstp as libc::c_int & 2 as libc::c_int != 0 {
        return -1;
    }

    match cmd {
        GcCommand::Stop => {
            (*g).gcstp = 1 as libc::c_int as u8;
        }
        GcCommand::Restart => {
            luaE_setdebt(g, 0 as libc::c_int as isize);
            (*g).gcstp = 0 as libc::c_int as u8;
        }
        GcCommand::Collect => {
            luaC_fullgc(L, 0 as libc::c_int);
        }
        GcCommand::Count => {
            res = (((*g).totalbytes + (*g).GCdebt) as usize >> 10 as libc::c_int) as libc::c_int;
        }
        GcCommand::CountByte => {
            res = (((*g).totalbytes + (*g).GCdebt) as usize & 0x3ff as libc::c_int as usize)
                as libc::c_int;
        }
        GcCommand::Step(data) => {
            let mut debt: isize = 1 as libc::c_int as isize;
            let mut oldstp: u8 = (*g).gcstp;
            (*g).gcstp = 0 as libc::c_int as u8;
            if data == 0 as libc::c_int {
                luaE_setdebt(g, 0 as libc::c_int as isize);
                luaC_step(L);
            } else {
                debt = data as isize * 1024 as libc::c_int as isize + (*g).GCdebt;
                luaE_setdebt(g, debt);
                if (*(*L).l_G).GCdebt > 0 as libc::c_int as isize {
                    luaC_step(L);
                }
            }
            (*g).gcstp = oldstp;
            if debt > 0 as libc::c_int as isize && (*g).gcstate as libc::c_int == 8 as libc::c_int {
                res = 1 as libc::c_int;
            }
        }
        GcCommand::SetPause(data_0) => {
            res = (*g).gcpause as libc::c_int * 4 as libc::c_int;
            (*g).gcpause = (data_0 / 4 as libc::c_int) as u8;
        }
        GcCommand::SetStepMul(data_1) => {
            res = (*g).gcstepmul as libc::c_int * 4 as libc::c_int;
            (*g).gcstepmul = (data_1 / 4 as libc::c_int) as u8;
        }
        GcCommand::GetRunning => {
            res = ((*g).gcstp as libc::c_int == 0 as libc::c_int) as libc::c_int;
        }
        GcCommand::SetGen(minormul, majormul) => {
            res = if (*g).gckind as libc::c_int == 1 as libc::c_int
                || (*g).lastatomic != 0 as libc::c_int as usize
            {
                10 as libc::c_int
            } else {
                11 as libc::c_int
            };
            if minormul != 0 as libc::c_int {
                (*g).genminormul = minormul as u8;
            }
            if majormul != 0 as libc::c_int {
                (*g).genmajormul = (majormul / 4 as libc::c_int) as u8;
            }
            luaC_changemode(L, 1 as libc::c_int);
        }
        GcCommand::SetInc(pause, stepmul, stepsize) => {
            res = if (*g).gckind as libc::c_int == 1 as libc::c_int
                || (*g).lastatomic != 0 as libc::c_int as usize
            {
                10 as libc::c_int
            } else {
                11 as libc::c_int
            };
            if pause != 0 as libc::c_int {
                (*g).gcpause = (pause / 4 as libc::c_int) as u8;
            }
            if stepmul != 0 as libc::c_int {
                (*g).gcstepmul = (stepmul / 4 as libc::c_int) as u8;
            }
            if stepsize != 0 as libc::c_int {
                (*g).gcstepsize = stepsize as u8;
            }
            luaC_changemode(L, 0 as libc::c_int);
        }
    }

    return res;
}

pub unsafe fn lua_next(
    mut L: *mut lua_State,
    mut idx: libc::c_int,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut t: *mut Table = 0 as *mut Table;
    let mut more: libc::c_int = 0;
    t = gettable(L, idx);
    more = luaH_next(L, t, ((*L).top.p).offset(-(1 as libc::c_int as isize)))?;
    if more != 0 {
        api_incr_top(L);
    } else {
        (*L).top.p = ((*L).top.p).offset(-(1 as libc::c_int as isize));
    }
    return Ok(more);
}

pub unsafe fn lua_toclose(
    mut L: *mut lua_State,
    mut idx: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut nresults: libc::c_int = 0;
    let mut o: StkId = 0 as *mut StackValue;
    o = index2stack(L, idx);
    nresults = (*(*L).ci).nresults as libc::c_int;
    luaF_newtbcupval(L, o)?;
    if !(nresults < -(1 as libc::c_int)) {
        (*(*L).ci).nresults = (-nresults - 3 as libc::c_int) as libc::c_short;
    }
    Ok(())
}

pub unsafe fn lua_concat(
    mut L: *mut lua_State,
    mut n: c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    if n > 0 as libc::c_int {
        luaV_concat(L, n)?;
    } else {
        let mut io: *mut TValue = &mut (*(*L).top.p).val;
        let mut x_: *mut TString = luaS_newlstr(
            L,
            b"\0" as *const u8 as *const libc::c_char,
            0 as libc::c_int as usize,
        )?;
        (*io).value_.gc = &mut (*(x_ as *mut GCUnion)).gc;
        (*io).tt_ = ((*x_).tt as libc::c_int | (1 as libc::c_int) << 6 as libc::c_int) as u8;
        api_incr_top(L);
    }
    if (*(*L).l_G).GCdebt > 0 as libc::c_int as isize {
        luaC_step(L);
    }
    Ok(())
}

pub unsafe fn lua_len(
    mut L: *mut lua_State,
    mut idx: c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut t: *mut TValue = 0 as *mut TValue;
    t = index2value(L, idx);
    luaV_objlen(L, (*L).top.p, t)?;
    api_incr_top(L);
    Ok(())
}

pub unsafe fn lua_setwarnf(
    mut L: *mut lua_State,
    mut f: lua_WarnFunction,
    mut ud: *mut libc::c_void,
) {
    (*(*L).l_G).ud_warn = ud;
    (*(*L).l_G).warnf = f;
}

pub unsafe fn lua_warning(
    mut L: *mut lua_State,
    mut msg: *const libc::c_char,
    mut tocont: libc::c_int,
) {
    luaE_warning(L, msg, tocont);
}

pub unsafe fn lua_newuserdatauv(
    mut L: *mut lua_State,
    mut size: usize,
    mut nuvalue: libc::c_int,
) -> Result<*mut c_void, Box<dyn std::error::Error>> {
    let mut u: *mut Udata = 0 as *mut Udata;
    u = luaS_newudata(L, size, nuvalue)?;
    let mut io: *mut TValue = &mut (*(*L).top.p).val;
    let mut x_: *mut Udata = u;
    (*io).value_.gc = &mut (*(x_ as *mut GCUnion)).gc;
    (*io).tt_ = (7 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 6 as libc::c_int) as u8;
    api_incr_top(L);
    if (*(*L).l_G).GCdebt > 0 as libc::c_int as isize {
        luaC_step(L);
    }
    return Ok((u as *mut libc::c_char).offset(
        (if (*u).nuvalue as libc::c_int == 0 as libc::c_int {
            32 as libc::c_ulong
        } else {
            (40 as libc::c_ulong).wrapping_add(
                (::core::mem::size_of::<UValue>() as libc::c_ulong)
                    .wrapping_mul((*u).nuvalue as libc::c_ulong),
            )
        }) as isize,
    ) as *mut libc::c_void);
}

unsafe fn aux_upvalue(
    mut fi: *mut TValue,
    mut n: libc::c_int,
    mut val: *mut *mut TValue,
    mut owner: *mut *mut GCObject,
) -> *const libc::c_char {
    match (*fi).tt_ as libc::c_int & 0x3f as libc::c_int {
        38 => {
            let mut f: *mut CClosure = &mut (*((*fi).value_.gc as *mut GCUnion)).cl.c;
            if !((n as libc::c_uint).wrapping_sub(1 as libc::c_uint)
                < (*f).nupvalues as libc::c_uint)
            {
                return 0 as *const libc::c_char;
            }
            *val = &mut *((*f).upvalue)
                .as_mut_ptr()
                .offset((n - 1 as libc::c_int) as isize) as *mut TValue;
            if !owner.is_null() {
                *owner = &mut (*(f as *mut GCUnion)).gc;
            }
            return b"\0" as *const u8 as *const libc::c_char;
        }
        6 => {
            let mut f_0: *mut LClosure = &mut (*((*fi).value_.gc as *mut GCUnion)).cl.l;
            let mut name: *mut TString = 0 as *mut TString;
            let mut p: *mut Proto = (*f_0).p;
            if !((n as libc::c_uint).wrapping_sub(1 as libc::c_uint)
                < (*p).sizeupvalues as libc::c_uint)
            {
                return 0 as *const libc::c_char;
            }
            *val = (**((*f_0).upvals)
                .as_mut_ptr()
                .offset((n - 1 as libc::c_int) as isize))
            .v
            .p;
            if !owner.is_null() {
                *owner = &mut (*(*((*f_0).upvals)
                    .as_mut_ptr()
                    .offset((n - 1 as libc::c_int) as isize)
                    as *mut GCUnion))
                    .gc;
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
    mut L: *mut lua_State,
    mut funcindex: libc::c_int,
    mut n: libc::c_int,
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
        let mut io1: *mut TValue = &mut (*(*L).top.p).val;
        let mut io2: *const TValue = val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        api_incr_top(L);
    }
    return name;
}

pub unsafe fn lua_setupvalue(
    mut L: *mut lua_State,
    mut funcindex: libc::c_int,
    mut n: libc::c_int,
) -> *const libc::c_char {
    let mut name: *const libc::c_char = 0 as *const libc::c_char;
    let mut val: *mut TValue = 0 as *mut TValue;
    let mut owner: *mut GCObject = 0 as *mut GCObject;
    let mut fi: *mut TValue = 0 as *mut TValue;
    fi = index2value(L, funcindex);
    name = aux_upvalue(fi, n, &mut val, &mut owner);
    if !name.is_null() {
        (*L).top.p = ((*L).top.p).offset(-1);
        (*L).top.p;
        let mut io1: *mut TValue = val;
        let mut io2: *const TValue = &mut (*(*L).top.p).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        if (*val).tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0 {
            if (*owner).marked as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
                && (*(*val).value_.gc).marked as libc::c_int
                    & ((1 as libc::c_int) << 3 as libc::c_int
                        | (1 as libc::c_int) << 4 as libc::c_int)
                    != 0
            {
                luaC_barrier_(
                    L,
                    &mut (*(owner as *mut GCUnion)).gc,
                    &mut (*((*val).value_.gc as *mut GCUnion)).gc,
                );
            } else {
            };
        } else {
        };
    }
    return name;
}

unsafe fn getupvalref(
    mut L: *mut lua_State,
    mut fidx: libc::c_int,
    mut n: libc::c_int,
    mut pf: *mut *mut LClosure,
) -> *mut *mut UpVal {
    static mut nullup: *const UpVal = 0 as *const UpVal;
    let mut f: *mut LClosure = 0 as *mut LClosure;
    let mut fi: *mut TValue = index2value(L, fidx);
    f = &mut (*((*fi).value_.gc as *mut GCUnion)).cl.l;
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
    mut L: *mut lua_State,
    mut fidx: libc::c_int,
    mut n: libc::c_int,
) -> *mut libc::c_void {
    let mut fi: *mut TValue = index2value(L, fidx);
    match (*fi).tt_ as libc::c_int & 0x3f as libc::c_int {
        6 => {
            return *getupvalref(L, fidx, n, 0 as *mut *mut LClosure) as *mut libc::c_void;
        }
        38 => {
            let mut f: *mut CClosure = &mut (*((*fi).value_.gc as *mut GCUnion)).cl.c;
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
    mut L: *mut lua_State,
    mut fidx1: libc::c_int,
    mut n1: libc::c_int,
    mut fidx2: libc::c_int,
    mut n2: libc::c_int,
) {
    let mut f1: *mut LClosure = 0 as *mut LClosure;
    let mut up1: *mut *mut UpVal = getupvalref(L, fidx1, n1, &mut f1);
    let mut up2: *mut *mut UpVal = getupvalref(L, fidx2, n2, 0 as *mut *mut LClosure);
    *up1 = *up2;
    if (*f1).marked as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
        && (**up1).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            != 0
    {
        luaC_barrier_(
            L,
            &mut (*(f1 as *mut GCUnion)).gc,
            &mut (*(*up1 as *mut GCUnion)).gc,
        );
    } else {
    };
}
