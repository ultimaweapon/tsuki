#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments
)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::ldebug::{luaG_callerror, luaG_runerror};
use crate::lfunc::{luaF_close, luaF_initupvals};
use crate::lmem::{luaM_free_, luaM_saferealloc_};
use crate::lobject::{CClosure, Proto, StackValue, StkId, TValue, UpVal};
use crate::lparser::{C2RustUnnamed_9, Dyndata, Labeldesc, Labellist, Vardesc, luaY_parser};
use crate::lstate::{CallInfo, lua_CFunction, lua_Debug, lua_Hook, luaE_extendCI, luaE_shrinkCI};
use crate::ltm::{TM_CALL, luaT_gettmbyobj};
use crate::lvm::luaV_execute;
use crate::lzio::{Mbuffer, ZIO, Zio};
use crate::{ChunkInfo, Lua, LuaClosure, ParseError, Ref, Thread};
use std::alloc::{Layout, handle_alloc_error};
use std::ffi::c_int;
use std::ops::Deref;
use std::pin::Pin;
use std::rc::Rc;

#[repr(C)]
struct SParser {
    z: *mut ZIO,
    buff: Mbuffer,
    dyd: Dyndata,
}

#[repr(C)]
pub struct CloseP {
    pub level: StkId,
    pub status: Result<(), Box<dyn std::error::Error>>,
}

unsafe fn relstack(L: *const Thread) {
    let mut ci: *mut CallInfo = 0 as *mut CallInfo;
    let mut up: *mut UpVal = 0 as *mut UpVal;

    (*L).top
        .set(((*L).top.get()).byte_offset_from_unsigned((*L).stack.get()) as _);
    (*L).tbclist
        .set(((*L).tbclist.get()).byte_offset_from_unsigned((*L).stack.get()) as _);

    up = (*L).openupval.get();

    while !up.is_null() {
        (*up)
            .v
            .set(((*up).v.get()).byte_offset_from_unsigned((*L).stack.get()) as _);
        up = (*(*up).u.get()).open.next;
    }

    ci = (*L).ci.get();

    while !ci.is_null() {
        (*ci).top = ((*ci).top).byte_offset_from_unsigned((*L).stack.get()) as _;
        (*ci).func = ((*ci).func).byte_offset_from_unsigned((*L).stack.get()) as _;
        ci = (*ci).previous;
    }
}

unsafe fn correctstack(L: *const Thread) {
    let mut ci: *mut CallInfo = 0 as *mut CallInfo;
    let mut up: *mut UpVal = 0 as *mut UpVal;

    (*L).top
        .set(((*L).stack.get()).byte_add((*L).top.get() as usize) as StkId);
    (*L).tbclist
        .set(((*L).stack.get()).byte_add((*L).tbclist.get() as usize) as StkId);

    up = (*L).openupval.get();

    while !up.is_null() {
        (*up)
            .v
            .set(&raw mut (*(((*L).stack.get()).byte_add((*up).v.get() as usize) as StkId)).val);
        up = (*(*up).u.get()).open.next;
    }

    ci = (*L).ci.get();

    while !ci.is_null() {
        (*ci).top = ((*L).stack.get()).byte_add((*ci).top as usize) as StkId;
        (*ci).func = ((*L).stack.get()).byte_add((*ci).func as usize) as StkId;

        if (*ci).callstatus & 1 << 1 == 0 {
            (*ci).u.trap = 1;
        }

        ci = (*ci).previous;
    }
}

pub unsafe fn luaD_reallocstack(th: *const Thread, newsize: usize) {
    let lua = (*th).global;
    let oldsize = ((*th).stack_last.get()).offset_from_unsigned((*th).stack.get());
    let oldgcstop: libc::c_int = (*lua).gcstopem.get() as libc::c_int;

    relstack(th);
    (*lua).gcstopem.set(1 as libc::c_int as u8);

    // Re-allocate the stack.
    let newstack = std::alloc::realloc(
        (*th).stack.get().cast(),
        Layout::array::<StackValue>(oldsize + 5).unwrap(),
        (newsize + 5) * size_of::<StackValue>(),
    ) as *mut StackValue;

    if newstack.is_null() {
        handle_alloc_error(Layout::array::<StackValue>(newsize + 5).unwrap());
    }

    (*th).stack.set(newstack);
    correctstack(th);
    (*th).stack_last.set(((*th).stack.get()).add(newsize));

    (*lua).gcstopem.set(oldgcstop as u8);

    // Fill the new space with nil.
    let mut i = oldsize + 5;

    while i < newsize + 5 {
        (*newstack.add(i)).val.tt_ = 0 | 0 << 4;
        i += 1;
    }
}

#[inline(never)]
pub unsafe fn luaD_growstack(L: *const Thread, n: usize) -> Result<(), Box<dyn std::error::Error>> {
    let size = ((*L).stack_last.get()).offset_from_unsigned((*L).stack.get());

    if size > 1000000 {
        return luaG_runerror(L, "stack overflow");
    } else if n < 1000000 {
        let mut newsize = 2 * size;
        let needed = ((*L).top.get()).offset_from_unsigned((*L).stack.get()) + n;

        if newsize > 1000000 {
            newsize = 1000000;
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

unsafe fn stackinuse(L: *const Thread) -> usize {
    let mut res = 0;
    let mut lim: StkId = (*L).top.get();
    let mut ci = (*L).ci.get();

    while !ci.is_null() {
        if lim < (*ci).top {
            lim = (*ci).top;
        }
        ci = (*ci).previous;
    }

    res = lim.offset_from_unsigned((*L).stack.get()) + 1;

    if res < 20 {
        res = 20;
    }

    return res;
}

pub unsafe fn luaD_shrinkstack(L: *const Thread) {
    let inuse = stackinuse(L);
    let max = if inuse > 1000000 / 3 {
        1000000
    } else {
        inuse * 3
    };

    if inuse <= 1000000 && ((*L).stack_last.get()).offset_from_unsigned((*L).stack.get()) > max {
        let nsize = if inuse > 1000000 / 2 {
            1000000
        } else {
            inuse * 2
        };

        luaD_reallocstack(L, nsize);
    }

    luaE_shrinkCI(L);
}

#[inline(always)]
pub unsafe fn luaD_inctop(L: *const Thread) -> Result<(), Box<dyn std::error::Error>> {
    if (*L).stack_last.get().offset_from((*L).top.get()) <= 1 {
        luaD_growstack(L, 1)?;
    }

    (*L).top.add(1);

    Ok(())
}

pub unsafe fn luaD_hook(
    L: *const Thread,
    event: libc::c_int,
    line: libc::c_int,
    ftransfer: libc::c_int,
    ntransfer: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let hook: lua_Hook = (*L).hook.get();
    if hook.is_some() && (*L).allowhook.get() != 0 {
        let mut mask: libc::c_int = (1 as libc::c_int) << 3 as libc::c_int;
        let ci: *mut CallInfo = (*L).ci.get();
        let top: isize = ((*L).top.get() as *mut libc::c_char)
            .offset_from((*L).stack.get() as *mut libc::c_char);
        let ci_top: isize =
            ((*ci).top as *mut libc::c_char).offset_from((*L).stack.get() as *mut libc::c_char);
        let mut ar: lua_Debug = lua_Debug {
            event: 0,
            name: 0 as *const libc::c_char,
            namewhat: 0 as *const libc::c_char,
            what: 0 as *const libc::c_char,
            source: ChunkInfo::default(),
            currentline: 0,
            linedefined: 0,
            lastlinedefined: 0,
            nups: 0,
            nparams: 0,
            isvararg: 0,
            istailcall: 0,
            ftransfer: 0,
            ntransfer: 0,
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
            && (*L).top.get() < (*ci).top
        {
            (*L).top.set((*ci).top);
        }
        if ((((*L).stack_last.get()).offset_from((*L).top.get()) as libc::c_long
            <= 20 as libc::c_int as libc::c_long) as libc::c_int
            != 0 as libc::c_int) as libc::c_int as libc::c_long
            != 0
        {
            luaD_growstack(L, 20)?;
        }
        if (*ci).top < ((*L).top.get()).offset(20 as libc::c_int as isize) {
            (*ci).top = ((*L).top.get()).offset(20 as libc::c_int as isize);
        }
        (*L).allowhook.set(0);
        (*ci).callstatus = ((*ci).callstatus as libc::c_int | mask) as libc::c_ushort;
        (Some(hook.expect("non-null function pointer"))).expect("non-null function pointer")(
            L, &mut ar,
        );
        (*L).allowhook.set(1);
        (*ci).top = ((*L).stack.get() as *mut libc::c_char).offset(ci_top as isize) as StkId;
        (*L).top
            .set(((*L).stack.get() as *mut libc::c_char).offset(top as isize) as StkId);
        (*ci).callstatus = ((*ci).callstatus as libc::c_int & !mask) as libc::c_ushort;
    }
    Ok(())
}

pub unsafe fn luaD_hookcall(
    L: *const Thread,
    ci: *mut CallInfo,
) -> Result<(), Box<dyn std::error::Error>> {
    (*L).oldpc.set(0);

    if (*L).hookmask.get() & 1 << 0 != 0 {
        let event: libc::c_int =
            if (*ci).callstatus as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0 {
                4 as libc::c_int
            } else {
                0 as libc::c_int
            };
        let p: *mut Proto = (*(*(*ci).func).val.value_.gc.cast::<LuaClosure>()).p.get();
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
    L: *const Thread,
    mut ci: *mut CallInfo,
    nres: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    if (*L).hookmask.get() & 1 << 1 != 0 {
        let firstres: StkId = ((*L).top.get()).offset(-(nres as isize));
        let mut delta: libc::c_int = 0 as libc::c_int;
        let mut ftransfer: libc::c_int = 0;
        if (*ci).callstatus as libc::c_int & (1 as libc::c_int) << 1 as libc::c_int == 0 {
            let p: *mut Proto = (*(*(*ci).func).val.value_.gc.cast::<LuaClosure>()).p.get();
            if (*p).is_vararg != 0 {
                delta = (*ci).u.nextraargs + (*p).numparams as libc::c_int + 1 as libc::c_int;
            }
        }
        (*ci).func = ((*ci).func).offset(delta as isize);
        ftransfer =
            firstres.offset_from((*ci).func) as libc::c_long as libc::c_ushort as libc::c_int;
        luaD_hook(L, 1 as libc::c_int, -(1 as libc::c_int), ftransfer, nres)?;
        (*ci).func = ((*ci).func).offset(-(delta as isize));
    }
    ci = (*ci).previous;

    if (*ci).callstatus as libc::c_int & (1 as libc::c_int) << 1 as libc::c_int == 0 {
        (*L).oldpc.set(
            ((*ci).u.savedpc)
                .offset_from((*(*(*(*ci).func).val.value_.gc.cast::<LuaClosure>()).p.get()).code)
                as libc::c_long as libc::c_int
                - 1,
        );
    }

    Ok(())
}

unsafe fn tryfuncTM(
    L: *const Thread,
    mut func: StkId,
) -> Result<StkId, Box<dyn std::error::Error>> {
    let mut tm: *const TValue = 0 as *const TValue;
    let mut p: StkId = 0 as *mut StackValue;
    if ((((*L).stack_last.get()).offset_from((*L).top.get()) as libc::c_long
        <= 1 as libc::c_int as libc::c_long) as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        let t__: isize =
            (func as *mut libc::c_char).offset_from((*L).stack.get() as *mut libc::c_char);
        if (*(*L).global).gc.debt() > 0 as libc::c_int as isize {
            crate::gc::step((*L).global);
        }
        luaD_growstack(L, 1)?;
        func = ((*L).stack.get() as *mut libc::c_char).offset(t__ as isize) as StkId;
    }
    tm = luaT_gettmbyobj(L, &mut (*func).val, TM_CALL);
    if (((*tm).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int) as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        luaG_callerror(L, &mut (*func).val)?;
    }
    p = (*L).top.get();
    while p > func {
        let io1: *mut TValue = &mut (*p).val;
        let io2: *const TValue = &mut (*p.offset(-(1 as libc::c_int as isize))).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        p = p.offset(-1);
    }

    (*L).top.add(1);

    let io1_0: *mut TValue = &mut (*func).val;
    let io2_0: *const TValue = tm;
    (*io1_0).value_ = (*io2_0).value_;
    (*io1_0).tt_ = (*io2_0).tt_;
    return Ok(func);
}

unsafe fn moveresults(
    L: *const Thread,
    mut res: StkId,
    mut nres: libc::c_int,
    mut wanted: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut firstresult: StkId = 0 as *mut StackValue;
    let mut i: libc::c_int = 0;
    match wanted {
        0 => {
            (*L).top.set(res);
            return Ok(());
        }
        1 => {
            if nres == 0 as libc::c_int {
                (*res).val.tt_ = (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
            } else {
                let io1: *mut TValue = &raw mut (*res).val;
                let io2: *const TValue = &raw mut (*((*L).top.get()).offset(-(nres as isize))).val;
                (*io1).value_ = (*io2).value_;
                (*io1).tt_ = (*io2).tt_;
            }
            (*L).top.set(res.offset(1 as libc::c_int as isize));
            return Ok(());
        }
        -1 => {
            wanted = nres;
        }
        _ => {
            if wanted < -(1 as libc::c_int) {
                (*(*L).ci.get()).callstatus = ((*(*L).ci.get()).callstatus as libc::c_int
                    | (1 as libc::c_int) << 9 as libc::c_int)
                    as libc::c_ushort;
                (*(*L).ci.get()).u2.nres = nres;
                res = luaF_close(L, res)?;
                (*(*L).ci.get()).callstatus = ((*(*L).ci.get()).callstatus as libc::c_int
                    & !((1 as libc::c_int) << 9 as libc::c_int))
                    as libc::c_ushort;
                if (*L).hookmask.get() != 0 {
                    let savedres: isize = (res as *mut libc::c_char)
                        .offset_from((*L).stack.get() as *mut libc::c_char);
                    rethook(L, (*L).ci.get(), nres)?;
                    res =
                        ((*L).stack.get() as *mut libc::c_char).offset(savedres as isize) as StkId;
                }
                wanted = -wanted - 3 as libc::c_int;
                if wanted == -(1 as libc::c_int) {
                    wanted = nres;
                }
            }
        }
    }
    firstresult = ((*L).top.get()).offset(-(nres as isize));
    if nres > wanted {
        nres = wanted;
    }
    i = 0 as libc::c_int;
    while i < nres {
        let io1_0: *mut TValue = &mut (*res.offset(i as isize)).val;
        let io2_0: *const TValue = &mut (*firstresult.offset(i as isize)).val;
        (*io1_0).value_ = (*io2_0).value_;
        (*io1_0).tt_ = (*io2_0).tt_;
        i += 1;
    }
    while i < wanted {
        (*res.offset(i as isize)).val.tt_ =
            (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
        i += 1;
    }
    (*L).top.set(res.offset(wanted as isize));
    Ok(())
}

pub unsafe fn luaD_poscall(
    L: *const Thread,
    ci: *mut CallInfo,
    nres: c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let wanted: libc::c_int = (*ci).nresults as libc::c_int;
    if (((*L).hookmask.get() != 0 && !(wanted < -(1 as libc::c_int))) as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        rethook(L, ci, nres)?;
    }
    moveresults(L, (*ci).func, nres, wanted)?;
    (*L).ci.set((*ci).previous);
    Ok(())
}

unsafe fn prepCallInfo(
    L: *const Thread,
    func: StkId,
    nret: libc::c_int,
    mask: libc::c_int,
    top: StkId,
) -> *mut CallInfo {
    (*L).ci.set(if !((*(*L).ci.get()).next).is_null() {
        (*(*L).ci.get()).next
    } else {
        luaE_extendCI(L)
    });
    let ci: *mut CallInfo = (*L).ci.get();
    (*ci).func = func;
    (*ci).nresults = nret as libc::c_short;
    (*ci).callstatus = mask as libc::c_ushort;
    (*ci).top = top;
    return ci;
}

unsafe fn precallC(
    L: *const Thread,
    mut func: StkId,
    nresults: libc::c_int,
    f: lua_CFunction,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut n: libc::c_int = 0;
    let mut ci: *mut CallInfo = 0 as *mut CallInfo;

    if ((((*L).stack_last.get()).offset_from((*L).top.get()) as libc::c_long
        <= 20 as libc::c_int as libc::c_long) as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        let t__: isize =
            (func as *mut libc::c_char).offset_from((*L).stack.get() as *mut libc::c_char);
        if (*(*L).global).gc.debt() > 0 as libc::c_int as isize {
            crate::gc::step((*L).global);
        }
        luaD_growstack(L, 20)?;
        func = ((*L).stack.get() as *mut libc::c_char).offset(t__ as isize) as StkId;
    }

    ci = prepCallInfo(
        L,
        func,
        nresults,
        (1 as libc::c_int) << 1 as libc::c_int,
        ((*L).top.get()).offset(20 as libc::c_int as isize),
    );

    (*L).ci.set(ci);

    if ((*L).hookmask.get() & (1 as libc::c_int) << 0 != 0) as libc::c_int as libc::c_long != 0 {
        let narg: libc::c_int =
            ((*L).top.get()).offset_from(func) as libc::c_long as libc::c_int - 1 as libc::c_int;
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
    L: *const Thread,
    ci: *mut CallInfo,
    mut func: StkId,
    mut narg1: libc::c_int,
    delta: libc::c_int,
) -> Result<c_int, Box<dyn std::error::Error>> {
    loop {
        match (*func).val.tt_ as libc::c_int & 0x3f as libc::c_int {
            38 => {
                return precallC(
                    L,
                    func,
                    -(1 as libc::c_int),
                    (*((*func).val.value_.gc as *mut CClosure)).f,
                );
            }
            22 => return precallC(L, func, -(1 as libc::c_int), (*func).val.value_.f),
            6 => {
                let p: *mut Proto = (*(*func).val.value_.gc.cast::<LuaClosure>()).p.get();
                let fsize: libc::c_int = (*p).maxstacksize as libc::c_int;
                let nfixparams: libc::c_int = (*p).numparams as libc::c_int;
                let mut i: libc::c_int = 0;

                if ((((*L).stack_last.get()).offset_from((*L).top.get()) as libc::c_long
                    <= (fsize - delta) as libc::c_long) as libc::c_int
                    != 0 as libc::c_int) as libc::c_int as libc::c_long
                    != 0
                {
                    let t__: isize = (func as *mut libc::c_char)
                        .offset_from((*L).stack.get() as *mut libc::c_char);
                    if (*(*L).global).gc.debt() > 0 as libc::c_int as isize {
                        crate::gc::step((*L).global);
                    }
                    luaD_growstack(L, (fsize - delta).try_into().unwrap())?;
                    func = ((*L).stack.get() as *mut libc::c_char).offset(t__ as isize) as StkId;
                }

                (*ci).func = ((*ci).func).offset(-(delta as isize));
                i = 0 as libc::c_int;
                while i < narg1 {
                    let io1: *mut TValue = &raw mut (*((*ci).func).offset(i as isize)).val;
                    let io2: *const TValue = &raw mut (*func.offset(i as isize)).val;
                    (*io1).value_ = (*io2).value_;
                    (*io1).tt_ = (*io2).tt_;
                    i += 1;
                }
                func = (*ci).func;
                while narg1 <= nfixparams {
                    (*func.offset(narg1 as isize)).val.tt_ =
                        (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                    narg1 += 1;
                }
                (*ci).top = func
                    .offset(1 as libc::c_int as isize)
                    .offset(fsize as isize);
                (*ci).u.savedpc = (*p).code;
                (*ci).callstatus = ((*ci).callstatus as libc::c_int
                    | (1 as libc::c_int) << 5 as libc::c_int)
                    as libc::c_ushort;
                (*L).top.set(func.offset(narg1 as isize));
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
    L: *const Thread,
    mut func: StkId,
    nresults: libc::c_int,
) -> Result<*mut CallInfo, Box<dyn std::error::Error>> {
    loop {
        match (*func).val.tt_ as libc::c_int & 0x3f as libc::c_int {
            38 => {
                precallC(
                    L,
                    func,
                    nresults,
                    (*((*func).val.value_.gc as *mut CClosure)).f,
                )?;
                return Ok(0 as *mut CallInfo);
            }
            22 => {
                precallC(L, func, nresults, (*func).val.value_.f)?;
                return Ok(0 as *mut CallInfo);
            }
            6 => {
                let mut ci: *mut CallInfo = 0 as *mut CallInfo;
                let p: *mut Proto = (*(*func).val.value_.gc.cast::<LuaClosure>()).p.get();
                let mut narg: libc::c_int = ((*L).top.get()).offset_from(func) as libc::c_long
                    as libc::c_int
                    - 1 as libc::c_int;
                let nfixparams: libc::c_int = (*p).numparams as libc::c_int;
                let fsize = usize::from((*p).maxstacksize);

                if ((*L).stack_last.get()).offset_from_unsigned((*L).top.get()) <= fsize {
                    let t__: isize = (func as *mut libc::c_char)
                        .offset_from((*L).stack.get() as *mut libc::c_char);
                    if (*(*L).global).gc.debt() > 0 as libc::c_int as isize {
                        crate::gc::step((*L).global);
                    }
                    luaD_growstack(L, fsize)?;
                    func = ((*L).stack.get() as *mut libc::c_char).offset(t__ as isize) as StkId;
                }

                ci = prepCallInfo(
                    L,
                    func,
                    nresults,
                    0 as libc::c_int,
                    func.offset(1 as libc::c_int as isize)
                        .offset(fsize as isize),
                );
                (*L).ci.set(ci);
                (*ci).u.savedpc = (*p).code;
                while narg < nfixparams {
                    let fresh2 = (*L).top.get();
                    (*L).top.add(1);
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

pub unsafe fn luaD_call(
    L: *const Thread,
    func: StkId,
    nResults: c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let ci = luaD_precall(L, func, nResults)?;

    if !ci.is_null() {
        (*ci).callstatus = ((1 as libc::c_int) << 2 as libc::c_int) as libc::c_ushort;
        luaV_execute(L, ci)?;
    }

    Ok(())
}

pub unsafe fn luaD_closeprotected(
    L: *const Thread,
    level: usize,
    mut status: Result<(), Box<dyn std::error::Error>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let old_ci: *mut CallInfo = (*L).ci.get();
    let old_allowhooks: u8 = (*L).allowhook.get();

    loop {
        let pcl = CloseP {
            level: (*L).stack.get().byte_add(level),
            status,
        };

        status = luaF_close(L, pcl.level).map(|_| ());

        if status.is_ok() {
            return pcl.status;
        } else {
            (*L).ci.set(old_ci);
            (*L).allowhook.set(old_allowhooks);
        }
    }
}

pub unsafe fn luaD_pcall<F>(
    L: *const Thread,
    old_top: usize,
    f: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnOnce(*const Thread) -> Result<(), Box<dyn std::error::Error>>,
{
    let old_ci = (*L).ci.get();
    let old_allowhooks: u8 = (*L).allowhook.get();
    let mut status = f(L);

    if status.is_err() {
        (*L).ci.set(old_ci);
        (*L).allowhook.set(old_allowhooks);
        status = luaD_closeprotected(L, old_top, status);
        (*L).top.set((*L).stack.get().byte_add(old_top));
        luaD_shrinkstack(L);
    }

    status
}

pub unsafe fn luaD_protectedparser(
    g: &Pin<Rc<Lua>>,
    mut z: Zio,
    info: ChunkInfo,
) -> Result<Ref<LuaClosure>, ParseError> {
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
    };

    p.z = &mut z;
    p.dyd.actvar.arr = 0 as *mut Vardesc;
    p.dyd.actvar.size = 0 as libc::c_int;
    p.dyd.gt.arr = 0 as *mut Labeldesc;
    p.dyd.gt.size = 0 as libc::c_int;
    p.dyd.label.arr = 0 as *mut Labeldesc;
    p.dyd.label.size = 0 as libc::c_int;
    p.buff.buffer = 0 as *mut libc::c_char;
    p.buff.buffsize = 0 as libc::c_int as usize;

    // Parse.
    let fresh3 = (*p.z).n;
    (*p.z).n = ((*p.z).n).wrapping_sub(1);
    let c: libc::c_int = if fresh3 > 0 {
        let fresh4 = (*p.z).p;
        (*p.z).p = ((*p.z).p).offset(1);
        *fresh4 as libc::c_uchar as libc::c_int
    } else {
        -1
    };

    let status = luaY_parser(g, p.z, &raw mut p.buff, &raw mut p.dyd, info, c);

    if let Ok(cl) = &status {
        luaF_initupvals(g.deref(), cl.deref());
    }

    p.buff.buffer = luaM_saferealloc_(
        g.deref(),
        p.buff.buffer as *mut libc::c_void,
        (p.buff.buffsize).wrapping_mul(::core::mem::size_of::<libc::c_char>()),
        0usize.wrapping_mul(::core::mem::size_of::<libc::c_char>()),
    ) as *mut libc::c_char;
    p.buff.buffsize = 0 as libc::c_int as usize;
    luaM_free_(
        g.deref(),
        p.dyd.actvar.arr as *mut libc::c_void,
        (p.dyd.actvar.size as usize).wrapping_mul(::core::mem::size_of::<Vardesc>()),
    );
    luaM_free_(
        g.deref(),
        p.dyd.gt.arr as *mut libc::c_void,
        (p.dyd.gt.size as usize).wrapping_mul(::core::mem::size_of::<Labeldesc>()),
    );
    luaM_free_(
        g.deref(),
        p.dyd.label.arr as *mut libc::c_void,
        (p.dyd.label.size as usize).wrapping_mul(::core::mem::size_of::<Labeldesc>()),
    );

    status
}
