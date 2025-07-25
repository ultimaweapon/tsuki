#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments
)]
#![allow(unsafe_op_in_unsafe_fn)]

pub(crate) use self::mark::*;
pub(crate) use self::object::*;
pub use self::r#ref::*;

use crate::ldo::luaD_shrinkstack;
use crate::lfunc::{luaF_freeproto, luaF_unlinkupval};
use crate::lobject::{CClosure, Proto, StkId, Udata, UpVal};
use crate::ltm::{TM_MODE, luaT_gettm};
use crate::table::{luaH_free, luaH_realasize};
use crate::value::UnsafeValue;
use crate::{Lua, LuaFn, Node, Str, Table, Thread, UserId};
use core::alloc::Layout;
use core::cell::Cell;
use core::mem::offset_of;
use core::ptr::{null, null_mut};

mod mark;
mod object;
mod r#ref;

type c_int = i32;
type c_uint = u32;
type c_long = i64;
type c_ulong = u64;

/// # Safety
/// After this function return any unreachable objects may be freed.
pub(crate) unsafe fn step(g: *const Lua) {
    if !((*g).gcstp.get() == 0) {
        (*g).gc.set_debt(-2000);
    } else {
        incstep(&*g);
    }
}

unsafe fn restart(g: &Lua) {
    g.reset_gray();
    mark_roots(g);
}

unsafe fn mark_roots(g: &Lua) {
    // Mark object with strong references.
    let mut o = g.refs.get();

    while !o.is_null() {
        if ((*o).marked.get() & (1 << 3 | 1 << 4)) != 0 {
            reallymarkobject(g, o);
        }

        o = (*o).refp.get();
    }

    // Mark registry.
    if (*(*g.l_registry.get()).value_.gc).marked.get() & (1 << 3 | 1 << 4) != 0 {
        reallymarkobject(g, (*g.l_registry.get()).value_.gc);
    }

    // Mark primitive metatable.
    for mt in g
        .primitive_mt
        .iter()
        .map(|v| v.get())
        .filter(|v| !v.is_null())
    {
        if (*mt).hdr.marked.get() & (1 << 3 | 1 << 4) != 0 {
            reallymarkobject(g, mt.cast());
        }
    }
}

#[inline(always)]
unsafe fn getgclist(o: *const Object) -> *mut *const Object {
    match (*o).tt {
        5 | 6 | 7 | 8 | 10 | 38 => (*o).gclist.as_ptr(),
        _ => null_mut(),
    }
}

unsafe fn linkgclist_(o: *const Object, pnext: *mut *const Object, list: *mut *const Object) {
    *pnext = *list;
    *list = o;

    (*o).marked.set_gray();
}

unsafe fn clearkey(n: *mut Node) {
    if (*n).u.key_tt as c_int & (1 as c_int) << 6 as c_int != 0 {
        (*n).u.key_tt = (9 as c_int + 2 as c_int) as u8;
    }
}

unsafe fn iscleared(g: *const Lua, o: *const Object) -> c_int {
    if o.is_null() {
        return 0 as c_int;
    } else if (*o).tt as c_int & 0xf as c_int == 4 as c_int {
        if (*o).marked.get() as c_int & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
            != 0
        {
            reallymarkobject(g, o as *mut Object);
        }
        return 0 as c_int;
    } else {
        return (*o).marked.get() as c_int
            & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int);
    };
}

pub(crate) unsafe fn luaC_barrier_(g: *const Lua, o: *const Object, v: *const Object) {
    if (*g).gcstate.get() <= 2 {
        reallymarkobject(g, v);
        if (*o).marked.get() as c_int & 7 as c_int > 1 as c_int {
            (*v).marked
                .set(((*v).marked.get() as c_int & !(7 as c_int) | 2) as u8);
        }
    } else {
        (*o).marked.set(
            (*o).marked.get() & !(1 << 5 | (1 << 3 | 1 << 4))
                | ((*g).currentwhite.get() & (1 << 3 | 1 << 4)),
        );
    }
}

pub(crate) unsafe fn luaC_barrierback_(o: *const Object) {
    let g = (*o).global;

    if (*o).marked.get() as c_int & 7 as c_int == 6 as c_int {
        (*o).marked
            .set((*o).marked.get() & !(1 << 5 | (1 << 3 | 1 << 4)));
    } else {
        linkgclist_(o, getgclist(o), (*g).grayagain.as_ptr());
    }

    if (*o).marked.get() as c_int & 7 as c_int > 1 as c_int {
        (*o).marked
            .set(((*o).marked.get() as c_int & !(7 as c_int) | 5 as c_int) as u8);
    }
}

pub(crate) unsafe fn luaC_fix(g: &Lua, o: *const Object) {
    (*o).marked
        .set((*o).marked.get() & !(1 << 5 | (1 << 3 | 1 << 4)));
    (*o).marked
        .set(((*o).marked.get() as c_int & !(7 as c_int) | 4 as c_int) as u8);
    g.all.set((*o).next.get());
    (*o).next.set((*g).fixedgc.get());
    (*g).fixedgc.set(o);
}

unsafe fn reallymarkobject(g: *const Lua, o: *const Object) {
    match (*o).tt {
        4 | 20 | 11 => {
            (*o).marked
                .set((*o).marked.get() & !(1 << 3 | 1 << 4) | 1 << 5);
            return;
        }
        9 => {
            let uv = o as *const UpVal;

            if (*uv).v.get() != &raw mut (*(*uv).u.get()).value as *mut UnsafeValue {
                (*uv)
                    .hdr
                    .marked
                    .set((*uv).hdr.marked.get() & !(1 << 5 | (1 << 3 | 1 << 4)));
            } else {
                (*uv)
                    .hdr
                    .marked
                    .set((*uv).hdr.marked.get() & !(1 << 3 | 1 << 4) | 1 << 5);
            }

            if (*(*uv).v.get()).tt_ & 1 << 6 != 0
                && (*(*(*uv).v.get()).value_.gc).marked.get() & (1 << 3 | 1 << 4) != 0
            {
                reallymarkobject(g, (*(*uv).v.get()).value_.gc);
            }

            return;
        }
        7 => {
            let u = o as *const Udata;

            if (*u).nuvalue == 0 {
                if !((*u).metatable).is_null() {
                    if (*(*u).metatable).hdr.marked.get() as c_int
                        & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                        != 0
                    {
                        reallymarkobject(g, (*u).metatable as *mut Object);
                    }
                }
                (*u).hdr
                    .marked
                    .set((*u).hdr.marked.get() & !(1 << 3 | 1 << 4) | 1 << 5);
                return;
            }
        }
        6 | 38 | 5 | 8 | 10 => {}
        _ => return,
    }

    linkgclist_(o, getgclist(o), (*g).gray.as_ptr());
}

unsafe fn remarkupvals(g: *const Lua) -> c_int {
    let mut p = (*g).twups.as_ptr();
    let mut work: c_int = 0 as c_int;

    loop {
        let thread = *p;

        if thread.is_null() {
            break;
        }

        work += 1;

        if (*thread).hdr.marked.get() & (1 << 3 | 1 << 4) == 0
            && !(*thread).openupval.get().is_null()
        {
            p = (*thread).twups.as_ptr();
        } else {
            let mut uv: *mut UpVal = 0 as *mut UpVal;
            *p = (*thread).twups.get();
            (*thread).twups.set(thread);
            uv = (*thread).openupval.get();
            while !uv.is_null() {
                work += 1;
                if (*uv).hdr.marked.get() as c_int
                    & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                    == 0
                {
                    if (*(*uv).v.get()).tt_ as c_int & (1 as c_int) << 6 as c_int != 0
                        && (*(*(*uv).v.get()).value_.gc).marked.get() as c_int
                            & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                            != 0
                    {
                        reallymarkobject(g, (*(*uv).v.get()).value_.gc);
                    }
                }
                uv = (*(*uv).u.get()).open.next;
            }
        }
    }

    return work;
}

unsafe fn genlink(g: *const Lua, o: *const Object) {
    if (*o).marked.get() as c_int & 7 as c_int == 5 as c_int {
        linkgclist_(o, getgclist(o), (*g).grayagain.as_ptr());
    } else if (*o).marked.get() as c_int & 7 as c_int == 6 as c_int {
        (*o).marked.set((*o).marked.get() ^ (6 ^ 4));
    }
}

unsafe fn traverseweakvalue(g: *const Lua, h: *const Table) {
    let mut n: *mut Node = 0 as *mut Node;
    let limit: *mut Node = ((*h).node.get())
        .offset(((1 as c_int) << (*h).lsizenode.get() as c_int) as usize as isize)
        as *mut Node;
    let mut hasclears: c_int = ((*h).alimit.get() > 0 as c_int as c_uint) as c_int;
    n = ((*h).node.get()).offset(0 as c_int as isize) as *mut Node;
    while n < limit {
        if (*n).i_val.tt_ as c_int & 0xf as c_int == 0 as c_int {
            clearkey(n);
        } else {
            if (*n).u.key_tt as c_int & (1 as c_int) << 6 as c_int != 0
                && (*(*n).u.key_val.gc).marked.get() as c_int
                    & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                    != 0
            {
                reallymarkobject(g, (*n).u.key_val.gc);
            }
            if hasclears == 0
                && iscleared(
                    g,
                    if (*n).i_val.tt_ as c_int & (1 as c_int) << 6 as c_int != 0 {
                        (*n).i_val.value_.gc
                    } else {
                        0 as *mut Object
                    },
                ) != 0
            {
                hasclears = 1 as c_int;
            }
        }
        n = n.offset(1);
    }
    if (*g).gcstate.get() == 2 && hasclears != 0 {
        linkgclist_(
            h as *const Object,
            (*h).hdr.gclist.as_ptr(),
            (*g).weak.as_ptr(),
        );
    } else {
        linkgclist_(
            h as *const Object,
            (*h).hdr.gclist.as_ptr(),
            (*g).grayagain.as_ptr(),
        );
    };
}

unsafe fn traverseephemeron(g: *const Lua, h: *const Table, inv: c_int) -> c_int {
    let mut marked: c_int = 0 as c_int;
    let mut hasclears: c_int = 0 as c_int;
    let mut hasww: c_int = 0 as c_int;
    let mut i: c_uint = 0;
    let asize: c_uint = luaH_realasize(h);
    let nsize: c_uint = ((1 as c_int) << (*h).lsizenode.get() as c_int) as c_uint;
    i = 0 as c_int as c_uint;
    while i < asize {
        if (*(*h).array.get().offset(i as isize)).tt_ as c_int & (1 as c_int) << 6 as c_int != 0
            && (*(*(*h).array.get().offset(i as isize)).value_.gc)
                .marked
                .get() as c_int
                & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                != 0
        {
            marked = 1 as c_int;
            reallymarkobject(g, (*(*h).array.get().offset(i as isize)).value_.gc);
        }
        i = i.wrapping_add(1);
    }
    i = 0 as c_int as c_uint;
    while i < nsize {
        let n: *mut Node = if inv != 0 {
            ((*h).node.get())
                .offset(nsize.wrapping_sub(1 as c_int as c_uint).wrapping_sub(i) as isize)
                as *mut Node
        } else {
            ((*h).node.get()).offset(i as isize) as *mut Node
        };
        if (*n).i_val.tt_ as c_int & 0xf as c_int == 0 as c_int {
            clearkey(n);
        } else if iscleared(
            g,
            if (*n).u.key_tt as c_int & (1 as c_int) << 6 as c_int != 0 {
                (*n).u.key_val.gc
            } else {
                0 as *mut Object
            },
        ) != 0
        {
            hasclears = 1 as c_int;
            if (*n).i_val.tt_ as c_int & (1 as c_int) << 6 as c_int != 0
                && (*(*n).i_val.value_.gc).marked.get() as c_int
                    & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                    != 0
            {
                hasww = 1 as c_int;
            }
        } else if (*n).i_val.tt_ as c_int & (1 as c_int) << 6 as c_int != 0
            && (*(*n).i_val.value_.gc).marked.get() as c_int
                & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                != 0
        {
            marked = 1 as c_int;
            reallymarkobject(g, (*n).i_val.value_.gc);
        }
        i = i.wrapping_add(1);
    }
    if (*g).gcstate.get() == 0 {
        linkgclist_(
            h as *const Object,
            (*h).hdr.gclist.as_ptr(),
            (*g).grayagain.as_ptr(),
        );
    } else if hasww != 0 {
        linkgclist_(
            h as *const Object,
            (*h).hdr.gclist.as_ptr(),
            (*g).ephemeron.as_ptr(),
        );
    } else if hasclears != 0 {
        linkgclist_(
            h as *const Object,
            (*h).hdr.gclist.as_ptr(),
            (*g).allweak.as_ptr(),
        );
    } else {
        genlink(g, h as *const Object);
    }
    return marked;
}

unsafe fn traversestrongtable(g: *const Lua, h: *const Table) {
    let mut n: *mut Node = 0 as *mut Node;
    let limit: *mut Node = ((*h).node.get())
        .offset(((1 as c_int) << (*h).lsizenode.get() as c_int) as usize as isize)
        as *mut Node;
    let mut i: c_uint = 0;
    let asize: c_uint = luaH_realasize(h);
    i = 0 as c_int as c_uint;
    while i < asize {
        if (*(*h).array.get().offset(i as isize)).tt_ as c_int & (1 as c_int) << 6 as c_int != 0
            && (*(*(*h).array.get().offset(i as isize)).value_.gc)
                .marked
                .get() as c_int
                & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                != 0
        {
            reallymarkobject(g, (*(*h).array.get().offset(i as isize)).value_.gc);
        }
        i = i.wrapping_add(1);
    }
    n = ((*h).node.get()).offset(0 as c_int as isize) as *mut Node;
    while n < limit {
        if (*n).i_val.tt_ as c_int & 0xf as c_int == 0 as c_int {
            clearkey(n);
        } else {
            if (*n).u.key_tt as c_int & (1 as c_int) << 6 as c_int != 0
                && (*(*n).u.key_val.gc).marked.get() as c_int
                    & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                    != 0
            {
                reallymarkobject(g, (*n).u.key_val.gc);
            }
            if (*n).i_val.tt_ as c_int & (1 as c_int) << 6 as c_int != 0
                && (*(*n).i_val.value_.gc).marked.get() as c_int
                    & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                    != 0
            {
                reallymarkobject(g, (*n).i_val.value_.gc);
            }
        }
        n = n.offset(1);
    }
    genlink(g, h as *const Object);
}

unsafe fn traversetable(g: *const Lua, h: *const Table) -> usize {
    // Get table mode.
    let mode = if (*h).metatable.get().is_null() {
        null()
    } else if (*(*h).metatable.get()).flags.get() & 1 << TM_MODE != 0 {
        null()
    } else {
        let s = luaT_gettm(
            (*h).metatable.get(),
            TM_MODE,
            (*g).tmname[TM_MODE as usize].get(),
        );

        if !s.is_null() && (*s).tt_ == 4 | 0 << 4 | 1 << 6 {
            (*s).value_.gc.cast::<Str>()
        } else {
            null()
        }
    };

    // Mark metatable.
    if !((*h).metatable.get()).is_null() {
        if (*(*h).metatable.get()).hdr.marked.get() as c_int
            & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
            != 0
        {
            reallymarkobject(g, (*h).metatable.get().cast());
        }
    }

    // Traverse table.
    let (wk, wv) = match mode.as_ref().map(|v| v.as_bytes()) {
        Some(v) => (v.contains(&b'k'), v.contains(&b'v')),
        None => (false, false),
    };

    match (wk, wv) {
        (true, true) => linkgclist_(
            h as *mut Object,
            (*h).hdr.gclist.as_ptr(),
            (*g).allweak.as_ptr(),
        ),
        (true, false) => {
            traverseephemeron(g, h, 0);
        }
        (false, true) => traverseweakvalue(g, h),
        (false, false) => traversestrongtable(g, h),
    }

    return (1 as c_int as c_uint)
        .wrapping_add((*h).alimit.get())
        .wrapping_add(
            (2 as c_int
                * (if ((*h).lastfree.get()).is_null() {
                    0 as c_int
                } else {
                    (1 as c_int) << (*h).lsizenode.get() as c_int
                })) as c_uint,
        ) as usize;
}

unsafe fn traverseudata(g: *const Lua, u: *const Udata) -> c_int {
    let mut i: c_int = 0;
    if !((*u).metatable).is_null() {
        if (*(*u).metatable).hdr.marked.get() as c_int
            & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
            != 0
        {
            reallymarkobject(g, (*u).metatable as *const Object);
        }
    }
    i = 0 as c_int;

    while i < (*u).nuvalue as c_int {
        if (*((*u).uv).as_ptr().offset(i as isize)).tt_ & 1 << 6 != 0
            && (*(*((*u).uv).as_ptr().offset(i as isize)).value_.gc)
                .marked
                .get() as c_int
                & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                != 0
        {
            reallymarkobject(g, (*((*u).uv).as_ptr().offset(i as isize)).value_.gc);
        }

        i += 1;
    }

    genlink(g, u as *const Object);
    return 1 as c_int + (*u).nuvalue as c_int;
}

unsafe fn traverseproto(g: *const Lua, f: *const Proto) -> c_int {
    let mut i = 0 as c_int;

    while i < (*f).sizek {
        if (*((*f).k).offset(i as isize)).tt_ as c_int & (1 as c_int) << 6 as c_int != 0
            && (*(*((*f).k).offset(i as isize)).value_.gc).marked.get() as c_int
                & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                != 0
        {
            reallymarkobject(g, (*((*f).k).offset(i as isize)).value_.gc);
        }
        i += 1;
    }

    i = 0 as c_int;
    while i < (*f).sizeupvalues {
        if !((*((*f).upvalues).offset(i as isize)).name).is_null() {
            if (*(*((*f).upvalues).offset(i as isize)).name)
                .hdr
                .marked
                .get() as c_int
                & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                != 0
            {
                reallymarkobject(
                    g,
                    (*((*f).upvalues).offset(i as isize)).name as *const Object,
                );
            }
        }
        i += 1;
    }
    i = 0 as c_int;
    while i < (*f).sizep {
        if !(*((*f).p).offset(i as isize)).is_null() {
            if (**((*f).p).offset(i as isize)).hdr.marked.get() as c_int
                & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                != 0
            {
                reallymarkobject(g, *((*f).p).offset(i as isize) as *const Object);
            }
        }
        i += 1;
    }
    i = 0 as c_int;
    while i < (*f).sizelocvars {
        if !((*((*f).locvars).offset(i as isize)).varname).is_null() {
            if (*(*((*f).locvars).offset(i as isize)).varname)
                .hdr
                .marked
                .get() as c_int
                & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                != 0
            {
                reallymarkobject(
                    g,
                    (*((*f).locvars).offset(i as isize)).varname as *const Object,
                );
            }
        }
        i += 1;
    }
    return 1 as c_int + (*f).sizek + (*f).sizeupvalues + (*f).sizep + (*f).sizelocvars;
}

unsafe fn traverseCclosure(g: *const Lua, cl: *const CClosure) -> c_int {
    let mut i: c_int = 0;
    i = 0 as c_int;
    while i < (*cl).nupvalues as c_int {
        if (*((*cl).upvalue).as_ptr().offset(i as isize)).tt_ as c_int & (1 as c_int) << 6 as c_int
            != 0
            && (*(*((*cl).upvalue).as_ptr().offset(i as isize)).value_.gc)
                .marked
                .get() as c_int
                & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                != 0
        {
            reallymarkobject(g, (*((*cl).upvalue).as_ptr().offset(i as isize)).value_.gc);
        }
        i += 1;
    }
    return 1 as c_int + (*cl).nupvalues as c_int;
}

unsafe fn traverseLclosure(g: &Lua, cl: *const LuaFn) -> usize {
    let p = (*cl).p.get();

    if !p.is_null() && (*p).hdr.marked.get() & (1 << 3 | 1 << 4) != 0 {
        reallymarkobject(g, p.cast());
    }

    for uv in (*cl)
        .upvals
        .iter()
        .map(|v| v.get())
        .filter(|v| !v.is_null())
    {
        if (*uv).hdr.marked.get() as c_int & ((1 as c_int) << 3 | 1 << 4) != 0 {
            reallymarkobject(g, uv.cast());
        }
    }

    1 + (&(*cl).upvals).len()
}

unsafe fn traversethread(g: *const Lua, th: *const Thread) -> c_int {
    let mut uv: *mut UpVal = 0 as *mut UpVal;
    let mut o: StkId = (*th).stack.get();
    if (*th).hdr.marked.get() & 7 > 1 || (*g).gcstate.get() == 0 {
        linkgclist_(
            th as *const Object,
            (*th).hdr.gclist.as_ptr(),
            (*g).grayagain.as_ptr(),
        );
    }
    if o.is_null() {
        return 1 as c_int;
    }
    while o < (*th).top.get() {
        if (*o).val.tt_ as c_int & (1 as c_int) << 6 as c_int != 0
            && (*(*o).val.value_.gc).marked.get() as c_int
                & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                != 0
        {
            reallymarkobject(g, (*o).val.value_.gc);
        }
        o = o.offset(1);
    }
    uv = (*th).openupval.get();
    while !uv.is_null() {
        if (*uv).hdr.marked.get() as c_int
            & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
            != 0
        {
            reallymarkobject(g, uv as *mut Object);
        }
        uv = (*(*uv).u.get()).open.next;
    }

    if (*g).gcstate.get() == 2 {
        luaD_shrinkstack(th);

        o = (*th).top.get();
        while o < ((*th).stack_last.get()).offset(5 as c_int as isize) {
            (*o).val.tt_ = (0 as c_int | (0 as c_int) << 4 as c_int) as u8;
            o = o.offset(1);
        }
        if !((*th).twups.get() != th) && !((*th).openupval.get()).is_null() {
            (*th).twups.set((*g).twups.get());
            (*g).twups.set(th);
        }
    }

    1 + ((*th).stack_last.get()).offset_from((*th).stack.get()) as c_long as c_int
}

unsafe fn propagatemark(g: &Lua) -> usize {
    let o = g.gray.get();

    (*o).marked.set((*o).marked.get() | 1 << 5);
    (*g).gray.set(*getgclist(o));

    match (*o).tt {
        5 => traversetable(g, o as *const Table),
        7 => traverseudata(g, o as *const Udata) as usize,
        6 => traverseLclosure(g, o as *const LuaFn),
        38 => traverseCclosure(g, o as *const CClosure) as usize,
        10 => traverseproto(g, o as *const Proto) as usize,
        8 => traversethread(g, o as *const Thread) as usize,
        _ => 0,
    }
}

unsafe fn propagateall(g: *const Lua) -> usize {
    let mut tot: usize = 0 as c_int as usize;
    while !((*g).gray.get()).is_null() {
        tot = tot.wrapping_add(propagatemark(&*g));
    }
    return tot;
}

unsafe fn convergeephemerons(g: *const Lua) {
    let mut changed: c_int = 0;
    let mut dir: c_int = 0 as c_int;
    loop {
        let mut next = (*g).ephemeron.get();

        (*g).ephemeron.set(0 as *mut Object);
        changed = 0 as c_int;

        loop {
            let w = next;
            if w.is_null() {
                break;
            }
            let h: *mut Table = w as *mut Table;
            next = (*h).hdr.gclist.get();
            (*h).hdr.marked.set((*h).hdr.marked.get() | 1 << 5);

            if traverseephemeron(g, h, dir) != 0 {
                propagateall(g);
                changed = 1 as c_int;
            }
        }
        dir = (dir == 0) as c_int;
        if !(changed != 0) {
            break;
        }
    }
}

unsafe fn clearbykeys(g: *const Lua, mut l: *const Object) {
    while !l.is_null() {
        let h: *mut Table = l as *mut Table;
        let limit: *mut Node = ((*h).node.get())
            .offset(((1 as c_int) << (*h).lsizenode.get() as c_int) as usize as isize)
            as *mut Node;
        let mut n: *mut Node = 0 as *mut Node;
        n = ((*h).node.get()).offset(0 as c_int as isize) as *mut Node;
        while n < limit {
            if iscleared(
                g,
                if (*n).u.key_tt as c_int & (1 as c_int) << 6 as c_int != 0 {
                    (*n).u.key_val.gc
                } else {
                    0 as *mut Object
                },
            ) != 0
            {
                (*n).i_val.tt_ = (0 as c_int | (1 as c_int) << 4 as c_int) as u8;
            }
            if (*n).i_val.tt_ as c_int & 0xf as c_int == 0 as c_int {
                clearkey(n);
            }
            n = n.offset(1);
        }
        l = (*(l as *mut Table)).hdr.gclist.get();
    }
}

unsafe fn clearbyvalues(g: *const Lua, mut l: *const Object, f: *const Object) {
    while l != f {
        let h: *mut Table = l as *mut Table;
        let mut n: *mut Node = 0 as *mut Node;
        let limit: *mut Node = ((*h).node.get())
            .offset(((1 as c_int) << (*h).lsizenode.get() as c_int) as usize as isize)
            as *mut Node;
        let mut i: c_uint = 0;
        let asize: c_uint = luaH_realasize(h);
        i = 0 as c_int as c_uint;
        while i < asize {
            let o: *mut UnsafeValue = (*h).array.get().offset(i as isize) as *mut UnsafeValue;
            if iscleared(
                g,
                if (*o).tt_ as c_int & (1 as c_int) << 6 as c_int != 0 {
                    (*o).value_.gc
                } else {
                    0 as *mut Object
                },
            ) != 0
            {
                (*o).tt_ = (0 as c_int | (1 as c_int) << 4 as c_int) as u8;
            }
            i = i.wrapping_add(1);
        }
        n = ((*h).node.get()).offset(0 as c_int as isize) as *mut Node;
        while n < limit {
            if iscleared(
                g,
                if (*n).i_val.tt_ as c_int & (1 as c_int) << 6 as c_int != 0 {
                    (*n).i_val.value_.gc
                } else {
                    0 as *mut Object
                },
            ) != 0
            {
                (*n).i_val.tt_ = (0 as c_int | (1 as c_int) << 4 as c_int) as u8;
            }
            if (*n).i_val.tt_ as c_int & 0xf as c_int == 0 as c_int {
                clearkey(n);
            }
            n = n.offset(1);
        }
        l = (*(l as *mut Table)).hdr.gclist.get();
    }
}

unsafe fn freeupval(g: *const Lua, uv: *mut UpVal) {
    let layout = Layout::new::<UpVal>();

    if (*uv).v.get() != &raw mut (*(*uv).u.get()).value as *mut UnsafeValue {
        luaF_unlinkupval(uv);
    }

    (*g).gc.dealloc(uv.cast(), layout);
}

unsafe fn freeobj(g: *const Lua, o: *mut Object) {
    (*g).gcstp.set((*g).gcstp.get() | 2);

    match (*o).tt {
        10 => luaF_freeproto(g, o as *mut Proto),
        9 => freeupval(g, o as *mut UpVal),
        6 => {
            core::ptr::drop_in_place(o.cast::<LuaFn>());
            (*g).gc.dealloc(o.cast(), Layout::new::<LuaFn>());
        }
        38 => {
            let cl_0: *mut CClosure = o as *mut CClosure;
            let nupvalues = usize::from((*cl_0).nupvalues);
            let size = offset_of!(CClosure, upvalue) + size_of::<UnsafeValue>() * nupvalues;
            let align = align_of::<CClosure>();
            let layout = Layout::from_size_align(size, align).unwrap().pad_to_align();

            (*g).gc.dealloc(cl_0.cast(), layout);
        }
        5 => luaH_free(o.cast()),
        8 => {
            core::ptr::drop_in_place(o.cast::<Thread>());
            (*g).gc.dealloc(o.cast(), Layout::new::<Thread>());
        }
        7 => {
            let u: *mut Udata = o as *mut Udata;
            let layout = Layout::from_size_align(
                offset_of!(Udata, uv)
                    + size_of::<UnsafeValue>()
                        .wrapping_mul((*u).nuvalue.into())
                        .wrapping_add((*u).len),
                align_of::<Udata>(),
            )
            .unwrap()
            .pad_to_align();

            (*g).gc.dealloc(o.cast(), layout);
        }
        4 => {
            let ts: *mut Str = o as *mut Str;
            let size = offset_of!(Str, contents) + usize::from((*ts).shrlen.get()) + 1;
            let align = align_of::<Str>();
            let layout = Layout::from_size_align(size, align).unwrap().pad_to_align();

            core::ptr::drop_in_place(ts);
            (*g).gc.dealloc(ts.cast(), layout);
        }
        20 => {
            let ts_0: *mut Str = o as *mut Str;
            let size = offset_of!(Str, contents) + (*(*ts_0).u.get()).lnglen + 1;
            let align = align_of::<Str>();
            let layout = Layout::from_size_align(size, align).unwrap().pad_to_align();

            core::ptr::drop_in_place(ts_0);
            (*g).gc.dealloc(ts_0.cast(), layout);
        }
        11 => {
            core::ptr::drop_in_place(o.cast::<UserId>());
            (*g).gc.dealloc(o.cast(), Layout::new::<UserId>());
        }
        _ => unreachable!(),
    }

    (*g).gcstp.set((*g).gcstp.get() & !2);
}

unsafe fn sweeplist(
    g: &Lua,
    mut p: *mut *const Object,
    countin: c_int,
    countout: *mut c_int,
) -> *mut *const Object {
    let ow = g.currentwhite.get() ^ (1 << 3 | 1 << 4);
    let mut i = 0;
    let white = g.currentwhite.get() & (1 << 3 | 1 << 4);

    while !(*p).is_null() && i < countin {
        let curr = *p;
        let marked = (*curr).marked.get();

        if marked & ow != 0 {
            *p = (*curr).next.get();
            freeobj(g, curr.cast_mut());
        } else {
            (*curr)
                .marked
                .set(marked & !(1 << 5 | (1 << 3 | 1 << 4) | 7) | white);
            p = (*curr).next.as_ptr();
        }

        i += 1;
    }

    if !countout.is_null() {
        *countout = i;
    }

    if (*p).is_null() { null_mut() } else { p }
}

/// Sweep a list until a live object (or end of list).
unsafe fn sweeptolive(g: &Lua, mut p: *mut *const Object) -> *mut *const Object {
    let old = p;

    loop {
        p = sweeplist(g, p, 1, 0 as *mut c_int);
        if p != old {
            break;
        }
    }

    return p;
}

unsafe fn setpause(g: *const Lua) {
    let mut threshold: isize = 0;
    let mut debt: isize = 0;
    let pause: c_int = (*g).gcpause.get() as c_int * 4 as c_int;
    let estimate: isize = ((*g).GCestimate.get() / 100) as isize;

    threshold = if (pause as isize) < (!(0 as c_int as usize) >> 1 as c_int) as isize / estimate {
        estimate * pause as isize
    } else {
        (!(0 as c_int as usize) >> 1 as c_int) as isize
    };

    debt = (((*g).gc.totalbytes.get() + (*g).gc.debt.get()) as usize)
        .wrapping_sub(threshold as usize) as isize;

    if debt > 0 as c_int as isize {
        debt = 0 as c_int as isize;
    }

    (*g).gc.set_debt(debt);
}

unsafe fn entersweep(g: &Lua) {
    g.gcstate.set(3);
    g.sweepgc.set(sweeptolive(g, g.all.as_ptr()));
}

unsafe fn deletelist(g: &Lua, mut p: *const Object) {
    while !p.is_null() {
        let next = (*p).next.get();
        freeobj(g, p.cast_mut());
        p = next;
    }
}

pub(crate) unsafe fn luaC_freeallobjects(g: &Lua) {
    g.gcstp.set(4);
    g.lastatomic.set(0);

    deletelist(g, (*g).all.get());
    deletelist(g, (*g).fixedgc.get());
}

unsafe fn atomic(g: &Lua) -> usize {
    let mut work: usize = 0 as c_int as usize;
    let grayagain = g.grayagain.get();

    g.grayagain.set(null_mut());
    g.gcstate.set(2);

    mark_roots(g);

    work = work.wrapping_add(propagateall(g));
    work = work.wrapping_add(remarkupvals(g) as usize);
    work = work.wrapping_add(propagateall(g));
    g.gray.set(grayagain);
    work = work.wrapping_add(propagateall(g));
    convergeephemerons(g);
    clearbyvalues(g, g.weak.get(), 0 as *mut Object);
    clearbyvalues(g, g.allweak.get(), 0 as *mut Object);

    let origweak = g.weak.get();
    let origall = g.allweak.get();
    work = work.wrapping_add(propagateall(g));
    convergeephemerons(g);
    clearbykeys(g, g.ephemeron.get());
    clearbykeys(g, g.allweak.get());
    clearbyvalues(g, g.weak.get(), origweak);
    clearbyvalues(g, g.allweak.get(), origall);

    g.currentwhite.set(g.currentwhite.get() ^ (1 << 3 | 1 << 4));

    work
}

unsafe fn sweepstep(g: &Lua, nextstate: c_int, nextlist: *mut *const Object) -> c_int {
    if !(*g).sweepgc.get().is_null() {
        let olddebt: isize = (*g).gc.debt.get();
        let mut count: c_int = 0;

        g.sweepgc
            .set(sweeplist(g, g.sweepgc.get(), 100, &mut count));
        g.GCestimate
            .set((g.GCestimate.get()).wrapping_add((g.gc.debt.get() - olddebt) as usize));

        count
    } else {
        (*g).gcstate.set(nextstate as u8);
        (*g).sweepgc.set(nextlist);
        return 0 as c_int;
    }
}

unsafe fn singlestep(g: &Lua) -> usize {
    let mut work: usize = 0;

    g.gcstopem.set(1);

    match g.gcstate.get() {
        8 => {
            restart(g);
            g.gcstate.set(0);
            work = 1;
        }
        0 => {
            if g.gray.get().is_null() {
                g.gcstate.set(1);
                work = 0;
            } else {
                work = propagatemark(g);
            }
        }
        1 => {
            work = atomic(g);
            entersweep(g);
            g.GCestimate
                .set((g.gc.totalbytes.get() + g.gc.debt.get()) as usize);
        }
        3 => work = sweepstep(g, 6, null_mut()) as usize,
        6 => {
            g.gcstate.set(7);
            work = 0;
        }
        7 => {
            g.gcstate.set(8);
            work = 0;
        }
        _ => return 0,
    }

    g.gcstopem.set(0);

    work
}

/// # Safety
/// After this function return any unreachable objects may be freed.
unsafe fn incstep(g: &Lua) {
    let stepmul = g.gcstepmul.get() * 4 | 1;
    let mut debt: isize = (g.gc.debt.get() as c_ulong)
        .wrapping_div(size_of::<UnsafeValue>() as c_ulong)
        .wrapping_mul(stepmul as c_ulong) as isize;
    let stepsize: isize = (if (*g).gcstepsize.get() as c_ulong
        <= (::core::mem::size_of::<isize>() as c_ulong)
            .wrapping_mul(8 as c_int as c_ulong)
            .wrapping_sub(2 as c_int as c_ulong)
    {
        (((1 as c_int as isize) << (*g).gcstepsize.get() as c_int) as c_ulong)
            .wrapping_div(::core::mem::size_of::<UnsafeValue>() as c_ulong)
            .wrapping_mul(stepmul as c_ulong)
    } else {
        (!(0 as c_int as usize) >> 1 as c_int) as isize as c_ulong
    }) as isize;
    loop {
        let work: usize = singlestep(g);
        debt = (debt as usize).wrapping_sub(work) as isize as isize;

        if !(debt > -stepsize && (*g).gcstate.get() != 8) {
            break;
        }
    }

    if (*g).gcstate.get() == 8 {
        setpause(g);
    } else {
        debt = ((debt / stepmul as isize) as c_ulong)
            .wrapping_mul(::core::mem::size_of::<UnsafeValue>() as c_ulong) as isize;
        (*g).gc.set_debt(debt);
    };
}

/// Garbage Collector for Lua objects.
pub struct Gc {
    totalbytes: Cell<isize>,
    debt: Cell<isize>,
}

impl Gc {
    pub(super) fn new(totalbytes: usize) -> Self {
        Self {
            totalbytes: Cell::new(totalbytes.try_into().unwrap()),
            debt: Cell::new(0),
        }
    }

    #[inline(always)]
    pub(crate) fn debt(&self) -> isize {
        self.debt.get()
    }

    pub(crate) fn set_debt(&self, mut debt: isize) {
        let tb: isize = self.totalbytes.get() + self.debt.get();

        if debt < tb - (!(0 as c_int as usize) >> 1) as isize {
            debt = tb - (!(0 as c_int as usize) >> 1 as c_int) as isize;
        }

        self.totalbytes.set(tb - debt);
        self.debt.set(debt);
    }

    pub(crate) unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { alloc::alloc::dealloc(ptr, layout) };
        self.decrease_debt(layout.size());
    }

    pub(crate) fn increase_debt(&self, bytes: usize) {
        self.debt
            .set(self.debt.get().checked_add_unsigned(bytes).unwrap());
    }

    pub(crate) fn decrease_debt(&self, bytes: usize) {
        self.debt
            .set(self.debt.get().checked_sub_unsigned(bytes).unwrap());
    }
}
