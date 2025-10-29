#![allow(non_camel_case_types, non_snake_case, unused_assignments)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::context::{Args, Context, Ret};
use crate::ldebug::luaG_callerror;
use crate::lfunc::{luaF_close, luaF_initupvals};
use crate::lmem::{luaM_free_, luaM_saferealloc_};
use crate::lobject::CClosure;
use crate::lparser::{C2RustUnnamed_9, Dyndata, Labeldesc, Labellist, Vardesc, luaY_parser};
use crate::lstate::{CallInfo, lua_Debug, luaE_extendCI, luaE_shrinkCI};
use crate::ltm::{TM_CALL, luaT_gettmbyobj};
use crate::lzio::{Mbuffer, Zio};
use crate::vm::luaV_execute;
use crate::{
    CallError, ChunkInfo, Lua, LuaFn, NON_YIELDABLE_WAKER, ParseError, Ref, StackOverflow,
    StackValue, Thread,
};
use alloc::alloc::handle_alloc_error;
use alloc::boxed::Box;
use core::alloc::Layout;
use core::error::Error;
use core::ffi::{c_char, c_void};
use core::ops::{Deref, DerefMut};
use core::pin::Pin;
use core::ptr::{addr_eq, null, null_mut};
use core::task::Poll;

type c_uchar = u8;
type c_short = i16;
type c_ushort = u16;
type c_int = i32;
type c_long = i64;

unsafe fn relstack<D>(L: *const Thread<D>) {
    let mut ci = null_mut();
    let mut up = null_mut();

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

unsafe fn correctstack<D>(L: *const Thread<D>) {
    let mut ci = null_mut();
    let mut up = null_mut();

    (*L).top
        .set(((*L).stack.get()).byte_add((*L).top.get() as usize));
    (*L).tbclist
        .set(((*L).stack.get()).byte_add((*L).tbclist.get() as usize));

    up = (*L).openupval.get();

    while !up.is_null() {
        (*up)
            .v
            .set((((*L).stack.get()).byte_add((*up).v.get() as usize)).cast());
        up = (*(*up).u.get()).open.next;
    }

    ci = (*L).ci.get();

    while !ci.is_null() {
        (*ci).top = ((*L).stack.get()).byte_add((*ci).top as usize);
        (*ci).func = ((*L).stack.get()).byte_add((*ci).func as usize);
        ci = (*ci).previous;
    }
}

pub unsafe fn luaD_reallocstack<D>(th: *const Thread<D>, newsize: usize) {
    let oldsize = ((*th).stack_last.get()).offset_from_unsigned((*th).stack.get());

    relstack(th);

    // Re-allocate the stack.
    let newstack = alloc::alloc::realloc(
        (*th).stack.get().cast(),
        Layout::array::<StackValue<D>>(oldsize + 5).unwrap(),
        (newsize + 5) * size_of::<StackValue<D>>(),
    ) as *mut StackValue<D>;

    if newstack.is_null() {
        handle_alloc_error(Layout::array::<StackValue<D>>(newsize + 5).unwrap());
    }

    (*th).stack.set(newstack);
    correctstack(th);
    (*th).stack_last.set(((*th).stack.get()).add(newsize));

    // Fill the new space with nil.
    let mut i = oldsize + 5;

    while i < newsize + 5 {
        (*newstack.add(i)).tt_ = 0 | 0 << 4;
        i += 1;
    }
}

#[inline(never)]
pub unsafe fn luaD_growstack<D>(L: *const Thread<D>, n: usize) -> Result<(), StackOverflow> {
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

unsafe fn stackinuse<D>(L: *const Thread<D>) -> usize {
    let mut res = 0;
    let mut lim = (*L).top.get();
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

pub unsafe fn luaD_shrinkstack<D>(L: *const Thread<D>) {
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

pub unsafe fn luaD_hook<D>(
    L: *const Thread<D>,
    event: c_int,
    line: c_int,
    ftransfer: c_int,
    ntransfer: usize,
) -> Result<(), Box<dyn Error>> {
    let hook = (*L).hook.get();
    if hook.is_some() && (*L).allowhook.get() != 0 {
        let mut mask: c_int = (1 as c_int) << 3 as c_int;
        let ci = (*L).ci.get();
        let top: isize =
            ((*L).top.get() as *mut c_char).offset_from((*L).stack.get() as *mut c_char);
        let ci_top: isize = ((*ci).top as *mut c_char).offset_from((*L).stack.get() as *mut c_char);
        let mut ar = lua_Debug::default();
        ar.event = event;
        ar.currentline = line;
        ar.i_ci = ci;

        if ntransfer != 0 {
            mask |= (1 as c_int) << 8 as c_int;
            (*ci).u2.transferinfo.ftransfer = ftransfer as c_ushort;
            (*ci).u2.transferinfo.ntransfer = ntransfer;
        }

        if (*ci).callstatus as c_int & (1 as c_int) << 1 == 0 && (*L).top.get() < (*ci).top {
            (*L).top.set((*ci).top);
        }

        if ((((*L).stack_last.get()).offset_from((*L).top.get()) as c_long <= 20 as c_int as c_long)
            as c_int
            != 0 as c_int) as c_int as c_long
            != 0
        {
            luaD_growstack(L, 20)?;
        }
        if (*ci).top < ((*L).top.get()).offset(20 as c_int as isize) {
            (*ci).top = ((*L).top.get()).offset(20 as c_int as isize);
        }
        (*L).allowhook.set(0);
        (*ci).callstatus = ((*ci).callstatus as c_int | mask) as c_ushort;
        (Some(hook.expect("non-null function pointer"))).expect("non-null function pointer")(
            L, &mut ar,
        );
        (*L).allowhook.set(1);
        (*ci).top = ((*L).stack.get() as *mut c_char).offset(ci_top as isize) as _;
        (*L).top
            .set(((*L).stack.get() as *mut c_char).offset(top as isize) as _);
        (*ci).callstatus = ((*ci).callstatus as c_int & !mask) as c_ushort;
    }
    Ok(())
}

pub unsafe fn luaD_hookcall<D>(
    L: *const Thread<D>,
    ci: *mut CallInfo<D>,
) -> Result<(), Box<dyn Error>> {
    (*L).oldpc.set(0);

    if (*L).hookmask.get() & 1 << 0 != 0 {
        let event: c_int = if (*ci).callstatus as c_int & (1 as c_int) << 5 as c_int != 0 {
            4 as c_int
        } else {
            0 as c_int
        };
        let p = (*(*(*ci).func).value_.gc.cast::<LuaFn<D>>()).p.get();
        (*ci).u.savedpc = ((*ci).u.savedpc).offset(1);
        (*ci).u.savedpc;
        luaD_hook(L, event, -(1 as c_int), 1 as c_int, (*p).numparams.into())?;
        (*ci).u.savedpc = ((*ci).u.savedpc).offset(-1);
        (*ci).u.savedpc;
    }
    Ok(())
}

unsafe fn rethook<D>(
    L: *const Thread<D>,
    mut ci: *mut CallInfo<D>,
    nres: c_int,
) -> Result<(), Box<dyn Error>> {
    if (*L).hookmask.get() & 1 << 1 != 0 {
        let firstres = ((*L).top.get()).offset(-(nres as isize));
        let mut delta: c_int = 0 as c_int;
        let mut ftransfer: c_int = 0;
        if (*ci).callstatus as c_int & (1 as c_int) << 1 as c_int == 0 {
            let p = (*(*(*ci).func).value_.gc.cast::<LuaFn<D>>()).p.get();
            if (*p).is_vararg != 0 {
                delta = (*ci).u.nextraargs + (*p).numparams as c_int + 1 as c_int;
            }
        }
        (*ci).func = ((*ci).func).offset(delta as isize);
        ftransfer = firstres.offset_from((*ci).func) as c_long as c_ushort as c_int;

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
                .offset_from((*(*(*(*ci).func).value_.gc.cast::<LuaFn<D>>()).p.get()).code)
                as c_long as c_int
                - 1,
        );
    }

    Ok(())
}

unsafe fn tryfuncTM<D>(
    L: *const Thread<D>,
    mut func: *mut StackValue<D>,
) -> Result<*mut StackValue<D>, Box<dyn Error>> {
    let mut tm = null();
    let mut p = null_mut();

    if ((*L).stack_last.get()).offset_from((*L).top.get()) <= 1 {
        let t__: isize = (func as *mut c_char).offset_from((*L).stack.get() as *mut c_char);

        luaD_growstack(L, 1)?;
        func = ((*L).stack.get() as *mut c_char).offset(t__ as isize) as _;
    }
    tm = luaT_gettmbyobj(L, func.cast(), TM_CALL);

    if (*tm).tt_ & 0xf == 0 {
        return Err(luaG_callerror(L, func.cast()));
    }

    p = (*L).top.get();
    while p > func {
        let io1 = p;
        let io2 = p.offset(-(1 as c_int as isize));
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        p = p.offset(-1);
    }

    (*L).top.add(1);

    let io1_0 = func;
    let io2_0 = tm;
    (*io1_0).value_ = (*io2_0).value_;
    (*io1_0).tt_ = (*io2_0).tt_;
    return Ok(func);
}

unsafe fn moveresults<D>(
    L: *const Thread<D>,
    mut res: *mut StackValue<D>,
    mut nres: c_int,
    mut wanted: c_int,
) -> Result<(), Box<dyn Error>> {
    let mut firstresult = null_mut();
    let mut i: c_int = 0;
    match wanted {
        0 => {
            (*L).top.set(res);
            return Ok(());
        }
        1 => {
            if nres == 0 as c_int {
                (*res).tt_ = (0 as c_int | (0 as c_int) << 4 as c_int) as u8;
            } else {
                let io1 = res;
                let io2 = ((*L).top.get()).offset(-(nres as isize));
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
                (*(*L).ci.get()).callstatus =
                    ((*(*L).ci.get()).callstatus as c_int | (1 as c_int) << 9 as c_int) as c_ushort;
                (*(*L).ci.get()).u2.nres = nres;
                res = match luaF_close(L, res) {
                    Ok(v) => v,
                    Err(e) => return Err(e), // Requires unsized coercion.
                };
                (*(*L).ci.get()).callstatus = ((*(*L).ci.get()).callstatus as c_int
                    & !((1 as c_int) << 9 as c_int))
                    as c_ushort;
                if (*L).hookmask.get() != 0 {
                    let savedres: isize =
                        (res as *mut c_char).offset_from((*L).stack.get() as *mut c_char);
                    rethook(L, (*L).ci.get(), nres)?;
                    res = ((*L).stack.get() as *mut c_char).offset(savedres as isize) as _;
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
        let io1_0 = res.offset(i as isize);
        let io2_0 = firstresult.offset(i as isize);
        (*io1_0).value_ = (*io2_0).value_;
        (*io1_0).tt_ = (*io2_0).tt_;
        i += 1;
    }
    while i < wanted {
        (*res.offset(i as isize)).tt_ = (0 as c_int | (0 as c_int) << 4 as c_int) as u8;
        i += 1;
    }
    (*L).top.set(res.offset(wanted as isize));
    Ok(())
}

#[inline(always)]
pub unsafe fn luaD_poscall<D>(
    L: *const Thread<D>,
    ci: *mut CallInfo<D>,
    nres: c_int,
) -> Result<(), Box<dyn Error>> {
    let wanted: c_int = (*ci).nresults as c_int;
    if (((*L).hookmask.get() != 0 && !(wanted < -(1 as c_int))) as c_int != 0 as c_int) as c_int
        as c_long
        != 0
    {
        rethook(L, ci, nres)?;
    }
    moveresults(L, (*ci).func, nres, wanted)?;
    (*L).ci.set((*ci).previous);
    Ok(())
}

unsafe fn prepCallInfo<D>(
    L: *const Thread<D>,
    func: *mut StackValue<D>,
    nret: c_int,
    mask: c_int,
    top: *mut StackValue<D>,
) -> *mut CallInfo<D> {
    (*L).ci.set(if !((*(*L).ci.get()).next).is_null() {
        (*(*L).ci.get()).next
    } else {
        luaE_extendCI(L)
    });
    let ci = (*L).ci.get();
    (*ci).func = func;
    (*ci).nresults = nret as c_short;
    (*ci).callstatus = mask as c_ushort;
    (*ci).top = top;
    return ci;
}

async unsafe fn precallC<A>(
    L: &Thread<A>,
    func: *mut StackValue<A>,
    nresults: c_int,
    f: Func<A>,
) -> Result<c_int, Box<dyn Error>> {
    // Set current CI.
    let ci = prepCallInfo(L, func, nresults, 1 << 1, L.top.get());

    L.ci.set(ci);

    // Invoke hook.
    let narg = L.top.get().offset_from_unsigned(func) - 1;

    if (L.hookmask.get() & (1 as c_int) << 0 != 0) as c_int as c_long != 0 {
        luaD_hook(L, 0 as c_int, -(1 as c_int), 1 as c_int, narg)?;
    }

    // Invoke Rust function.
    let cx = Context::new(L, Args::new(narg));
    let cx = match f {
        Func::NonYieldableFp(f) => {
            let active = ActiveCall::new(L.hdr.global());

            if active.get() >= 100 {
                return Err("too many nested call into Rust functions".into());
            }

            f(cx)?
        }
        Func::AsyncFp(f) => {
            AsyncInvoker {
                g: L.hdr.global(),
                f: f(cx),
            }
            .await?
        }
    };

    // Get number of results.
    let n = cx.results().try_into().unwrap();

    drop(cx);

    luaD_poscall(L, ci, n)?;

    Ok(n)
}

pub async unsafe fn luaD_pretailcall<D>(
    L: *const Thread<D>,
    ci: *mut CallInfo<D>,
    mut func: *mut StackValue<D>,
    mut narg1: c_int,
    delta: c_int,
) -> Result<c_int, Box<dyn Error>> {
    loop {
        match (*func).tt_ & 0x3f {
            38 => {
                return precallC(
                    &*L,
                    func,
                    -(1 as c_int),
                    Func::NonYieldableFp((*((*func).value_.gc as *mut CClosure<D>)).f),
                )
                .await;
            }
            2 => return precallC(&*L, func, -1, Func::NonYieldableFp((*func).value_.f)).await,
            18 | 50 => todo!(),
            34 => return precallC(&*L, func, -1, Func::AsyncFp((*func).value_.a)).await,
            6 => {
                let p = (*(*func).value_.gc.cast::<LuaFn<D>>()).p.get();
                let fsize: c_int = (*p).maxstacksize as c_int;
                let nfixparams: c_int = (*p).numparams as c_int;
                let mut i: c_int = 0;

                if ((*L).stack_last.get()).offset_from((*L).top.get()) as c_long
                    <= (fsize - delta) as c_long
                {
                    let t__: isize =
                        (func as *mut c_char).offset_from((*L).stack.get() as *mut c_char);

                    luaD_growstack(L, (fsize - delta).try_into().unwrap())?;
                    func = ((*L).stack.get() as *mut c_char).offset(t__ as isize) as _;
                }

                (*ci).func = ((*ci).func).offset(-(delta as isize));
                i = 0 as c_int;
                while i < narg1 {
                    let io1 = ((*ci).func).offset(i as isize);
                    let io2 = func.offset(i as isize);
                    (*io1).value_ = (*io2).value_;
                    (*io1).tt_ = (*io2).tt_;
                    i += 1;
                }
                func = (*ci).func;
                while narg1 <= nfixparams {
                    (*func.offset(narg1 as isize)).tt_ =
                        (0 as c_int | (0 as c_int) << 4 as c_int) as u8;
                    narg1 += 1;
                }
                (*ci).top = func.offset(1 as c_int as isize).offset(fsize as isize);
                (*ci).u.savedpc = (*p).code;
                (*ci).callstatus =
                    ((*ci).callstatus as c_int | (1 as c_int) << 5 as c_int) as c_ushort;
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

pub async unsafe fn luaD_precall<D>(
    L: *const Thread<D>,
    mut func: *mut StackValue<D>,
    nresults: c_int,
) -> Result<*mut CallInfo<D>, Box<dyn Error>> {
    loop {
        match (*func).tt_ & 0x3f {
            38 => {
                precallC(
                    &*L,
                    func,
                    nresults,
                    Func::NonYieldableFp((*((*func).value_.gc as *mut CClosure<D>)).f),
                )
                .await?;

                return Ok(null_mut());
            }
            2 => {
                precallC(&*L, func, nresults, Func::NonYieldableFp((*func).value_.f)).await?;

                return Ok(null_mut());
            }
            18 | 50 => todo!(),
            34 => {
                precallC(&*L, func, nresults, Func::AsyncFp((*func).value_.a)).await?;
                return Ok(null_mut());
            }
            6 => {
                let mut ci = null_mut();
                let p = (*(*func).value_.gc.cast::<LuaFn<D>>()).p.get();
                let mut narg = (*L).top.get().offset_from(func) as c_long as c_int - 1;
                let nfixparams: c_int = (*p).numparams as c_int;
                let fsize = usize::from((*p).maxstacksize);

                if ((*L).stack_last.get()).offset_from_unsigned((*L).top.get()) <= fsize {
                    let t__: isize =
                        (func as *mut c_char).offset_from((*L).stack.get() as *mut c_char);

                    luaD_growstack(L, fsize)?;
                    func = ((*L).stack.get() as *mut c_char).offset(t__ as isize) as _;
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
                    (*fresh2).tt_ = (0 as c_int | (0 as c_int) << 4 as c_int) as u8;
                    narg += 1;
                }
                return Ok(ci);
            }
            _ => func = tryfuncTM(L, func)?,
        }
    }
}

/// A call to this function should **never** use a try operator otherwise [`CallError`] will not
/// properly forwarded. See https://users.rust-lang.org/t/mystified-by-downcast-failure/52459 for
/// more details.
pub async unsafe fn luaD_call<D>(
    L: *const Thread<D>,
    func: *mut StackValue<D>,
    nResults: c_int,
) -> Result<(), Box<CallError>> {
    let old_top = func.byte_offset_from_unsigned((*L).stack.get());
    let old_ci = (*L).ci.get();
    let old_allowhooks = (*L).allowhook.get();
    let r = match luaD_precall(L, func, nResults).await {
        Ok(ci) => match ci.is_null() {
            true => Ok(()),
            false => {
                (*ci).callstatus = 1 << 2;
                luaV_execute(&*L, ci).await
            }
        },
        Err(e) => Err(e),
    };

    match r {
        Ok(_) => {
            if nResults <= -1 && (*(*L).ci.get()).top < (*L).top.get() {
                (*(*L).ci.get()).top = (*L).top.get();
            }

            Ok(())
        }
        Err(e) => {
            let mut r = Err(CallError::new(L, e));

            (*L).ci.set(old_ci);
            (*L).allowhook.set(old_allowhooks);
            r = luaD_closeprotected(L, old_top, r);
            (*L).top.set((*L).stack.get().byte_add(old_top));

            r
        }
    }
}

pub unsafe fn luaD_closeprotected<D>(
    L: *const Thread<D>,
    level: usize,
    mut status: Result<(), Box<CallError>>,
) -> Result<(), Box<CallError>> {
    let old_ci = (*L).ci.get();
    let old_allowhooks: u8 = (*L).allowhook.get();

    loop {
        let e = match luaF_close(L, (*L).stack.get().byte_add(level)) {
            Ok(_) => break status,
            Err(e) => e,
        };

        status = Err(e);

        (*L).ci.set(old_ci);
        (*L).allowhook.set(old_allowhooks);
    }
}

pub unsafe fn luaD_protectedparser<D>(
    g: &Lua<D>,
    mut z: Zio,
    info: ChunkInfo,
) -> Result<Ref<'_, LuaFn<D>>, ParseError> {
    let mut buff = Mbuffer {
        buffer: 0 as *mut c_char,
        n: 0,
        buffsize: 0,
    };
    let mut dyd = Dyndata {
        actvar: C2RustUnnamed_9 {
            arr: null_mut(),
            n: 0,
            size: 0,
        },
        gt: Labellist {
            arr: null_mut(),
            n: 0,
            size: 0,
        },
        label: Labellist {
            arr: null_mut(),
            n: 0,
            size: 0,
        },
    };

    // Parse.
    let fresh3 = z.n;
    z.n = z.n.wrapping_sub(1);
    let c: c_int = if fresh3 > 0 {
        let fresh4 = z.p;
        z.p = z.p.offset(1);
        *fresh4 as c_uchar as c_int
    } else {
        -1
    };

    let status = luaY_parser(g, &raw mut z, &raw mut buff, &raw mut dyd, info, c);

    if let Ok(cl) = &status {
        luaF_initupvals(g, cl.deref());
    }

    buff.buffer = luaM_saferealloc_(g, buff.buffer as *mut c_void, buff.buffsize, 0).cast();

    luaM_free_(
        dyd.actvar.arr as *mut c_void,
        (dyd.actvar.size as usize).wrapping_mul(size_of::<Vardesc<D>>()),
    );
    luaM_free_(
        dyd.gt.arr as *mut c_void,
        (dyd.gt.size as usize).wrapping_mul(size_of::<Labeldesc<D>>()),
    );
    luaM_free_(
        dyd.label.arr as *mut c_void,
        (dyd.label.size as usize).wrapping_mul(size_of::<Labeldesc<D>>()),
    );

    status
}

/// Encapsulates a function pointer.
enum Func<A> {
    NonYieldableFp(for<'a> fn(Context<'a, A, Args>) -> Result<Context<'a, A, Ret>, Box<dyn Error>>),
    AsyncFp(
        fn(
            Context<A, Args>,
        ) -> Pin<Box<dyn Future<Output = Result<Context<A, Ret>, Box<dyn Error>>> + '_>>,
    ),
}

/// Implementation of [Future] to poll [Func::AsyncFp].
struct AsyncInvoker<'a, A> {
    g: &'a Lua<A>,
    f: Pin<Box<dyn Future<Output = Result<Context<'a, A, Ret>, Box<dyn Error>>> + 'a>>,
}

impl<'a, A> Future for AsyncInvoker<'a, A> {
    type Output = Result<Context<'a, A, Ret>, Box<dyn Error>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> Poll<Self::Output> {
        // Check if calling from async context.
        if addr_eq(cx.waker().vtable(), &NON_YIELDABLE_WAKER) {
            return Poll::Ready(Err(
                "attempt to call async function fron non-async context".into()
            ));
        }

        // Check recursive limit.
        let i = self.deref_mut();
        let active = ActiveCall::new(i.g);

        if active.get() >= 100 {
            return Poll::Ready(Err("too many nested call into Rust functions".into()));
        }

        i.f.as_mut().poll(cx)
    }
}

/// RAII struct to increase/decrease [Lua::active_rust_call].
struct ActiveCall<'a, A> {
    g: &'a Lua<A>,
    active: usize,
}

impl<'a, A> ActiveCall<'a, A> {
    #[inline(always)]
    fn new(g: &'a Lua<A>) -> Self {
        let active = g.active_rust_call.get() + 1;

        g.active_rust_call.set(active);

        Self { g, active }
    }

    #[inline(always)]
    fn get(&self) -> usize {
        self.active
    }
}

impl<'a, A> Drop for ActiveCall<'a, A> {
    #[inline(always)]
    fn drop(&mut self) {
        self.g.active_rust_call.update(|v| v - 1);
    }
}
