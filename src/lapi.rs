#![allow(non_camel_case_types, non_snake_case, unused_assignments)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::context::{Args, Context, Ret};
use crate::ldo::luaD_growstack;
use crate::lfunc::{luaF_close, luaF_newCclosure};
use crate::lobject::CClosure;
use crate::ltm::luaT_typenames_;
use crate::table::{luaH_get, luaH_getint, luaH_getn, luaH_getstr};
use crate::value::UnsafeValue;
use crate::vm::{
    luaV_concat, luaV_equalobj, luaV_finishget, luaV_finishset, luaV_lessequal, luaV_lessthan,
};
use crate::{LuaFn, Object, StackOverflow, StackValue, Str, Table, Thread, UserData, api_incr_top};
use alloc::boxed::Box;
use core::cmp::max;
use core::convert::identity;
use core::ffi::{CStr, c_char};
use core::ptr::{null, null_mut};

type c_int = i32;
type c_uint = u32;

unsafe fn index2value<D>(L: *const Thread<D>, mut idx: c_int) -> *mut UnsafeValue<D> {
    let ci = (*L).ci.get();
    if idx > 0 as c_int {
        let o = ((*ci).func).offset(idx as isize);
        if o >= (*L).top.get() {
            return (*(*L).hdr.global).nilvalue.get();
        } else {
            return o.cast();
        }
    } else if !(idx <= -(1000000 as c_int) - 1000 as c_int) {
        return ((*L).top.get()).offset(idx as isize).cast();
    } else if idx == -(1000000 as c_int) - 1000 as c_int {
        return (*(*L).hdr.global).l_registry.get();
    } else {
        idx = -(1000000 as c_int) - 1000 as c_int - idx;
        if (*(*ci).func).tt_ as c_int
            == 6 as c_int | (2 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int
        {
            let func = (*(*ci).func).value_.gc as *mut CClosure<D>;
            return if idx <= (*func).nupvalues as c_int {
                &mut *((*func).upvalue)
                    .as_mut_ptr()
                    .offset((idx - 1 as c_int) as isize) as *mut UnsafeValue<D>
            } else {
                (*(*L).hdr.global).nilvalue.get()
            };
        } else {
            return (*(*L).hdr.global).nilvalue.get();
        }
    };
}

unsafe fn index2stack<D>(L: *const Thread<D>, idx: c_int) -> *mut StackValue<D> {
    let ci = (*L).ci.get();
    if idx > 0 as c_int {
        let o = ((*ci).func).offset(idx as isize);
        return o;
    } else {
        return ((*L).top.get()).offset(idx as isize);
    };
}

#[inline(always)]
pub unsafe fn lua_checkstack<A>(
    L: *const Thread<A>,
    need: usize,
    reserve: usize,
) -> Result<(), StackOverflow> {
    let ci = (*L).ci.get();

    if (*L).top.get().add(need) <= (*ci).top {
        Ok(())
    } else {
        growstack(L, max(need, reserve))
    }
}

#[inline(never)]
unsafe fn growstack<A>(L: *const Thread<A>, n: usize) -> Result<(), StackOverflow> {
    let ci = (*L).ci.get();

    // Check if remaining space is enough.
    if (*L).stack_last.get().offset_from_unsigned((*L).top.get()) <= n {
        luaD_growstack(L, n)?;
    }

    (*ci).top = (*L).top.get().add(n);

    Ok(())
}

pub unsafe fn lua_xmove<D>(from: *mut Thread<D>, to: *mut Thread<D>, n: c_int) {
    let mut i: c_int = 0;
    if from == to {
        return;
    }
    (*from).top.sub(n.try_into().unwrap());
    i = 0 as c_int;
    while i < n {
        let io1 = (*to).top.get();
        let io2 = ((*from).top.get()).offset(i as isize);
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        (*to).top.add(1);

        i += 1;
    }
}

pub unsafe fn lua_settop<D>(
    L: *const Thread<D>,
    idx: c_int,
) -> Result<(), Box<dyn core::error::Error>> {
    let mut ci = null_mut();
    let mut func = null_mut();
    let mut newtop = null_mut();
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
            (*fresh1).tt_ = (0 as c_int | (0 as c_int) << 4 as c_int) as u8;
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

pub unsafe fn lua_closeslot<D>(
    L: *mut Thread<D>,
    idx: c_int,
) -> Result<(), Box<dyn core::error::Error>> {
    let mut level = null_mut();
    level = index2stack(L, idx);
    level = luaF_close(L, level)?;
    (*level).tt_ = (0 as c_int | (0 as c_int) << 4 as c_int) as u8;
    Ok(())
}

unsafe fn reverse<D>(mut from: *mut StackValue<D>, mut to: *mut StackValue<D>) {
    while from < to {
        let mut temp = UnsafeValue::default();
        let io1 = &raw mut temp;
        let io2 = from;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        let io1_0 = from;
        let io2_0 = to;
        (*io1_0).value_ = (*io2_0).value_;
        (*io1_0).tt_ = (*io2_0).tt_;
        let io1_1 = to;
        let io2_1 = &raw mut temp;
        (*io1_1).value_ = (*io2_1).value_;
        (*io1_1).tt_ = (*io2_1).tt_;
        from = from.offset(1);
        to = to.offset(-1);
    }
}

pub unsafe fn lua_rotate<D>(L: *const Thread<D>, idx: c_int, n: c_int) {
    let mut p = null_mut();
    let mut t = null_mut();
    let mut m = null_mut();
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

pub unsafe fn lua_copy<D>(L: *const Thread<D>, fromidx: c_int, toidx: c_int) {
    let mut fr = null_mut();
    let mut to = null_mut();
    fr = index2value(L, fromidx);
    to = index2value(L, toidx);
    let io1 = to;
    let io2 = fr;
    (*io1).value_ = (*io2).value_;
    (*io1).tt_ = (*io2).tt_;
    if toidx < -(1000000 as c_int) - 1000 as c_int {
        if (*fr).tt_ as c_int & (1 as c_int) << 6 as c_int != 0 {
            if (*((*(*(*L).ci.get()).func).value_.gc as *mut CClosure<D>))
                .hdr
                .marked
                .get() as c_int
                & (1 as c_int) << 5 as c_int
                != 0
                && (*(*fr).value_.gc).marked.get() as c_int
                    & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                    != 0
            {
                (*L).hdr.global().gc.barrier(
                    ((*(*(*L).ci.get()).func).value_.gc as *mut CClosure<D>) as *mut Object<D>,
                    (*fr).value_.gc as *mut Object<D>,
                );
            }
        }
    }
}

pub unsafe fn lua_type<D>(L: *const Thread<D>, idx: c_int) -> c_int {
    let o = index2value(L, idx);
    return if !((*o).tt_ as c_int & 0xf as c_int == 0 as c_int)
        || o != (*(*L).hdr.global).nilvalue.get()
    {
        (*o).tt_ as c_int & 0xf as c_int
    } else {
        -(1 as c_int)
    };
}

#[inline(always)]
pub const fn lua_typename(t: c_int) -> &'static str {
    luaT_typenames_[(t + 1) as usize]
}

pub unsafe fn lua_iscfunction<D>(L: *mut Thread<D>, idx: c_int) -> c_int {
    let o = index2value(L, idx);
    return (((*o).tt_ & 0xF) == 2
        || (*o).tt_ as c_int
            == 6 as c_int | (2 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int)
        as c_int;
}

pub unsafe fn lua_isstring<D>(L: *const Thread<D>, idx: c_int) -> c_int {
    let o = index2value(L, idx);
    return ((*o).tt_ as c_int & 0xf as c_int == 4 as c_int
        || (*o).tt_ as c_int & 0xf as c_int == 3 as c_int) as c_int;
}

pub unsafe fn lua_rawequal<D>(
    L: *const Thread<D>,
    index1: c_int,
    index2: c_int,
) -> Result<c_int, Box<dyn core::error::Error>> {
    let o1 = index2value(L, index1);
    let o2 = index2value(L, index2);

    return if (!((*o1).tt_ & 0xf == 0) || o1 != (*(*L).hdr.global).nilvalue.get())
        && (!((*o2).tt_ & 0xf == 0) || o2 != (*(*L).hdr.global).nilvalue.get())
    {
        luaV_equalobj(null(), o1, o2).map(|v| v.into())
    } else {
        Ok(0 as c_int)
    };
}

pub unsafe fn lua_compare<D>(
    L: *const Thread<D>,
    index1: c_int,
    index2: c_int,
    op: c_int,
) -> Result<c_int, Box<dyn core::error::Error>> {
    let mut i: c_int = 0 as c_int;
    let o1 = index2value(L, index1);
    let o2 = index2value(L, index2);

    if (!((*o1).tt_ & 0xf == 0) || o1 != (*(*L).hdr.global).nilvalue.get())
        && (!((*o2).tt_ & 0xf == 0) || o2 != (*(*L).hdr.global).nilvalue.get())
    {
        match op {
            0 => i = luaV_equalobj(L, o1, o2)?.into(),
            1 => i = luaV_lessthan(L, o1, o2)?,
            2 => i = luaV_lessequal(L, o1, o2)?,
            _ => {}
        }
    }

    return Ok(i);
}

pub unsafe fn lua_rawlen<D>(L: *const Thread<D>, idx: c_int) -> u64 {
    let o = index2value(L, idx);

    match (*o).tt_ & 0x3f {
        4 => return (*((*o).value_.gc.cast::<Str<D>>())).len as u64,
        7 => {
            let u = (*o).value_.gc.cast::<UserData<D, ()>>();
            let v = (*u).ptr;

            size_of_val(&*v).try_into().unwrap()
        }
        5 => return luaH_getn((*o).value_.gc as *mut Table<D>),
        _ => return 0 as c_int as u64,
    }
}

pub unsafe fn lua_pushcclosure<D>(
    L: *const Thread<D>,
    fn_0: for<'a> fn(
        Context<'a, D, Args>,
    ) -> Result<Context<'a, D, Ret>, Box<dyn core::error::Error>>,
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
        let io1 = &raw mut *((*cl).upvalue).as_mut_ptr().offset(n as isize) as *mut UnsafeValue<D>;
        let io2 = ((*L).top.get()).offset(n as isize);
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
    }

    let io_0 = (*L).top.get();
    let x_ = cl;

    (*io_0).value_.gc = x_.cast();
    (*io_0).tt_ = (6 as c_int | (2 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int) as u8;

    api_incr_top(L);
}

unsafe fn auxgetstr<D>(
    L: *const Thread<D>,
    t: *const UnsafeValue<D>,
    k: &[u8],
) -> Result<c_int, Box<dyn core::error::Error>> {
    let mut slot = null();
    let str = Str::from_bytes((*L).hdr.global, k).unwrap_or_else(identity);

    if if !((*t).tt_ as c_int == 5 as c_int | (0 as c_int) << 4 as c_int | (1 as c_int) << 6) {
        slot = null();
        0 as c_int
    } else {
        slot = luaH_getstr((*t).value_.gc.cast(), str);
        !((*slot).tt_ as c_int & 0xf as c_int == 0 as c_int) as c_int
    } != 0
    {
        let io1 = (*L).top.get();
        let io2 = slot;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        api_incr_top(L);
    } else {
        let io = (*L).top.get();

        (*io).value_.gc = str.cast();
        (*io).tt_ = ((*str).hdr.tt as c_int | (1 as c_int) << 6 as c_int) as u8;

        api_incr_top(L);

        let val = luaV_finishget(L, t, ((*L).top.get()).offset(-1).cast(), slot)?;
        let io = (*L).top.get().offset(-1);

        (*io).value_ = val.value_;
        (*io).tt_ = val.tt_;
    }

    return Ok((*((*L).top.get()).offset(-(1 as c_int as isize))).tt_ as c_int & 0xf as c_int);
}

pub unsafe fn lua_getfield<D>(
    L: *const Thread<D>,
    idx: c_int,
    k: impl AsRef<[u8]>,
) -> Result<c_int, Box<dyn core::error::Error>> {
    return auxgetstr(L, index2value(L, idx), k.as_ref());
}

unsafe fn finishrawget<D>(L: *const Thread<D>, val: *const UnsafeValue<D>) -> c_int {
    if (*val).tt_ as c_int & 0xf as c_int == 0 as c_int {
        (*(*L).top.get()).tt_ = (0 as c_int | (0 as c_int) << 4 as c_int) as u8;
    } else {
        let io1 = (*L).top.get();
        let io2 = val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
    }
    api_incr_top(L);
    return (*((*L).top.get()).offset(-(1 as c_int as isize))).tt_ as c_int & 0xf as c_int;
}

unsafe fn gettable<D>(L: *const Thread<D>, idx: c_int) -> *const Table<D> {
    let t = index2value(L, idx);

    (*t).value_.gc.cast()
}

pub unsafe fn lua_rawget<D>(L: *const Thread<D>, idx: c_int) -> c_int {
    let t = gettable(L, idx);
    let val = luaH_get(t, ((*L).top.get()).offset(-(1 as c_int as isize)).cast());
    (*L).top.sub(1);

    return finishrawget(L, val);
}

pub unsafe fn lua_rawgeti<D>(L: *const Thread<D>, idx: c_int, n: i64) -> c_int {
    let t = gettable(L, idx);
    return finishrawget(L, luaH_getint(t, n));
}

unsafe fn auxsetstr<D>(
    L: *const Thread<D>,
    t: *const UnsafeValue<D>,
    k: *const c_char,
) -> Result<(), Box<dyn core::error::Error>> {
    let mut slot = null();
    let str =
        Str::from_bytes((*L).hdr.global, CStr::from_ptr(k).to_bytes()).unwrap_or_else(identity);

    if if !((*t).tt_ as c_int == 5 as c_int | (0 as c_int) << 4 as c_int | (1 as c_int) << 6) {
        slot = null();
        0 as c_int
    } else {
        slot = luaH_getstr((*t).value_.gc.cast(), str);
        !((*slot).tt_ as c_int & 0xf as c_int == 0 as c_int) as c_int
    } != 0
    {
        let io1 = slot.cast_mut();
        let io2 = ((*L).top.get()).offset(-(1 as c_int as isize));
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        if (*((*L).top.get()).offset(-(1 as c_int as isize))).tt_ as c_int
            & (1 as c_int) << 6 as c_int
            != 0
        {
            if (*(*t).value_.gc).marked.get() as c_int & (1 as c_int) << 5 as c_int != 0
                && (*(*((*L).top.get()).offset(-(1 as c_int as isize))).value_.gc)
                    .marked
                    .get() as c_int
                    & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                    != 0
            {
                (*L).hdr.global().gc.barrier_back((*t).value_.gc);
            }
        }

        (*L).top.sub(1);
    } else {
        let io = (*L).top.get();

        (*io).value_.gc = str.cast();
        (*io).tt_ = ((*str).hdr.tt as c_int | (1 as c_int) << 6 as c_int) as u8;

        api_incr_top(L);
        luaV_finishset(
            L,
            t,
            ((*L).top.get()).offset(-(1 as c_int as isize)).cast(),
            ((*L).top.get()).offset(-(2 as c_int as isize)).cast(),
            slot,
        )?;
        (*L).top.sub(2);
    };
    Ok(())
}

pub unsafe fn lua_settable<D>(
    L: *const Thread<D>,
    idx: c_int,
) -> Result<(), Box<dyn core::error::Error>> {
    let mut slot = null();
    let t = index2value(L, idx);

    if if !((*t).tt_ as c_int
        == 5 as c_int | (0 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int)
    {
        slot = null();
        0 as c_int
    } else {
        slot = luaH_get((*t).value_.gc.cast(), ((*L).top.get()).offset(-2).cast());
        !((*slot).tt_ as c_int & 0xf as c_int == 0 as c_int) as c_int
    } != 0
    {
        let io1 = slot.cast_mut();
        let io2 = ((*L).top.get()).offset(-(1 as c_int as isize));
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        if (*((*L).top.get()).offset(-(1 as c_int as isize))).tt_ as c_int
            & (1 as c_int) << 6 as c_int
            != 0
        {
            if (*(*t).value_.gc).marked.get() as c_int & (1 as c_int) << 5 as c_int != 0
                && (*(*((*L).top.get()).offset(-(1 as c_int as isize))).value_.gc)
                    .marked
                    .get() as c_int
                    & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                    != 0
            {
                (*L).hdr.global().gc.barrier_back((*t).value_.gc);
            }
        }
    } else {
        luaV_finishset(
            L,
            t,
            ((*L).top.get()).offset(-(2 as c_int as isize)).cast(),
            ((*L).top.get()).offset(-(1 as c_int as isize)).cast(),
            slot,
        )?;
    }
    (*L).top.sub(2);
    Ok(())
}

pub unsafe fn lua_seti<D>(
    L: *const Thread<D>,
    idx: c_int,
    n: i64,
) -> Result<(), Box<dyn core::error::Error>> {
    let mut slot = null();
    let t = index2value(L, idx);

    if if !((*t).tt_ as c_int
        == 5 as c_int | (0 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int)
    {
        slot = null();
        0 as c_int
    } else {
        slot = if (n as u64).wrapping_sub(1 as c_uint as u64)
            < (*((*t).value_.gc as *mut Table<D>)).alimit.get() as u64
        {
            (*((*t).value_.gc as *mut Table<D>))
                .array
                .get()
                .offset((n - 1 as c_int as i64) as isize) as *const UnsafeValue<D>
        } else {
            luaH_getint((*t).value_.gc.cast(), n)
        };
        !((*slot).tt_ as c_int & 0xf as c_int == 0 as c_int) as c_int
    } != 0
    {
        let io1 = slot.cast_mut();
        let io2 = ((*L).top.get()).offset(-(1 as c_int as isize));
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        if (*((*L).top.get()).offset(-(1 as c_int as isize))).tt_ as c_int
            & (1 as c_int) << 6 as c_int
            != 0
        {
            if (*(*t).value_.gc).marked.get() as c_int & (1 as c_int) << 5 as c_int != 0
                && (*(*((*L).top.get()).offset(-(1 as c_int as isize))).value_.gc)
                    .marked
                    .get() as c_int
                    & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                    != 0
            {
                (*L).hdr.global().gc.barrier_back((*t).value_.gc);
            }
        }
    } else {
        let mut aux = UnsafeValue::default();
        let io = &raw mut aux;
        (*io).value_.i = n;
        (*io).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
        luaV_finishset(
            L,
            t,
            &mut aux,
            ((*L).top.get()).offset(-(1 as c_int as isize)).cast(),
            slot,
        )?;
    }

    (*L).top.sub(1);

    Ok(())
}

pub unsafe fn lua_concat<D>(
    L: *const Thread<D>,
    n: c_int,
) -> Result<(), Box<dyn core::error::Error>> {
    (*L).hdr.global().gc.step();

    if n > 0 as c_int {
        luaV_concat(L, n)?;
    } else {
        let io = (*L).top.get();
        let x_ = Str::from_str((*L).hdr.global, "").unwrap_or_else(identity);

        (*io).value_.gc = x_.cast();
        (*io).tt_ = ((*x_).hdr.tt as c_int | (1 as c_int) << 6 as c_int) as u8;

        api_incr_top(L);
    }

    Ok(())
}

unsafe fn aux_upvalue<D>(
    fi: *mut UnsafeValue<D>,
    n: c_int,
    val: *mut *mut UnsafeValue<D>,
    owner: *mut *mut Object<D>,
) -> *const c_char {
    match (*fi).tt_ & 0x3f {
        38 => {
            let f = (*fi).value_.gc as *mut CClosure<D>;
            if !((n as c_uint).wrapping_sub(1 as c_uint) < (*f).nupvalues as c_uint) {
                return 0 as *const c_char;
            }
            *val = &mut *((*f).upvalue)
                .as_mut_ptr()
                .offset((n - 1 as c_int) as isize) as *mut UnsafeValue<D>;
            if !owner.is_null() {
                *owner = f.cast();
            }
            return b"\0" as *const u8 as *const c_char;
        }
        6 => {
            let f_0 = (*fi).value_.gc as *mut LuaFn<D>;
            let p = (*f_0).p.get();

            if !((n as c_uint).wrapping_sub(1 as c_uint) < (*p).sizeupvalues as c_uint) {
                return 0 as *const c_char;
            }

            *val = (*(*f_0).upvals[(n - 1) as usize].get()).v.get();

            if !owner.is_null() {
                *owner = (*f_0).upvals[(n - 1) as usize].get().cast();
            }

            let name = (*((*p).upvalues).offset((n - 1 as c_int) as isize)).name;
            return if name.is_null() {
                b"(no name)\0" as *const u8 as *const c_char
            } else {
                ((*name).contents).as_ptr() as *const c_char
            };
        }
        _ => return 0 as *const c_char,
    };
}

pub unsafe fn lua_getupvalue<D>(L: *mut Thread<D>, funcindex: c_int, n: c_int) -> *const c_char {
    let mut name: *const c_char = 0 as *const c_char;
    let mut val = null_mut();
    name = aux_upvalue(index2value(L, funcindex), n, &mut val, null_mut());
    if !name.is_null() {
        let io1 = (*L).top.get();
        let io2 = val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        api_incr_top(L);
    }
    return name;
}

pub unsafe fn lua_setupvalue<D>(L: *const Thread<D>, funcindex: c_int, n: c_int) -> *const c_char {
    let mut name: *const c_char = 0 as *const c_char;
    let mut val = null_mut();
    let mut owner = null_mut();
    let fi = index2value(L, funcindex);
    name = aux_upvalue(fi, n, &mut val, &mut owner);
    if !name.is_null() {
        (*L).top.sub(1);

        let io1 = val;
        let io2 = (*L).top.get();
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        if (*val).tt_ as c_int & (1 as c_int) << 6 as c_int != 0 {
            if (*owner).marked.get() as c_int & (1 as c_int) << 5 as c_int != 0
                && (*(*val).value_.gc).marked.get() as c_int
                    & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                    != 0
            {
                (*L).hdr.global().gc.barrier(owner, (*val).value_.gc);
            }
        }
    }
    return name;
}
