#![allow(
    mutable_transmutes,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments,
    unused_mut
)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::Thread;
use crate::ldo::{luaD_closeprotected, luaD_reallocstack};
use crate::lmem::{luaM_free_, luaM_malloc_};
use crate::lobject::StkId;
use std::ffi::{c_char, c_int, c_void};

pub type lua_Hook = Option<unsafe extern "C" fn(*mut Thread, *mut lua_Debug) -> ()>;
pub type lua_Reader =
    unsafe fn(*mut c_void, *mut usize) -> Result<*const c_char, Box<dyn std::error::Error>>;
pub type lua_Writer = unsafe fn(
    *mut Thread,
    *const c_void,
    usize,
    *mut c_void,
) -> Result<c_int, Box<dyn std::error::Error>>;

#[derive(Copy, Clone)]
#[repr(C)]
pub struct lua_Debug {
    pub event: libc::c_int,
    pub name: *const libc::c_char,
    pub namewhat: *const libc::c_char,
    pub what: *const libc::c_char,
    pub source: *const libc::c_char,
    pub srclen: usize,
    pub currentline: libc::c_int,
    pub linedefined: libc::c_int,
    pub lastlinedefined: libc::c_int,
    pub nups: libc::c_uchar,
    pub nparams: libc::c_uchar,
    pub isvararg: libc::c_char,
    pub istailcall: libc::c_char,
    pub ftransfer: libc::c_ushort,
    pub ntransfer: libc::c_ushort,
    pub short_src: [libc::c_char; 60],
    pub(crate) i_ci: *mut CallInfo,
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
    pub funcidx: c_int,
    pub nres: c_int,
    pub transferinfo: C2RustUnnamed_0,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct C2RustUnnamed_0 {
    pub ftransfer: libc::c_ushort,
    pub ntransfer: libc::c_ushort,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct C2RustUnnamed_3 {
    pub savedpc: *const u32,
    pub trap: libc::c_int,
    pub nextraargs: libc::c_int,
}

pub type lua_CFunction = unsafe fn(*mut Thread) -> Result<c_int, Box<dyn std::error::Error>>;

pub unsafe fn luaE_extendCI(mut L: *mut Thread) -> *mut CallInfo {
    let mut ci: *mut CallInfo = 0 as *mut CallInfo;
    ci = luaM_malloc_((*L).l_G, ::core::mem::size_of::<CallInfo>()) as *mut CallInfo;
    (*(*L).ci).next = ci;
    (*ci).previous = (*L).ci;
    (*ci).next = 0 as *mut CallInfo;
    ::core::ptr::write_volatile(&mut (*ci).u.trap as *mut libc::c_int, 0 as libc::c_int);
    (*L).nci.set((*L).nci.get().wrapping_add(1));

    return ci;
}

pub unsafe fn luaE_shrinkCI(mut L: *mut Thread) {
    let mut ci: *mut CallInfo = (*(*L).ci).next;
    let mut next: *mut CallInfo = 0 as *mut CallInfo;
    if ci.is_null() {
        return;
    }
    loop {
        next = (*ci).next;
        if next.is_null() {
            break;
        }
        let mut next2: *mut CallInfo = (*next).next;
        (*ci).next = next2;
        (*L).nci.set((*L).nci.get().wrapping_sub(1));

        luaM_free_(
            (*L).l_G,
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

pub unsafe fn lua_closethread(L: *mut Thread) -> Result<(), Box<dyn std::error::Error>> {
    (*L).ci = (*L).base_ci.get();
    let mut ci: *mut CallInfo = (*L).ci;
    (*(*L).stack).val.tt_ = (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    (*ci).func = (*L).stack;
    (*ci).callstatus = ((1 as libc::c_int) << 1 as libc::c_int) as libc::c_ushort;

    let status = luaD_closeprotected(L, 1, Ok(()));

    (*L).top = ((*L).stack).offset(1 as libc::c_int as isize);
    (*ci).top = ((*L).top).offset(20 as libc::c_int as isize);

    luaD_reallocstack(L, ((*ci).top).offset_from_unsigned((*L).stack));

    return status;
}
