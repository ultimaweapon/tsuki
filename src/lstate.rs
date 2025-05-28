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

use crate::Lua;
use crate::ldo::{luaD_closeprotected, luaD_reallocstack};
use crate::lfunc::luaF_closeupval;
use crate::lmem::{luaM_free_, luaM_malloc_};
use crate::lobject::{GCObject, StackValue, StkIdRel, UpVal};
use std::alloc::Layout;
use std::ffi::{c_char, c_int, c_void};

pub type lua_Hook = Option<unsafe extern "C" fn(*mut lua_State, *mut lua_Debug) -> ()>;
pub type lua_Reader =
    unsafe fn(*mut c_void, *mut usize) -> Result<*const c_char, Box<dyn std::error::Error>>;
pub type lua_Writer = unsafe fn(
    *mut lua_State,
    *const c_void,
    usize,
    *mut c_void,
) -> Result<c_int, Box<dyn std::error::Error>>;

#[repr(C)]
pub struct lua_State {
    pub(crate) next: *mut GCObject,
    pub(crate) tt: u8,
    pub(crate) marked: u8,
    pub(crate) refs: usize,
    pub(crate) handle: usize,
    pub(crate) allowhook: u8,
    pub(crate) nci: libc::c_ushort,
    pub(crate) top: StkIdRel,
    pub(crate) l_G: *const Lua,
    pub(crate) ci: *mut CallInfo,
    pub(crate) stack_last: StkIdRel,
    pub(crate) stack: StkIdRel,
    pub(crate) openupval: *mut UpVal,
    pub(crate) tbclist: StkIdRel,
    pub(crate) gclist: *mut GCObject,
    pub(crate) twups: *mut lua_State,
    pub(crate) base_ci: CallInfo,
    pub(crate) hook: lua_Hook,
    pub(crate) oldpc: libc::c_int,
    pub(crate) basehookcount: libc::c_int,
    pub(crate) hookcount: libc::c_int,
    pub(crate) hookmask: libc::c_int,
}

impl Drop for lua_State {
    fn drop(&mut self) {
        unsafe { luaF_closeupval(self, self.stack.p) };

        if unsafe { self.stack.p.is_null() } {
            return;
        }

        self.ci = &raw mut self.base_ci;

        unsafe { freeCI(self) };

        // Free stack.
        let layout = Layout::array::<StackValue>(unsafe {
            (self.stack_last.p.offset_from(self.stack.p) + 5) as usize
        })
        .unwrap();

        unsafe { std::alloc::dealloc(self.stack.p.cast(), layout) };
        unsafe { (*self.l_G).gc.decrease_debt(layout.size()) };
    }
}

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
    pub func: StkIdRel,
    pub top: StkIdRel,
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

pub type lua_CFunction = unsafe fn(*mut lua_State) -> Result<c_int, Box<dyn std::error::Error>>;

pub unsafe fn luaE_extendCI(mut L: *mut lua_State) -> *mut CallInfo {
    let mut ci: *mut CallInfo = 0 as *mut CallInfo;
    ci = luaM_malloc_((*L).l_G, ::core::mem::size_of::<CallInfo>()) as *mut CallInfo;
    (*(*L).ci).next = ci;
    (*ci).previous = (*L).ci;
    (*ci).next = 0 as *mut CallInfo;
    ::core::ptr::write_volatile(&mut (*ci).u.trap as *mut libc::c_int, 0 as libc::c_int);
    (*L).nci = ((*L).nci).wrapping_add(1);
    (*L).nci;
    return ci;
}

unsafe fn freeCI(mut L: *mut lua_State) {
    let mut ci: *mut CallInfo = (*L).ci;
    let mut next: *mut CallInfo = (*ci).next;
    (*ci).next = 0 as *mut CallInfo;
    loop {
        ci = next;
        if ci.is_null() {
            break;
        }
        next = (*ci).next;
        luaM_free_(
            (*L).l_G,
            ci as *mut libc::c_void,
            ::core::mem::size_of::<CallInfo>(),
        );
        (*L).nci = ((*L).nci).wrapping_sub(1);
        (*L).nci;
    }
}

pub unsafe fn luaE_shrinkCI(mut L: *mut lua_State) {
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
        (*L).nci = ((*L).nci).wrapping_sub(1);
        (*L).nci;
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

pub unsafe fn luaE_freethread(g: *const Lua, mut L1: *mut lua_State) {
    std::ptr::drop_in_place(L1);
    (*g).gc.dealloc(L1.cast(), Layout::new::<lua_State>());
}

pub unsafe fn lua_closethread(L: *mut lua_State) -> Result<(), Box<dyn std::error::Error>> {
    (*L).ci = &mut (*L).base_ci;
    let mut ci: *mut CallInfo = (*L).ci;
    (*(*L).stack.p).val.tt_ = (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    (*ci).func.p = (*L).stack.p;
    (*ci).callstatus = ((1 as libc::c_int) << 1 as libc::c_int) as libc::c_ushort;

    let status = luaD_closeprotected(L, 1, Ok(()));

    (*L).top.p = ((*L).stack.p).offset(1 as libc::c_int as isize);
    (*ci).top.p = ((*L).top.p).offset(20 as libc::c_int as isize);

    luaD_reallocstack(
        L,
        ((*ci).top.p).offset_from((*L).stack.p) as libc::c_long as libc::c_int,
    );

    return status;
}
