#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments
)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::gc::luaC_barrier_;
use crate::ldebug::{luaG_findlocal, luaG_runerror};
use crate::ldo::luaD_call;
use crate::lmem::luaM_free_;
use crate::lobject::{
    AbsLineInfo, CClosure, LocVar, Proto, StackValue, StkId, UnsafeValue, UpVal, Upvaldesc,
};
use crate::ltm::{TM_CLOSE, luaT_gettmbyobj};
use crate::{ChunkInfo, Lua, LuaFn, Object, Thread};
use alloc::boxed::Box;
use alloc::format;
use alloc::vec::Vec;
use core::alloc::Layout;
use core::cell::Cell;
use core::ffi::CStr;
use core::mem::offset_of;
use core::ptr::{addr_of_mut, null_mut};

pub unsafe fn luaF_newCclosure(g: *const Lua, nupvals: libc::c_int) -> *mut CClosure {
    let nupvals = u8::try_from(nupvals).unwrap();
    let size = offset_of!(CClosure, upvalue) + size_of::<UnsafeValue>() * usize::from(nupvals);
    let align = align_of::<CClosure>();
    let layout = Layout::from_size_align(size, align).unwrap().pad_to_align();
    let o = Object::new(g, 6 | 2 << 4, layout).cast::<CClosure>();

    (*o).nupvalues = nupvals;

    o
}

pub unsafe fn luaF_newLclosure(g: *const Lua, nupvals: libc::c_int) -> *mut LuaFn {
    let nupvals = u8::try_from(nupvals).unwrap();
    let layout = Layout::new::<LuaFn>();
    let o = Object::new(g, 6 | 0 << 4, layout).cast::<LuaFn>();
    let mut upvals = Vec::with_capacity(nupvals.into());

    for _ in 0..nupvals {
        upvals.push(Cell::new(null_mut()));
    }

    addr_of_mut!((*o).p).write(Cell::new(null_mut()));
    addr_of_mut!((*o).upvals).write(upvals.into_boxed_slice());

    o
}

pub unsafe fn luaF_initupvals(g: *const Lua, cl: *const LuaFn) {
    for v in &(*cl).upvals {
        let layout = Layout::new::<UpVal>();
        let uv = Object::new(g, 9 | 0 << 4, layout).cast::<UpVal>();

        (*uv).v.set(&raw mut (*(*uv).u.get()).value);
        (*(*uv).v.get()).tt_ = (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;

        v.set(uv);

        if (*cl).hdr.marked.get() & 1 << 5 != 0 && (*uv).hdr.marked.get() & (1 << 3 | 1 << 4) != 0 {
            luaC_barrier_(g, cl as *mut Object, uv as *mut Object);
        }
    }
}

unsafe fn newupval(L: *const Thread, level: StkId, prev: *mut *mut UpVal) -> *mut UpVal {
    let layout = Layout::new::<UpVal>();
    let uv = Object::new((*L).hdr.global, 9 | 0 << 4, layout).cast::<UpVal>();
    let next: *mut UpVal = *prev;

    (*uv).v.set(&raw mut (*level).val);
    (*(*uv).u.get()).open.next = next;
    (*(*uv).u.get()).open.previous = prev;
    if !next.is_null() {
        (*(*next).u.get()).open.previous = &raw mut (*(*uv).u.get()).open.next;
    }
    *prev = uv;

    if !((*L).twups.get() != L) {
        (*L).twups.set((*(*L).hdr.global).twups.get());
        (*(*L).hdr.global).twups.set(L);
    }

    return uv;
}

pub unsafe fn luaF_findupval(L: *const Thread, level: StkId) -> *mut UpVal {
    let mut pp: *mut *mut UpVal = (*L).openupval.as_ptr();
    let mut p: *mut UpVal = 0 as *mut UpVal;
    loop {
        p = *pp;
        if !(!p.is_null() && (*p).v.get() as StkId >= level) {
            break;
        }
        if (*p).v.get() as StkId == level {
            return p;
        }
        pp = &raw mut (*(*p).u.get()).open.next;
    }
    return newupval(L, level, pp);
}

unsafe fn callclosemethod(
    L: *const Thread,
    obj: *mut UnsafeValue,
    err: *mut UnsafeValue,
) -> Result<(), Box<dyn core::error::Error>> {
    let top: StkId = (*L).top.get();
    let tm: *const UnsafeValue = luaT_gettmbyobj(L, obj, TM_CLOSE);
    let io1: *mut UnsafeValue = &mut (*top).val;
    let io2: *const UnsafeValue = tm;
    (*io1).value_ = (*io2).value_;
    (*io1).tt_ = (*io2).tt_;
    let io1_0: *mut UnsafeValue = &mut (*top.offset(1 as libc::c_int as isize)).val;
    let io2_0: *const UnsafeValue = obj;
    (*io1_0).value_ = (*io2_0).value_;
    (*io1_0).tt_ = (*io2_0).tt_;
    let io1_1: *mut UnsafeValue = &mut (*top.offset(2 as libc::c_int as isize)).val;
    let io2_1: *const UnsafeValue = err;
    (*io1_1).value_ = (*io2_1).value_;
    (*io1_1).tt_ = (*io2_1).tt_;
    (*L).top.set(top.offset(3 as libc::c_int as isize));

    luaD_call(L, top, 0 as libc::c_int)
}

unsafe fn checkclosemth(L: *const Thread, level: StkId) -> Result<(), Box<dyn core::error::Error>> {
    let tm: *const UnsafeValue = luaT_gettmbyobj(L, &mut (*level).val, TM_CLOSE);
    if (*tm).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int {
        let idx: libc::c_int =
            level.offset_from((*(*L).ci.get()).func) as libc::c_long as libc::c_int;
        let mut vname: *const libc::c_char = luaG_findlocal(L, (*L).ci.get(), idx, 0 as *mut StkId);
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

unsafe fn prepcallclosemth(
    L: *const Thread,
    level: StkId,
) -> Result<(), Box<dyn core::error::Error>> {
    let uv: *mut UnsafeValue = &mut (*level).val;
    let errobj = (*(*L).hdr.global).nilvalue.get();

    callclosemethod(L, uv, errobj)
}

pub unsafe fn luaF_newtbcupval(
    L: *const Thread,
    level: StkId,
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

pub unsafe fn luaF_unlinkupval(uv: *mut UpVal) {
    *(*(*uv).u.get()).open.previous = (*(*uv).u.get()).open.next;
    if !((*(*uv).u.get()).open.next).is_null() {
        (*(*(*(*uv).u.get()).open.next).u.get()).open.previous = (*(*uv).u.get()).open.previous;
    }
}

pub unsafe fn luaF_closeupval(L: *const Thread, level: StkId) {
    let mut uv: *mut UpVal = 0 as *mut UpVal;
    let mut upl: StkId = 0 as *mut StackValue;
    loop {
        uv = (*L).openupval.get();
        if !(!uv.is_null() && {
            upl = (*uv).v.get() as StkId;
            upl >= level
        }) {
            break;
        }
        let slot: *mut UnsafeValue = &raw mut (*(*uv).u.get()).value;
        luaF_unlinkupval(uv);
        let io1: *mut UnsafeValue = slot;
        let io2: *const UnsafeValue = (*uv).v.get();
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
                    luaC_barrier_(
                        (*L).hdr.global,
                        uv as *mut Object,
                        (*slot).value_.gc as *mut Object,
                    );
                } else {
                };
            } else {
            };
        }
    }
}

unsafe fn poptbclist(L: *const Thread) {
    let mut tbc: StkId = (*L).tbclist.get();
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

pub unsafe fn luaF_close(
    L: *const Thread,
    mut level: StkId,
) -> Result<StkId, Box<dyn core::error::Error>> {
    let levelrel = (level as *mut libc::c_char).offset_from((*L).stack.get() as *mut libc::c_char);

    luaF_closeupval(L, level);

    while (*L).tbclist.get() >= level {
        let tbc: StkId = (*L).tbclist.get();
        poptbclist(L);
        prepcallclosemth(L, tbc)?;
        level = ((*L).stack.get() as *mut libc::c_char).offset(levelrel as isize) as StkId;
    }

    return Ok(level);
}

pub unsafe fn luaF_newproto(g: *const Lua, chunk: ChunkInfo) -> *mut Proto {
    let layout = Layout::new::<Proto>();
    let f = Object::new(g, 9 + 1 | 0 << 4, layout).cast::<Proto>();

    (*f).k = 0 as *mut UnsafeValue;
    (*f).sizek = 0 as libc::c_int;
    (*f).p = 0 as *mut *mut Proto;
    (*f).sizep = 0 as libc::c_int;
    (*f).code = 0 as *mut u32;
    (*f).sizecode = 0 as libc::c_int;
    (*f).lineinfo = 0 as *mut i8;
    (*f).sizelineinfo = 0 as libc::c_int;
    (*f).abslineinfo = 0 as *mut AbsLineInfo;
    (*f).sizeabslineinfo = 0 as libc::c_int;
    (*f).upvalues = 0 as *mut Upvaldesc;
    (*f).sizeupvalues = 0 as libc::c_int;
    (*f).numparams = 0 as libc::c_int as u8;
    (*f).is_vararg = 0 as libc::c_int as u8;
    (*f).maxstacksize = 0 as libc::c_int as u8;
    (*f).locvars = 0 as *mut LocVar;
    (*f).sizelocvars = 0 as libc::c_int;
    (*f).linedefined = 0 as libc::c_int;
    (*f).lastlinedefined = 0 as libc::c_int;
    addr_of_mut!((*f).chunk).write(chunk);

    return f;
}

pub unsafe fn luaF_freeproto(g: *const Lua, f: *mut Proto) {
    luaM_free_(
        g,
        (*f).code as *mut libc::c_void,
        ((*f).sizecode as usize).wrapping_mul(::core::mem::size_of::<u32>()),
    );
    luaM_free_(
        g,
        (*f).p as *mut libc::c_void,
        ((*f).sizep as usize).wrapping_mul(::core::mem::size_of::<*mut Proto>()),
    );
    luaM_free_(
        g,
        (*f).k as *mut libc::c_void,
        ((*f).sizek as usize).wrapping_mul(::core::mem::size_of::<UnsafeValue>()),
    );
    luaM_free_(
        g,
        (*f).lineinfo as *mut libc::c_void,
        ((*f).sizelineinfo as usize).wrapping_mul(::core::mem::size_of::<i8>()),
    );
    luaM_free_(
        g,
        (*f).abslineinfo as *mut libc::c_void,
        ((*f).sizeabslineinfo as usize).wrapping_mul(::core::mem::size_of::<AbsLineInfo>()),
    );
    luaM_free_(
        g,
        (*f).locvars as *mut libc::c_void,
        ((*f).sizelocvars as usize).wrapping_mul(::core::mem::size_of::<LocVar>()),
    );
    luaM_free_(
        g,
        (*f).upvalues as *mut libc::c_void,
        ((*f).sizeupvalues as usize).wrapping_mul(::core::mem::size_of::<Upvaldesc>()),
    );

    // Free proto.
    let layout = Layout::new::<Proto>();

    core::ptr::drop_in_place(f);
    (*g).gc.dealloc(f.cast(), layout);
}

pub unsafe fn luaF_getlocalname(
    f: *const Proto,
    mut local_number: libc::c_int,
    pc: libc::c_int,
) -> *const libc::c_char {
    let mut i: libc::c_int = 0;
    i = 0 as libc::c_int;
    while i < (*f).sizelocvars && (*((*f).locvars).offset(i as isize)).startpc <= pc {
        if pc < (*((*f).locvars).offset(i as isize)).endpc {
            local_number -= 1;
            if local_number == 0 as libc::c_int {
                return ((*(*((*f).locvars).offset(i as isize)).varname).contents).as_mut_ptr();
            }
        }
        i += 1;
    }
    return 0 as *const libc::c_char;
}
