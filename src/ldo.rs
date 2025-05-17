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

use crate::ldebug::{luaG_callerror, luaG_runerror};
use crate::lfunc::{luaF_close, luaF_initupvals};
use crate::lgc::luaC_step;
use crate::lmem::{luaM_free_, luaM_realloc_, luaM_saferealloc_};
use crate::lobject::{LClosure, Proto, StackValue, StkId, TValue, UpVal};
use crate::lparser::{C2RustUnnamed_9, Dyndata, Labeldesc, Labellist, Vardesc, luaY_parser};
use crate::lstate::{
    CallInfo, GCUnion, lua_CFunction, lua_Debug, lua_Hook, lua_State, luaE_checkcstack,
    luaE_extendCI, luaE_shrinkCI,
};
use crate::ltm::{TM_CALL, luaT_gettmbyobj};
use crate::lundump::luaU_undump;
use crate::lvm::{luaV_execute, luaV_finishOp};
use crate::lzio::{Mbuffer, ZIO, luaZ_fill};
use libc::strchr;
use std::ffi::{CStr, c_char, c_int, c_void};

pub type Pfunc = unsafe fn(*mut lua_State, *mut c_void) -> Result<(), Box<dyn std::error::Error>>;

#[derive(Copy, Clone)]
#[repr(C)]
struct SParser {
    z: *mut ZIO,
    buff: Mbuffer,
    dyd: Dyndata,
    mode: *const libc::c_char,
    name: *const libc::c_char,
}

#[repr(C)]
pub struct CloseP {
    pub level: StkId,
    pub status: Result<(), Box<dyn std::error::Error>>,
}

pub unsafe fn luaD_rawrunprotected(
    mut L: *mut lua_State,
    mut f: Pfunc,
    mut ud: *mut libc::c_void,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut oldnCcalls: u32 = (*L).nCcalls;
    let r = f(L, ud);

    (*L).nCcalls = oldnCcalls;

    r
}

unsafe extern "C" fn relstack(mut L: *mut lua_State) {
    let mut ci: *mut CallInfo = 0 as *mut CallInfo;
    let mut up: *mut UpVal = 0 as *mut UpVal;
    (*L).top.offset =
        ((*L).top.p as *mut libc::c_char).offset_from((*L).stack.p as *mut libc::c_char);
    (*L).tbclist.offset =
        ((*L).tbclist.p as *mut libc::c_char).offset_from((*L).stack.p as *mut libc::c_char);
    up = (*L).openupval;
    while !up.is_null() {
        (*up).v.offset = ((*up).v.p as StkId as *mut libc::c_char)
            .offset_from((*L).stack.p as *mut libc::c_char);
        up = (*up).u.open.next;
    }
    ci = (*L).ci;
    while !ci.is_null() {
        (*ci).top.offset =
            ((*ci).top.p as *mut libc::c_char).offset_from((*L).stack.p as *mut libc::c_char);
        (*ci).func.offset =
            ((*ci).func.p as *mut libc::c_char).offset_from((*L).stack.p as *mut libc::c_char);
        ci = (*ci).previous;
    }
}

unsafe extern "C" fn correctstack(mut L: *mut lua_State) {
    let mut ci: *mut CallInfo = 0 as *mut CallInfo;
    let mut up: *mut UpVal = 0 as *mut UpVal;
    (*L).top.p = ((*L).stack.p as *mut libc::c_char).offset((*L).top.offset as isize) as StkId;
    (*L).tbclist.p =
        ((*L).stack.p as *mut libc::c_char).offset((*L).tbclist.offset as isize) as StkId;
    up = (*L).openupval;
    while !up.is_null() {
        (*up).v.p = &mut (*(((*L).stack.p as *mut libc::c_char).offset((*up).v.offset as isize)
            as StkId))
            .val;
        up = (*up).u.open.next;
    }
    ci = (*L).ci;
    while !ci.is_null() {
        (*ci).top.p =
            ((*L).stack.p as *mut libc::c_char).offset((*ci).top.offset as isize) as StkId;
        (*ci).func.p =
            ((*L).stack.p as *mut libc::c_char).offset((*ci).func.offset as isize) as StkId;
        if (*ci).callstatus as libc::c_int & (1 as libc::c_int) << 1 as libc::c_int == 0 {
            ::core::ptr::write_volatile(&mut (*ci).u.trap as *mut libc::c_int, 1 as libc::c_int);
        }
        ci = (*ci).previous;
    }
}

pub unsafe fn luaD_reallocstack(mut L: *mut lua_State, mut newsize: c_int) {
    let mut oldsize: libc::c_int =
        ((*L).stack_last.p).offset_from((*L).stack.p) as libc::c_long as libc::c_int;
    let mut i: libc::c_int = 0;
    let mut newstack: StkId = 0 as *mut StackValue;
    let mut oldgcstop: libc::c_int = (*(*L).l_G).gcstopem as libc::c_int;
    relstack(L);
    (*(*L).l_G).gcstopem = 1 as libc::c_int as u8;
    newstack = luaM_realloc_(
        L,
        (*L).stack.p as *mut libc::c_void,
        ((oldsize + 5 as libc::c_int) as usize).wrapping_mul(::core::mem::size_of::<StackValue>()),
        ((newsize + 5 as libc::c_int) as usize).wrapping_mul(::core::mem::size_of::<StackValue>()),
    ) as *mut StackValue;
    (*(*L).l_G).gcstopem = oldgcstop as u8;
    if ((newstack == 0 as *mut libc::c_void as StkId) as libc::c_int != 0 as libc::c_int)
        as libc::c_int as libc::c_long
        != 0
    {
        correctstack(L);
        todo!("use handle_alloc_error");
    }
    (*L).stack.p = newstack;
    correctstack(L);
    (*L).stack_last.p = ((*L).stack.p).offset(newsize as isize);
    i = oldsize + 5 as libc::c_int;
    while i < newsize + 5 as libc::c_int {
        (*newstack.offset(i as isize)).val.tt_ =
            (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
        i += 1;
    }
}

pub unsafe fn luaD_growstack(
    L: *mut lua_State,
    n: c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let size = ((*L).stack_last.p).offset_from((*L).stack.p) as libc::c_long as libc::c_int;

    if size > 1000000 {
        return luaG_runerror(L, "stack overflow");
    } else if n < 1000000 {
        let mut newsize: libc::c_int = 2 as libc::c_int * size;
        let mut needed = ((*L).top.p).offset_from((*L).stack.p) as libc::c_long as libc::c_int + n;

        if newsize > 1000000 as libc::c_int {
            newsize = 1000000 as libc::c_int;
        }

        if newsize < needed {
            newsize = needed;
        }

        if newsize <= 1000000 {
            luaD_reallocstack(L, newsize);
            return Ok(());
        }
    }

    luaG_runerror(L, "stack overflow")
}

unsafe extern "C" fn stackinuse(mut L: *mut lua_State) -> libc::c_int {
    let mut ci: *mut CallInfo = 0 as *mut CallInfo;
    let mut res: libc::c_int = 0;
    let mut lim: StkId = (*L).top.p;
    ci = (*L).ci;
    while !ci.is_null() {
        if lim < (*ci).top.p {
            lim = (*ci).top.p;
        }
        ci = (*ci).previous;
    }
    res = lim.offset_from((*L).stack.p) as libc::c_long as libc::c_int + 1 as libc::c_int;
    if res < 20 as libc::c_int {
        res = 20 as libc::c_int;
    }
    return res;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaD_shrinkstack(mut L: *mut lua_State) {
    let mut inuse: libc::c_int = stackinuse(L);
    let mut max: libc::c_int = if inuse > 1000000 as libc::c_int / 3 as libc::c_int {
        1000000 as libc::c_int
    } else {
        inuse * 3 as libc::c_int
    };
    if inuse <= 1000000 as libc::c_int
        && ((*L).stack_last.p).offset_from((*L).stack.p) as libc::c_long as libc::c_int > max
    {
        let mut nsize: libc::c_int = if inuse > 1000000 as libc::c_int / 2 as libc::c_int {
            1000000 as libc::c_int
        } else {
            inuse * 2 as libc::c_int
        };
        luaD_reallocstack(L, nsize);
    }
    luaE_shrinkCI(L);
}

pub unsafe fn luaD_inctop(mut L: *mut lua_State) -> Result<(), Box<dyn std::error::Error>> {
    if ((((*L).stack_last.p).offset_from((*L).top.p) as libc::c_long
        <= 1 as libc::c_int as libc::c_long) as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        luaD_growstack(L, 1)?;
    }
    (*L).top.p = ((*L).top.p).offset(1);
    (*L).top.p;
    Ok(())
}

pub unsafe fn luaD_hook(
    mut L: *mut lua_State,
    mut event: libc::c_int,
    mut line: libc::c_int,
    mut ftransfer: libc::c_int,
    mut ntransfer: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut hook: lua_Hook = (*L).hook;
    if hook.is_some() && (*L).allowhook as libc::c_int != 0 {
        let mut mask: libc::c_int = (1 as libc::c_int) << 3 as libc::c_int;
        let mut ci: *mut CallInfo = (*L).ci;
        let mut top: isize =
            ((*L).top.p as *mut libc::c_char).offset_from((*L).stack.p as *mut libc::c_char);
        let mut ci_top: isize =
            ((*ci).top.p as *mut libc::c_char).offset_from((*L).stack.p as *mut libc::c_char);
        let mut ar: lua_Debug = lua_Debug {
            event: 0,
            name: 0 as *const libc::c_char,
            namewhat: 0 as *const libc::c_char,
            what: 0 as *const libc::c_char,
            source: 0 as *const libc::c_char,
            srclen: 0,
            currentline: 0,
            linedefined: 0,
            lastlinedefined: 0,
            nups: 0,
            nparams: 0,
            isvararg: 0,
            istailcall: 0,
            ftransfer: 0,
            ntransfer: 0,
            short_src: [0; 60],
            i_ci: 0 as *mut CallInfo,
        };
        ar.event = event;
        ar.currentline = line;
        ar.i_ci = ci;
        if ntransfer != 0 as libc::c_int {
            mask |= (1 as libc::c_int) << 8 as libc::c_int;
            (*ci).u2.transferinfo.ftransfer = ftransfer as libc::c_ushort;
            (*ci).u2.transferinfo.ntransfer = ntransfer as libc::c_ushort;
        }
        if (*ci).callstatus as libc::c_int & (1 as libc::c_int) << 1 as libc::c_int == 0
            && (*L).top.p < (*ci).top.p
        {
            (*L).top.p = (*ci).top.p;
        }
        if ((((*L).stack_last.p).offset_from((*L).top.p) as libc::c_long
            <= 20 as libc::c_int as libc::c_long) as libc::c_int
            != 0 as libc::c_int) as libc::c_int as libc::c_long
            != 0
        {
            luaD_growstack(L, 20)?;
        }
        if (*ci).top.p < ((*L).top.p).offset(20 as libc::c_int as isize) {
            (*ci).top.p = ((*L).top.p).offset(20 as libc::c_int as isize);
        }
        (*L).allowhook = 0 as libc::c_int as u8;
        (*ci).callstatus = ((*ci).callstatus as libc::c_int | mask) as libc::c_ushort;
        (Some(hook.expect("non-null function pointer"))).expect("non-null function pointer")(
            L, &mut ar,
        );
        (*L).allowhook = 1 as libc::c_int as u8;
        (*ci).top.p = ((*L).stack.p as *mut libc::c_char).offset(ci_top as isize) as StkId;
        (*L).top.p = ((*L).stack.p as *mut libc::c_char).offset(top as isize) as StkId;
        (*ci).callstatus = ((*ci).callstatus as libc::c_int & !mask) as libc::c_ushort;
    }
    Ok(())
}

pub unsafe fn luaD_hookcall(
    mut L: *mut lua_State,
    mut ci: *mut CallInfo,
) -> Result<(), Box<dyn std::error::Error>> {
    (*L).oldpc = 0 as libc::c_int;
    if (*L).hookmask & (1 as libc::c_int) << 0 as libc::c_int != 0 {
        let mut event: libc::c_int =
            if (*ci).callstatus as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0 {
                4 as libc::c_int
            } else {
                0 as libc::c_int
            };
        let mut p: *mut Proto = (*((*(*ci).func.p).val.value_.gc as *mut GCUnion)).cl.l.p;
        (*ci).u.savedpc = ((*ci).u.savedpc).offset(1);
        (*ci).u.savedpc;
        luaD_hook(
            L,
            event,
            -(1 as libc::c_int),
            1 as libc::c_int,
            (*p).numparams as libc::c_int,
        )?;
        (*ci).u.savedpc = ((*ci).u.savedpc).offset(-1);
        (*ci).u.savedpc;
    }
    Ok(())
}

unsafe fn rethook(
    mut L: *mut lua_State,
    mut ci: *mut CallInfo,
    mut nres: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    if (*L).hookmask & (1 as libc::c_int) << 1 as libc::c_int != 0 {
        let mut firstres: StkId = ((*L).top.p).offset(-(nres as isize));
        let mut delta: libc::c_int = 0 as libc::c_int;
        let mut ftransfer: libc::c_int = 0;
        if (*ci).callstatus as libc::c_int & (1 as libc::c_int) << 1 as libc::c_int == 0 {
            let mut p: *mut Proto = (*((*(*ci).func.p).val.value_.gc as *mut GCUnion)).cl.l.p;
            if (*p).is_vararg != 0 {
                delta = (*ci).u.nextraargs + (*p).numparams as libc::c_int + 1 as libc::c_int;
            }
        }
        (*ci).func.p = ((*ci).func.p).offset(delta as isize);
        ftransfer =
            firstres.offset_from((*ci).func.p) as libc::c_long as libc::c_ushort as libc::c_int;
        luaD_hook(L, 1 as libc::c_int, -(1 as libc::c_int), ftransfer, nres)?;
        (*ci).func.p = ((*ci).func.p).offset(-(delta as isize));
    }
    ci = (*ci).previous;
    if (*ci).callstatus as libc::c_int & (1 as libc::c_int) << 1 as libc::c_int == 0 {
        (*L).oldpc = ((*ci).u.savedpc)
            .offset_from((*(*((*(*ci).func.p).val.value_.gc as *mut GCUnion)).cl.l.p).code)
            as libc::c_long as libc::c_int
            - 1 as libc::c_int;
    }
    Ok(())
}

unsafe fn tryfuncTM(
    mut L: *mut lua_State,
    mut func: StkId,
) -> Result<StkId, Box<dyn std::error::Error>> {
    let mut tm: *const TValue = 0 as *const TValue;
    let mut p: StkId = 0 as *mut StackValue;
    if ((((*L).stack_last.p).offset_from((*L).top.p) as libc::c_long
        <= 1 as libc::c_int as libc::c_long) as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        let mut t__: isize =
            (func as *mut libc::c_char).offset_from((*L).stack.p as *mut libc::c_char);
        if (*(*L).l_G).GCdebt > 0 as libc::c_int as isize {
            luaC_step(L);
        }
        luaD_growstack(L, 1)?;
        func = ((*L).stack.p as *mut libc::c_char).offset(t__ as isize) as StkId;
    }
    tm = luaT_gettmbyobj(L, &mut (*func).val, TM_CALL);
    if (((*tm).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int) as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        luaG_callerror(L, &mut (*func).val)?;
    }
    p = (*L).top.p;
    while p > func {
        let mut io1: *mut TValue = &mut (*p).val;
        let mut io2: *const TValue = &mut (*p.offset(-(1 as libc::c_int as isize))).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        p = p.offset(-1);
    }
    (*L).top.p = ((*L).top.p).offset(1);
    (*L).top.p;
    let mut io1_0: *mut TValue = &mut (*func).val;
    let mut io2_0: *const TValue = tm;
    (*io1_0).value_ = (*io2_0).value_;
    (*io1_0).tt_ = (*io2_0).tt_;
    return Ok(func);
}

unsafe fn moveresults(
    mut L: *mut lua_State,
    mut res: StkId,
    mut nres: libc::c_int,
    mut wanted: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut firstresult: StkId = 0 as *mut StackValue;
    let mut i: libc::c_int = 0;
    match wanted {
        0 => {
            (*L).top.p = res;
            return Ok(());
        }
        1 => {
            if nres == 0 as libc::c_int {
                (*res).val.tt_ = (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
            } else {
                let mut io1: *mut TValue = &mut (*res).val;
                let mut io2: *const TValue = &mut (*((*L).top.p).offset(-(nres as isize))).val;
                (*io1).value_ = (*io2).value_;
                (*io1).tt_ = (*io2).tt_;
            }
            (*L).top.p = res.offset(1 as libc::c_int as isize);
            return Ok(());
        }
        -1 => {
            wanted = nres;
        }
        _ => {
            if wanted < -(1 as libc::c_int) {
                (*(*L).ci).callstatus = ((*(*L).ci).callstatus as libc::c_int
                    | (1 as libc::c_int) << 9 as libc::c_int)
                    as libc::c_ushort;
                (*(*L).ci).u2.nres = nres;
                res = luaF_close(L, res, 1)?;
                (*(*L).ci).callstatus = ((*(*L).ci).callstatus as libc::c_int
                    & !((1 as libc::c_int) << 9 as libc::c_int))
                    as libc::c_ushort;
                if (*L).hookmask != 0 {
                    let mut savedres: isize =
                        (res as *mut libc::c_char).offset_from((*L).stack.p as *mut libc::c_char);
                    rethook(L, (*L).ci, nres)?;
                    res = ((*L).stack.p as *mut libc::c_char).offset(savedres as isize) as StkId;
                }
                wanted = -wanted - 3 as libc::c_int;
                if wanted == -(1 as libc::c_int) {
                    wanted = nres;
                }
            }
        }
    }
    firstresult = ((*L).top.p).offset(-(nres as isize));
    if nres > wanted {
        nres = wanted;
    }
    i = 0 as libc::c_int;
    while i < nres {
        let mut io1_0: *mut TValue = &mut (*res.offset(i as isize)).val;
        let mut io2_0: *const TValue = &mut (*firstresult.offset(i as isize)).val;
        (*io1_0).value_ = (*io2_0).value_;
        (*io1_0).tt_ = (*io2_0).tt_;
        i += 1;
    }
    while i < wanted {
        (*res.offset(i as isize)).val.tt_ =
            (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
        i += 1;
    }
    (*L).top.p = res.offset(wanted as isize);
    Ok(())
}

pub unsafe fn luaD_poscall(
    mut L: *mut lua_State,
    mut ci: *mut CallInfo,
    mut nres: c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut wanted: libc::c_int = (*ci).nresults as libc::c_int;
    if (((*L).hookmask != 0 && !(wanted < -(1 as libc::c_int))) as libc::c_int != 0 as libc::c_int)
        as libc::c_int as libc::c_long
        != 0
    {
        rethook(L, ci, nres)?;
    }
    moveresults(L, (*ci).func.p, nres, wanted)?;
    (*L).ci = (*ci).previous;
    Ok(())
}

#[inline]
unsafe extern "C" fn prepCallInfo(
    mut L: *mut lua_State,
    mut func: StkId,
    mut nret: libc::c_int,
    mut mask: libc::c_int,
    mut top: StkId,
) -> *mut CallInfo {
    (*L).ci = if !((*(*L).ci).next).is_null() {
        (*(*L).ci).next
    } else {
        luaE_extendCI(L)
    };
    let mut ci: *mut CallInfo = (*L).ci;
    (*ci).func.p = func;
    (*ci).nresults = nret as libc::c_short;
    (*ci).callstatus = mask as libc::c_ushort;
    (*ci).top.p = top;
    return ci;
}

unsafe fn precallC(
    mut L: *mut lua_State,
    mut func: StkId,
    mut nresults: libc::c_int,
    mut f: lua_CFunction,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut n: libc::c_int = 0;
    let mut ci: *mut CallInfo = 0 as *mut CallInfo;

    if ((((*L).stack_last.p).offset_from((*L).top.p) as libc::c_long
        <= 20 as libc::c_int as libc::c_long) as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        let mut t__: isize =
            (func as *mut libc::c_char).offset_from((*L).stack.p as *mut libc::c_char);
        if (*(*L).l_G).GCdebt > 0 as libc::c_int as isize {
            luaC_step(L);
        }
        luaD_growstack(L, 20)?;
        func = ((*L).stack.p as *mut libc::c_char).offset(t__ as isize) as StkId;
    }

    ci = prepCallInfo(
        L,
        func,
        nresults,
        (1 as libc::c_int) << 1 as libc::c_int,
        ((*L).top.p).offset(20 as libc::c_int as isize),
    );

    (*L).ci = ci;

    if ((*L).hookmask & (1 as libc::c_int) << 0 as libc::c_int != 0 as libc::c_int) as libc::c_int
        as libc::c_long
        != 0
    {
        let mut narg: libc::c_int =
            ((*L).top.p).offset_from(func) as libc::c_long as libc::c_int - 1 as libc::c_int;
        luaD_hook(
            L,
            0 as libc::c_int,
            -(1 as libc::c_int),
            1 as libc::c_int,
            narg,
        )?;
    }

    n = f(L)?;
    luaD_poscall(L, ci, n)?;

    Ok(n)
}

pub unsafe fn luaD_pretailcall(
    mut L: *mut lua_State,
    mut ci: *mut CallInfo,
    mut func: StkId,
    mut narg1: libc::c_int,
    mut delta: libc::c_int,
) -> Result<c_int, Box<dyn std::error::Error>> {
    loop {
        match (*func).val.tt_ as libc::c_int & 0x3f as libc::c_int {
            38 => {
                return precallC(
                    L,
                    func,
                    -(1 as libc::c_int),
                    (*((*func).val.value_.gc as *mut GCUnion)).cl.c.f,
                );
            }
            22 => return precallC(L, func, -(1 as libc::c_int), (*func).val.value_.f),
            6 => {
                let mut p: *mut Proto = (*((*func).val.value_.gc as *mut GCUnion)).cl.l.p;
                let mut fsize: libc::c_int = (*p).maxstacksize as libc::c_int;
                let mut nfixparams: libc::c_int = (*p).numparams as libc::c_int;
                let mut i: libc::c_int = 0;
                if ((((*L).stack_last.p).offset_from((*L).top.p) as libc::c_long
                    <= (fsize - delta) as libc::c_long) as libc::c_int
                    != 0 as libc::c_int) as libc::c_int as libc::c_long
                    != 0
                {
                    let mut t__: isize =
                        (func as *mut libc::c_char).offset_from((*L).stack.p as *mut libc::c_char);
                    if (*(*L).l_G).GCdebt > 0 as libc::c_int as isize {
                        luaC_step(L);
                    }
                    luaD_growstack(L, fsize - delta)?;
                    func = ((*L).stack.p as *mut libc::c_char).offset(t__ as isize) as StkId;
                }
                (*ci).func.p = ((*ci).func.p).offset(-(delta as isize));
                i = 0 as libc::c_int;
                while i < narg1 {
                    let mut io1: *mut TValue = &mut (*((*ci).func.p).offset(i as isize)).val;
                    let mut io2: *const TValue = &mut (*func.offset(i as isize)).val;
                    (*io1).value_ = (*io2).value_;
                    (*io1).tt_ = (*io2).tt_;
                    i += 1;
                }
                func = (*ci).func.p;
                while narg1 <= nfixparams {
                    (*func.offset(narg1 as isize)).val.tt_ =
                        (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                    narg1 += 1;
                }
                (*ci).top.p = func
                    .offset(1 as libc::c_int as isize)
                    .offset(fsize as isize);
                (*ci).u.savedpc = (*p).code;
                (*ci).callstatus = ((*ci).callstatus as libc::c_int
                    | (1 as libc::c_int) << 5 as libc::c_int)
                    as libc::c_ushort;
                (*L).top.p = func.offset(narg1 as isize);
                return Ok(-(1 as libc::c_int));
            }
            _ => {
                func = tryfuncTM(L, func)?;
                narg1 += 1;
            }
        }
    }
}

pub unsafe fn luaD_precall(
    mut L: *mut lua_State,
    mut func: StkId,
    mut nresults: libc::c_int,
) -> Result<*mut CallInfo, Box<dyn std::error::Error>> {
    loop {
        match (*func).val.tt_ as libc::c_int & 0x3f as libc::c_int {
            38 => {
                precallC(
                    L,
                    func,
                    nresults,
                    (*((*func).val.value_.gc as *mut GCUnion)).cl.c.f,
                )?;
                return Ok(0 as *mut CallInfo);
            }
            22 => {
                precallC(L, func, nresults, (*func).val.value_.f)?;
                return Ok(0 as *mut CallInfo);
            }
            6 => {
                let mut ci: *mut CallInfo = 0 as *mut CallInfo;
                let mut p: *mut Proto = (*((*func).val.value_.gc as *mut GCUnion)).cl.l.p;
                let mut narg: libc::c_int = ((*L).top.p).offset_from(func) as libc::c_long
                    as libc::c_int
                    - 1 as libc::c_int;
                let mut nfixparams: libc::c_int = (*p).numparams as libc::c_int;
                let mut fsize: libc::c_int = (*p).maxstacksize as libc::c_int;
                if ((((*L).stack_last.p).offset_from((*L).top.p) as libc::c_long
                    <= fsize as libc::c_long) as libc::c_int
                    != 0 as libc::c_int) as libc::c_int as libc::c_long
                    != 0
                {
                    let mut t__: isize =
                        (func as *mut libc::c_char).offset_from((*L).stack.p as *mut libc::c_char);
                    if (*(*L).l_G).GCdebt > 0 as libc::c_int as isize {
                        luaC_step(L);
                    }
                    luaD_growstack(L, fsize)?;
                    func = ((*L).stack.p as *mut libc::c_char).offset(t__ as isize) as StkId;
                }
                ci = prepCallInfo(
                    L,
                    func,
                    nresults,
                    0 as libc::c_int,
                    func.offset(1 as libc::c_int as isize)
                        .offset(fsize as isize),
                );
                (*L).ci = ci;
                (*ci).u.savedpc = (*p).code;
                while narg < nfixparams {
                    let fresh2 = (*L).top.p;
                    (*L).top.p = ((*L).top.p).offset(1);
                    (*fresh2).val.tt_ =
                        (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                    narg += 1;
                }
                return Ok(ci);
            }
            _ => func = tryfuncTM(L, func)?,
        }
    }
}

unsafe fn ccall(
    mut L: *mut lua_State,
    mut func: StkId,
    mut nResults: libc::c_int,
    mut inc: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut ci: *mut CallInfo = 0 as *mut CallInfo;
    (*L).nCcalls = ((*L).nCcalls).wrapping_add(inc);
    if (((*L).nCcalls & 0xffff as libc::c_int as u32 >= 200 as libc::c_int as u32) as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        if ((((*L).stack_last.p).offset_from((*L).top.p) as libc::c_long
            <= 0 as libc::c_int as libc::c_long) as libc::c_int
            != 0 as libc::c_int) as libc::c_int as libc::c_long
            != 0
        {
            let mut t__: isize =
                (func as *mut libc::c_char).offset_from((*L).stack.p as *mut libc::c_char);
            luaD_growstack(L, 0)?;
            func = ((*L).stack.p as *mut libc::c_char).offset(t__ as isize) as StkId;
        }
        luaE_checkcstack(L)?;
    }
    ci = luaD_precall(L, func, nResults)?;
    if !ci.is_null() {
        (*ci).callstatus = ((1 as libc::c_int) << 2 as libc::c_int) as libc::c_ushort;
        luaV_execute(L, ci)?;
    }
    (*L).nCcalls = ((*L).nCcalls).wrapping_sub(inc);
    Ok(())
}

pub unsafe fn luaD_call(
    L: *mut lua_State,
    func: StkId,
    nResults: c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    ccall(L, func, nResults, 1 as libc::c_int as u32)
}

pub unsafe fn luaD_callnoyield(
    L: *mut lua_State,
    func: StkId,
    nResults: c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    ccall(
        L,
        func,
        nResults,
        (0x10000 as libc::c_int | 1 as libc::c_int) as u32,
    )
}

unsafe fn finishCcall(
    mut L: *mut lua_State,
    mut ci: *mut CallInfo,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut n: libc::c_int = 0;

    if (*ci).callstatus as libc::c_int & (1 as libc::c_int) << 9 as libc::c_int != 0 {
        n = (*ci).u2.nres;
    } else {
        unreachable!("attempt to run coroutine continuation");
    }

    luaD_poscall(L, ci, n)
}

unsafe fn unroll(L: *mut lua_State, ud: *mut c_void) -> Result<(), Box<dyn std::error::Error>> {
    let mut ci: *mut CallInfo = 0 as *mut CallInfo;

    loop {
        ci = (*L).ci;
        if !(ci != &mut (*L).base_ci as *mut CallInfo) {
            break Ok(());
        }
        if (*ci).callstatus as libc::c_int & (1 as libc::c_int) << 1 as libc::c_int != 0 {
            finishCcall(L, ci)?;
        } else {
            luaV_finishOp(L)?;
            luaV_execute(L, ci)?;
        }
    }
}

unsafe fn findpcall(mut L: *mut lua_State) -> *mut CallInfo {
    let mut ci: *mut CallInfo = 0 as *mut CallInfo;
    ci = (*L).ci;
    while !ci.is_null() {
        if (*ci).callstatus as libc::c_int & (1 as libc::c_int) << 4 as libc::c_int != 0 {
            return ci;
        }
        ci = (*ci).previous;
    }
    return 0 as *mut CallInfo;
}

unsafe fn closepaux(L: *mut lua_State, ud: *mut c_void) -> Result<(), Box<dyn std::error::Error>> {
    let pcl = ud as *mut CloseP;
    luaF_close(L, (*pcl).level, 0)?;
    Ok(())
}

pub unsafe fn luaD_closeprotected(
    L: *mut lua_State,
    level: isize,
    mut status: Result<(), Box<dyn std::error::Error>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let old_ci: *mut CallInfo = (*L).ci;
    let old_allowhooks: u8 = (*L).allowhook;

    loop {
        let mut pcl = CloseP {
            level: ((*L).stack.p as *mut libc::c_char).offset(level) as StkId,
            status,
        };

        status = luaD_rawrunprotected(L, closepaux, &raw mut pcl as *mut libc::c_void);

        if status.is_ok() {
            return pcl.status;
        } else {
            (*L).ci = old_ci;
            (*L).allowhook = old_allowhooks;
        }
    }
}

pub unsafe fn luaD_pcall(
    mut L: *mut lua_State,
    mut func: Pfunc,
    mut u: *mut libc::c_void,
    old_top: isize,
) -> Result<(), Box<dyn std::error::Error>> {
    let old_ci = (*L).ci;
    let old_allowhooks: u8 = (*L).allowhook;
    let mut status = luaD_rawrunprotected(L, func, u);

    if status.is_err() {
        (*L).ci = old_ci;
        (*L).allowhook = old_allowhooks;
        status = luaD_closeprotected(L, old_top, status);
        (*L).top.p = ((*L).stack.p as *mut libc::c_char).offset(old_top) as StkId;
        luaD_shrinkstack(L);
    }

    status
}

unsafe fn checkmode(
    mut L: *mut lua_State,
    mode: *const c_char,
    x: *const c_char,
) -> Result<(), Box<dyn std::error::Error>> {
    if !mode.is_null() && (strchr(mode, *x.offset(0) as libc::c_int)).is_null() {
        return Err(format!(
            "attempt to load a {} chunk (mode is '{}')",
            CStr::from_ptr(x).to_string_lossy(),
            CStr::from_ptr(mode).to_string_lossy()
        )
        .into());
    }

    Ok(())
}

unsafe fn f_parser(
    mut L: *mut lua_State,
    ud: *mut c_void,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut cl: *mut LClosure = 0 as *mut LClosure;
    let p = &mut *(ud as *mut SParser);
    let fresh3 = (*p.z).n;
    (*p.z).n = ((*p.z).n).wrapping_sub(1);
    let mut c: libc::c_int = if fresh3 > 0 as libc::c_int as usize {
        let fresh4 = (*p.z).p;
        (*p.z).p = ((*p.z).p).offset(1);
        *fresh4 as libc::c_uchar as libc::c_int
    } else {
        luaZ_fill(p.z)?
    };

    if c == (*::core::mem::transmute::<&[u8; 5], &[libc::c_char; 5]>(b"\x1BLua\0"))
        [0 as libc::c_int as usize] as libc::c_int
    {
        checkmode(
            L,
            (*p).mode,
            b"binary\0" as *const u8 as *const libc::c_char,
        )?;
        cl = luaU_undump(L, (*p).z, (*p).name)?;
    } else {
        checkmode(L, (*p).mode, b"text\0" as *const u8 as *const libc::c_char)?;
        cl = luaY_parser(L, (*p).z, &mut (*p).buff, &mut (*p).dyd, (*p).name, c)?;
    }

    luaF_initupvals(L, cl);
    Ok(())
}

pub unsafe fn luaD_protectedparser(
    mut L: *mut lua_State,
    mut z: *mut ZIO,
    mut name: *const libc::c_char,
    mut mode: *const libc::c_char,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut p = SParser {
        z: 0 as *mut ZIO,
        buff: Mbuffer {
            buffer: 0 as *mut libc::c_char,
            n: 0,
            buffsize: 0,
        },
        dyd: Dyndata {
            actvar: C2RustUnnamed_9 {
                arr: 0 as *mut Vardesc,
                n: 0,
                size: 0,
            },
            gt: Labellist {
                arr: 0 as *mut Labeldesc,
                n: 0,
                size: 0,
            },
            label: Labellist {
                arr: 0 as *mut Labeldesc,
                n: 0,
                size: 0,
            },
        },
        mode: 0 as *const libc::c_char,
        name: 0 as *const libc::c_char,
    };

    p.z = z;
    p.name = name;
    p.mode = mode;
    p.dyd.actvar.arr = 0 as *mut Vardesc;
    p.dyd.actvar.size = 0 as libc::c_int;
    p.dyd.gt.arr = 0 as *mut Labeldesc;
    p.dyd.gt.size = 0 as libc::c_int;
    p.dyd.label.arr = 0 as *mut Labeldesc;
    p.dyd.label.size = 0 as libc::c_int;
    p.buff.buffer = 0 as *mut libc::c_char;
    p.buff.buffsize = 0 as libc::c_int as usize;

    (*L).nCcalls = ((*L).nCcalls).wrapping_add(0x10000 as libc::c_int as u32);

    // Parse.
    let status = luaD_pcall(
        L,
        f_parser,
        &mut p as *mut SParser as *mut libc::c_void,
        ((*L).top.p as *mut libc::c_char).offset_from((*L).stack.p as *mut libc::c_char),
    );

    p.buff.buffer = luaM_saferealloc_(
        L,
        p.buff.buffer as *mut libc::c_void,
        (p.buff.buffsize).wrapping_mul(::core::mem::size_of::<libc::c_char>()),
        0usize.wrapping_mul(::core::mem::size_of::<libc::c_char>()),
    ) as *mut libc::c_char;
    p.buff.buffsize = 0 as libc::c_int as usize;
    luaM_free_(
        L,
        p.dyd.actvar.arr as *mut libc::c_void,
        (p.dyd.actvar.size as usize).wrapping_mul(::core::mem::size_of::<Vardesc>()),
    );
    luaM_free_(
        L,
        p.dyd.gt.arr as *mut libc::c_void,
        (p.dyd.gt.size as usize).wrapping_mul(::core::mem::size_of::<Labeldesc>()),
    );
    luaM_free_(
        L,
        p.dyd.label.arr as *mut libc::c_void,
        (p.dyd.label.size as usize).wrapping_mul(::core::mem::size_of::<Labeldesc>()),
    );
    (*L).nCcalls = ((*L).nCcalls).wrapping_sub(0x10000 as libc::c_int as u32);

    status
}
