#![allow(non_camel_case_types, non_snake_case, unused_assignments)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::context::{Args, Context, Ret};
use crate::ldo::luaD_growstack;
use crate::lfunc::{luaF_close, luaF_newCclosure};
use crate::lobject::CClosure;
use crate::ltm::luaT_typenames_;
use crate::value::UnsafeValue;
use crate::{LuaFn, Object, StackOverflow, StackValue, Table, Thread, api_incr_top};
use alloc::boxed::Box;
use core::cmp::max;
use core::ffi::c_char;
use core::ptr::null_mut;

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

pub unsafe fn lua_closeslot<D>(
    L: &Thread<D>,
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

unsafe fn gettable<D>(L: *const Thread<D>, idx: c_int) -> *const Table<D> {
    let t = index2value(L, idx);

    (*t).value_.gc.cast()
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
