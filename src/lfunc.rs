#![allow(non_camel_case_types, non_snake_case, unused_assignments)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::ldebug::{luaG_findlocal, luaG_runerror};
use crate::ldo::luaD_call;
use crate::lobject::{AbsLineInfo, CClosure, Proto, StackValue, UpVal};
use crate::ltm::{TM_CLOSE, luaT_gettmbyobj};
use crate::value::UnsafeValue;
use crate::{CallError, ChunkInfo, Lua, LuaFn, NON_YIELDABLE_WAKER, Thread};
use alloc::boxed::Box;
use alloc::format;
use alloc::vec::Vec;
use core::alloc::Layout;
use core::cell::Cell;
use core::ffi::CStr;
use core::mem::offset_of;
use core::pin::pin;
use core::ptr::{addr_of_mut, null, null_mut};
use core::task::{Context, Poll, Waker};

pub unsafe fn luaF_newCclosure<D>(g: *const Lua<D>, nupvals: libc::c_int) -> *mut CClosure<D> {
    let nupvals = u8::try_from(nupvals).unwrap();
    let size =
        offset_of!(CClosure<D>, upvalue) + size_of::<UnsafeValue<D>>() * usize::from(nupvals);
    let align = align_of::<CClosure<D>>();
    let layout = Layout::from_size_align(size, align).unwrap().pad_to_align();
    let o = (*g).gc.alloc(6 | 2 << 4, layout).cast::<CClosure<D>>();

    (*o).nupvalues = nupvals;

    o
}

pub unsafe fn luaF_newLclosure<D>(g: *const Lua<D>, nupvals: libc::c_int) -> *const LuaFn<D> {
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
        (*(*uv).v.get()).tt_ = (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;

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

    (*uv).v.set(&raw mut (*level).val);
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

unsafe fn callclosemethod<D>(
    L: *const Thread<D>,
    obj: *mut UnsafeValue<D>,
    err: *mut UnsafeValue<D>,
) -> Result<(), Box<CallError>> {
    let top = (*L).top.get();
    let tm = luaT_gettmbyobj(L, obj, TM_CLOSE);
    let io1 = &raw mut (*top).val;
    let io2 = tm;
    (*io1).value_ = (*io2).value_;
    (*io1).tt_ = (*io2).tt_;
    let io1_0 = &raw mut (*top.offset(1 as libc::c_int as isize)).val;
    let io2_0 = obj;
    (*io1_0).value_ = (*io2_0).value_;
    (*io1_0).tt_ = (*io2_0).tt_;
    let io1_1 = &raw mut (*top.offset(2 as libc::c_int as isize)).val;
    let io2_1 = err;
    (*io1_1).value_ = (*io2_1).value_;
    (*io1_1).tt_ = (*io2_1).tt_;
    (*L).top.set(top.offset(3 as libc::c_int as isize));

    // Invoke.
    let f = pin!(luaD_call(L, top, 0));
    let w = Waker::new(null(), &NON_YIELDABLE_WAKER);

    match f.poll(&mut Context::from_waker(&w)) {
        Poll::Ready(v) => v,
        Poll::Pending => unreachable!(),
    }
}

unsafe fn checkclosemth<D>(
    L: *const Thread<D>,
    level: *mut StackValue<D>,
) -> Result<(), Box<dyn core::error::Error>> {
    let tm = luaT_gettmbyobj(L, &mut (*level).val, TM_CLOSE);

    if (*tm).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int {
        let idx: libc::c_int =
            level.offset_from((*(*L).ci.get()).func) as libc::c_long as libc::c_int;
        let mut vname: *const libc::c_char = luaG_findlocal(L, (*L).ci.get(), idx, null_mut());
        if vname.is_null() {
            vname = b"?\0" as *const u8 as *const libc::c_char;
        }

        luaG_runerror(
            L,
            format!(
                "variable '{}' got a non-closable value",
                CStr::from_ptr(vname).to_string_lossy()
            ),
        )?;
    }
    Ok(())
}

unsafe fn prepcallclosemth<D>(
    L: *const Thread<D>,
    level: *mut StackValue<D>,
) -> Result<(), Box<CallError>> {
    let uv = &raw mut (*level).val;
    let errobj = (*(*L).hdr.global).nilvalue.get();

    callclosemethod(L, uv, errobj)
}

pub unsafe fn luaF_newtbcupval<D>(
    L: *const Thread<D>,
    level: *mut StackValue<D>,
) -> Result<(), Box<dyn core::error::Error>> {
    if (*level).val.tt_ as libc::c_int == 1 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
        || (*level).val.tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int
    {
        return Ok(());
    }
    checkclosemth(L, level)?;
    while level.offset_from((*L).tbclist.get()) as libc::c_long as libc::c_uint as libc::c_ulong
        > ((256 as libc::c_ulong)
            << (::core::mem::size_of::<libc::c_ushort>() as libc::c_ulong)
                .wrapping_sub(1 as libc::c_int as libc::c_ulong)
                .wrapping_mul(8 as libc::c_int as libc::c_ulong))
        .wrapping_sub(1)
    {
        (*L).tbclist.set(
            ((*L).tbclist.get()).offset(
                ((256 as libc::c_ulong)
                    << (::core::mem::size_of::<libc::c_ushort>() as libc::c_ulong)
                        .wrapping_sub(1 as libc::c_int as libc::c_ulong)
                        .wrapping_mul(8 as libc::c_int as libc::c_ulong))
                .wrapping_sub(1) as isize,
            ),
        );
        (*(*L).tbclist.get()).tbclist.delta = 0 as libc::c_int as libc::c_ushort;
    }

    (*level).tbclist.delta = level.offset_from((*L).tbclist.get()).try_into().unwrap();
    (*L).tbclist.set(level);

    Ok(())
}

pub unsafe fn luaF_unlinkupval<D>(uv: *mut UpVal<D>) {
    *(*(*uv).u.get()).open.previous = (*(*uv).u.get()).open.next;
    if !((*(*uv).u.get()).open.next).is_null() {
        (*(*(*(*uv).u.get()).open.next).u.get()).open.previous = (*(*uv).u.get()).open.previous;
    }
}

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

        if (*uv).hdr.marked.get() as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            == 0
        {
            (*uv).hdr.marked.set((*uv).hdr.marked.get() | 1 << 5);

            if (*slot).tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0 {
                if (*uv).hdr.marked.get() as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int
                    != 0
                    && (*(*slot).value_.gc).marked.get() as libc::c_int
                        & ((1 as libc::c_int) << 3 as libc::c_int
                            | (1 as libc::c_int) << 4 as libc::c_int)
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
    tbc = tbc.offset(-((*tbc).tbclist.delta as libc::c_int as isize));
    while tbc > (*L).stack.get() && (*tbc).tbclist.delta as libc::c_int == 0 as libc::c_int {
        tbc = tbc.offset(
            -(((256 as libc::c_ulong)
                << (::core::mem::size_of::<libc::c_ushort>() as libc::c_ulong)
                    .wrapping_sub(1 as libc::c_int as libc::c_ulong)
                    .wrapping_mul(8 as libc::c_int as libc::c_ulong))
            .wrapping_sub(1 as libc::c_int as libc::c_ulong) as isize),
        );
    }
    (*L).tbclist.set(tbc);
}

pub unsafe fn luaF_close<D>(
    L: *const Thread<D>,
    mut level: *mut StackValue<D>,
) -> Result<*mut StackValue<D>, Box<CallError>> {
    let levelrel = (level as *mut libc::c_char).offset_from((*L).stack.get() as *mut libc::c_char);

    luaF_closeupval(L, level);

    while (*L).tbclist.get() >= level {
        let tbc = (*L).tbclist.get();
        poptbclist(L);
        prepcallclosemth(L, tbc)?;
        level =
            ((*L).stack.get() as *mut libc::c_char).offset(levelrel as isize) as *mut StackValue<D>;
    }

    return Ok(level);
}

pub unsafe fn luaF_newproto<D>(g: *const Lua<D>, chunk: ChunkInfo) -> *mut Proto<D> {
    let layout = Layout::new::<Proto<D>>();
    let f = (*g).gc.alloc(10 | 0 << 4, layout).cast::<Proto<D>>();

    (*f).k = null_mut();
    (*f).sizek = 0 as libc::c_int;
    (*f).p = null_mut();
    (*f).sizep = 0 as libc::c_int;
    (*f).code = 0 as *mut u32;
    (*f).sizecode = 0 as libc::c_int;
    (*f).lineinfo = 0 as *mut i8;
    (*f).sizelineinfo = 0 as libc::c_int;
    (*f).abslineinfo = 0 as *mut AbsLineInfo;
    (*f).sizeabslineinfo = 0 as libc::c_int;
    (*f).upvalues = null_mut();
    (*f).sizeupvalues = 0 as libc::c_int;
    (*f).numparams = 0 as libc::c_int as u8;
    (*f).is_vararg = 0 as libc::c_int as u8;
    (*f).maxstacksize = 0 as libc::c_int as u8;
    (*f).locvars = null_mut();
    (*f).sizelocvars = 0 as libc::c_int;
    (*f).linedefined = 0 as libc::c_int;
    (*f).lastlinedefined = 0 as libc::c_int;
    addr_of_mut!((*f).chunk).write(chunk);

    return f;
}

pub unsafe fn luaF_getlocalname<D>(
    f: *const Proto<D>,
    mut local_number: libc::c_int,
    pc: libc::c_int,
) -> *const libc::c_char {
    let mut i: libc::c_int = 0;
    i = 0 as libc::c_int;
    while i < (*f).sizelocvars && (*((*f).locvars).offset(i as isize)).startpc <= pc {
        if pc < (*((*f).locvars).offset(i as isize)).endpc {
            local_number -= 1;
            if local_number == 0 as libc::c_int {
                return ((*(*((*f).locvars).offset(i as isize)).varname).contents).as_ptr();
            }
        }
        i += 1;
    }
    return 0 as *const libc::c_char;
}
