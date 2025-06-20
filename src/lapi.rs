#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments
)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::gc::{luaC_barrier_, luaC_barrierback_};
use crate::ldebug::lua_getinfo;
use crate::ldo::{luaD_call, luaD_closeprotected, luaD_growstack, luaD_shrinkstack};
use crate::lfunc::{luaF_close, luaF_newCclosure, luaF_newtbcupval};
use crate::lobject::{
    CClosure, Proto, StackValue, StkId, Udata, UpVal, luaO_arith, luaO_str2num, luaO_tostring,
};
use crate::lstate::{CallInfo, lua_Debug};
use crate::lstring::luaS_newudata;
use crate::ltm::{TM_GC, luaT_gettm, luaT_typenames_};
use crate::lvm::{
    F2Ieq, luaV_concat, luaV_equalobj, luaV_finishget, luaV_finishset, luaV_lessequal,
    luaV_lessthan, luaV_objlen, luaV_tointeger, luaV_tonumber_,
};
use crate::table::{
    luaH_get, luaH_getint, luaH_getn, luaH_getstr, luaH_new, luaH_next, luaH_resize, luaH_setint,
};
use crate::value::{UnsafeValue, UntaggedValue};
use crate::{
    Args, Context, LuaFn, Object, Ret, StackOverflow, Str, Table, TableError, Thread, api_incr_top,
};
use alloc::boxed::Box;
use alloc::string::String;
use core::ffi::{CStr, c_void};
use core::mem::offset_of;
use core::ptr::{null, null_mut};

type c_int = i32;

unsafe fn index2value(L: *const Thread, mut idx: c_int) -> *mut UnsafeValue {
    let ci: *mut CallInfo = (*L).ci.get();
    if idx > 0 as c_int {
        let o: StkId = ((*ci).func).offset(idx as isize);
        if o >= (*L).top.get() {
            return (*(*L).hdr.global).nilvalue.get();
        } else {
            return &raw mut (*o).val;
        }
    } else if !(idx <= -(1000000 as c_int) - 1000 as c_int) {
        return &raw mut (*((*L).top.get()).offset(idx as isize)).val;
    } else if idx == -(1000000 as c_int) - 1000 as c_int {
        return (*(*L).hdr.global).l_registry.get();
    } else {
        idx = -(1000000 as c_int) - 1000 as c_int - idx;
        if (*(*ci).func).val.tt_ as c_int
            == 6 as c_int | (2 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int
        {
            let func: *mut CClosure = (*(*ci).func).val.value_.gc as *mut CClosure;
            return if idx <= (*func).nupvalues as c_int {
                &mut *((*func).upvalue)
                    .as_mut_ptr()
                    .offset((idx - 1 as c_int) as isize) as *mut UnsafeValue
            } else {
                (*(*L).hdr.global).nilvalue.get()
            };
        } else {
            return (*(*L).hdr.global).nilvalue.get();
        }
    };
}

unsafe fn index2stack(L: *const Thread, idx: c_int) -> StkId {
    let ci: *mut CallInfo = (*L).ci.get();
    if idx > 0 as c_int {
        let o: StkId = ((*ci).func).offset(idx as isize);
        return o;
    } else {
        return ((*L).top.get()).offset(idx as isize);
    };
}

#[inline(always)]
pub unsafe fn lua_checkstack(L: *const Thread, n: usize) -> Result<(), StackOverflow> {
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

pub unsafe fn lua_xmove(from: *mut Thread, to: *mut Thread, n: c_int) {
    let mut i: c_int = 0;
    if from == to {
        return;
    }
    (*from).top.sub(n.try_into().unwrap());
    i = 0 as c_int;
    while i < n {
        let io1: *mut UnsafeValue = &raw mut (*(*to).top.get()).val;
        let io2: *const UnsafeValue = &raw mut (*((*from).top.get()).offset(i as isize)).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        (*to).top.add(1);

        i += 1;
    }
}

pub unsafe fn lua_absindex(L: *const Thread, idx: c_int) -> c_int {
    return if idx > 0 as c_int || idx <= -(1000000 as c_int) - 1000 as c_int {
        idx
    } else {
        ((*L).top.get()).offset_from((*(*L).ci.get()).func) as libc::c_long as c_int + idx
    };
}

pub unsafe fn lua_gettop(L: *const Thread) -> c_int {
    return ((*L).top.get()).offset_from(((*(*L).ci.get()).func).offset(1 as c_int as isize))
        as libc::c_long as c_int;
}

pub unsafe fn lua_settop(L: *const Thread, idx: c_int) -> Result<(), Box<dyn core::error::Error>> {
    let mut ci: *mut CallInfo = 0 as *mut CallInfo;
    let mut func: StkId = 0 as *mut StackValue;
    let mut newtop: StkId = 0 as *mut StackValue;
    let mut diff: isize = 0;
    ci = (*L).ci.get();
    func = (*ci).func;
    if idx >= 0 as c_int {
        diff = func
            .offset(1 as c_int as isize)
            .offset(idx as isize)
            .offset_from((*L).top.get());
        while diff > 0 as c_int as isize {
            let fresh1 = (*L).top.get();
            (*L).top.add(1);
            (*fresh1).val.tt_ = (0 as c_int | (0 as c_int) << 4 as c_int) as u8;
            diff -= 1;
        }
    } else {
        diff = (idx + 1 as c_int) as isize;
    }

    newtop = ((*L).top.get()).offset(diff as isize);

    if diff < 0 as c_int as isize && (*L).tbclist.get() >= newtop {
        newtop = luaF_close(L, newtop)?;
    }

    (*L).top.set(newtop);

    Ok(())
}

pub unsafe fn lua_closeslot(L: *mut Thread, idx: c_int) -> Result<(), Box<dyn core::error::Error>> {
    let mut level: StkId = 0 as *mut StackValue;
    level = index2stack(L, idx);
    level = luaF_close(L, level)?;
    (*level).val.tt_ = (0 as c_int | (0 as c_int) << 4 as c_int) as u8;
    Ok(())
}

unsafe fn reverse(mut from: StkId, mut to: StkId) {
    while from < to {
        let mut temp: UnsafeValue = UnsafeValue {
            value_: UntaggedValue {
                gc: 0 as *mut Object,
            },
            tt_: 0,
        };
        let io1: *mut UnsafeValue = &mut temp;
        let io2: *const UnsafeValue = &mut (*from).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        let io1_0: *mut UnsafeValue = &mut (*from).val;
        let io2_0: *const UnsafeValue = &mut (*to).val;
        (*io1_0).value_ = (*io2_0).value_;
        (*io1_0).tt_ = (*io2_0).tt_;
        let io1_1: *mut UnsafeValue = &mut (*to).val;
        let io2_1: *const UnsafeValue = &mut temp;
        (*io1_1).value_ = (*io2_1).value_;
        (*io1_1).tt_ = (*io2_1).tt_;
        from = from.offset(1);
        to = to.offset(-1);
    }
}

pub unsafe fn lua_rotate(L: *const Thread, idx: c_int, n: c_int) {
    let mut p: StkId = 0 as *mut StackValue;
    let mut t: StkId = 0 as *mut StackValue;
    let mut m: StkId = 0 as *mut StackValue;
    t = ((*L).top.get()).offset(-(1 as c_int as isize));
    p = index2stack(L, idx);
    m = if n >= 0 as c_int {
        t.offset(-(n as isize))
    } else {
        p.offset(-(n as isize)).offset(-(1 as c_int as isize))
    };
    reverse(p, m);
    reverse(m.offset(1 as c_int as isize), t);
    reverse(p, t);
}

pub unsafe fn lua_copy(L: *const Thread, fromidx: c_int, toidx: c_int) {
    let mut fr: *mut UnsafeValue = 0 as *mut UnsafeValue;
    let mut to: *mut UnsafeValue = 0 as *mut UnsafeValue;
    fr = index2value(L, fromidx);
    to = index2value(L, toidx);
    let io1: *mut UnsafeValue = to;
    let io2: *const UnsafeValue = fr;
    (*io1).value_ = (*io2).value_;
    (*io1).tt_ = (*io2).tt_;
    if toidx < -(1000000 as c_int) - 1000 as c_int {
        if (*fr).tt_ as c_int & (1 as c_int) << 6 as c_int != 0 {
            if (*((*(*(*L).ci.get()).func).val.value_.gc as *mut CClosure))
                .hdr
                .marked
                .get() as c_int
                & (1 as c_int) << 5 as c_int
                != 0
                && (*(*fr).value_.gc).marked.get() as c_int
                    & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                    != 0
            {
                luaC_barrier_(
                    (*L).hdr.global,
                    ((*(*(*L).ci.get()).func).val.value_.gc as *mut CClosure) as *mut CClosure
                        as *mut Object,
                    (*fr).value_.gc as *mut Object,
                );
            }
        }
    }
}

pub unsafe fn lua_pushvalue(L: *const Thread, idx: c_int) {
    let io1: *mut UnsafeValue = &raw mut (*(*L).top.get()).val;
    let io2: *const UnsafeValue = index2value(L, idx);
    (*io1).value_ = (*io2).value_;
    (*io1).tt_ = (*io2).tt_;
    api_incr_top(L);
}

pub unsafe fn lua_type(L: *const Thread, idx: c_int) -> c_int {
    let o: *const UnsafeValue = index2value(L, idx);
    return if !((*o).tt_ as c_int & 0xf as c_int == 0 as c_int)
        || o != (*(*L).hdr.global).nilvalue.get() as *mut UnsafeValue as *const UnsafeValue
    {
        (*o).tt_ as c_int & 0xf as c_int
    } else {
        -(1 as c_int)
    };
}

pub const fn lua_typename(t: c_int) -> &'static str {
    luaT_typenames_[(t + 1) as usize]
}

pub unsafe fn lua_iscfunction(L: *mut Thread, idx: c_int) -> c_int {
    let o: *const UnsafeValue = index2value(L, idx);
    return (((*o).tt_ & 0xF) == 2
        || (*o).tt_ as c_int
            == 6 as c_int | (2 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int)
        as c_int;
}

pub unsafe fn lua_isinteger(L: *const Thread, idx: c_int) -> c_int {
    let o: *const UnsafeValue = index2value(L, idx);
    return ((*o).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int) as c_int;
}

#[inline(never)]
pub unsafe fn lua_isnumber(L: *const Thread, idx: c_int) -> c_int {
    let mut n: f64 = 0.;
    let o: *const UnsafeValue = index2value(L, idx);
    return if (*o).tt_ == 3 | 1 << 4 {
        n = (*o).value_.n;
        1 as c_int
    } else {
        luaV_tonumber_(o, &mut n)
    };
}

pub unsafe fn lua_isstring(L: *const Thread, idx: c_int) -> c_int {
    let o: *const UnsafeValue = index2value(L, idx);
    return ((*o).tt_ as c_int & 0xf as c_int == 4 as c_int
        || (*o).tt_ as c_int & 0xf as c_int == 3 as c_int) as c_int;
}

pub unsafe fn lua_isuserdata(L: *mut Thread, idx: c_int) -> c_int {
    let o: *const UnsafeValue = index2value(L, idx);
    return ((*o).tt_ as c_int
        == 7 as c_int | (0 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int)
        as c_int;
}

pub unsafe fn lua_rawequal(
    L: *const Thread,
    index1: c_int,
    index2: c_int,
) -> Result<c_int, Box<dyn core::error::Error>> {
    let o1: *const UnsafeValue = index2value(L, index1);
    let o2: *const UnsafeValue = index2value(L, index2);
    return if (!((*o1).tt_ as c_int & 0xf as c_int == 0 as c_int)
        || o1 != (*(*L).hdr.global).nilvalue.get() as *mut UnsafeValue as *const UnsafeValue)
        && (!((*o2).tt_ as c_int & 0xf as c_int == 0 as c_int)
            || o2 != (*(*L).hdr.global).nilvalue.get() as *mut UnsafeValue as *const UnsafeValue)
    {
        luaV_equalobj(0 as *mut Thread, o1, o2)
    } else {
        Ok(0 as c_int)
    };
}

pub unsafe fn lua_arith(L: *const Thread, op: c_int) -> Result<(), Box<dyn core::error::Error>> {
    if !(op != 12 as c_int && op != 13 as c_int) {
        let io1: *mut UnsafeValue = &raw mut (*(*L).top.get()).val;
        let io2: *const UnsafeValue =
            &raw mut (*((*L).top.get()).offset(-(1 as c_int as isize))).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        api_incr_top(L);
    }
    luaO_arith(
        L,
        op,
        &raw mut (*((*L).top.get()).offset(-(2 as c_int as isize))).val,
        &raw mut (*((*L).top.get()).offset(-(1 as c_int as isize))).val,
        ((*L).top.get()).offset(-(2 as c_int as isize)),
    )?;
    (*L).top.sub(1);

    Ok(())
}

pub unsafe fn lua_compare(
    L: *const Thread,
    index1: c_int,
    index2: c_int,
    op: c_int,
) -> Result<c_int, Box<dyn core::error::Error>> {
    let mut o1: *const UnsafeValue = 0 as *const UnsafeValue;
    let mut o2: *const UnsafeValue = 0 as *const UnsafeValue;
    let mut i: c_int = 0 as c_int;
    o1 = index2value(L, index1);
    o2 = index2value(L, index2);
    if (!((*o1).tt_ as c_int & 0xf as c_int == 0 as c_int)
        || o1 != (*(*L).hdr.global).nilvalue.get() as *mut UnsafeValue as *const UnsafeValue)
        && (!((*o2).tt_ as c_int & 0xf as c_int == 0 as c_int)
            || o2 != (*(*L).hdr.global).nilvalue.get() as *mut UnsafeValue as *const UnsafeValue)
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
    if sz != 0 as c_int as usize {
        api_incr_top(L);
    }
    return sz;
}

pub unsafe fn lua_tonumberx(L: *const Thread, idx: c_int, pisnum: *mut c_int) -> f64 {
    let mut n: f64 = 0 as c_int as f64;
    let o: *const UnsafeValue = index2value(L, idx);
    let isnum: c_int = if (*o).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
        n = (*o).value_.n;
        1 as c_int
    } else {
        luaV_tonumber_(o, &mut n)
    };
    if !pisnum.is_null() {
        *pisnum = isnum;
    }
    return n;
}

#[inline(never)]
pub unsafe fn lua_tointegerx(L: *const Thread, idx: c_int, pisnum: *mut c_int) -> i64 {
    let mut res: i64 = 0 as c_int as i64;
    let o: *const UnsafeValue = index2value(L, idx);
    let isnum: c_int = if (*o).tt_ == 3 | 0 << 4 {
        res = (*o).value_.i;
        1 as c_int
    } else {
        luaV_tointeger(o, &mut res, F2Ieq)
    };

    if !pisnum.is_null() {
        *pisnum = isnum;
    }

    return res;
}

pub unsafe fn lua_toboolean(L: *const Thread, idx: c_int) -> c_int {
    let o: *const UnsafeValue = index2value(L, idx);

    return !((*o).tt_ == 1 | 0 << 4 || (*o).tt_ & 0xf == 0) as c_int;
}

#[inline(never)]
pub unsafe fn lua_tolstring(L: *const Thread, idx: c_int, convert: bool) -> *const Str {
    let mut o = index2value(L, idx);

    if !((*o).tt_ & 0xf == 4) {
        if !convert || !((*o).tt_ & 0xf == 3) {
            return null();
        }

        luaO_tostring((*L).hdr.global, o);

        if (*(*L).hdr.global).gc.debt() > 0 as c_int as isize {
            crate::gc::step((*L).hdr.global);
        }

        o = index2value(L, idx);
    }

    (*o).value_.gc.cast::<Str>()
}

pub unsafe fn lua_rawlen(L: *const Thread, idx: c_int) -> u64 {
    let o: *const UnsafeValue = index2value(L, idx);
    match (*o).tt_ as c_int & 0x3f as c_int {
        4 => return (*((*o).value_.gc as *mut Str)).shrlen.get() as u64,
        20 => return (*(*((*o).value_.gc as *mut Str)).u.get()).lnglen as u64,
        7 => return (*((*o).value_.gc as *mut Udata)).len as u64,
        5 => return luaH_getn((*o).value_.gc as *mut Table),
        _ => return 0 as c_int as u64,
    };
}

unsafe fn touserdata(o: *const UnsafeValue) -> *mut libc::c_void {
    match (*o).tt_ as c_int & 0xf as c_int {
        7 => (*o)
            .value_
            .gc
            .byte_add(
                offset_of!(Udata, uv)
                    + size_of::<UnsafeValue>()
                        * usize::from((*((*o).value_.gc as *mut Udata)).nuvalue),
            )
            .cast_mut()
            .cast(),
        _ => null_mut(),
    }
}

pub unsafe fn lua_touserdata(L: *const Thread, idx: c_int) -> *mut libc::c_void {
    let o: *const UnsafeValue = index2value(L, idx);
    return touserdata(o);
}

pub unsafe fn lua_tothread(L: *mut Thread, idx: c_int) -> *mut Thread {
    let o: *const UnsafeValue = index2value(L, idx);
    return if !((*o).tt_ as c_int
        == 8 as c_int | (0 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int)
    {
        0 as *mut Thread
    } else {
        (*o).value_.gc as *mut Thread
    };
}

pub unsafe fn lua_topointer(L: *const Thread, idx: c_int) -> *const libc::c_void {
    let o: *const UnsafeValue = index2value(L, idx);

    match (*o).tt_ as c_int & 0x3f as c_int {
        2 => (*o).value_.f as *const libc::c_void,
        18 | 34 | 50 => todo!(),
        7 => return touserdata(o),
        _ => {
            if (*o).tt_ as c_int & (1 as c_int) << 6 as c_int != 0 {
                return (*o).value_.gc as *const libc::c_void;
            } else {
                return 0 as *const libc::c_void;
            }
        }
    }
}

pub unsafe fn lua_pushnil(L: *const Thread) {
    (*(*L).top.get()).val.tt_ = (0 as c_int | (0 as c_int) << 4 as c_int) as u8;
    api_incr_top(L);
}

pub unsafe fn lua_pushnumber(L: *const Thread, n: f64) {
    let io: *mut UnsafeValue = &raw mut (*(*L).top.get()).val;
    (*io).value_.n = n;
    (*io).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
    api_incr_top(L);
}

pub unsafe fn lua_pushinteger(L: *const Thread, n: i64) {
    let io: *mut UnsafeValue = &raw mut (*(*L).top.get()).val;
    (*io).value_.i = n;
    (*io).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
    api_incr_top(L);
}

pub unsafe fn lua_pushlstring(L: *const Thread, s: impl AsRef<[u8]>) -> *const libc::c_char {
    let ts = Str::new((*L).hdr.global, s);
    let io: *mut UnsafeValue = &raw mut (*(*L).top.get()).val;

    (*io).value_.gc = ts.cast();
    (*io).tt_ = ((*ts).hdr.tt as c_int | (1 as c_int) << 6 as c_int) as u8;

    api_incr_top(L);

    if (*(*L).hdr.global).gc.debt() > 0 as c_int as isize {
        crate::gc::step((*L).hdr.global);
    }

    ((*ts).contents).as_ptr()
}

pub unsafe fn lua_pushstring(L: *const Thread, mut s: *const libc::c_char) -> *const libc::c_char {
    if s.is_null() {
        (*(*L).top.get()).val.tt_ = (0 as c_int | (0 as c_int) << 4 as c_int) as u8;
    } else {
        let ts = Str::new((*L).hdr.global, CStr::from_ptr(s).to_bytes());
        let io: *mut UnsafeValue = &raw mut (*(*L).top.get()).val;

        (*io).value_.gc = ts.cast();
        (*io).tt_ = ((*ts).hdr.tt as c_int | (1 as c_int) << 6 as c_int) as u8;

        s = ((*ts).contents).as_ptr();
    }

    api_incr_top(L);

    if (*(*L).hdr.global).gc.debt() > 0 as c_int as isize {
        crate::gc::step((*L).hdr.global);
    }

    s
}

pub unsafe fn lua_pushcclosure(
    L: *const Thread,
    fn_0: for<'a> fn(Context<'a, Args>) -> Result<Context<'a, Ret>, Box<dyn core::error::Error>>,
    mut n: c_int,
) {
    let cl = luaF_newCclosure((*L).hdr.global, n);

    (*cl).f = fn_0;
    (*L).top.sub(n.try_into().unwrap());

    loop {
        let fresh2 = n;
        n = n - 1;
        if !(fresh2 != 0) {
            break;
        }
        let io1: *mut UnsafeValue =
            &raw mut *((*cl).upvalue).as_mut_ptr().offset(n as isize) as *mut UnsafeValue;
        let io2: *const UnsafeValue = &raw mut (*((*L).top.get()).offset(n as isize)).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
    }

    let io_0: *mut UnsafeValue = &raw mut (*(*L).top.get()).val;
    let x_: *mut CClosure = cl;

    (*io_0).value_.gc = x_ as *mut Object;
    (*io_0).tt_ = (6 as c_int | (2 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int) as u8;

    api_incr_top(L);

    if (*(*L).hdr.global).gc.debt() > 0 as c_int as isize {
        crate::gc::step((*L).hdr.global);
    }
}

pub unsafe fn lua_pushthread(L: *mut Thread) {
    let io: *mut UnsafeValue = &raw mut (*(*L).top.get()).val;
    let x_: *mut Thread = L;
    (*io).value_.gc = x_ as *mut Object;
    (*io).tt_ = (8 as c_int | (0 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int) as u8;
    api_incr_top(L);
}

unsafe fn auxgetstr(
    L: *const Thread,
    t: *const UnsafeValue,
    k: &[u8],
) -> Result<c_int, Box<dyn core::error::Error>> {
    let mut slot: *const UnsafeValue = 0 as *const UnsafeValue;
    let str = Str::new((*L).hdr.global, k);

    if if !((*t).tt_ as c_int == 5 as c_int | (0 as c_int) << 4 as c_int | (1 as c_int) << 6) {
        slot = 0 as *const UnsafeValue;
        0 as c_int
    } else {
        slot = luaH_getstr((*t).value_.gc.cast(), str);
        !((*slot).tt_ as c_int & 0xf as c_int == 0 as c_int) as c_int
    } != 0
    {
        let io1: *mut UnsafeValue = &raw mut (*(*L).top.get()).val;
        let io2: *const UnsafeValue = slot;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        api_incr_top(L);
    } else {
        let io: *mut UnsafeValue = &raw mut (*(*L).top.get()).val;

        (*io).value_.gc = str.cast();
        (*io).tt_ = ((*str).hdr.tt as c_int | (1 as c_int) << 6 as c_int) as u8;

        api_incr_top(L);
        luaV_finishget(
            L,
            t,
            &raw mut (*((*L).top.get()).offset(-(1 as c_int as isize))).val,
            ((*L).top.get()).offset(-(1 as c_int as isize)),
            slot,
        )?;
    }

    return Ok((*((*L).top.get()).offset(-(1 as c_int as isize))).val.tt_ as c_int & 0xf as c_int);
}

pub unsafe fn lua_getglobal(
    L: *mut Thread,
    name: impl AsRef<[u8]>,
) -> Result<c_int, Box<dyn core::error::Error>> {
    let mut G: *const UnsafeValue = 0 as *const UnsafeValue;
    G = (*((*(*(*L).hdr.global).l_registry.get()).value_.gc as *mut Table))
        .array
        .get()
        .offset((2 as c_int - 1 as c_int) as isize) as *mut UnsafeValue;
    return auxgetstr(L, G, name.as_ref());
}

pub unsafe fn lua_gettable(
    L: *const Thread,
    idx: c_int,
) -> Result<c_int, Box<dyn core::error::Error>> {
    let mut slot: *const UnsafeValue = 0 as *const UnsafeValue;
    let mut t: *mut UnsafeValue = 0 as *mut UnsafeValue;
    t = index2value(L, idx);
    if if !((*t).tt_ as c_int
        == 5 as c_int | (0 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int)
    {
        slot = 0 as *const UnsafeValue;
        0 as c_int
    } else {
        slot = luaH_get(
            (*t).value_.gc as *mut Table,
            &raw mut (*((*L).top.get()).offset(-(1 as c_int as isize))).val,
        );
        !((*slot).tt_ as c_int & 0xf as c_int == 0 as c_int) as c_int
    } != 0
    {
        let io1: *mut UnsafeValue = &raw mut (*((*L).top.get()).offset(-(1 as c_int as isize))).val;
        let io2: *const UnsafeValue = slot;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
    } else {
        luaV_finishget(
            L,
            t,
            &raw mut (*((*L).top.get()).offset(-(1 as c_int as isize))).val,
            ((*L).top.get()).offset(-(1 as c_int as isize)),
            slot,
        )?;
    }
    return Ok((*((*L).top.get()).offset(-(1 as c_int as isize))).val.tt_ as c_int & 0xf as c_int);
}

pub unsafe fn lua_getfield(
    L: *const Thread,
    idx: c_int,
    k: impl AsRef<[u8]>,
) -> Result<c_int, Box<dyn core::error::Error>> {
    return auxgetstr(L, index2value(L, idx), k.as_ref());
}

pub unsafe fn lua_geti(
    L: *const Thread,
    idx: c_int,
    n: i64,
) -> Result<c_int, Box<dyn core::error::Error>> {
    let mut t: *mut UnsafeValue = 0 as *mut UnsafeValue;
    let mut slot: *const UnsafeValue = 0 as *const UnsafeValue;
    t = index2value(L, idx);
    if if !((*t).tt_ as c_int
        == 5 as c_int | (0 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int)
    {
        slot = 0 as *const UnsafeValue;
        0 as c_int
    } else {
        slot = if (n as u64).wrapping_sub(1 as libc::c_uint as u64)
            < (*((*t).value_.gc as *mut Table)).alimit.get() as u64
        {
            (*((*t).value_.gc as *mut Table))
                .array
                .get()
                .offset((n - 1 as c_int as i64) as isize) as *mut UnsafeValue
                as *const UnsafeValue
        } else {
            luaH_getint((*t).value_.gc as *mut Table, n)
        };
        !((*slot).tt_ as c_int & 0xf as c_int == 0 as c_int) as c_int
    } != 0
    {
        let io1: *mut UnsafeValue = &raw mut (*(*L).top.get()).val;
        let io2: *const UnsafeValue = slot;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
    } else {
        let mut aux: UnsafeValue = UnsafeValue {
            value_: UntaggedValue {
                gc: 0 as *mut Object,
            },
            tt_: 0,
        };
        let io: *mut UnsafeValue = &mut aux;
        (*io).value_.i = n;
        (*io).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
        luaV_finishget(L, t, &mut aux, (*L).top.get(), slot)?;
    }
    api_incr_top(L);

    return Ok((*((*L).top.get()).offset(-(1 as c_int as isize))).val.tt_ as c_int & 0xf as c_int);
}

unsafe fn finishrawget(L: *const Thread, val: *const UnsafeValue) -> c_int {
    if (*val).tt_ as c_int & 0xf as c_int == 0 as c_int {
        (*(*L).top.get()).val.tt_ = (0 as c_int | (0 as c_int) << 4 as c_int) as u8;
    } else {
        let io1: *mut UnsafeValue = &raw mut (*(*L).top.get()).val;
        let io2: *const UnsafeValue = val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
    }
    api_incr_top(L);
    return (*((*L).top.get()).offset(-(1 as c_int as isize))).val.tt_ as c_int & 0xf as c_int;
}

unsafe fn gettable(L: *const Thread, idx: c_int) -> *mut Table {
    let t: *mut UnsafeValue = index2value(L, idx);
    return (*t).value_.gc as *mut Table;
}

pub unsafe fn lua_rawget(L: *const Thread, idx: c_int) -> c_int {
    let mut t: *mut Table = 0 as *mut Table;
    let mut val: *const UnsafeValue = 0 as *const UnsafeValue;
    t = gettable(L, idx);
    val = luaH_get(
        t,
        &raw mut (*((*L).top.get()).offset(-(1 as c_int as isize))).val,
    );
    (*L).top.sub(1);

    return finishrawget(L, val);
}

pub unsafe fn lua_rawgeti(L: *const Thread, idx: c_int, n: i64) -> c_int {
    let mut t: *mut Table = 0 as *mut Table;
    t = gettable(L, idx);
    return finishrawget(L, luaH_getint(t, n));
}

pub unsafe fn lua_createtable(L: *const Thread, narray: c_int, nrec: c_int) {
    let t = luaH_new((*L).hdr.global);
    let io: *mut UnsafeValue = &raw mut (*(*L).top.get()).val;

    (*io).value_.gc = t.cast();
    (*io).tt_ = (5 as c_int | (0 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int) as u8;

    api_incr_top(L);

    if narray > 0 as c_int || nrec > 0 as c_int {
        luaH_resize(t, narray as libc::c_uint, nrec as libc::c_uint);
    }

    if (*(*L).hdr.global).gc.debt() > 0 as c_int as isize {
        crate::gc::step((*L).hdr.global);
    }
}

pub unsafe fn lua_getmetatable(L: *const Thread, objindex: c_int) -> c_int {
    let mut res: c_int = 0 as c_int;
    let obj = index2value(L, objindex);
    let mt = match (*obj).tt_ as c_int & 0xf as c_int {
        5 => (*((*obj).value_.gc as *mut Table)).metatable.get(),
        7 => (*((*obj).value_.gc as *mut Udata)).metatable,
        _ => (*(*L).hdr.global).primitive_mt[usize::from((*obj).tt_ & 0xf)].get(),
    };

    if !mt.is_null() {
        let io: *mut UnsafeValue = &raw mut (*(*L).top.get()).val;

        (*io).value_.gc = mt.cast();
        (*io).tt_ = (5 as c_int | (0 as c_int) << 4 as c_int | 1 << 6) as u8;

        api_incr_top(L);
        res = 1 as c_int;
    }

    return res;
}

pub unsafe fn lua_getiuservalue(L: *mut Thread, idx: c_int, n: c_int) -> c_int {
    let mut o: *mut UnsafeValue = 0 as *mut UnsafeValue;
    let mut t: c_int = 0;
    o = index2value(L, idx);
    if n <= 0 as c_int || n > (*((*o).value_.gc as *mut Udata)).nuvalue as c_int {
        (*(*L).top.get()).val.tt_ = (0 as c_int | (0 as c_int) << 4 as c_int) as u8;
        t = -(1 as c_int);
    } else {
        let io1: *mut UnsafeValue = &raw mut (*(*L).top.get()).val;
        let io2: *const UnsafeValue = (*((*o).value_.gc as *mut Udata))
            .uv
            .as_mut_ptr()
            .offset((n - 1 as c_int) as isize);

        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;

        t = (*(*L).top.get()).val.tt_ as c_int & 0xf as c_int;
    }
    api_incr_top(L);
    return t;
}

unsafe fn auxsetstr(
    L: *const Thread,
    t: *const UnsafeValue,
    k: *const libc::c_char,
) -> Result<(), Box<dyn core::error::Error>> {
    let mut slot: *const UnsafeValue = 0 as *const UnsafeValue;
    let str = Str::new((*L).hdr.global, CStr::from_ptr(k).to_bytes());

    if if !((*t).tt_ as c_int == 5 as c_int | (0 as c_int) << 4 as c_int | (1 as c_int) << 6) {
        slot = 0 as *const UnsafeValue;
        0 as c_int
    } else {
        slot = luaH_getstr((*t).value_.gc as *mut Table, str);
        !((*slot).tt_ as c_int & 0xf as c_int == 0 as c_int) as c_int
    } != 0
    {
        let io1: *mut UnsafeValue = slot as *mut UnsafeValue;
        let io2: *const UnsafeValue =
            &raw mut (*((*L).top.get()).offset(-(1 as c_int as isize))).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        if (*((*L).top.get()).offset(-(1 as c_int as isize))).val.tt_ as c_int
            & (1 as c_int) << 6 as c_int
            != 0
        {
            if (*(*t).value_.gc).marked.get() as c_int & (1 as c_int) << 5 as c_int != 0
                && (*(*((*L).top.get()).offset(-(1 as c_int as isize)))
                    .val
                    .value_
                    .gc)
                    .marked
                    .get() as c_int
                    & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                    != 0
            {
                luaC_barrierback_((*t).value_.gc);
            }
        }

        (*L).top.sub(1);
    } else {
        let io: *mut UnsafeValue = &raw mut (*(*L).top.get()).val;

        (*io).value_.gc = str.cast();
        (*io).tt_ = ((*str).hdr.tt as c_int | (1 as c_int) << 6 as c_int) as u8;

        api_incr_top(L);
        luaV_finishset(
            L,
            t,
            &raw mut (*((*L).top.get()).offset(-(1 as c_int as isize))).val,
            &raw mut (*((*L).top.get()).offset(-(2 as c_int as isize))).val,
            slot,
        )?;
        (*L).top.sub(2);
    };
    Ok(())
}

pub unsafe fn lua_setglobal(
    L: *const Thread,
    name: *const libc::c_char,
) -> Result<(), Box<dyn core::error::Error>> {
    let mut G: *const UnsafeValue = 0 as *const UnsafeValue;
    G = (*((*(*(*L).hdr.global).l_registry.get()).value_.gc as *mut Table))
        .array
        .get()
        .offset((2 as c_int - 1 as c_int) as isize) as *mut UnsafeValue;
    auxsetstr(L, G, name)
}

pub unsafe fn lua_settable(L: *mut Thread, idx: c_int) -> Result<(), Box<dyn core::error::Error>> {
    let mut t: *mut UnsafeValue = 0 as *mut UnsafeValue;
    let mut slot: *const UnsafeValue = 0 as *const UnsafeValue;
    t = index2value(L, idx);
    if if !((*t).tt_ as c_int
        == 5 as c_int | (0 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int)
    {
        slot = 0 as *const UnsafeValue;
        0 as c_int
    } else {
        slot = luaH_get(
            (*t).value_.gc as *mut Table,
            &raw mut (*((*L).top.get()).offset(-(2 as c_int as isize))).val,
        );
        !((*slot).tt_ as c_int & 0xf as c_int == 0 as c_int) as c_int
    } != 0
    {
        let io1: *mut UnsafeValue = slot as *mut UnsafeValue;
        let io2: *const UnsafeValue =
            &raw mut (*((*L).top.get()).offset(-(1 as c_int as isize))).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        if (*((*L).top.get()).offset(-(1 as c_int as isize))).val.tt_ as c_int
            & (1 as c_int) << 6 as c_int
            != 0
        {
            if (*(*t).value_.gc).marked.get() as c_int & (1 as c_int) << 5 as c_int != 0
                && (*(*((*L).top.get()).offset(-(1 as c_int as isize)))
                    .val
                    .value_
                    .gc)
                    .marked
                    .get() as c_int
                    & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                    != 0
            {
                luaC_barrierback_((*t).value_.gc);
            }
        }
    } else {
        luaV_finishset(
            L,
            t,
            &raw mut (*((*L).top.get()).offset(-(2 as c_int as isize))).val,
            &raw mut (*((*L).top.get()).offset(-(1 as c_int as isize))).val,
            slot,
        )?;
    }
    (*L).top.sub(2);
    Ok(())
}

pub unsafe fn lua_setfield(
    L: *const Thread,
    idx: c_int,
    k: *const libc::c_char,
) -> Result<(), Box<dyn core::error::Error>> {
    auxsetstr(L, index2value(L, idx), k)
}

pub unsafe fn lua_seti(
    L: *const Thread,
    idx: c_int,
    n: i64,
) -> Result<(), Box<dyn core::error::Error>> {
    let mut t: *mut UnsafeValue = 0 as *mut UnsafeValue;
    let mut slot: *const UnsafeValue = 0 as *const UnsafeValue;
    t = index2value(L, idx);
    if if !((*t).tt_ as c_int
        == 5 as c_int | (0 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int)
    {
        slot = 0 as *const UnsafeValue;
        0 as c_int
    } else {
        slot = if (n as u64).wrapping_sub(1 as libc::c_uint as u64)
            < (*((*t).value_.gc as *mut Table)).alimit.get() as u64
        {
            (*((*t).value_.gc as *mut Table))
                .array
                .get()
                .offset((n - 1 as c_int as i64) as isize) as *mut UnsafeValue
                as *const UnsafeValue
        } else {
            luaH_getint((*t).value_.gc as *mut Table, n)
        };
        !((*slot).tt_ as c_int & 0xf as c_int == 0 as c_int) as c_int
    } != 0
    {
        let io1: *mut UnsafeValue = slot as *mut UnsafeValue;
        let io2: *const UnsafeValue =
            &raw mut (*((*L).top.get()).offset(-(1 as c_int as isize))).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        if (*((*L).top.get()).offset(-(1 as c_int as isize))).val.tt_ as c_int
            & (1 as c_int) << 6 as c_int
            != 0
        {
            if (*(*t).value_.gc).marked.get() as c_int & (1 as c_int) << 5 as c_int != 0
                && (*(*((*L).top.get()).offset(-(1 as c_int as isize)))
                    .val
                    .value_
                    .gc)
                    .marked
                    .get() as c_int
                    & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                    != 0
            {
                luaC_barrierback_((*t).value_.gc);
            }
        }
    } else {
        let mut aux: UnsafeValue = UnsafeValue {
            value_: UntaggedValue {
                gc: 0 as *mut Object,
            },
            tt_: 0,
        };
        let io: *mut UnsafeValue = &mut aux;
        (*io).value_.i = n;
        (*io).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
        luaV_finishset(
            L,
            t,
            &mut aux,
            &raw mut (*((*L).top.get()).offset(-(1 as c_int as isize))).val,
            slot,
        )?;
    }

    (*L).top.sub(1);

    Ok(())
}

pub unsafe fn lua_rawset(L: *const Thread, idx: c_int) -> Result<(), TableError> {
    let v = (*((*L).top.get()).offset(-1)).val;
    let k = (*(*L).top.get().offset(-2)).val;
    let t = gettable(L, idx);

    (*t).set(k, v)?;
    (*L).top.sub(2);

    Ok(())
}

pub unsafe fn lua_rawseti(L: *mut Thread, idx: c_int, n: i64) {
    let t = gettable(L, idx);

    luaH_setint(
        t,
        n,
        &raw mut (*((*L).top.get()).offset(-(1 as c_int as isize))).val,
    );

    if (*((*L).top.get()).offset(-(1 as c_int as isize))).val.tt_ as c_int
        & (1 as c_int) << 6 as c_int
        != 0
    {
        if (*t).hdr.marked.get() as c_int & (1 as c_int) << 5 as c_int != 0
            && (*(*((*L).top.get()).offset(-(1 as c_int as isize)))
                .val
                .value_
                .gc)
                .marked
                .get() as c_int
                & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                != 0
        {
            luaC_barrierback_(t.cast());
        }
    }

    (*L).top.sub(1);
}

pub unsafe fn lua_setmetatable(
    L: *const Thread,
    objindex: c_int,
) -> Result<(), Box<dyn core::error::Error>> {
    let obj = index2value(L, objindex);
    let mt = if (*((*L).top.get()).offset(-1)).val.tt_ & 0xf == 0 {
        0 as *mut Table
    } else {
        (*((*L).top.get()).offset(-1)).val.value_.gc as *mut Table
    };

    // Prevent __gc metamethod.
    let g = (*L).hdr.global;

    if !mt.is_null()
        && (*mt).flags.get() & 1 << TM_GC == 0
        && !luaT_gettm(mt, TM_GC, (*g).tmname[TM_GC as usize].get()).is_null()
    {
        return Err("__gc metamethod is not supported".into());
    }

    match (*obj).tt_ & 0xf {
        5 => {
            (*((*obj).value_.gc as *mut Table)).metatable.set(mt);

            if !mt.is_null() {
                if (*(*obj).value_.gc).marked.get() as c_int & (1 as c_int) << 5 as c_int != 0
                    && (*mt).hdr.marked.get() as c_int
                        & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                        != 0
                {
                    luaC_barrier_(
                        (*L).hdr.global,
                        (*obj).value_.gc as *mut Object,
                        mt as *mut Object,
                    );
                };
            }
        }
        7 => {
            let ref mut fresh4 = (*((*obj).value_.gc as *mut Udata)).metatable;
            *fresh4 = mt;
            if !mt.is_null() {
                if (*((*obj).value_.gc as *mut Udata)).hdr.marked.get() as c_int
                    & (1 as c_int) << 5 as c_int
                    != 0
                    && (*mt).hdr.marked.get() as c_int
                        & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                        != 0
                {
                    luaC_barrier_(
                        (*L).hdr.global,
                        ((*obj).value_.gc as *mut Udata) as *mut Udata as *mut Object,
                        mt as *mut Object,
                    );
                };
            }
        }
        _ => (*(*L).hdr.global).primitive_mt[usize::from((*obj).tt_ & 0xf)].set(mt),
    }

    (*L).top.sub(1);

    Ok(())
}

pub unsafe fn lua_setiuservalue(L: *mut Thread, idx: c_int, n: c_int) -> c_int {
    let mut o: *mut UnsafeValue = 0 as *mut UnsafeValue;
    let mut res: c_int = 0;
    o = index2value(L, idx);
    if !((n as libc::c_uint).wrapping_sub(1 as libc::c_uint)
        < (*((*o).value_.gc as *mut Udata)).nuvalue as libc::c_uint)
    {
        res = 0 as c_int;
    } else {
        let io1: *mut UnsafeValue = (*((*o).value_.gc as *mut Udata))
            .uv
            .as_mut_ptr()
            .offset((n - 1 as c_int) as isize);
        let io2: *const UnsafeValue =
            &raw mut (*((*L).top.get()).offset(-(1 as c_int as isize))).val;

        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;

        if (*((*L).top.get()).offset(-(1 as c_int as isize))).val.tt_ as c_int
            & (1 as c_int) << 6 as c_int
            != 0
        {
            if (*(*o).value_.gc).marked.get() as c_int & (1 as c_int) << 5 as c_int != 0
                && (*(*((*L).top.get()).offset(-(1 as c_int as isize)))
                    .val
                    .value_
                    .gc)
                    .marked
                    .get() as c_int
                    & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                    != 0
            {
                luaC_barrierback_((*o).value_.gc);
            }
        }

        res = 1 as c_int;
    }

    (*L).top.sub(1);

    return res;
}

pub async unsafe fn lua_call(
    L: *const Thread,
    nargs: usize,
    nresults: c_int,
) -> Result<(), Box<dyn core::error::Error>> {
    let func = (*L).top.get().sub(nargs + 1);

    luaD_call(L, func, nresults).await?;

    // Adjust current CI.
    if nresults <= -1 && (*(*L).ci.get()).top < (*L).top.get() {
        (*(*L).ci.get()).top = (*L).top.get();
    }

    Ok(())
}

pub async unsafe fn lua_pcall(
    L: *const Thread,
    nargs: usize,
    nresults: c_int,
) -> Result<(), PcallError> {
    let func = ((*L).top.get()).sub(nargs + 1);
    let old_top = func.byte_offset_from_unsigned((*L).stack.get());
    let old_ci = (*L).ci.get();
    let old_allowhooks: u8 = (*L).allowhook.get();
    let mut status = luaD_call(L, func, nresults)
        .await
        .map_err(move |e| PcallError::new(L, old_ci, e));

    if status.is_err() {
        (*L).ci.set(old_ci);
        (*L).allowhook.set(old_allowhooks);
        status = luaD_closeprotected(L, old_top, status);
        (*L).top.set((*L).stack.get().byte_add(old_top));
        luaD_shrinkstack(L);
    }

    // Adjust current CI.
    if nresults <= -1 && (*(*L).ci.get()).top < (*L).top.get() {
        (*(*L).ci.get()).top = (*L).top.get();
    }

    status
}

pub unsafe fn lua_next(L: *const Thread, idx: c_int) -> Result<c_int, Box<dyn core::error::Error>> {
    let mut t: *mut Table = 0 as *mut Table;
    let mut more: c_int = 0;
    t = gettable(L, idx);
    more = luaH_next(L, t, ((*L).top.get()).offset(-(1 as c_int as isize)))?;
    if more != 0 {
        api_incr_top(L);
    } else {
        (*L).top.sub(1);
    }
    return Ok(more);
}

pub unsafe fn lua_toclose(L: *mut Thread, idx: c_int) -> Result<(), Box<dyn core::error::Error>> {
    let mut nresults: c_int = 0;
    let mut o: StkId = 0 as *mut StackValue;
    o = index2stack(L, idx);
    nresults = (*(*L).ci.get()).nresults as c_int;
    luaF_newtbcupval(L, o)?;
    if !(nresults < -(1 as c_int)) {
        (*(*L).ci.get()).nresults = (-nresults - 3 as c_int) as libc::c_short;
    }
    Ok(())
}

pub unsafe fn lua_concat(L: *const Thread, n: c_int) -> Result<(), Box<dyn core::error::Error>> {
    if n > 0 as c_int {
        luaV_concat(L, n)?;
    } else {
        let io: *mut UnsafeValue = &raw mut (*(*L).top.get()).val;
        let x_ = Str::new((*L).hdr.global, "");

        (*io).value_.gc = x_ as *mut Object;
        (*io).tt_ = ((*x_).hdr.tt as c_int | (1 as c_int) << 6 as c_int) as u8;

        api_incr_top(L);
    }

    if (*(*L).hdr.global).gc.debt() > 0 as c_int as isize {
        crate::gc::step((*L).hdr.global);
    }

    Ok(())
}

pub unsafe fn lua_len(L: *const Thread, idx: c_int) -> Result<(), Box<dyn core::error::Error>> {
    let mut t: *mut UnsafeValue = 0 as *mut UnsafeValue;
    t = index2value(L, idx);
    luaV_objlen(L, (*L).top.get(), t)?;
    api_incr_top(L);
    Ok(())
}

pub unsafe fn lua_newuserdatauv(L: *const Thread, size: usize, nuvalue: c_int) -> *mut c_void {
    let u = luaS_newudata((*L).hdr.global, size, nuvalue);
    let io: *mut UnsafeValue = &raw mut (*(*L).top.get()).val;
    let x_: *mut Udata = u;

    (*io).value_.gc = x_ as *mut Object;
    (*io).tt_ = (7 as c_int | (0 as c_int) << 4 as c_int | 1 << 6) as u8;

    api_incr_top(L);

    if (*(*L).hdr.global).gc.debt() > 0 as c_int as isize {
        crate::gc::step((*L).hdr.global);
    }

    u.byte_add(offset_of!(Udata, uv) + size_of::<UnsafeValue>() * usize::from((*u).nuvalue))
        .cast()
}

unsafe fn aux_upvalue(
    fi: *mut UnsafeValue,
    n: c_int,
    val: *mut *mut UnsafeValue,
    owner: *mut *mut Object,
) -> *const libc::c_char {
    match (*fi).tt_ as c_int & 0x3f as c_int {
        38 => {
            let f: *mut CClosure = (*fi).value_.gc as *mut CClosure;
            if !((n as libc::c_uint).wrapping_sub(1 as libc::c_uint)
                < (*f).nupvalues as libc::c_uint)
            {
                return 0 as *const libc::c_char;
            }
            *val = &mut *((*f).upvalue)
                .as_mut_ptr()
                .offset((n - 1 as c_int) as isize) as *mut UnsafeValue;
            if !owner.is_null() {
                *owner = f as *mut Object;
            }
            return b"\0" as *const u8 as *const libc::c_char;
        }
        6 => {
            let f_0: *mut LuaFn = (*fi).value_.gc as *mut LuaFn;
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

            let name = (*((*p).upvalues).offset((n - 1 as c_int) as isize)).name;
            return if name.is_null() {
                b"(no name)\0" as *const u8 as *const libc::c_char
            } else {
                ((*name).contents).as_ptr() as *const libc::c_char
            };
        }
        _ => return 0 as *const libc::c_char,
    };
}

pub unsafe fn lua_getupvalue(L: *mut Thread, funcindex: c_int, n: c_int) -> *const libc::c_char {
    let mut name: *const libc::c_char = 0 as *const libc::c_char;
    let mut val: *mut UnsafeValue = 0 as *mut UnsafeValue;
    name = aux_upvalue(
        index2value(L, funcindex),
        n,
        &mut val,
        0 as *mut *mut Object,
    );
    if !name.is_null() {
        let io1: *mut UnsafeValue = &raw mut (*(*L).top.get()).val;
        let io2: *const UnsafeValue = val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        api_incr_top(L);
    }
    return name;
}

pub unsafe fn lua_setupvalue(L: *const Thread, funcindex: c_int, n: c_int) -> *const libc::c_char {
    let mut name: *const libc::c_char = 0 as *const libc::c_char;
    let mut val: *mut UnsafeValue = 0 as *mut UnsafeValue;
    let mut owner: *mut Object = 0 as *mut Object;
    let mut fi: *mut UnsafeValue = 0 as *mut UnsafeValue;
    fi = index2value(L, funcindex);
    name = aux_upvalue(fi, n, &mut val, &mut owner);
    if !name.is_null() {
        (*L).top.sub(1);

        let io1: *mut UnsafeValue = val;
        let io2: *const UnsafeValue = &raw mut (*(*L).top.get()).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        if (*val).tt_ as c_int & (1 as c_int) << 6 as c_int != 0 {
            if (*owner).marked.get() as c_int & (1 as c_int) << 5 as c_int != 0
                && (*(*val).value_.gc).marked.get() as c_int
                    & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                    != 0
            {
                luaC_barrier_(
                    (*L).hdr.global,
                    owner as *mut Object,
                    (*val).value_.gc as *mut Object,
                );
            }
        }
    }
    return name;
}

unsafe fn getupvalref(
    L: *mut Thread,
    fidx: c_int,
    n: c_int,
    pf: *mut *const LuaFn,
) -> *mut *mut UpVal {
    static mut nullup: *const UpVal = 0 as *const UpVal;
    let fi: *mut UnsafeValue = index2value(L, fidx);
    let f = (*fi).value_.gc.cast::<LuaFn>();

    if !pf.is_null() {
        *pf = f;
    }

    if 1 as c_int <= n && n <= (*(*f).p.get()).sizeupvalues {
        return (*f).upvals[(n - 1) as usize].as_ptr();
    } else {
        return &raw const nullup as *const *const UpVal as *mut *mut UpVal;
    };
}

pub unsafe fn lua_upvalueid(L: *mut Thread, fidx: c_int, n: c_int) -> *mut libc::c_void {
    let fi: *mut UnsafeValue = index2value(L, fidx);

    match (*fi).tt_ as c_int & 0x3f as c_int {
        6 => return *getupvalref(L, fidx, n, null_mut()).cast(),
        38 => {
            let f: *mut CClosure = (*fi).value_.gc as *mut CClosure;
            if 1 as c_int <= n && n <= (*f).nupvalues as c_int {
                return &mut *((*f).upvalue)
                    .as_mut_ptr()
                    .offset((n - 1 as c_int) as isize) as *mut UnsafeValue
                    as *mut libc::c_void;
            }
        }
        2 | 18 | 34 | 50 => {}
        _ => return 0 as *mut libc::c_void,
    }

    return 0 as *mut libc::c_void;
}

pub unsafe fn lua_upvaluejoin(L: *mut Thread, fidx1: c_int, n1: c_int, fidx2: c_int, n2: c_int) {
    let mut f1 = 0 as *const LuaFn;
    let up1: *mut *mut UpVal = getupvalref(L, fidx1, n1, &mut f1);
    let up2: *mut *mut UpVal = getupvalref(L, fidx2, n2, null_mut());

    *up1 = *up2;

    if (*f1).hdr.marked.get() as c_int & (1 as c_int) << 5 as c_int != 0
        && (**up1).hdr.marked.get() as c_int
            & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
            != 0
    {
        luaC_barrier_((*L).hdr.global, f1 as *mut Object, *up1 as *mut Object);
    }
}

/// Represents an error when [`lua_pcall()`] fails.
pub struct PcallError {
    pub chunk: Option<(String, u32)>,
    pub reason: Box<dyn core::error::Error>,
}

impl PcallError {
    pub unsafe fn new(
        th: *const Thread,
        caller: *mut CallInfo,
        reason: Box<dyn core::error::Error>,
    ) -> Self {
        // Traverse up until reaching a Lua function.
        let mut ci = (*th).ci.get();
        let mut chunk = None;

        while ci != caller && ci != (*th).base_ci.get() {
            let mut ar = lua_Debug {
                i_ci: ci,
                ..Default::default()
            };

            lua_getinfo(th, c"Sl".as_ptr(), &mut ar);

            if let Some(v) = ar.source {
                chunk = Some((v.name, u32::try_from(ar.currentline).unwrap()));
                break;
            }

            ci = (*ci).previous;
        }

        Self { chunk, reason }
    }
}
