#![allow(non_camel_case_types, non_snake_case, unused_assignments)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::ldo::luaD_growstack;
use crate::ltm::luaT_typenames_;
use crate::{StackOverflow, Thread};
use core::cmp::max;

type c_int = i32;

#[inline(always)]
pub unsafe fn lua_checkstack<A>(
    L: *const Thread<A>,
    need: usize,
    reserve: usize,
) -> Result<(), StackOverflow> {
    let ci = (*L).ci.get();

    if (*L).top.get().add(need) <= (*L).stack.get().add((*ci).top.get()) {
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

    (*ci).top = (*L)
        .top
        .get()
        .add(n)
        .offset_from_unsigned((*L).stack.get())
        .try_into()
        .unwrap();

    Ok(())
}

#[inline(always)]
pub const fn lua_typename(t: c_int) -> &'static str {
    luaT_typenames_[(t + 1) as usize]
}
