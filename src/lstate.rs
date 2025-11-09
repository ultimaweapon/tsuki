#![allow(non_camel_case_types, non_snake_case, unused_assignments)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::ldo::{luaD_closeprotected, luaD_reallocstack};
use crate::lmem::{luaM_free_, luaM_malloc_};
use crate::{CallError, ChunkInfo, Thread};
use alloc::boxed::Box;
use core::ffi::c_char;
use core::num::NonZero;
use core::ptr::{null, null_mut};

type c_uchar = u8;
type c_short = i16;
type c_ushort = u16;
type c_int = i32;

#[repr(C)]
pub struct lua_Debug {
    pub event: c_int,
    pub name: *const c_char,
    pub namewhat: *const c_char,
    pub what: *const c_char,
    pub source: Option<ChunkInfo>,
    pub currentline: c_int,
    pub linedefined: c_int,
    pub lastlinedefined: c_int,
    pub nups: c_uchar,
    pub nparams: c_uchar,
    pub isvararg: c_char,
    pub istailcall: c_char,
    pub ftransfer: usize,
    pub ntransfer: usize,
    pub(crate) i_ci: *mut CallInfo,
}

impl Default for lua_Debug {
    #[inline(always)]
    fn default() -> Self {
        Self {
            event: 0,
            name: null(),
            namewhat: null(),
            what: null(),
            source: None,
            currentline: 0,
            linedefined: 0,
            lastlinedefined: 0,
            nups: 0,
            nparams: 0,
            isvararg: 0,
            istailcall: 0,
            ftransfer: 0,
            ntransfer: 0,
            i_ci: null_mut(),
        }
    }
}

#[repr(C)]
pub struct CallInfo {
    pub func: usize,
    pub top: NonZero<usize>,
    pub previous: *mut Self,
    pub next: *mut Self,
    pub u2: C2RustUnnamed,
    pub pc: usize,
    pub nresults: c_short,
    pub callstatus: c_ushort,
    pub nextraargs: c_int,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub union C2RustUnnamed {
    pub funcidx: c_int,
    pub nres: c_int,
    pub transferinfo: C2RustUnnamed_0,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct C2RustUnnamed_0 {
    pub ftransfer: c_ushort,
    pub ntransfer: usize,
}

#[inline(never)]
pub unsafe fn luaE_extendCI<A>(L: *const Thread<A>) -> *mut CallInfo {
    let ci = luaM_malloc_((*L).hdr.global, size_of::<CallInfo>()) as *mut CallInfo;

    (*(*L).ci.get()).next = ci;
    (*ci).previous = (*L).ci.get();
    (*ci).next = null_mut();
    (*L).nci.set((*L).nci.get().wrapping_add(1));

    return ci;
}

pub unsafe fn luaE_shrinkCI<D>(L: *const Thread<D>) {
    let mut ci = (*(*L).ci.get()).next;

    if ci.is_null() {
        return;
    }
    loop {
        let next = (*ci).next;
        if next.is_null() {
            break;
        }
        let next2 = (*next).next;
        (*ci).next = next2;
        (*L).nci.set((*L).nci.get().wrapping_sub(1));

        luaM_free_(next.cast(), size_of::<CallInfo>());

        if next2.is_null() {
            break;
        }
        (*next2).previous = ci;
        ci = next2;
    }
}

pub unsafe fn lua_closethread<D>(L: &Thread<D>) -> Result<(), Box<CallError>> {
    (*L).ci.set((*L).base_ci.get());
    let ci = (*L).ci.get();

    (*(*L).stack.get()).tt_ = 0 | 0 << 4;
    (*ci).func = 0;
    (*ci).callstatus = ((1 as c_int) << 1 as c_int) as c_ushort;

    let status = luaD_closeprotected(L, 1, Ok(()));

    (*L).top.set(((*L).stack.get()).offset(1 as c_int as isize));
    (*ci).top = NonZero::new(1).unwrap();

    luaD_reallocstack(L, (*ci).top.get());

    return status;
}
