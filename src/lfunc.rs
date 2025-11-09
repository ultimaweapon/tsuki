#![allow(non_camel_case_types, non_snake_case, unused_assignments)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::ldebug::luaG_findlocal;
use crate::ldo::luaD_call;
use crate::lobject::{AbsLineInfo, CClosure, Proto, UpVal};
use crate::ltm::{TM_CLOSE, luaT_gettmbyobj};
use crate::value::UnsafeValue;
use crate::{CallError, ChunkInfo, Lua, LuaFn, NON_YIELDABLE_WAKER, StackValue, Thread};
use alloc::boxed::Box;
use alloc::format;
use alloc::vec::Vec;
use core::alloc::Layout;
use core::cell::Cell;
use core::ffi::{CStr, c_char};
use core::mem::offset_of;
use core::pin::pin;
use core::ptr::{addr_of_mut, null, null_mut};
use core::task::{Context, Poll, Waker};

type c_ushort = u16;
type c_int = i32;
type c_uint = u32;
type c_long = i64;
type c_ulong = u64;

pub unsafe fn luaF_newCclosure<D>(g: *const Lua<D>, nupvals: c_int) -> *mut CClosure<D> {
    let nupvals = u8::try_from(nupvals).unwrap();
    let size =
        offset_of!(CClosure<D>, upvalue) + size_of::<UnsafeValue<D>>() * usize::from(nupvals);
    let align = align_of::<CClosure<D>>();
    let layout = Layout::from_size_align(size, align).unwrap().pad_to_align();
    let o = (*g).gc.alloc(6 | 2 << 4, layout).cast::<CClosure<D>>();

    (*o).nupvalues = nupvals;

    o
}

pub unsafe fn luaF_newLclosure<D>(g: *const Lua<D>, nupvals: c_int) -> *const LuaFn<D> {
    let nupvals = u8::try_from(nupvals).unwrap();
    let layout = Layout::new::<LuaFn<D>>();
    let o = (*g).gc.alloc(6 | 0 << 4, layout).cast::<LuaFn<D>>();
    let mut upvals = Vec::with_capacity(nupvals.into());

    for _ in 0..nupvals {
        upvals.push(Cell::new(null_mut()));
    }

    addr_of_mut!((*o).p).write(Cell::new(null_mut()));
    addr_of_mut!((*o).upvals).write(upvals.into_boxed_slice());

    o
}

pub unsafe fn luaF_initupvals<D>(g: *const Lua<D>, cl: *const LuaFn<D>) {
    for v in &(*cl).upvals {
        let layout = Layout::new::<UpVal<D>>();
        let uv = (*g).gc.alloc(9 | 0 << 4, layout).cast::<UpVal<D>>();

        (*uv).v.set(&raw mut (*(*uv).u.get()).value);
        (*(*uv).v.get()).tt_ = (0 as c_int | (0 as c_int) << 4 as c_int) as u8;

        v.set(uv);

        if (*cl).hdr.marked.get() & 1 << 5 != 0 && (*uv).hdr.marked.get() & (1 << 3 | 1 << 4) != 0 {
            (*g).gc.barrier(cl.cast(), uv.cast());
        }
    }
}

unsafe fn newupval<D>(
    L: *const Thread<D>,
    level: *mut StackValue<D>,
    prev: *mut *mut UpVal<D>,
) -> *mut UpVal<D> {
    let layout = Layout::new::<UpVal<D>>();
    let uv = (*L)
        .hdr
        .global()
        .gc
        .alloc(9 | 0 << 4, layout)
        .cast::<UpVal<D>>();
    let next = *prev;

    (*uv).v.set(level.cast());
    (*(*uv).u.get()).open.next = next;
    (*(*uv).u.get()).open.previous = prev;
    if !next.is_null() {
        (*(*next).u.get()).open.previous = &raw mut (*(*uv).u.get()).open.next;
    }
    *prev = uv;

    if !((*L).twups.get() != L) {
        (*L).hdr.global().gc.set_twups(L);
    }

    return uv;
}

pub unsafe fn luaF_findupval<D>(L: *const Thread<D>, level: *mut StackValue<D>) -> *mut UpVal<D> {
    let mut pp = (*L).openupval.as_ptr();

    loop {
        let p = *pp;
        if !(!p.is_null() && (*p).v.get() as *mut StackValue<D> >= level) {
            break;
        }
        if (*p).v.get() as *mut StackValue<D> == level {
            return p;
        }
        pp = &raw mut (*(*p).u.get()).open.next;
    }
    return newupval(L, level, pp);
}

unsafe fn callclosemethod<A>(
    L: &Thread<A>,
    obj: *const StackValue<A>,
) -> Result<(), Box<CallError>> {
    let top = (*L).top.get();
    let tm = luaT_gettmbyobj(L, obj.cast(), TM_CLOSE);
    let io1 = top;
    let io2 = tm;
    (*io1).value_ = (*io2).value_;
    (*io1).tt_ = (*io2).tt_;
    let io1_0 = top.offset(1 as c_int as isize);
    let io2_0 = obj;
    (*io1_0).value_ = (*io2_0).value_;
    (*io1_0).tt_ = (*io2_0).tt_;
    let io1_1 = top.offset(2 as c_int as isize);
    (*io1_1).tt_ = 0 | 0 << 4;
    (*L).top.set(top.offset(3 as c_int as isize));

    // Invoke.
    let f = pin!(luaD_call(L, top, 0));
    let w = Waker::new(null(), &NON_YIELDABLE_WAKER);

    match f.poll(&mut Context::from_waker(&w)) {
        Poll::Ready(v) => v,
        Poll::Pending => unreachable!(),
    }
}

#[inline(never)]
unsafe fn checkclosemth<A>(
    L: *const Thread<A>,
    level: *mut StackValue<A>,
) -> Result<(), Box<dyn core::error::Error>> {
    let tm = luaT_gettmbyobj(L, level.cast(), TM_CLOSE);

    if (*tm).tt_ as c_int & 0xf as c_int == 0 as c_int {
        let f = (*L).stack.get().add((*(*L).ci.get()).func);
        let idx: c_int = level.offset_from(f) as c_long as c_int;
        let mut vname: *const c_char = luaG_findlocal(L, (*L).ci.get(), idx, null_mut());
        if vname.is_null() {
            vname = b"?\0" as *const u8 as *const c_char;
        }

        return Err(format!(
            "variable '{}' got a non-closable value",
            CStr::from_ptr(vname).to_string_lossy()
        )
        .into());
    }
    Ok(())
}

#[inline(always)]
pub unsafe fn luaF_newtbcupval<D>(
    L: *const Thread<D>,
    level: *mut StackValue<D>,
) -> Result<(), Box<dyn core::error::Error>> {
    if (*level).tt_ as c_int == 1 as c_int | (0 as c_int) << 4 as c_int
        || (*level).tt_ as c_int & 0xf as c_int == 0 as c_int
    {
        return Ok(());
    }
    checkclosemth(L, level)?;
    while level.offset_from((*L).tbclist.get()) as c_long as c_uint as c_ulong
        > ((256 as c_ulong)
            << (::core::mem::size_of::<c_ushort>() as c_ulong)
                .wrapping_sub(1 as c_int as c_ulong)
                .wrapping_mul(8 as c_int as c_ulong))
        .wrapping_sub(1)
    {
        (*L).tbclist.set(
            ((*L).tbclist.get()).offset(
                ((256 as c_ulong)
                    << (::core::mem::size_of::<c_ushort>() as c_ulong)
                        .wrapping_sub(1 as c_int as c_ulong)
                        .wrapping_mul(8 as c_int as c_ulong))
                .wrapping_sub(1) as isize,
            ),
        );
        (*(*L).tbclist.get()).tbcdelta = 0 as c_int as c_ushort;
    }

    (*level).tbcdelta = level.offset_from((*L).tbclist.get()).try_into().unwrap();
    (*L).tbclist.set(level);

    Ok(())
}

#[inline(always)]
pub unsafe fn luaF_unlinkupval<D>(uv: *mut UpVal<D>) {
    *(*(*uv).u.get()).open.previous = (*(*uv).u.get()).open.next;
    if !((*(*uv).u.get()).open.next).is_null() {
        (*(*(*(*uv).u.get()).open.next).u.get()).open.previous = (*(*uv).u.get()).open.previous;
    }
}

#[inline(always)]
pub unsafe fn luaF_closeupval<D>(L: *const Thread<D>, level: *mut StackValue<D>) {
    let mut upl = null_mut();

    loop {
        let uv = (*L).openupval.get();
        if !(!uv.is_null() && {
            upl = (*uv).v.get() as *mut StackValue<D>;
            upl >= level
        }) {
            break;
        }
        let slot = &raw mut (*(*uv).u.get()).value;
        luaF_unlinkupval(uv);
        let io1 = slot;
        let io2 = (*uv).v.get();
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        (*uv).v.set(slot);

        if (*uv).hdr.marked.get() as c_int
            & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
            == 0
        {
            (*uv).hdr.marked.set((*uv).hdr.marked.get() | 1 << 5);

            if (*slot).tt_ as c_int & (1 as c_int) << 6 as c_int != 0 {
                if (*uv).hdr.marked.get() as c_int & (1 as c_int) << 5 as c_int != 0
                    && (*(*slot).value_.gc).marked.get() as c_int
                        & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                        != 0
                {
                    (*L).hdr.global().gc.barrier(uv.cast(), (*slot).value_.gc);
                }
            }
        }
    }
}

unsafe fn poptbclist<D>(L: *const Thread<D>) {
    let mut tbc = (*L).tbclist.get();
    tbc = tbc.offset(-((*tbc).tbcdelta as c_int as isize));
    while tbc > (*L).stack.get() && (*tbc).tbcdelta as c_int == 0 as c_int {
        tbc = tbc.offset(
            -(((256 as c_ulong)
                << (::core::mem::size_of::<c_ushort>() as c_ulong)
                    .wrapping_sub(1 as c_int as c_ulong)
                    .wrapping_mul(8 as c_int as c_ulong))
            .wrapping_sub(1 as c_int as c_ulong) as isize),
        );
    }
    (*L).tbclist.set(tbc);
}

#[inline(never)]
pub unsafe fn luaF_close<A>(
    L: &Thread<A>,
    mut level: *mut StackValue<A>,
) -> Result<*mut StackValue<A>, Box<CallError>> {
    let levelrel = level.offset_from_unsigned((*L).stack.get());

    luaF_closeupval(L, level);

    while (*L).tbclist.get() >= level {
        let tbc = (*L).tbclist.get();
        poptbclist(L);
        callclosemethod(L, tbc)?;
        level = (*L).stack.get().add(levelrel);
    }

    return Ok(level);
}

pub unsafe fn luaF_newproto<D>(g: *const Lua<D>, chunk: ChunkInfo) -> *mut Proto<D> {
    let layout = Layout::new::<Proto<D>>();
    let f = (*g).gc.alloc(10 | 0 << 4, layout).cast::<Proto<D>>();

    (*f).k = null_mut();
    (*f).sizek = 0 as c_int;
    (*f).p = null_mut();
    (*f).sizep = 0 as c_int;
    (*f).code = 0 as *mut u32;
    (*f).sizecode = 0 as c_int;
    (*f).lineinfo = 0 as *mut i8;
    (*f).sizelineinfo = 0 as c_int;
    (*f).abslineinfo = 0 as *mut AbsLineInfo;
    (*f).sizeabslineinfo = 0 as c_int;
    (*f).upvalues = null_mut();
    (*f).sizeupvalues = 0 as c_int;
    (*f).numparams = 0 as c_int as u8;
    (*f).is_vararg = 0 as c_int as u8;
    (*f).maxstacksize = 0 as c_int as u8;
    (*f).locvars = null_mut();
    (*f).sizelocvars = 0 as c_int;
    (*f).linedefined = 0 as c_int;
    (*f).lastlinedefined = 0 as c_int;
    addr_of_mut!((*f).chunk).write(chunk);

    return f;
}

pub unsafe fn luaF_getlocalname<D>(
    f: *const Proto<D>,
    mut local_number: c_int,
    pc: c_int,
) -> *const c_char {
    let mut i: c_int = 0;
    i = 0 as c_int;
    while i < (*f).sizelocvars && (*((*f).locvars).offset(i as isize)).startpc <= pc {
        if pc < (*((*f).locvars).offset(i as isize)).endpc {
            local_number -= 1;
            if local_number == 0 as c_int {
                return ((*(*((*f).locvars).offset(i as isize)).varname).contents).as_ptr();
            }
        }
        i += 1;
    }
    return 0 as *const c_char;
}
