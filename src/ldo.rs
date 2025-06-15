#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments
)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::lapi::PcallError;
use crate::ldebug::luaG_callerror;
use crate::lfunc::{luaF_close, luaF_initupvals};
use crate::lmem::{luaM_free_, luaM_saferealloc_};
use crate::lobject::{CClosure, Proto, StackValue, StkId, UpVal};
use crate::lparser::{C2RustUnnamed_9, Dyndata, Labeldesc, Labellist, Vardesc, luaY_parser};
use crate::lstate::{CallInfo, lua_Debug, lua_Hook, luaE_extendCI, luaE_shrinkCI};
use crate::ltm::{TM_CALL, luaT_gettmbyobj};
use crate::lvm::luaV_execute;
use crate::lzio::{Mbuffer, ZIO, Zio};
use crate::value::UnsafeValue;
use crate::{ChunkInfo, Context, Lua, LuaFn, ParseError, Ref, StackOverflow, Thread};
use alloc::alloc::handle_alloc_error;
use alloc::boxed::Box;
use alloc::rc::Rc;
use core::alloc::Layout;
use core::ops::Deref;
use core::pin::Pin;

type c_int = i32;

#[repr(C)]
struct SParser {
    z: *mut ZIO,
    buff: Mbuffer,
    dyd: Dyndata,
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
    let lua = (*th).hdr.global;
    let oldsize = ((*th).stack_last.get()).offset_from_unsigned((*th).stack.get());
    let oldgcstop: c_int = (*lua).gcstopem.get() as c_int;

    relstack(th);
    (*lua).gcstopem.set(1 as c_int as u8);

    // Re-allocate the stack.
    let newstack = alloc::alloc::realloc(
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
pub unsafe fn luaD_growstack(L: *const Thread, n: usize) -> Result<(), StackOverflow> {
    let size = ((*L).stack_last.get()).offset_from_unsigned((*L).stack.get());

    if size > 1000000 {
        return Err(StackOverflow);
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

    Err(StackOverflow)
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

pub unsafe fn luaD_hook(
    L: *const Thread,
    event: c_int,
    line: c_int,
    ftransfer: c_int,
    ntransfer: usize,
) -> Result<(), Box<dyn core::error::Error>> {
    let hook: lua_Hook = (*L).hook.get();
    if hook.is_some() && (*L).allowhook.get() != 0 {
        let mut mask: c_int = (1 as c_int) << 3 as c_int;
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
            i_ci: 0 as *mut CallInfo,
        };
        ar.event = event;
        ar.currentline = line;
        ar.i_ci = ci;

        if ntransfer != 0 {
            mask |= (1 as c_int) << 8 as c_int;
            (*ci).u2.transferinfo.ftransfer = ftransfer as libc::c_ushort;
            (*ci).u2.transferinfo.ntransfer = ntransfer;
        }

        if (*ci).callstatus as c_int & (1 as c_int) << 1 == 0 && (*L).top.get() < (*ci).top {
            (*L).top.set((*ci).top);
        }

        if ((((*L).stack_last.get()).offset_from((*L).top.get()) as libc::c_long
            <= 20 as c_int as libc::c_long) as c_int
            != 0 as c_int) as c_int as libc::c_long
            != 0
        {
            luaD_growstack(L, 20)?;
        }
        if (*ci).top < ((*L).top.get()).offset(20 as c_int as isize) {
            (*ci).top = ((*L).top.get()).offset(20 as c_int as isize);
        }
        (*L).allowhook.set(0);
        (*ci).callstatus = ((*ci).callstatus as c_int | mask) as libc::c_ushort;
        (Some(hook.expect("non-null function pointer"))).expect("non-null function pointer")(
            L, &mut ar,
        );
        (*L).allowhook.set(1);
        (*ci).top = ((*L).stack.get() as *mut libc::c_char).offset(ci_top as isize) as StkId;
        (*L).top
            .set(((*L).stack.get() as *mut libc::c_char).offset(top as isize) as StkId);
        (*ci).callstatus = ((*ci).callstatus as c_int & !mask) as libc::c_ushort;
    }
    Ok(())
}

pub unsafe fn luaD_hookcall(
    L: *const Thread,
    ci: *mut CallInfo,
) -> Result<(), Box<dyn core::error::Error>> {
    (*L).oldpc.set(0);

    if (*L).hookmask.get() & 1 << 0 != 0 {
        let event: c_int = if (*ci).callstatus as c_int & (1 as c_int) << 5 as c_int != 0 {
            4 as c_int
        } else {
            0 as c_int
        };
        let p: *mut Proto = (*(*(*ci).func).val.value_.gc.cast::<LuaFn>()).p.get();
        (*ci).u.savedpc = ((*ci).u.savedpc).offset(1);
        (*ci).u.savedpc;
        luaD_hook(L, event, -(1 as c_int), 1 as c_int, (*p).numparams.into())?;
        (*ci).u.savedpc = ((*ci).u.savedpc).offset(-1);
        (*ci).u.savedpc;
    }
    Ok(())
}

unsafe fn rethook(
    L: *const Thread,
    mut ci: *mut CallInfo,
    nres: c_int,
) -> Result<(), Box<dyn core::error::Error>> {
    if (*L).hookmask.get() & 1 << 1 != 0 {
        let firstres: StkId = ((*L).top.get()).offset(-(nres as isize));
        let mut delta: c_int = 0 as c_int;
        let mut ftransfer: c_int = 0;
        if (*ci).callstatus as c_int & (1 as c_int) << 1 as c_int == 0 {
            let p: *mut Proto = (*(*(*ci).func).val.value_.gc.cast::<LuaFn>()).p.get();
            if (*p).is_vararg != 0 {
                delta = (*ci).u.nextraargs + (*p).numparams as c_int + 1 as c_int;
            }
        }
        (*ci).func = ((*ci).func).offset(delta as isize);
        ftransfer = firstres.offset_from((*ci).func) as libc::c_long as libc::c_ushort as c_int;

        luaD_hook(
            L,
            1 as c_int,
            -(1 as c_int),
            ftransfer,
            nres.try_into().unwrap(),
        )?;

        (*ci).func = ((*ci).func).offset(-(delta as isize));
    }
    ci = (*ci).previous;

    if (*ci).callstatus as c_int & (1 as c_int) << 1 as c_int == 0 {
        (*L).oldpc.set(
            ((*ci).u.savedpc)
                .offset_from((*(*(*(*ci).func).val.value_.gc.cast::<LuaFn>()).p.get()).code)
                as libc::c_long as c_int
                - 1,
        );
    }

    Ok(())
}

unsafe fn tryfuncTM(
    L: *const Thread,
    mut func: StkId,
) -> Result<StkId, Box<dyn core::error::Error>> {
    let mut tm: *const UnsafeValue = 0 as *const UnsafeValue;
    let mut p: StkId = 0 as *mut StackValue;
    if ((((*L).stack_last.get()).offset_from((*L).top.get()) as libc::c_long
        <= 1 as c_int as libc::c_long) as c_int
        != 0 as c_int) as c_int as libc::c_long
        != 0
    {
        let t__: isize =
            (func as *mut libc::c_char).offset_from((*L).stack.get() as *mut libc::c_char);
        if (*(*L).hdr.global).gc.debt() > 0 as c_int as isize {
            crate::gc::step((*L).hdr.global);
        }
        luaD_growstack(L, 1)?;
        func = ((*L).stack.get() as *mut libc::c_char).offset(t__ as isize) as StkId;
    }
    tm = luaT_gettmbyobj(L, &mut (*func).val, TM_CALL);

    if (*tm).tt_ & 0xf == 0 {
        return Err(luaG_callerror(L, &raw const (*func).val));
    }

    p = (*L).top.get();
    while p > func {
        let io1: *mut UnsafeValue = &mut (*p).val;
        let io2: *const UnsafeValue = &mut (*p.offset(-(1 as c_int as isize))).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        p = p.offset(-1);
    }

    (*L).top.add(1);

    let io1_0: *mut UnsafeValue = &mut (*func).val;
    let io2_0: *const UnsafeValue = tm;
    (*io1_0).value_ = (*io2_0).value_;
    (*io1_0).tt_ = (*io2_0).tt_;
    return Ok(func);
}

unsafe fn moveresults(
    L: *const Thread,
    mut res: StkId,
    mut nres: c_int,
    mut wanted: c_int,
) -> Result<(), Box<dyn core::error::Error>> {
    let mut firstresult: StkId = 0 as *mut StackValue;
    let mut i: c_int = 0;
    match wanted {
        0 => {
            (*L).top.set(res);
            return Ok(());
        }
        1 => {
            if nres == 0 as c_int {
                (*res).val.tt_ = (0 as c_int | (0 as c_int) << 4 as c_int) as u8;
            } else {
                let io1: *mut UnsafeValue = &raw mut (*res).val;
                let io2: *const UnsafeValue =
                    &raw mut (*((*L).top.get()).offset(-(nres as isize))).val;
                (*io1).value_ = (*io2).value_;
                (*io1).tt_ = (*io2).tt_;
            }
            (*L).top.set(res.offset(1 as c_int as isize));
            return Ok(());
        }
        -1 => {
            wanted = nres;
        }
        _ => {
            if wanted < -(1 as c_int) {
                (*(*L).ci.get()).callstatus = ((*(*L).ci.get()).callstatus as c_int
                    | (1 as c_int) << 9 as c_int)
                    as libc::c_ushort;
                (*(*L).ci.get()).u2.nres = nres;
                res = luaF_close(L, res)?;
                (*(*L).ci.get()).callstatus = ((*(*L).ci.get()).callstatus as c_int
                    & !((1 as c_int) << 9 as c_int))
                    as libc::c_ushort;
                if (*L).hookmask.get() != 0 {
                    let savedres: isize = (res as *mut libc::c_char)
                        .offset_from((*L).stack.get() as *mut libc::c_char);
                    rethook(L, (*L).ci.get(), nres)?;
                    res =
                        ((*L).stack.get() as *mut libc::c_char).offset(savedres as isize) as StkId;
                }
                wanted = -wanted - 3 as c_int;
                if wanted == -(1 as c_int) {
                    wanted = nres;
                }
            }
        }
    }
    firstresult = ((*L).top.get()).offset(-(nres as isize));
    if nres > wanted {
        nres = wanted;
    }
    i = 0 as c_int;
    while i < nres {
        let io1_0: *mut UnsafeValue = &mut (*res.offset(i as isize)).val;
        let io2_0: *const UnsafeValue = &mut (*firstresult.offset(i as isize)).val;
        (*io1_0).value_ = (*io2_0).value_;
        (*io1_0).tt_ = (*io2_0).tt_;
        i += 1;
    }
    while i < wanted {
        (*res.offset(i as isize)).val.tt_ = (0 as c_int | (0 as c_int) << 4 as c_int) as u8;
        i += 1;
    }
    (*L).top.set(res.offset(wanted as isize));
    Ok(())
}

#[inline(always)]
pub unsafe fn luaD_poscall(
    L: *const Thread,
    ci: *mut CallInfo,
    nres: c_int,
) -> Result<(), Box<dyn core::error::Error>> {
    let wanted: c_int = (*ci).nresults as c_int;
    if (((*L).hookmask.get() != 0 && !(wanted < -(1 as c_int))) as c_int != 0 as c_int) as c_int
        as libc::c_long
        != 0
    {
        rethook(L, ci, nres)?;
    }
    moveresults(L, (*ci).func, nres, wanted)?;
    (*L).ci.set((*ci).previous);
    Ok(())
}

#[inline(always)]
unsafe fn prepCallInfo(
    L: *const Thread,
    func: StkId,
    nret: c_int,
    mask: c_int,
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

async unsafe fn precallC(
    L: *const Thread,
    mut func: StkId,
    nresults: c_int,
    f: Func,
) -> Result<c_int, Box<dyn core::error::Error>> {
    // Grow stack at least 20 slots.
    if ((*L).stack_last.get()).offset_from((*L).top.get()) <= 20 {
        let t__: isize =
            (func as *mut libc::c_char).offset_from((*L).stack.get() as *mut libc::c_char);
        if (*(*L).hdr.global).gc.debt() > 0 as c_int as isize {
            crate::gc::step((*L).hdr.global);
        }
        luaD_growstack(L, 20)?;
        func = ((*L).stack.get() as *mut libc::c_char).offset(t__ as isize) as StkId;
    }

    // Set current CI.
    let ci = prepCallInfo(L, func, nresults, 1 << 1, ((*L).top.get()).offset(20));

    (*L).ci.set(ci);

    // Invoke hook.
    let narg = (*L).top.get().offset_from_unsigned(func) - 1;

    if ((*L).hookmask.get() & (1 as c_int) << 0 != 0) as c_int as libc::c_long != 0 {
        luaD_hook(L, 0 as c_int, -(1 as c_int), 1 as c_int, narg)?;
    }

    // Invoke Rust function.
    let cx = Context::new(L, narg);
    let ret = (*L).top.get().offset_from_unsigned((*L).stack.get()); // Rust may move the stack.

    match f {
        Func::NonYieldableFp(f) => f(&cx)?,
    }

    // Get number of results.
    let n = (*L)
        .top
        .get()
        .offset_from((*L).stack.get().add(ret))
        .clamp(0, isize::MAX)
        .try_into()
        .unwrap();

    luaD_poscall(L, ci, n)?;

    Ok(n)
}

pub async unsafe fn luaD_pretailcall(
    L: *const Thread,
    ci: *mut CallInfo,
    mut func: StkId,
    mut narg1: c_int,
    delta: c_int,
) -> Result<c_int, Box<dyn core::error::Error>> {
    loop {
        match (*func).val.tt_ as c_int & 0x3f as c_int {
            38 => {
                return precallC(
                    L,
                    func,
                    -(1 as c_int),
                    Func::NonYieldableFp((*((*func).val.value_.gc as *mut CClosure)).f),
                )
                .await;
            }
            2 => return precallC(L, func, -1, Func::NonYieldableFp((*func).val.value_.f)).await,
            18 | 34 | 50 => todo!(),
            6 => {
                let p: *mut Proto = (*(*func).val.value_.gc.cast::<LuaFn>()).p.get();
                let fsize: c_int = (*p).maxstacksize as c_int;
                let nfixparams: c_int = (*p).numparams as c_int;
                let mut i: c_int = 0;

                if ((((*L).stack_last.get()).offset_from((*L).top.get()) as libc::c_long
                    <= (fsize - delta) as libc::c_long) as c_int
                    != 0 as c_int) as c_int as libc::c_long
                    != 0
                {
                    let t__: isize = (func as *mut libc::c_char)
                        .offset_from((*L).stack.get() as *mut libc::c_char);
                    if (*(*L).hdr.global).gc.debt() > 0 as c_int as isize {
                        crate::gc::step((*L).hdr.global);
                    }
                    luaD_growstack(L, (fsize - delta).try_into().unwrap())?;
                    func = ((*L).stack.get() as *mut libc::c_char).offset(t__ as isize) as StkId;
                }

                (*ci).func = ((*ci).func).offset(-(delta as isize));
                i = 0 as c_int;
                while i < narg1 {
                    let io1: *mut UnsafeValue = &raw mut (*((*ci).func).offset(i as isize)).val;
                    let io2: *const UnsafeValue = &raw mut (*func.offset(i as isize)).val;
                    (*io1).value_ = (*io2).value_;
                    (*io1).tt_ = (*io2).tt_;
                    i += 1;
                }
                func = (*ci).func;
                while narg1 <= nfixparams {
                    (*func.offset(narg1 as isize)).val.tt_ =
                        (0 as c_int | (0 as c_int) << 4 as c_int) as u8;
                    narg1 += 1;
                }
                (*ci).top = func.offset(1 as c_int as isize).offset(fsize as isize);
                (*ci).u.savedpc = (*p).code;
                (*ci).callstatus =
                    ((*ci).callstatus as c_int | (1 as c_int) << 5 as c_int) as libc::c_ushort;
                (*L).top.set(func.offset(narg1 as isize));
                return Ok(-(1 as c_int));
            }
            _ => {
                func = tryfuncTM(L, func)?;
                narg1 += 1;
            }
        }
    }
}

pub async unsafe fn luaD_precall(
    L: *const Thread,
    mut func: StkId,
    nresults: c_int,
) -> Result<*mut CallInfo, Box<dyn core::error::Error>> {
    loop {
        match (*func).val.tt_ as c_int & 0x3f as c_int {
            38 => {
                precallC(
                    L,
                    func,
                    nresults,
                    Func::NonYieldableFp((*((*func).val.value_.gc as *mut CClosure)).f),
                )
                .await?;

                return Ok(0 as *mut CallInfo);
            }
            2 => {
                precallC(
                    L,
                    func,
                    nresults,
                    Func::NonYieldableFp((*func).val.value_.f),
                )
                .await?;

                return Ok(0 as *mut CallInfo);
            }
            18 | 34 | 50 => todo!(),
            6 => {
                let mut ci: *mut CallInfo = 0 as *mut CallInfo;
                let p: *mut Proto = (*(*func).val.value_.gc.cast::<LuaFn>()).p.get();
                let mut narg: c_int =
                    ((*L).top.get()).offset_from(func) as libc::c_long as c_int - 1 as c_int;
                let nfixparams: c_int = (*p).numparams as c_int;
                let fsize = usize::from((*p).maxstacksize);

                if ((*L).stack_last.get()).offset_from_unsigned((*L).top.get()) <= fsize {
                    let t__: isize = (func as *mut libc::c_char)
                        .offset_from((*L).stack.get() as *mut libc::c_char);
                    if (*(*L).hdr.global).gc.debt() > 0 as c_int as isize {
                        crate::gc::step((*L).hdr.global);
                    }
                    luaD_growstack(L, fsize)?;
                    func = ((*L).stack.get() as *mut libc::c_char).offset(t__ as isize) as StkId;
                }

                ci = prepCallInfo(
                    L,
                    func,
                    nresults,
                    0 as c_int,
                    func.offset(1 as c_int as isize).offset(fsize as isize),
                );
                (*L).ci.set(ci);
                (*ci).u.savedpc = (*p).code;
                while narg < nfixparams {
                    let fresh2 = (*L).top.get();
                    (*L).top.add(1);
                    (*fresh2).val.tt_ = (0 as c_int | (0 as c_int) << 4 as c_int) as u8;
                    narg += 1;
                }
                return Ok(ci);
            }
            _ => func = tryfuncTM(L, func)?,
        }
    }
}

pub async unsafe fn luaD_call(
    L: *const Thread,
    func: StkId,
    nResults: c_int,
) -> Result<(), Box<dyn core::error::Error>> {
    let ci = luaD_precall(L, func, nResults).await?;

    if !ci.is_null() {
        (*ci).callstatus = ((1 as c_int) << 2 as c_int) as libc::c_ushort;
        luaV_execute(L, ci).await?;
    }

    Ok(())
}

pub unsafe fn luaD_closeprotected(
    L: *const Thread,
    level: usize,
    mut status: Result<(), PcallError>,
) -> Result<(), PcallError> {
    let old_ci: *mut CallInfo = (*L).ci.get();
    let old_allowhooks: u8 = (*L).allowhook.get();

    loop {
        let e = match luaF_close(L, (*L).stack.get().byte_add(level)) {
            Ok(_) => break status,
            Err(e) => e,
        };

        status = Err(PcallError::new(L, e));

        (*L).ci.set(old_ci);
        (*L).allowhook.set(old_allowhooks);
    }
}

pub unsafe fn luaD_protectedparser(
    g: &Pin<Rc<Lua>>,
    mut z: Zio,
    info: ChunkInfo,
) -> Result<Ref<LuaFn>, ParseError> {
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
    p.dyd.actvar.size = 0 as c_int;
    p.dyd.gt.arr = 0 as *mut Labeldesc;
    p.dyd.gt.size = 0 as c_int;
    p.dyd.label.arr = 0 as *mut Labeldesc;
    p.dyd.label.size = 0 as c_int;
    p.buff.buffer = 0 as *mut libc::c_char;
    p.buff.buffsize = 0 as c_int as usize;

    // Parse.
    let fresh3 = (*p.z).n;
    (*p.z).n = ((*p.z).n).wrapping_sub(1);
    let c: c_int = if fresh3 > 0 {
        let fresh4 = (*p.z).p;
        (*p.z).p = ((*p.z).p).offset(1);
        *fresh4 as libc::c_uchar as c_int
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
    p.buff.buffsize = 0 as c_int as usize;
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

enum Func {
    NonYieldableFp(fn(&Context) -> Result<(), Box<dyn core::error::Error>>),
}
