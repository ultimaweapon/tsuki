#![allow(non_camel_case_types, non_snake_case, unused_assignments)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::ldo::{luaD_closeprotected, luaD_reallocstack};
use crate::lmem::{luaM_free_, luaM_malloc_};
use crate::{CallError, ChunkInfo, StackValue, Thread};
use alloc::boxed::Box;
use core::ffi::c_char;
use core::ptr::{null, null_mut};

type c_uchar = u8;
type c_short = i16;
type c_ushort = u16;
type c_int = i32;

#[repr(C)]
pub struct lua_Debug<D> {
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
    pub(crate) i_ci: *mut CallInfo<D>,
}

impl<D> Default for lua_Debug<D> {
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
pub struct CallInfo<D> {
    pub func: *mut StackValue<D>,
    pub top: *mut StackValue<D>,
    pub previous: *mut Self,
    pub next: *mut Self,
    pub u: C2RustUnnamed_3,
    pub u2: C2RustUnnamed,
    pub nresults: c_short,
    pub callstatus: c_ushort,
}

impl<D> Clone for CallInfo<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for CallInfo<D> {}

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

#[derive(Copy, Clone)]
#[repr(C)]
pub struct C2RustUnnamed_3 {
    pub savedpc: *const u32,
    pub trap: c_int,
    pub nextraargs: c_int,
}

pub unsafe fn luaE_extendCI<D>(L: *const Thread<D>) -> *mut CallInfo<D> {
    let mut ci = null_mut();
    ci = luaM_malloc_((*L).hdr.global, ::core::mem::size_of::<CallInfo<D>>()) as *mut CallInfo<D>;
    (*(*L).ci.get()).next = ci;
    (*ci).previous = (*L).ci.get();
    (*ci).next = null_mut();
    ::core::ptr::write_volatile(&mut (*ci).u.trap as *mut c_int, 0 as c_int);
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

        luaM_free_(next.cast(), ::core::mem::size_of::<CallInfo<D>>());
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
    (*ci).func = (*L).stack.get();
    (*ci).callstatus = ((1 as c_int) << 1 as c_int) as c_ushort;

    let status = luaD_closeprotected(L, 1, Ok(()));

    (*L).top.set(((*L).stack.get()).offset(1 as c_int as isize));
    (*ci).top = ((*L).top.get()).offset(20 as c_int as isize);

    luaD_reallocstack(L, ((*ci).top).offset_from_unsigned((*L).stack.get()));

    return status;
}
