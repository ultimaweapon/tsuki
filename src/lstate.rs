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
#![allow(path_statements)]

use crate::api_incr_top;
use crate::ldebug::luaG_runerror;
use crate::ldo::{
    lua_longjmp, luaD_closeprotected, luaD_rawrunprotected, luaD_reallocstack, luaD_seterrorobj,
    luaD_throw,
};
use crate::lfunc::luaF_closeupval;
use crate::lgc::{luaC_freeallobjects, luaC_newobjdt, luaC_step};
use crate::llex::luaX_init;
use crate::lmem::{luaM_free_, luaM_malloc_};
use crate::lobject::{
    Closure, GCObject, Proto, StackValue, StkId, StkIdRel, TString, TValue, Table, Udata, UpVal,
};
use crate::lstring::{luaS_hash, luaS_init};
use crate::ltable::{luaH_new, luaH_resize};
use crate::ltm::luaT_init;
use libc::{memcpy, time, time_t};
use std::ffi::{c_char, c_void};

pub type lua_Hook = Option<unsafe extern "C" fn(*mut lua_State, *mut lua_Debug) -> ()>;
pub type lua_Reader = unsafe fn(*mut lua_State, *mut c_void, *mut usize) -> *const c_char;
pub type lua_Writer = Option<
    unsafe extern "C" fn(
        *mut lua_State,
        *const libc::c_void,
        usize,
        *mut libc::c_void,
    ) -> libc::c_int,
>;

#[derive(Copy, Clone)]
#[repr(C)]
pub struct lua_State {
    pub next: *mut GCObject,
    pub tt: u8,
    pub marked: u8,
    pub status: u8,
    pub allowhook: u8,
    pub nci: libc::c_ushort,
    pub top: StkIdRel,
    pub l_G: *mut global_State,
    pub ci: *mut CallInfo,
    pub stack_last: StkIdRel,
    pub stack: StkIdRel,
    pub openupval: *mut UpVal,
    pub tbclist: StkIdRel,
    pub gclist: *mut GCObject,
    pub twups: *mut lua_State,
    pub errorJmp: *mut lua_longjmp,
    pub base_ci: CallInfo,
    pub hook: lua_Hook,
    pub errfunc: isize,
    pub nCcalls: u32,
    pub oldpc: libc::c_int,
    pub basehookcount: libc::c_int,
    pub hookcount: libc::c_int,
    pub hookmask: libc::c_int,
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
    pub i_ci: *mut CallInfo,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct CallInfo {
    pub func: StkIdRel,
    pub top: StkIdRel,
    pub previous: *mut CallInfo,
    pub next: *mut CallInfo,
    pub u: C2RustUnnamed_1,
    pub u2: C2RustUnnamed,
    pub nresults: libc::c_short,
    pub callstatus: libc::c_ushort,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub union C2RustUnnamed {
    pub funcidx: libc::c_int,
    pub nyield: libc::c_int,
    pub nres: libc::c_int,
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
pub union C2RustUnnamed_1 {
    pub l: C2RustUnnamed_3,
    pub c: C2RustUnnamed_2,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct C2RustUnnamed_2 {
    pub k: lua_KFunction,
    pub old_errfunc: isize,
    pub ctx: lua_KContext,
}

pub type lua_KContext = isize;
pub type lua_KFunction =
    Option<unsafe extern "C" fn(*mut lua_State, libc::c_int, lua_KContext) -> libc::c_int>;

#[derive(Copy, Clone)]
#[repr(C)]
pub struct C2RustUnnamed_3 {
    pub savedpc: *const u32,
    pub trap: libc::c_int,
    pub nextraargs: libc::c_int,
}

pub type lua_CFunction = Option<unsafe extern "C" fn(*mut lua_State) -> libc::c_int>;

#[derive(Copy, Clone)]
#[repr(C)]
pub struct global_State {
    pub frealloc: lua_Alloc,
    pub ud: *mut libc::c_void,
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
    pub finobj: *mut GCObject,
    pub gray: *mut GCObject,
    pub grayagain: *mut GCObject,
    pub weak: *mut GCObject,
    pub ephemeron: *mut GCObject,
    pub allweak: *mut GCObject,
    pub tobefnz: *mut GCObject,
    pub fixedgc: *mut GCObject,
    pub survival: *mut GCObject,
    pub old1: *mut GCObject,
    pub reallyold: *mut GCObject,
    pub firstold1: *mut GCObject,
    pub finobjsur: *mut GCObject,
    pub finobjold1: *mut GCObject,
    pub finobjrold: *mut GCObject,
    pub twups: *mut lua_State,
    pub mainthread: *mut lua_State,
    pub memerrmsg: *mut TString,
    pub tmname: [*mut TString; 25],
    pub mt: [*mut Table; 9],
    pub strcache: [[*mut TString; 2]; 53],
    pub warnf: lua_WarnFunction,
    pub ud_warn: *mut libc::c_void,
}

pub type lua_WarnFunction =
    Option<unsafe extern "C" fn(*mut libc::c_void, *const libc::c_char, libc::c_int) -> ()>;

#[derive(Copy, Clone)]
#[repr(C)]
pub struct stringtable {
    pub hash: *mut *mut TString,
    pub nuse: libc::c_int,
    pub size: libc::c_int,
}

pub type lua_Alloc = Option<
    unsafe extern "C" fn(*mut libc::c_void, *mut libc::c_void, usize, usize) -> *mut libc::c_void,
>;

#[derive(Copy, Clone)]
#[repr(C)]
pub struct LG {
    pub l: lua_State,
    pub g: global_State,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub union GCUnion {
    pub gc: GCObject,
    pub ts: TString,
    pub u: Udata,
    pub cl: Closure,
    pub h: Table,
    pub p: Proto,
    pub th: lua_State,
    pub upv: UpVal,
}

unsafe extern "C" fn luai_makeseed(mut L: *mut lua_State) -> libc::c_uint {
    let mut buff: [libc::c_char; 24] = [0; 24];
    let mut h: libc::c_uint = time(0 as *mut time_t) as libc::c_uint;
    let mut p: libc::c_int = 0 as libc::c_int;
    let mut t: usize = L as usize;
    memcpy(
        buff.as_mut_ptr().offset(p as isize) as *mut libc::c_void,
        &mut t as *mut usize as *const libc::c_void,
        ::core::mem::size_of::<usize>(),
    );
    p = (p as libc::c_ulong).wrapping_add(::core::mem::size_of::<usize>() as libc::c_ulong)
        as libc::c_int as libc::c_int;
    let mut t_0: usize = &mut h as *mut libc::c_uint as usize;
    memcpy(
        buff.as_mut_ptr().offset(p as isize) as *mut libc::c_void,
        &mut t_0 as *mut usize as *const libc::c_void,
        ::core::mem::size_of::<usize>(),
    );
    p = (p as libc::c_ulong).wrapping_add(::core::mem::size_of::<usize>() as libc::c_ulong)
        as libc::c_int as libc::c_int;
    let mut t_1: usize = ::core::mem::transmute::<
        Option<unsafe extern "C" fn(lua_Alloc, *mut libc::c_void) -> *mut lua_State>,
        usize,
    >(Some(
        lua_newstate as unsafe extern "C" fn(lua_Alloc, *mut libc::c_void) -> *mut lua_State,
    ));
    memcpy(
        buff.as_mut_ptr().offset(p as isize) as *mut libc::c_void,
        &mut t_1 as *mut usize as *const libc::c_void,
        ::core::mem::size_of::<usize>(),
    );
    p = (p as libc::c_ulong).wrapping_add(::core::mem::size_of::<usize>() as libc::c_ulong)
        as libc::c_int as libc::c_int;
    return luaS_hash(buff.as_mut_ptr(), p as usize, h);
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaE_setdebt(mut g: *mut global_State, mut debt: isize) {
    let mut tb: isize = ((*g).totalbytes + (*g).GCdebt) as usize as isize;
    if debt < tb - (!(0 as libc::c_int as usize) >> 1 as libc::c_int) as isize {
        debt = tb - (!(0 as libc::c_int as usize) >> 1 as libc::c_int) as isize;
    }
    (*g).totalbytes = tb - debt;
    (*g).GCdebt = debt;
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn lua_setcstacklimit(
    mut L: *mut lua_State,
    mut limit: libc::c_uint,
) -> libc::c_int {
    return 200 as libc::c_int;
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaE_extendCI(mut L: *mut lua_State) -> *mut CallInfo {
    let mut ci: *mut CallInfo = 0 as *mut CallInfo;
    ci = luaM_malloc_(L, ::core::mem::size_of::<CallInfo>(), 0 as libc::c_int) as *mut CallInfo;
    (*(*L).ci).next = ci;
    (*ci).previous = (*L).ci;
    (*ci).next = 0 as *mut CallInfo;
    ::core::ptr::write_volatile(&mut (*ci).u.l.trap as *mut libc::c_int, 0 as libc::c_int);
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
#[unsafe(no_mangle)]
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaE_checkcstack(mut L: *mut lua_State) {
    if (*L).nCcalls & 0xffff as libc::c_int as u32 == 200 as libc::c_int as u32 {
        luaG_runerror(L, "C stack overflow");
    } else if (*L).nCcalls & 0xffff as libc::c_int as u32
        >= (200 as libc::c_int / 10 as libc::c_int * 11 as libc::c_int) as u32
    {
        luaD_throw(L, 5 as libc::c_int);
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaE_incCstack(mut L: *mut lua_State) {
    (*L).nCcalls = ((*L).nCcalls).wrapping_add(1);
    (*L).nCcalls;
    if (((*L).nCcalls & 0xffff as libc::c_int as u32 >= 200 as libc::c_int as u32) as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        luaE_checkcstack(L);
    }
}
unsafe extern "C" fn stack_init(mut L1: *mut lua_State, mut L: *mut lua_State) {
    let mut i: libc::c_int = 0;
    let mut ci: *mut CallInfo = 0 as *mut CallInfo;
    (*L1).stack.p = luaM_malloc_(
        L,
        ((2 as libc::c_int * 20 as libc::c_int + 5 as libc::c_int) as usize)
            .wrapping_mul(::core::mem::size_of::<StackValue>()),
        0 as libc::c_int,
    ) as *mut StackValue;
    (*L1).tbclist.p = (*L1).stack.p;
    i = 0 as libc::c_int;
    while i < 2 as libc::c_int * 20 as libc::c_int + 5 as libc::c_int {
        (*((*L1).stack.p).offset(i as isize)).val.tt_ =
            (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
        i += 1;
        i;
    }
    (*L1).top.p = (*L1).stack.p;
    (*L1).stack_last.p = ((*L1).stack.p).offset((2 as libc::c_int * 20 as libc::c_int) as isize);
    ci = &mut (*L1).base_ci;
    (*ci).previous = 0 as *mut CallInfo;
    (*ci).next = (*ci).previous;
    (*ci).callstatus = ((1 as libc::c_int) << 1 as libc::c_int) as libc::c_ushort;
    (*ci).func.p = (*L1).top.p;
    (*ci).u.c.k = None;
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
unsafe extern "C" fn init_registry(mut L: *mut lua_State, mut g: *mut global_State) {
    let mut registry: *mut Table = luaH_new(L);
    let mut io: *mut TValue = &mut (*g).l_registry;
    let mut x_: *mut Table = registry;
    (*io).value_.gc = &mut (*(x_ as *mut GCUnion)).gc;
    (*io).tt_ = (5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 6 as libc::c_int) as u8;
    luaH_resize(
        L,
        registry,
        2 as libc::c_int as libc::c_uint,
        0 as libc::c_int as libc::c_uint,
    );
    let mut io_0: *mut TValue = &mut *((*registry).array)
        .offset((1 as libc::c_int - 1 as libc::c_int) as isize)
        as *mut TValue;
    let mut x__0: *mut lua_State = L;
    (*io_0).value_.gc = &mut (*(x__0 as *mut GCUnion)).gc;
    (*io_0).tt_ = (8 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 6 as libc::c_int) as u8;
    let mut io_1: *mut TValue = &mut *((*registry).array)
        .offset((2 as libc::c_int - 1 as libc::c_int) as isize)
        as *mut TValue;
    let mut x__1: *mut Table = luaH_new(L);
    (*io_1).value_.gc = &mut (*(x__1 as *mut GCUnion)).gc;
    (*io_1).tt_ = (5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 6 as libc::c_int) as u8;
}
unsafe extern "C" fn f_luaopen(mut L: *mut lua_State, mut ud: *mut libc::c_void) {
    let mut g: *mut global_State = (*L).l_G;
    stack_init(L, L);
    init_registry(L, g);
    luaS_init(L);
    luaT_init(L);
    luaX_init(L);
    (*g).gcstp = 0 as libc::c_int as u8;
    (*g).nilvalue.tt_ = (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
}
unsafe extern "C" fn preinit_thread(mut L: *mut lua_State, mut g: *mut global_State) {
    (*L).l_G = g;
    (*L).stack.p = 0 as StkId;
    (*L).ci = 0 as *mut CallInfo;
    (*L).nci = 0 as libc::c_int as libc::c_ushort;
    (*L).twups = L;
    (*L).nCcalls = 0 as libc::c_int as u32;
    (*L).errorJmp = 0 as *mut lua_longjmp;
    ::core::ptr::write_volatile(&mut (*L).hook as *mut lua_Hook, None);
    ::core::ptr::write_volatile(&mut (*L).hookmask as *mut libc::c_int, 0 as libc::c_int);
    (*L).basehookcount = 0 as libc::c_int;
    (*L).allowhook = 1 as libc::c_int as u8;
    (*L).hookcount = (*L).basehookcount;
    (*L).openupval = 0 as *mut UpVal;
    (*L).status = 0 as libc::c_int as u8;
    (*L).errfunc = 0 as libc::c_int as isize;
    (*L).oldpc = 0 as libc::c_int;
}

unsafe extern "C" fn close_state(mut L: *mut lua_State) {
    let mut g: *mut global_State = (*L).l_G;
    if !((*g).nilvalue.tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int) {
        luaC_freeallobjects(L);
    } else {
        (*L).ci = &mut (*L).base_ci;
        luaD_closeprotected(L, 1 as libc::c_int as isize, 0 as libc::c_int);
        luaC_freeallobjects(L);
    }
    luaM_free_(
        L,
        (*(*L).l_G).strt.hash as *mut libc::c_void,
        ((*(*L).l_G).strt.size as usize).wrapping_mul(::core::mem::size_of::<*mut TString>()),
    );
    freestack(L);

    (Some(((*g).frealloc).expect("non-null function pointer"))).expect("non-null function pointer")(
        (*g).ud,
        L as *mut libc::c_void,
        ::core::mem::size_of::<LG>(),
        0,
    );
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lua_newthread(mut L: *mut lua_State) -> *mut lua_State {
    let mut g: *mut global_State = (*L).l_G;
    let mut o: *mut GCObject = 0 as *mut GCObject;
    let mut L1: *mut lua_State = 0 as *mut lua_State;

    if (*(*L).l_G).GCdebt > 0 as libc::c_int as isize {
        luaC_step(L);
    }

    o = luaC_newobjdt(L, 8, ::core::mem::size_of::<lua_State>(), 0);
    L1 = &mut (*(o as *mut GCUnion)).th;

    let mut io: *mut TValue = &mut (*(*L).top.p).val;
    let mut x_: *mut lua_State = L1;

    (*io).value_.gc = &mut (*(x_ as *mut GCUnion)).gc;
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

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaE_freethread(mut L: *mut lua_State, mut L1: *mut lua_State) {
    luaF_closeupval(L1, (*L1).stack.p);
    freestack(L1);
    luaM_free_(
        L,
        L1 as *mut libc::c_void,
        ::core::mem::size_of::<lua_State>(),
    );
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaE_resetthread(
    mut L: *mut lua_State,
    mut status: libc::c_int,
) -> libc::c_int {
    (*L).ci = &mut (*L).base_ci;
    let mut ci: *mut CallInfo = (*L).ci;
    (*(*L).stack.p).val.tt_ = (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    (*ci).func.p = (*L).stack.p;
    (*ci).callstatus = ((1 as libc::c_int) << 1 as libc::c_int) as libc::c_ushort;
    if status == 1 as libc::c_int {
        status = 0 as libc::c_int;
    }
    (*L).status = 0 as libc::c_int as u8;
    status = luaD_closeprotected(L, 1 as libc::c_int as isize, status);
    if status != 0 as libc::c_int {
        luaD_seterrorobj(L, status, ((*L).stack.p).offset(1 as libc::c_int as isize));
    } else {
        (*L).top.p = ((*L).stack.p).offset(1 as libc::c_int as isize);
    }
    (*ci).top.p = ((*L).top.p).offset(20 as libc::c_int as isize);
    luaD_reallocstack(
        L,
        ((*ci).top.p).offset_from((*L).stack.p) as libc::c_long as libc::c_int,
        0 as libc::c_int,
    );
    return status;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lua_closethread(
    mut L: *mut lua_State,
    mut from: *mut lua_State,
) -> libc::c_int {
    let mut status: libc::c_int = 0;
    (*L).nCcalls = if !from.is_null() {
        (*from).nCcalls & 0xffff as libc::c_int as u32
    } else {
        0 as libc::c_int as u32
    };
    status = luaE_resetthread(L, (*L).status as libc::c_int);
    return status;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lua_resetthread(mut L: *mut lua_State) -> libc::c_int {
    return lua_closethread(L, 0 as *mut lua_State);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lua_newstate(
    mut f: lua_Alloc,
    mut ud: *mut libc::c_void,
) -> *mut lua_State {
    let mut i: libc::c_int = 0;
    let mut L: *mut lua_State = 0 as *mut lua_State;
    let mut g: *mut global_State = 0 as *mut global_State;
    let mut l: *mut LG = (Some(f.expect("non-null function pointer")))
        .expect("non-null function pointer")(
        ud,
        0 as *mut libc::c_void,
        8 as libc::c_int as usize,
        ::core::mem::size_of::<LG>(),
    ) as *mut LG;
    if l.is_null() {
        return 0 as *mut lua_State;
    }
    L = &mut (*l).l;
    g = &mut (*l).g;
    (*L).tt = (8 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    (*g).currentwhite = ((1 as libc::c_int) << 3 as libc::c_int) as u8;
    (*L).marked = ((*g).currentwhite as libc::c_int
        & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int))
        as u8;
    preinit_thread(L, g);
    (*g).allgc = &mut (*(L as *mut GCUnion)).gc;
    (*L).next = 0 as *mut GCObject;
    (*L).nCcalls = ((*L).nCcalls).wrapping_add(0x10000 as libc::c_int as u32);
    (*g).frealloc = f;
    (*g).ud = ud;
    (*g).warnf = None;
    (*g).ud_warn = 0 as *mut libc::c_void;
    (*g).mainthread = L;
    (*g).seed = luai_makeseed(L);
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
    (*g).tobefnz = (*g).fixedgc;
    (*g).finobj = (*g).tobefnz;
    (*g).reallyold = 0 as *mut GCObject;
    (*g).old1 = (*g).reallyold;
    (*g).survival = (*g).old1;
    (*g).firstold1 = (*g).survival;
    (*g).finobjrold = 0 as *mut GCObject;
    (*g).finobjold1 = (*g).finobjrold;
    (*g).finobjsur = (*g).finobjold1;
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
        i;
    }
    if luaD_rawrunprotected(
        L,
        Some(f_luaopen as unsafe extern "C" fn(*mut lua_State, *mut libc::c_void) -> ()),
        0 as *mut libc::c_void,
    ) != 0 as libc::c_int
    {
        close_state(L);
        L = 0 as *mut lua_State;
    }
    return L;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn lua_close(mut L: *mut lua_State) {
    L = (*(*L).l_G).mainthread;
    close_state(L);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaE_warning(
    mut L: *mut lua_State,
    mut msg: *const libc::c_char,
    mut tocont: libc::c_int,
) {
    let mut wf: lua_WarnFunction = (*(*L).l_G).warnf;
    if wf.is_some() {
        wf.expect("non-null function pointer")((*(*L).l_G).ud_warn, msg, tocont);
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaE_warnerror(mut L: *mut lua_State, mut where_0: *const libc::c_char) {
    let mut errobj: *mut TValue = &mut (*((*L).top.p).offset(-(1 as libc::c_int as isize))).val;
    let mut msg: *const libc::c_char = if (*errobj).tt_ as libc::c_int & 0xf as libc::c_int
        == 4 as libc::c_int
    {
        ((*((*errobj).value_.gc as *mut GCUnion)).ts.contents).as_mut_ptr() as *const libc::c_char
    } else {
        b"error object is not a string\0" as *const u8 as *const libc::c_char
    };
    luaE_warning(
        L,
        b"error in \0" as *const u8 as *const libc::c_char,
        1 as libc::c_int,
    );
    luaE_warning(L, where_0, 1 as libc::c_int);
    luaE_warning(
        L,
        b" (\0" as *const u8 as *const libc::c_char,
        1 as libc::c_int,
    );
    luaE_warning(L, msg, 1 as libc::c_int);
    luaE_warning(
        L,
        b")\0" as *const u8 as *const libc::c_char,
        0 as libc::c_int,
    );
}
