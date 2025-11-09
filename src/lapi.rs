#![allow(non_camel_case_types, non_snake_case, unused_assignments)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::context::{Args, Context, Ret};
use crate::ldo::luaD_growstack;
use crate::lfunc::luaF_newCclosure;
use crate::lobject::CClosure;
use crate::ltm::luaT_typenames_;
use crate::value::UnsafeValue;
use crate::{LuaFn, Object, StackOverflow, StackValue, Thread, api_incr_top};
use alloc::boxed::Box;
use core::cmp::max;
use core::ffi::c_char;

type c_int = i32;
type c_uint = u32;

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

#[inline(always)]
pub const fn lua_typename(t: c_int) -> &'static str {
    luaT_typenames_[(t + 1) as usize]
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
