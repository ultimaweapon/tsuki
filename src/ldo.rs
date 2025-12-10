#![allow(non_camel_case_types, non_snake_case, unused_assignments)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::context::{Args, Context, Resume, Ret};
use crate::ldebug::luaG_callerror;
use crate::lfunc::{luaF_close, luaF_initupvals};
use crate::lmem::{luaM_free_, luaM_saferealloc_};
use crate::lobject::CClosure;
use crate::lparser::{C2RustUnnamed_9, Dyndata, Labeldesc, Labellist, Vardesc, luaY_parser};
use crate::lstate::{CallInfo, luaE_extendCI, luaE_shrinkCI};
use crate::ltm::{TM_CALL, luaT_gettmbyobj};
use crate::lzio::{Mbuffer, Zio};
use crate::{
    CallError, ChunkInfo, Lua, LuaFn, NON_YIELDABLE_WAKER, ParseError, Ref, StackOverflow,
    StackValue, Thread, YIELDABLE_WAKER, Yield,
};
use alloc::alloc::handle_alloc_error;
use alloc::boxed::Box;
use core::alloc::Layout;
use core::error::Error;
use core::ffi::{c_char, c_void};
use core::ops::Deref;
use core::pin::Pin;
use core::ptr::{addr_eq, null, null_mut};
use core::task::Poll;

type c_uchar = u8;
type c_short = i16;
type c_ushort = u16;
type c_int = i32;
type c_long = i64;

unsafe fn relstack<D>(L: *const Thread<D>) {
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
}

unsafe fn correctstack<D>(L: *const Thread<D>) {
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
    let mut ci = (*L).ci.get();

    while !ci.is_null() {
        if res < (*ci).top.get() {
            res = (*ci).top.get();
        }
        ci = (*ci).previous;
    }

    res += 1;

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

#[inline(never)]
unsafe fn moveresults<A>(
    L: &Thread<A>,
    mut res: *mut StackValue<A>,
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
pub unsafe fn luaD_poscall<A>(
    L: &Thread<A>,
    ci: *mut CallInfo,
    nres: c_int,
) -> Result<(), Box<dyn Error>> {
    let wanted: c_int = (*ci).nresults as c_int;

    moveresults(L, L.stack.get().add((*ci).func), nres, wanted)?;
    (*L).ci.set((*ci).previous);

    Ok(())
}

pub async unsafe fn luaD_pretailcall<D>(
    L: *const Thread<D>,
    ci: *mut CallInfo,
    mut func: *mut StackValue<D>,
    mut narg1: c_int,
    delta: c_int,
) -> Result<c_int, Box<dyn Error>> {
    loop {
        match (*func).tt_ & 0x3f {
            0x02 => return call_fp(&*L, func, -1, (*func).value_.f),
            0x12 => {
                return YieldInvoker {
                    th: &*L,
                    func,
                    nresults: -1,
                    fp: (*func).value_.y,
                    ci: null_mut(),
                }
                .await;
            }
            0x22 => return call_async_fp(&*L, func, -1, (*func).value_.a).await,
            0x32 => todo!(),
            0x06 => {
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

                (*ci).func = (*ci).func.strict_sub_signed(delta as isize);

                i = 0 as c_int;
                while i < narg1 {
                    let io1 = (*L).stack.get().add((*ci).func + (i as usize));
                    let io2 = func.offset(i as isize);
                    (*io1).value_ = (*io2).value_;
                    (*io1).tt_ = (*io2).tt_;
                    i += 1;
                }
                func = (*L).stack.get().add((*ci).func);
                while narg1 <= nfixparams {
                    (*func.offset(narg1 as isize)).tt_ =
                        (0 as c_int | (0 as c_int) << 4 as c_int) as u8;
                    narg1 += 1;
                }

                (*ci).top = ((*ci).func + 1)
                    .strict_add_signed(fsize as isize)
                    .try_into()
                    .unwrap();
                (*ci).pc = 0;
                (*ci).callstatus =
                    ((*ci).callstatus as c_int | (1 as c_int) << 5 as c_int) as c_ushort;
                (*L).top.set(func.offset(narg1 as isize));
                return Ok(-(1 as c_int));
            }
            0x26 => {
                return call_fp(
                    &*L,
                    func,
                    -(1 as c_int),
                    (*((*func).value_.gc as *mut CClosure<D>)).f,
                );
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
) -> Result<*mut CallInfo, Box<dyn Error>> {
    loop {
        match (*func).tt_ & 0x3f {
            0x02 => {
                call_fp(&*L, func, nresults, (*func).value_.f)?;
                return Ok(null_mut());
            }
            0x12 => {
                YieldInvoker {
                    th: &*L,
                    func,
                    nresults,
                    fp: (*func).value_.y,
                    ci: null_mut(),
                }
                .await?;

                return Ok(null_mut());
            }
            0x22 => {
                call_async_fp(&*L, func, nresults, (*func).value_.a).await?;
                return Ok(null_mut());
            }
            0x32 => todo!(),
            0x06 => {
                let mut ci = null_mut();
                let p = (*(*func).value_.gc.cast::<LuaFn<D>>()).p.get();
                let mut narg = (*L).top.get().offset_from(func) as c_long as c_int - 1;
                let nfixparams: c_int = (*p).numparams as c_int;
                let fsize = usize::from((*p).maxstacksize);

                if (*L).stack_last.get().offset_from_unsigned((*L).top.get()) <= fsize {
                    let t__ = func.offset_from_unsigned((*L).stack.get());

                    luaD_growstack(L, fsize)?;

                    func = (*L).stack.get().add(t__);
                }

                ci = get_ci(
                    L,
                    func,
                    nresults,
                    0 as c_int,
                    func.offset(1 as c_int as isize).offset(fsize as isize),
                );
                (*L).ci.set(ci);
                (*ci).pc = 0;

                while narg < nfixparams {
                    let fresh2 = (*L).top.get();
                    (*L).top.add(1);
                    (*fresh2).tt_ = (0 as c_int | (0 as c_int) << 4 as c_int) as u8;
                    narg += 1;
                }
                return Ok(ci);
            }
            0x26 => {
                call_fp(
                    &*L,
                    func,
                    nresults,
                    (*((*func).value_.gc as *mut CClosure<D>)).f,
                )?;

                return Ok(null_mut());
            }
            _ => func = tryfuncTM(L, func)?,
        }
    }
}

/// A call to this function should **never** use a try operator otherwise [CallError] will not
/// properly forwarded. See https://users.rust-lang.org/t/mystified-by-downcast-failure/52459 for
/// more details.
pub async unsafe fn luaD_call<A>(
    th: *const Thread<A>,
    func: *mut StackValue<A>,
    nResults: c_int,
) -> Result<(), Box<CallError>> {
    let th = &*th;
    let old_top = func.byte_offset_from_unsigned((*th).stack.get());
    let old_ci = (*th).ci.get();
    let r = match luaD_precall(th, func, nResults).await {
        Ok(ci) => match ci.is_null() {
            true => Ok(()),
            false => {
                (*ci).callstatus = 1 << 2;

                crate::vm::run(th, ci).await
            }
        },
        Err(e) => Err(e),
    };

    match r {
        Ok(_) => {
            let l = th.top.get().offset_from_unsigned(th.stack.get());

            if nResults <= -1 && (*(*th).ci.get()).top.get() < l {
                (*(*th).ci.get()).top = l.try_into().unwrap();
            }

            Ok(())
        }
        Err(e) => {
            let mut r = Err(CallError::new(th, e));

            (*th).ci.set(old_ci);
            r = luaD_closeprotected(th, old_top, r);
            (*th).top.set((*th).stack.get().byte_add(old_top));

            r
        }
    }
}

pub unsafe fn luaD_closeprotected<A>(
    L: &Thread<A>,
    level: usize,
    mut status: Result<(), Box<CallError>>,
) -> Result<(), Box<CallError>> {
    let old_ci = (*L).ci.get();

    loop {
        let e = match luaF_close(L, (*L).stack.get().byte_add(level)) {
            Ok(_) => break status,
            Err(e) => e,
        };

        status = Err(e);

        (*L).ci.set(old_ci);
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

    buff.buffer = luaM_saferealloc_(buff.buffer as *mut c_void, buff.buffsize, 0).cast();

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

#[inline]
pub unsafe fn call_fp<A>(
    L: &Thread<A>,
    func: *mut StackValue<A>,
    nresults: c_int,
    fp: fn(Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn Error>>,
) -> Result<c_int, Box<dyn Error>> {
    // Invoke.
    let top = L.top.get();
    let ci = get_ci(L, func, nresults, 1 << 1, top);
    let narg = top.offset_from_unsigned(func) - 1;
    let cx = Context::new(L, Args::new(narg));
    let cx = fp(cx)?;

    // Get number of results.
    let n = cx.results().try_into().unwrap();

    luaD_poscall(L, ci, n)?;

    Ok(n)
}

pub async unsafe fn call_async_fp<A>(
    L: &Thread<A>,
    func: *mut StackValue<A>,
    nresults: c_int,
    fp: fn(
        Context<A, Args>,
    ) -> Pin<Box<dyn Future<Output = Result<Context<A, Ret>, Box<dyn Error>>> + '_>>,
) -> Result<c_int, Box<dyn Error>> {
    // Invoke.
    let top = L.top.get();
    let ci = get_ci(L, func, nresults, 1 << 1, top);
    let narg = top.offset_from_unsigned(func) - 1;
    let cx = Context::new(L, Args::new(narg));
    let cx = AsyncInvoker { f: fp(cx) }.await?;

    // Get number of results.
    let n = cx.results().try_into().unwrap();

    luaD_poscall(L, ci, n)?;

    Ok(n)
}

#[inline]
unsafe fn get_ci<A>(
    L: *const Thread<A>,
    func: *mut StackValue<A>,
    nret: c_int,
    mask: c_int,
    top: *mut StackValue<A>,
) -> *mut CallInfo {
    let mut ci = (*(*L).ci.get()).next;

    if ci.is_null() {
        ci = luaE_extendCI(L);
    }

    (*L).ci.set(ci);

    (*ci).func = func.offset_from_unsigned((*L).stack.get());
    (*ci).nresults = nret as c_short;
    (*ci).callstatus = mask as c_ushort;
    (*ci).top = top
        .offset_from_unsigned((*L).stack.get())
        .try_into()
        .unwrap();

    ci
}

/// Implementation of [Future] to poll [AsyncFp](crate::AsyncFp).
struct AsyncInvoker<'a, A> {
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

        // Poll.
        self.f.as_mut().poll(cx)
    }
}

/// Implementation of [Future] to call [YieldFp](crate::YieldFp).
struct YieldInvoker<'a, A> {
    th: &'a Thread<A>,
    func: *mut StackValue<A>,
    nresults: c_int,
    fp: fn(Yield<A>) -> Result<Context<A, Ret>, Box<dyn Error>>,
    ci: *mut CallInfo,
}

impl<'a, A> Future for YieldInvoker<'a, A> {
    type Output = Result<c_int, Box<dyn Error>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut core::task::Context<'_>) -> Poll<Self::Output> {
        // Check if yieldable.
        if !addr_eq(cx.waker().vtable(), &YIELDABLE_WAKER) {
            return Poll::Ready(Err("attempt to yield fron non-yieldable context".into()));
        }

        // Check state.
        match self.th.yielding.take() {
            Some(n) => {
                let cx = Context::new(self.th, Resume::new(n));
                let cx = (self.fp)(Yield::Resume(cx))?;
                let n = cx.results().try_into().unwrap();

                unsafe { luaD_poscall(self.th, self.ci, n)? };

                Poll::Ready(Ok(n))
            }
            None => {
                // Invoke.
                let top = self.th.top.get();
                let ci = unsafe { get_ci(self.th, self.func, self.nresults, 1 << 1, top) };
                let narg = unsafe { top.offset_from_unsigned(self.func) - 1 };
                let cx = Context::new(self.th, Args::new(narg));
                let cx = (self.fp)(Yield::Yield(cx))?;
                let ret = cx.results();

                self.th.yielding.set(Some(ret));
                self.ci = ci;

                Poll::Pending
            }
        }
    }
}
