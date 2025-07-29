#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments
)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::ldo::{luaD_closeprotected, luaD_reallocstack};
use crate::lmem::{luaM_free_, luaM_malloc_};
use crate::lobject::StkId;
use crate::{CallError, ChunkInfo, Thread};
use alloc::boxed::Box;
use core::ptr::{null, null_mut};

pub type lua_Hook = Option<unsafe extern "C" fn(*const Thread, *mut lua_Debug) -> ()>;

#[repr(C)]
pub struct lua_Debug {
    pub event: libc::c_int,
    pub name: *const libc::c_char,
    pub namewhat: *const libc::c_char,
    pub what: *const libc::c_char,
    pub source: Option<ChunkInfo>,
    pub currentline: libc::c_int,
    pub linedefined: libc::c_int,
    pub lastlinedefined: libc::c_int,
    pub nups: libc::c_uchar,
    pub nparams: libc::c_uchar,
    pub isvararg: libc::c_char,
    pub istailcall: libc::c_char,
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

#[derive(Copy, Clone)]
#[repr(C)]
pub struct CallInfo {
    pub func: StkId,
    pub top: StkId,
    pub previous: *mut CallInfo,
    pub next: *mut CallInfo,
    pub u: C2RustUnnamed_3,
    pub u2: C2RustUnnamed,
    pub nresults: libc::c_short,
    pub callstatus: libc::c_ushort,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub union C2RustUnnamed {
    pub funcidx: libc::c_int,
    pub nres: libc::c_int,
    pub transferinfo: C2RustUnnamed_0,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct C2RustUnnamed_0 {
    pub ftransfer: libc::c_ushort,
    pub ntransfer: usize,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct C2RustUnnamed_3 {
    pub savedpc: *const u32,
    pub trap: libc::c_int,
    pub nextraargs: libc::c_int,
}

pub unsafe fn luaE_extendCI(L: *const Thread) -> *mut CallInfo {
    let mut ci: *mut CallInfo = 0 as *mut CallInfo;
    ci = luaM_malloc_((*L).hdr.global, ::core::mem::size_of::<CallInfo>()) as *mut CallInfo;
    (*(*L).ci.get()).next = ci;
    (*ci).previous = (*L).ci.get();
    (*ci).next = 0 as *mut CallInfo;
    ::core::ptr::write_volatile(&mut (*ci).u.trap as *mut libc::c_int, 0 as libc::c_int);
    (*L).nci.set((*L).nci.get().wrapping_add(1));

    return ci;
}

pub unsafe fn luaE_shrinkCI(L: *const Thread) {
    let mut ci: *mut CallInfo = (*(*L).ci.get()).next;
    let mut next: *mut CallInfo = 0 as *mut CallInfo;
    if ci.is_null() {
        return;
    }
    loop {
        next = (*ci).next;
        if next.is_null() {
            break;
        }
        let next2: *mut CallInfo = (*next).next;
        (*ci).next = next2;
        (*L).nci.set((*L).nci.get().wrapping_sub(1));

        luaM_free_(
            next as *mut libc::c_void,
            ::core::mem::size_of::<CallInfo>(),
        );
        if next2.is_null() {
            break;
        }
        (*next2).previous = ci;
        ci = next2;
    }
}

pub unsafe fn lua_closethread(L: *const Thread) -> Result<(), Box<CallError>> {
    (*L).ci.set((*L).base_ci.get());
    let ci: *mut CallInfo = (*L).ci.get();

    (*(*L).stack.get()).val.tt_ = 0 | 0 << 4;
    (*ci).func = (*L).stack.get();
    (*ci).callstatus = ((1 as libc::c_int) << 1 as libc::c_int) as libc::c_ushort;

    let status = luaD_closeprotected(L, 1, Ok(()));

    (*L).top
        .set(((*L).stack.get()).offset(1 as libc::c_int as isize));
    (*ci).top = ((*L).top.get()).offset(20 as libc::c_int as isize);

    luaD_reallocstack(L, ((*ci).top).offset_from_unsigned((*L).stack.get()));

    return status;
}
