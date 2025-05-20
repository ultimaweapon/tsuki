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
#![allow(unused_variables)]

use crate::api_incr_top;
use crate::ldo::{luaD_closeprotected, luaD_reallocstack};
use crate::lfunc::luaF_closeupval;
use crate::lgc::{luaC_freeallobjects, luaC_newobj, luaC_step};
use crate::llex::luaX_init;
use crate::lmem::{luaM_free_, luaM_malloc_};
use crate::lobject::{GCObject, StackValue, StkId, StkIdRel, TString, TValue, Table, UpVal};
use crate::lstring::luaS_init;
use crate::ltable::{luaH_new, luaH_resize};
use crate::ltm::luaT_init;
use libc::{free, realloc};
use std::ffi::{c_char, c_int, c_void};
use std::ptr::{null, null_mut};

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
    pub(crate) allowhook: u8,
    pub(crate) nci: libc::c_ushort,
    pub(crate) top: StkIdRel,
    pub(crate) l_G: *mut global_State,
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

#[repr(C)]
pub struct global_State {
    pub totalbytes: isize,
    pub GCdebt: isize,
    pub GCestimate: usize,
    pub lastatomic: usize,
    pub strt: stringtable,
    pub l_registry: TValue,
    pub nilvalue: TValue,
    pub seed: libc::c_uint,
    pub currentwhite: u8,
    pub gcstate: u8,
    pub gckind: u8,
    pub gcstopem: u8,
    pub genminormul: u8,
    pub genmajormul: u8,
    pub gcstp: u8,
    pub gcemergency: u8,
    pub gcpause: u8,
    pub gcstepmul: u8,
    pub gcstepsize: u8,
    pub allgc: *mut GCObject,
    pub sweepgc: *mut *mut GCObject,
    pub gray: *mut GCObject,
    pub grayagain: *mut GCObject,
    pub weak: *mut GCObject,
    pub ephemeron: *mut GCObject,
    pub allweak: *mut GCObject,
    pub fixedgc: *mut GCObject,
    pub survival: *mut GCObject,
    pub old1: *mut GCObject,
    pub reallyold: *mut GCObject,
    pub firstold1: *mut GCObject,
    pub twups: *mut lua_State,
    pub memerrmsg: *mut TString,
    pub tmname: [*mut TString; 25],
    pub mt: [*mut Table; 9],
    pub strcache: [[*mut TString; 2]; 53],
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct stringtable {
    pub hash: *mut *mut TString,
    pub nuse: libc::c_int,
    pub size: libc::c_int,
}

#[repr(C)]
pub struct LG {
    pub l: lua_State,
    pub g: global_State,
}

pub unsafe extern "C" fn luaE_setdebt(mut g: *mut global_State, mut debt: isize) {
    let mut tb: isize = ((*g).totalbytes + (*g).GCdebt) as usize as isize;
    if debt < tb - (!(0 as libc::c_int as usize) >> 1 as libc::c_int) as isize {
        debt = tb - (!(0 as libc::c_int as usize) >> 1 as libc::c_int) as isize;
    }
    (*g).totalbytes = tb - debt;
    (*g).GCdebt = debt;
}

pub unsafe extern "C" fn lua_setcstacklimit(
    mut L: *mut lua_State,
    mut limit: libc::c_uint,
) -> libc::c_int {
    return 200 as libc::c_int;
}

pub unsafe extern "C" fn luaE_extendCI(mut L: *mut lua_State) -> *mut CallInfo {
    let mut ci: *mut CallInfo = 0 as *mut CallInfo;
    ci = luaM_malloc_(L, ::core::mem::size_of::<CallInfo>()) as *mut CallInfo;
    (*(*L).ci).next = ci;
    (*ci).previous = (*L).ci;
    (*ci).next = 0 as *mut CallInfo;
    ::core::ptr::write_volatile(&mut (*ci).u.trap as *mut libc::c_int, 0 as libc::c_int);
    (*L).nci = ((*L).nci).wrapping_add(1);
    (*L).nci;
    return ci;
}

unsafe extern "C" fn freeCI(mut L: *mut lua_State) {
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
            L,
            ci as *mut libc::c_void,
            ::core::mem::size_of::<CallInfo>(),
        );
        (*L).nci = ((*L).nci).wrapping_sub(1);
        (*L).nci;
    }
}

pub unsafe extern "C" fn luaE_shrinkCI(mut L: *mut lua_State) {
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
            L,
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

unsafe extern "C" fn stack_init(mut L1: *mut lua_State, mut L: *mut lua_State) {
    let mut i: libc::c_int = 0;
    let mut ci: *mut CallInfo = 0 as *mut CallInfo;
    (*L1).stack.p = luaM_malloc_(
        L,
        ((2 as libc::c_int * 20 as libc::c_int + 5 as libc::c_int) as usize)
            .wrapping_mul(::core::mem::size_of::<StackValue>()),
    ) as *mut StackValue;
    (*L1).tbclist.p = (*L1).stack.p;
    i = 0 as libc::c_int;

    while i < 2 as libc::c_int * 20 as libc::c_int + 5 as libc::c_int {
        (*((*L1).stack.p).offset(i as isize)).val.tt_ =
            (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
        i += 1;
    }

    (*L1).top.p = (*L1).stack.p;
    (*L1).stack_last.p = ((*L1).stack.p).offset((2 as libc::c_int * 20 as libc::c_int) as isize);
    ci = &mut (*L1).base_ci;
    (*ci).previous = 0 as *mut CallInfo;
    (*ci).next = (*ci).previous;
    (*ci).callstatus = ((1 as libc::c_int) << 1 as libc::c_int) as libc::c_ushort;
    (*ci).func.p = (*L1).top.p;
    (*ci).u.savedpc = null();
    (*ci).nresults = 0 as libc::c_int as libc::c_short;
    (*(*L1).top.p).val.tt_ = (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    (*L1).top.p = ((*L1).top.p).offset(1);
    (*L1).top.p;
    (*ci).top.p = ((*L1).top.p).offset(20 as libc::c_int as isize);
    (*L1).ci = ci;
}

unsafe extern "C" fn freestack(mut L: *mut lua_State) {
    if ((*L).stack.p).is_null() {
        return;
    }
    (*L).ci = &mut (*L).base_ci;
    freeCI(L);
    luaM_free_(
        L,
        (*L).stack.p as *mut libc::c_void,
        ((((*L).stack_last.p).offset_from((*L).stack.p) as libc::c_long as libc::c_int
            + 5 as libc::c_int) as usize)
            .wrapping_mul(::core::mem::size_of::<StackValue>()),
    );
}

unsafe fn init_registry(
    mut L: *mut lua_State,
    mut g: *mut global_State,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut registry: *mut Table = luaH_new(L)?;
    let mut io: *mut TValue = &mut (*g).l_registry;
    let mut x_: *mut Table = registry;
    (*io).value_.gc = x_ as *mut GCObject;
    (*io).tt_ = (5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 6 as libc::c_int) as u8;
    luaH_resize(
        L,
        registry,
        2 as libc::c_int as libc::c_uint,
        0 as libc::c_int as libc::c_uint,
    )?;

    // Create dummy object for LUA_RIDX_MAINTHREAD.
    let mut io_0: *mut TValue = &mut *((*registry).array)
        .offset((1 as libc::c_int - 1 as libc::c_int) as isize)
        as *mut TValue;

    (*io_0).value_.gc = luaH_new(L)? as *mut GCObject;
    (*io_0).tt_ = (5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 6 as libc::c_int) as u8;

    let mut io_1: *mut TValue = &mut *((*registry).array)
        .offset((2 as libc::c_int - 1 as libc::c_int) as isize)
        as *mut TValue;
    let mut x__1: *mut Table = luaH_new(L)?;
    (*io_1).value_.gc = x__1 as *mut GCObject;
    (*io_1).tt_ = (5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 6 as libc::c_int) as u8;
    Ok(())
}

unsafe fn f_luaopen(L: *mut lua_State) -> Result<(), Box<dyn std::error::Error>> {
    let mut g = (*L).l_G;
    stack_init(L, L);
    init_registry(L, g)?;
    luaS_init(L)?;
    luaT_init(L)?;
    luaX_init(L)?;
    (*g).gcstp = 0;
    (*g).nilvalue.tt_ = (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    Ok(())
}

unsafe fn preinit_thread(mut L: *mut lua_State, mut g: *mut global_State) {
    (*L).l_G = g;
    (*L).stack.p = 0 as StkId;
    (*L).ci = 0 as *mut CallInfo;
    (*L).nci = 0 as libc::c_int as libc::c_ushort;
    (*L).twups = L;
    ::core::ptr::write_volatile(&mut (*L).hook as *mut lua_Hook, None);
    ::core::ptr::write_volatile(&mut (*L).hookmask as *mut libc::c_int, 0 as libc::c_int);
    (*L).basehookcount = 0 as libc::c_int;
    (*L).allowhook = 1 as libc::c_int as u8;
    (*L).hookcount = (*L).basehookcount;
    (*L).openupval = 0 as *mut UpVal;
    (*L).oldpc = 0 as libc::c_int;
}

unsafe fn close_state(mut L: *mut lua_State) {
    let mut g: *mut global_State = (*L).l_G;
    if !((*g).nilvalue.tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int) {
        luaC_freeallobjects(L);
    } else {
        (*L).ci = &mut (*L).base_ci;
        let _ = luaD_closeprotected(L, 1, Ok(()));
        luaC_freeallobjects(L);
    }
    luaM_free_(
        L,
        (*(*L).l_G).strt.hash as *mut libc::c_void,
        ((*(*L).l_G).strt.size as usize).wrapping_mul(::core::mem::size_of::<*mut TString>()),
    );
    freestack(L);
    free(L.cast());
}

pub unsafe fn lua_newthread(mut L: *mut lua_State) -> *mut lua_State {
    let mut g: *mut global_State = (*L).l_G;
    let mut o: *mut GCObject = 0 as *mut GCObject;
    let mut L1: *mut lua_State = 0 as *mut lua_State;

    if (*(*L).l_G).GCdebt > 0 as libc::c_int as isize {
        luaC_step(L);
    }

    o = luaC_newobj(L, 8, ::core::mem::size_of::<lua_State>());
    L1 = o as *mut lua_State;

    let mut io: *mut TValue = &mut (*(*L).top.p).val;
    let mut x_: *mut lua_State = L1;

    (*io).value_.gc = x_ as *mut GCObject;
    (*io).tt_ = (8 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 6 as libc::c_int) as u8;

    api_incr_top(L);
    preinit_thread(L1, g);

    ::core::ptr::write_volatile(&mut (*L1).hookmask as *mut libc::c_int, (*L).hookmask);
    (*L1).basehookcount = (*L).basehookcount;
    ::core::ptr::write_volatile(&mut (*L1).hook as *mut lua_Hook, (*L).hook);
    (*L1).hookcount = (*L1).basehookcount;

    stack_init(L1, L);

    L1
}

pub unsafe fn luaE_freethread(mut L: *mut lua_State, mut L1: *mut lua_State) {
    luaF_closeupval(L1, (*L1).stack.p);
    freestack(L1);
    luaM_free_(
        L,
        L1 as *mut libc::c_void,
        ::core::mem::size_of::<lua_State>(),
    );
}

pub unsafe fn luaE_resetthread(mut L: *mut lua_State) -> Result<(), Box<dyn std::error::Error>> {
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

pub unsafe fn lua_closethread(
    mut L: *mut lua_State,
    mut from: *mut lua_State,
) -> Result<(), Box<dyn std::error::Error>> {
    luaE_resetthread(L)
}

pub unsafe fn lua_resetthread(L: *mut lua_State) -> Result<(), Box<dyn std::error::Error>> {
    return lua_closethread(L, 0 as *mut lua_State);
}

pub unsafe fn lua_newstate() -> *mut lua_State {
    let mut i: libc::c_int = 0;
    let mut L: *mut lua_State = 0 as *mut lua_State;
    let mut g: *mut global_State = 0 as *mut global_State;
    let mut l: *mut LG = realloc(0 as *mut libc::c_void, ::core::mem::size_of::<LG>()) as *mut LG;

    if l.is_null() {
        return 0 as *mut lua_State;
    }

    L = &raw mut (*l).l;
    g = &raw mut (*l).g;

    (*L).tt = 8 | 0 << 4;
    (*g).currentwhite = 1 << 3;
    (*L).marked = (*g).currentwhite & (1 << 3 | 1 << 4);
    preinit_thread(L, g);
    (*g).allgc = null_mut();
    (*g).seed = rand::random();
    (*g).gcstp = 2 as libc::c_int as u8;
    (*g).strt.nuse = 0 as libc::c_int;
    (*g).strt.size = (*g).strt.nuse;
    (*g).strt.hash = 0 as *mut *mut TString;
    (*g).l_registry.tt_ = (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    (*g).gcstate = 8 as libc::c_int as u8;
    (*g).gckind = 0 as libc::c_int as u8;
    (*g).gcstopem = 0 as libc::c_int as u8;
    (*g).gcemergency = 0 as libc::c_int as u8;
    (*g).fixedgc = 0 as *mut GCObject;
    (*g).reallyold = 0 as *mut GCObject;
    (*g).old1 = (*g).reallyold;
    (*g).survival = (*g).old1;
    (*g).firstold1 = (*g).survival;
    (*g).sweepgc = 0 as *mut *mut GCObject;
    (*g).grayagain = 0 as *mut GCObject;
    (*g).gray = (*g).grayagain;
    (*g).allweak = 0 as *mut GCObject;
    (*g).ephemeron = (*g).allweak;
    (*g).weak = (*g).ephemeron;
    (*g).twups = 0 as *mut lua_State;
    (*g).totalbytes = ::core::mem::size_of::<LG>() as libc::c_ulong as isize;
    (*g).GCdebt = 0 as libc::c_int as isize;
    (*g).lastatomic = 0 as libc::c_int as usize;
    let mut io: *mut TValue = &mut (*g).nilvalue;
    (*io).value_.i = 0 as libc::c_int as i64;
    (*io).tt_ = (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    (*g).gcpause = (200 as libc::c_int / 4 as libc::c_int) as u8;
    (*g).gcstepmul = (100 as libc::c_int / 4 as libc::c_int) as u8;
    (*g).gcstepsize = 13 as libc::c_int as u8;
    (*g).genmajormul = (100 as libc::c_int / 4 as libc::c_int) as u8;
    (*g).genminormul = 20 as libc::c_int as u8;
    i = 0 as libc::c_int;

    while i < 9 as libc::c_int {
        (*g).mt[i as usize] = 0 as *mut Table;
        i += 1;
    }

    if f_luaopen(L).is_err() {
        (*L).next = (*g).allgc;
        (*g).allgc = L as *mut GCObject;
        close_state(L);
        L = 0 as *mut lua_State;
    }

    (*L).next = (*g).allgc;
    (*g).allgc = L as *mut GCObject;

    return L;
}

pub unsafe fn lua_close(mut L: *mut lua_State) {
    close_state(L);
}
