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

use crate::Lua;
use crate::ldo::luaD_shrinkstack;
use crate::lfunc::{luaF_freeproto, luaF_unlinkupval};
use crate::lmem::{luaM_free_, luaM_malloc_};
use crate::lobject::{
    CClosure, GCObject, LClosure, Node, Proto, StkId, TString, TValue, Table, UValue, Udata, UpVal,
};
use crate::lstate::{lua_State, luaE_freethread, luaE_setdebt};
use crate::lstring::{luaS_clearcache, luaS_remove, luaS_resize};
use crate::ltable::{luaH_free, luaH_realasize};
use crate::ltm::{TM_MODE, luaT_gettm};
use libc::strchr;
use std::ptr::null_mut;

unsafe fn getgclist(mut o: *mut GCObject) -> *mut *mut GCObject {
    match (*o).tt as libc::c_int {
        5 => return &mut (*(o as *mut Table)).gclist,
        6 => return &mut (*(o as *mut LClosure)).gclist,
        38 => return &mut (*(o as *mut CClosure)).gclist,
        8 => return &mut (*(o as *mut lua_State)).gclist,
        10 => return &mut (*(o as *mut Proto)).gclist,
        7 => {
            let mut u: *mut Udata = o as *mut Udata;
            return &mut (*u).gclist;
        }
        _ => return 0 as *mut *mut GCObject,
    };
}

unsafe fn linkgclist_(
    mut o: *mut GCObject,
    mut pnext: *mut *mut GCObject,
    mut list: *mut *mut GCObject,
) {
    *pnext = *list;
    *list = o;

    (*o).marked = (*o).marked & !(1 << 5 | (1 << 3 | 1 << 4));
}

unsafe fn clearkey(mut n: *mut Node) {
    if (*n).u.key_tt as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0 {
        (*n).u.key_tt = (9 as libc::c_int + 2 as libc::c_int) as u8;
    }
}

unsafe fn iscleared(g: *const Lua, mut o: *const GCObject) -> libc::c_int {
    if o.is_null() {
        return 0 as libc::c_int;
    } else if (*o).tt as libc::c_int & 0xf as libc::c_int == 4 as libc::c_int {
        if (*o).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            != 0
        {
            reallymarkobject(g, o as *mut GCObject);
        }
        return 0 as libc::c_int;
    } else {
        return (*o).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int);
    };
}

pub unsafe fn luaC_barrier_(mut L: *mut lua_State, mut o: *mut GCObject, mut v: *mut GCObject) {
    let g = (*L).l_G;

    if (*g).gcstate.get() <= 2 {
        reallymarkobject(g, v);
        if (*o).marked as libc::c_int & 7 as libc::c_int > 1 as libc::c_int {
            (*v).marked =
                ((*v).marked as libc::c_int & !(7 as libc::c_int) | 2 as libc::c_int) as u8;
        }
    } else if (*g).gckind.get() == 0 {
        (*o).marked = ((*o).marked as libc::c_int
            & !((1 as libc::c_int) << 5 as libc::c_int
                | ((1 as libc::c_int) << 3 as libc::c_int
                    | (1 as libc::c_int) << 4 as libc::c_int))
            | ((*g).currentwhite.get() as libc::c_int
                & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int))
                as u8 as libc::c_int) as u8;
    }
}

pub unsafe fn luaC_barrierback_(mut L: *mut lua_State, mut o: *mut GCObject) {
    let g = (*L).l_G;
    if (*o).marked as libc::c_int & 7 as libc::c_int == 6 as libc::c_int {
        (*o).marked = ((*o).marked as libc::c_int
            & !((1 as libc::c_int) << 5 as libc::c_int
                | ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int))
                as u8 as libc::c_int) as u8;
    } else {
        linkgclist_(o as *mut GCObject, getgclist(o), (*g).grayagain.as_ptr());
    }
    if (*o).marked as libc::c_int & 7 as libc::c_int > 1 as libc::c_int {
        (*o).marked = ((*o).marked as libc::c_int & !(7 as libc::c_int) | 5 as libc::c_int) as u8;
    }
}

pub unsafe fn luaC_fix(mut L: *mut lua_State, mut o: *mut GCObject) {
    let g = (*L).l_G;
    (*o).marked = ((*o).marked as libc::c_int
        & !((1 as libc::c_int) << 5 as libc::c_int
            | ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int))
            as u8 as libc::c_int) as u8;
    (*o).marked = ((*o).marked as libc::c_int & !(7 as libc::c_int) | 4 as libc::c_int) as u8;
    (*g).allgc.set((*o).next);
    (*o).next = (*g).fixedgc.get();
    (*g).fixedgc.set(o);
}

pub unsafe fn luaC_newobj(g: *const Lua, mut tt: libc::c_int, mut sz: usize) -> *mut GCObject {
    let mut o = luaM_malloc_(g, sz) as *mut GCObject;

    (*o).marked = (*g).currentwhite.get() & (1 << 3 | 1 << 4);
    (*o).tt = tt as u8;
    (*o).next = (*g).allgc.get();
    (*g).allgc.set(o);

    return o;
}

unsafe fn reallymarkobject(g: *const Lua, mut o: *mut GCObject) {
    match (*o).tt {
        4 | 20 => {
            (*o).marked = (*o).marked & !(1 << 3 | 1 << 4) | 1 << 5;
            return;
        }
        9 => {
            let mut uv: *mut UpVal = o as *mut UpVal;
            if (*uv).v.p != &mut (*uv).u.value as *mut TValue {
                (*uv).marked = ((*uv).marked as libc::c_int
                    & !((1 as libc::c_int) << 5 as libc::c_int
                        | ((1 as libc::c_int) << 3 as libc::c_int
                            | (1 as libc::c_int) << 4 as libc::c_int)) as u8
                        as libc::c_int) as u8;
            } else {
                (*uv).marked = ((*uv).marked as libc::c_int
                    & !((1 as libc::c_int) << 3 as libc::c_int
                        | (1 as libc::c_int) << 4 as libc::c_int)
                    | (1 as libc::c_int) << 5 as libc::c_int) as u8;
            }
            if (*(*uv).v.p).tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0
                && (*(*(*uv).v.p).value_.gc).marked as libc::c_int
                    & ((1 as libc::c_int) << 3 as libc::c_int
                        | (1 as libc::c_int) << 4 as libc::c_int)
                    != 0
            {
                reallymarkobject(g, (*(*uv).v.p).value_.gc);
            }

            return;
        }
        7 => {
            let mut u: *mut Udata = o as *mut Udata;
            if (*u).nuvalue as libc::c_int == 0 as libc::c_int {
                if !((*u).metatable).is_null() {
                    if (*(*u).metatable).marked as libc::c_int
                        & ((1 as libc::c_int) << 3 as libc::c_int
                            | (1 as libc::c_int) << 4 as libc::c_int)
                        != 0
                    {
                        reallymarkobject(g, (*u).metatable as *mut GCObject);
                    }
                }
                (*u).marked = ((*u).marked as libc::c_int
                    & !((1 as libc::c_int) << 3 as libc::c_int
                        | (1 as libc::c_int) << 4 as libc::c_int)
                    | (1 as libc::c_int) << 5 as libc::c_int) as u8;
                return;
            }
        }
        6 | 38 | 5 | 8 | 10 => {}
        _ => return,
    }

    linkgclist_(o, getgclist(o), (*g).gray.as_ptr());
}

unsafe fn markmt(g: *const Lua) {
    let mut i: libc::c_int = 0;
    i = 0 as libc::c_int;
    while i < 9 as libc::c_int {
        if !((*g).mt[i as usize].get()).is_null() {
            if (*(*g).mt[i as usize].get()).marked as libc::c_int
                & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
                != 0
            {
                reallymarkobject(g, ((*g).mt[i as usize].get()) as *mut GCObject);
            }
        }
        i += 1;
    }
}

unsafe fn remarkupvals(g: *const Lua) -> libc::c_int {
    let mut thread: *mut lua_State = 0 as *mut lua_State;
    let mut p: *mut *mut lua_State = (*g).twups.as_ptr();
    let mut work: libc::c_int = 0 as libc::c_int;
    loop {
        thread = *p;
        if thread.is_null() {
            break;
        }
        work += 1;
        if (*thread).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            == 0
            && !((*thread).openupval).is_null()
        {
            p = &mut (*thread).twups;
        } else {
            let mut uv: *mut UpVal = 0 as *mut UpVal;
            *p = (*thread).twups;
            (*thread).twups = thread;
            uv = (*thread).openupval;
            while !uv.is_null() {
                work += 1;
                if (*uv).marked as libc::c_int
                    & ((1 as libc::c_int) << 3 as libc::c_int
                        | (1 as libc::c_int) << 4 as libc::c_int)
                    == 0
                {
                    if (*(*uv).v.p).tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0
                        && (*(*(*uv).v.p).value_.gc).marked as libc::c_int
                            & ((1 as libc::c_int) << 3 as libc::c_int
                                | (1 as libc::c_int) << 4 as libc::c_int)
                            != 0
                    {
                        reallymarkobject(g, (*(*uv).v.p).value_.gc);
                    }
                }
                uv = (*uv).u.open.next;
            }
        }
    }
    return work;
}

unsafe fn cleargraylists(g: *const Lua) {
    (*g).grayagain.set(0 as *mut GCObject);
    (*g).gray.set((*g).grayagain.get());
    (*g).ephemeron.set(0 as *mut GCObject);
    (*g).allweak.set((*g).ephemeron.get());
    (*g).weak.set((*g).allweak.get());
}

unsafe fn restartcollection(g: *const Lua) {
    cleargraylists(g);

    if (*(*g).l_registry.get()).tt_ & 1 << 6 != 0
        && (*(*(*g).l_registry.get()).value_.gc).marked & (1 << 3 | 1 << 4) != 0
    {
        reallymarkobject(g, (*(*g).l_registry.get()).value_.gc);
    }

    markmt(g);
}

unsafe fn genlink(g: *const Lua, mut o: *mut GCObject) {
    if (*o).marked as libc::c_int & 7 as libc::c_int == 5 as libc::c_int {
        linkgclist_(o as *mut GCObject, getgclist(o), (*g).grayagain.as_ptr());
    } else if (*o).marked as libc::c_int & 7 as libc::c_int == 6 as libc::c_int {
        (*o).marked = ((*o).marked as libc::c_int ^ (6 as libc::c_int ^ 4 as libc::c_int)) as u8;
    }
}

unsafe fn traverseweakvalue(g: *const Lua, mut h: *mut Table) {
    let mut n: *mut Node = 0 as *mut Node;
    let mut limit: *mut Node = &mut *((*h).node)
        .offset(((1 as libc::c_int) << (*h).lsizenode as libc::c_int) as usize as isize)
        as *mut Node;
    let mut hasclears: libc::c_int =
        ((*h).alimit > 0 as libc::c_int as libc::c_uint) as libc::c_int;
    n = &mut *((*h).node).offset(0 as libc::c_int as isize) as *mut Node;
    while n < limit {
        if (*n).i_val.tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int {
            clearkey(n);
        } else {
            if (*n).u.key_tt as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0
                && (*(*n).u.key_val.gc).marked as libc::c_int
                    & ((1 as libc::c_int) << 3 as libc::c_int
                        | (1 as libc::c_int) << 4 as libc::c_int)
                    != 0
            {
                reallymarkobject(g, (*n).u.key_val.gc);
            }
            if hasclears == 0
                && iscleared(
                    g,
                    if (*n).i_val.tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0 {
                        (*n).i_val.value_.gc
                    } else {
                        0 as *mut GCObject
                    },
                ) != 0
            {
                hasclears = 1 as libc::c_int;
            }
        }
        n = n.offset(1);
    }
    if (*g).gcstate.get() == 2 && hasclears != 0 {
        linkgclist_(h as *mut GCObject, &mut (*h).gclist, (*g).weak.as_ptr());
    } else {
        linkgclist_(
            h as *mut GCObject,
            &mut (*h).gclist,
            (*g).grayagain.as_ptr(),
        );
    };
}

unsafe fn traverseephemeron(g: *const Lua, mut h: *mut Table, mut inv: libc::c_int) -> libc::c_int {
    let mut marked: libc::c_int = 0 as libc::c_int;
    let mut hasclears: libc::c_int = 0 as libc::c_int;
    let mut hasww: libc::c_int = 0 as libc::c_int;
    let mut i: libc::c_uint = 0;
    let mut asize: libc::c_uint = luaH_realasize(h);
    let mut nsize: libc::c_uint =
        ((1 as libc::c_int) << (*h).lsizenode as libc::c_int) as libc::c_uint;
    i = 0 as libc::c_int as libc::c_uint;
    while i < asize {
        if (*((*h).array).offset(i as isize)).tt_ as libc::c_int
            & (1 as libc::c_int) << 6 as libc::c_int
            != 0
            && (*(*((*h).array).offset(i as isize)).value_.gc).marked as libc::c_int
                & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
                != 0
        {
            marked = 1 as libc::c_int;
            reallymarkobject(g, (*((*h).array).offset(i as isize)).value_.gc);
        }
        i = i.wrapping_add(1);
    }
    i = 0 as libc::c_int as libc::c_uint;
    while i < nsize {
        let mut n: *mut Node = if inv != 0 {
            &mut *((*h).node).offset(
                nsize
                    .wrapping_sub(1 as libc::c_int as libc::c_uint)
                    .wrapping_sub(i) as isize,
            ) as *mut Node
        } else {
            &mut *((*h).node).offset(i as isize) as *mut Node
        };
        if (*n).i_val.tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int {
            clearkey(n);
        } else if iscleared(
            g,
            if (*n).u.key_tt as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0 {
                (*n).u.key_val.gc
            } else {
                0 as *mut GCObject
            },
        ) != 0
        {
            hasclears = 1 as libc::c_int;
            if (*n).i_val.tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0
                && (*(*n).i_val.value_.gc).marked as libc::c_int
                    & ((1 as libc::c_int) << 3 as libc::c_int
                        | (1 as libc::c_int) << 4 as libc::c_int)
                    != 0
            {
                hasww = 1 as libc::c_int;
            }
        } else if (*n).i_val.tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0
            && (*(*n).i_val.value_.gc).marked as libc::c_int
                & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
                != 0
        {
            marked = 1 as libc::c_int;
            reallymarkobject(g, (*n).i_val.value_.gc);
        }
        i = i.wrapping_add(1);
    }
    if (*g).gcstate.get() == 0 {
        linkgclist_(
            h as *mut GCObject,
            &mut (*h).gclist,
            (*g).grayagain.as_ptr(),
        );
    } else if hasww != 0 {
        linkgclist_(
            h as *mut GCObject,
            &mut (*h).gclist,
            (*g).ephemeron.as_ptr(),
        );
    } else if hasclears != 0 {
        linkgclist_(h as *mut GCObject, &mut (*h).gclist, (*g).allweak.as_ptr());
    } else {
        genlink(g, h as *mut GCObject);
    }
    return marked;
}

unsafe fn traversestrongtable(g: *const Lua, mut h: *mut Table) {
    let mut n: *mut Node = 0 as *mut Node;
    let mut limit: *mut Node = &mut *((*h).node)
        .offset(((1 as libc::c_int) << (*h).lsizenode as libc::c_int) as usize as isize)
        as *mut Node;
    let mut i: libc::c_uint = 0;
    let mut asize: libc::c_uint = luaH_realasize(h);
    i = 0 as libc::c_int as libc::c_uint;
    while i < asize {
        if (*((*h).array).offset(i as isize)).tt_ as libc::c_int
            & (1 as libc::c_int) << 6 as libc::c_int
            != 0
            && (*(*((*h).array).offset(i as isize)).value_.gc).marked as libc::c_int
                & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
                != 0
        {
            reallymarkobject(g, (*((*h).array).offset(i as isize)).value_.gc);
        }
        i = i.wrapping_add(1);
    }
    n = &mut *((*h).node).offset(0 as libc::c_int as isize) as *mut Node;
    while n < limit {
        if (*n).i_val.tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int {
            clearkey(n);
        } else {
            if (*n).u.key_tt as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0
                && (*(*n).u.key_val.gc).marked as libc::c_int
                    & ((1 as libc::c_int) << 3 as libc::c_int
                        | (1 as libc::c_int) << 4 as libc::c_int)
                    != 0
            {
                reallymarkobject(g, (*n).u.key_val.gc);
            }
            if (*n).i_val.tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0
                && (*(*n).i_val.value_.gc).marked as libc::c_int
                    & ((1 as libc::c_int) << 3 as libc::c_int
                        | (1 as libc::c_int) << 4 as libc::c_int)
                    != 0
            {
                reallymarkobject(g, (*n).i_val.value_.gc);
            }
        }
        n = n.offset(1);
    }
    genlink(g, h as *mut GCObject);
}

unsafe fn traversetable(g: *const Lua, mut h: *mut Table) -> usize {
    let mut weakkey: *const libc::c_char = 0 as *const libc::c_char;
    let mut weakvalue: *const libc::c_char = 0 as *const libc::c_char;
    let mut mode: *const TValue = if ((*h).metatable).is_null() {
        0 as *const TValue
    } else if (*(*h).metatable).flags as libc::c_uint & 1 << TM_MODE != 0 {
        0 as *const TValue
    } else {
        luaT_gettm((*h).metatable, TM_MODE, (*g).tmname[TM_MODE as usize].get())
    };

    let mut smode: *mut TString = 0 as *mut TString;
    if !((*h).metatable).is_null() {
        if (*(*h).metatable).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            != 0
        {
            reallymarkobject(g, (*h).metatable as *mut GCObject);
        }
    }
    if !mode.is_null()
        && (*mode).tt_ as libc::c_int
            == 4 as libc::c_int
                | (0 as libc::c_int) << 4 as libc::c_int
                | (1 as libc::c_int) << 6 as libc::c_int
        && {
            smode = ((*mode).value_.gc as *mut TString) as *mut TString;
            weakkey = strchr(((*smode).contents).as_mut_ptr(), 'k' as i32);
            weakvalue = strchr(((*smode).contents).as_mut_ptr(), 'v' as i32);
            !weakkey.is_null() || !weakvalue.is_null()
        }
    {
        if weakkey.is_null() {
            traverseweakvalue(g, h);
        } else if weakvalue.is_null() {
            traverseephemeron(g, h, 0 as libc::c_int);
        } else {
            linkgclist_(h as *mut GCObject, &mut (*h).gclist, (*g).allweak.as_ptr());
        }
    } else {
        traversestrongtable(g, h);
    }
    return (1 as libc::c_int as libc::c_uint)
        .wrapping_add((*h).alimit)
        .wrapping_add(
            (2 as libc::c_int
                * (if ((*h).lastfree).is_null() {
                    0 as libc::c_int
                } else {
                    (1 as libc::c_int) << (*h).lsizenode as libc::c_int
                })) as libc::c_uint,
        ) as usize;
}

unsafe fn traverseudata(g: *const Lua, mut u: *mut Udata) -> libc::c_int {
    let mut i: libc::c_int = 0;
    if !((*u).metatable).is_null() {
        if (*(*u).metatable).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            != 0
        {
            reallymarkobject(g, (*u).metatable as *mut GCObject);
        }
    }
    i = 0 as libc::c_int;
    while i < (*u).nuvalue as libc::c_int {
        if (*((*u).uv).as_mut_ptr().offset(i as isize)).uv.tt_ as libc::c_int
            & (1 as libc::c_int) << 6 as libc::c_int
            != 0
            && (*(*((*u).uv).as_mut_ptr().offset(i as isize)).uv.value_.gc).marked as libc::c_int
                & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
                != 0
        {
            reallymarkobject(g, (*((*u).uv).as_mut_ptr().offset(i as isize)).uv.value_.gc);
        }
        i += 1;
    }
    genlink(g, u as *mut GCObject);
    return 1 as libc::c_int + (*u).nuvalue as libc::c_int;
}

unsafe fn traverseproto(g: *const Lua, mut f: *mut Proto) -> libc::c_int {
    let mut i: libc::c_int = 0;
    if !((*f).source).is_null() {
        if (*(*f).source).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            != 0
        {
            reallymarkobject(g, (*f).source as *mut GCObject);
        }
    }
    i = 0 as libc::c_int;
    while i < (*f).sizek {
        if (*((*f).k).offset(i as isize)).tt_ as libc::c_int
            & (1 as libc::c_int) << 6 as libc::c_int
            != 0
            && (*(*((*f).k).offset(i as isize)).value_.gc).marked as libc::c_int
                & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
                != 0
        {
            reallymarkobject(g, (*((*f).k).offset(i as isize)).value_.gc);
        }
        i += 1;
    }
    i = 0 as libc::c_int;
    while i < (*f).sizeupvalues {
        if !((*((*f).upvalues).offset(i as isize)).name).is_null() {
            if (*(*((*f).upvalues).offset(i as isize)).name).marked as libc::c_int
                & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
                != 0
            {
                reallymarkobject(
                    g,
                    (*((*f).upvalues).offset(i as isize)).name as *mut GCObject,
                );
            }
        }
        i += 1;
    }
    i = 0 as libc::c_int;
    while i < (*f).sizep {
        if !(*((*f).p).offset(i as isize)).is_null() {
            if (**((*f).p).offset(i as isize)).marked as libc::c_int
                & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
                != 0
            {
                reallymarkobject(g, *((*f).p).offset(i as isize) as *mut GCObject);
            }
        }
        i += 1;
    }
    i = 0 as libc::c_int;
    while i < (*f).sizelocvars {
        if !((*((*f).locvars).offset(i as isize)).varname).is_null() {
            if (*(*((*f).locvars).offset(i as isize)).varname).marked as libc::c_int
                & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
                != 0
            {
                reallymarkobject(
                    g,
                    (*((*f).locvars).offset(i as isize)).varname as *mut GCObject,
                );
            }
        }
        i += 1;
    }
    return 1 as libc::c_int + (*f).sizek + (*f).sizeupvalues + (*f).sizep + (*f).sizelocvars;
}

unsafe fn traverseCclosure(g: *const Lua, mut cl: *mut CClosure) -> libc::c_int {
    let mut i: libc::c_int = 0;
    i = 0 as libc::c_int;
    while i < (*cl).nupvalues as libc::c_int {
        if (*((*cl).upvalue).as_mut_ptr().offset(i as isize)).tt_ as libc::c_int
            & (1 as libc::c_int) << 6 as libc::c_int
            != 0
            && (*(*((*cl).upvalue).as_mut_ptr().offset(i as isize)).value_.gc).marked as libc::c_int
                & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
                != 0
        {
            reallymarkobject(
                g,
                (*((*cl).upvalue).as_mut_ptr().offset(i as isize)).value_.gc,
            );
        }
        i += 1;
    }
    return 1 as libc::c_int + (*cl).nupvalues as libc::c_int;
}

unsafe fn traverseLclosure(g: *const Lua, mut cl: *mut LClosure) -> libc::c_int {
    let mut i: libc::c_int = 0;
    if !((*cl).p).is_null() {
        if (*(*cl).p).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            != 0
        {
            reallymarkobject(g, (*cl).p as *mut GCObject);
        }
    }
    i = 0 as libc::c_int;
    while i < (*cl).nupvalues as libc::c_int {
        let mut uv: *mut UpVal = *((*cl).upvals).as_mut_ptr().offset(i as isize);
        if !uv.is_null() {
            if (*uv).marked as libc::c_int
                & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
                != 0
            {
                reallymarkobject(g, uv as *mut GCObject);
            }
        }
        i += 1;
    }
    return 1 as libc::c_int + (*cl).nupvalues as libc::c_int;
}

unsafe fn traversethread(g: *const Lua, mut th: *mut lua_State) -> libc::c_int {
    let mut uv: *mut UpVal = 0 as *mut UpVal;
    let mut o: StkId = (*th).stack.p;
    if (*th).marked as libc::c_int & 7 as libc::c_int > 1 || (*g).gcstate.get() == 0 {
        linkgclist_(
            th as *mut GCObject,
            &mut (*th).gclist,
            (*g).grayagain.as_ptr(),
        );
    }
    if o.is_null() {
        return 1 as libc::c_int;
    }
    while o < (*th).top.p {
        if (*o).val.tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0
            && (*(*o).val.value_.gc).marked as libc::c_int
                & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
                != 0
        {
            reallymarkobject(g, (*o).val.value_.gc);
        }
        o = o.offset(1);
    }
    uv = (*th).openupval;
    while !uv.is_null() {
        if (*uv).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            != 0
        {
            reallymarkobject(g, uv as *mut GCObject);
        }
        uv = (*uv).u.open.next;
    }

    if (*g).gcstate.get() == 2 {
        if (*g).gcemergency.get() == 0 {
            luaD_shrinkstack(th);
        }

        o = (*th).top.p;
        while o < ((*th).stack_last.p).offset(5 as libc::c_int as isize) {
            (*o).val.tt_ = (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
            o = o.offset(1);
        }
        if !((*th).twups != th) && !((*th).openupval).is_null() {
            (*th).twups = (*g).twups.get();
            (*g).twups.set(th);
        }
    }
    return 1 as libc::c_int
        + ((*th).stack_last.p).offset_from((*th).stack.p) as libc::c_long as libc::c_int;
}

unsafe fn propagatemark(g: *const Lua) -> usize {
    let mut o: *mut GCObject = (*g).gray.get();
    (*o).marked = ((*o).marked as libc::c_int | (1 as libc::c_int) << 5 as libc::c_int) as u8;
    (*g).gray.set(*getgclist(o));
    match (*o).tt as libc::c_int {
        5 => return traversetable(g, o as *mut Table),
        7 => return traverseudata(g, o as *mut Udata) as usize,
        6 => return traverseLclosure(g, o as *mut LClosure) as usize,
        38 => return traverseCclosure(g, o as *mut CClosure) as usize,
        10 => return traverseproto(g, o as *mut Proto) as usize,
        8 => return traversethread(g, o as *mut lua_State) as usize,
        _ => return 0 as libc::c_int as usize,
    };
}

unsafe fn propagateall(g: *const Lua) -> usize {
    let mut tot: usize = 0 as libc::c_int as usize;
    while !((*g).gray.get()).is_null() {
        tot = tot.wrapping_add(propagatemark(g));
    }
    return tot;
}

unsafe fn convergeephemerons(g: *const Lua) {
    let mut changed: libc::c_int = 0;
    let mut dir: libc::c_int = 0 as libc::c_int;
    loop {
        let mut w: *mut GCObject = 0 as *mut GCObject;
        let mut next: *mut GCObject = (*g).ephemeron.get();
        (*g).ephemeron.set(0 as *mut GCObject);
        changed = 0 as libc::c_int;
        loop {
            w = next;
            if w.is_null() {
                break;
            }
            let mut h: *mut Table = w as *mut Table;
            next = (*h).gclist;
            (*h).marked =
                ((*h).marked as libc::c_int | (1 as libc::c_int) << 5 as libc::c_int) as u8;
            if traverseephemeron(g, h, dir) != 0 {
                propagateall(g);
                changed = 1 as libc::c_int;
            }
        }
        dir = (dir == 0) as libc::c_int;
        if !(changed != 0) {
            break;
        }
    }
}

unsafe fn clearbykeys(g: *const Lua, mut l: *mut GCObject) {
    while !l.is_null() {
        let mut h: *mut Table = l as *mut Table;
        let mut limit: *mut Node = &mut *((*h).node)
            .offset(((1 as libc::c_int) << (*h).lsizenode as libc::c_int) as usize as isize)
            as *mut Node;
        let mut n: *mut Node = 0 as *mut Node;
        n = &mut *((*h).node).offset(0 as libc::c_int as isize) as *mut Node;
        while n < limit {
            if iscleared(
                g,
                if (*n).u.key_tt as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0 {
                    (*n).u.key_val.gc
                } else {
                    0 as *mut GCObject
                },
            ) != 0
            {
                (*n).i_val.tt_ = (0 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
            }
            if (*n).i_val.tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int {
                clearkey(n);
            }
            n = n.offset(1);
        }
        l = (*(l as *mut Table)).gclist;
    }
}

unsafe fn clearbyvalues(g: *const Lua, mut l: *mut GCObject, mut f: *mut GCObject) {
    while l != f {
        let mut h: *mut Table = l as *mut Table;
        let mut n: *mut Node = 0 as *mut Node;
        let mut limit: *mut Node = &mut *((*h).node)
            .offset(((1 as libc::c_int) << (*h).lsizenode as libc::c_int) as usize as isize)
            as *mut Node;
        let mut i: libc::c_uint = 0;
        let mut asize: libc::c_uint = luaH_realasize(h);
        i = 0 as libc::c_int as libc::c_uint;
        while i < asize {
            let mut o: *mut TValue = &mut *((*h).array).offset(i as isize) as *mut TValue;
            if iscleared(
                g,
                if (*o).tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0 {
                    (*o).value_.gc
                } else {
                    0 as *mut GCObject
                },
            ) != 0
            {
                (*o).tt_ = (0 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
            }
            i = i.wrapping_add(1);
        }
        n = &mut *((*h).node).offset(0 as libc::c_int as isize) as *mut Node;
        while n < limit {
            if iscleared(
                g,
                if (*n).i_val.tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0 {
                    (*n).i_val.value_.gc
                } else {
                    0 as *mut GCObject
                },
            ) != 0
            {
                (*n).i_val.tt_ = (0 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
            }
            if (*n).i_val.tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int {
                clearkey(n);
            }
            n = n.offset(1);
        }
        l = (*(l as *mut Table)).gclist;
    }
}

unsafe fn freeupval(g: *const Lua, mut uv: *mut UpVal) {
    if (*uv).v.p != &mut (*uv).u.value as *mut TValue {
        luaF_unlinkupval(uv);
    }
    luaM_free_(g, uv as *mut libc::c_void, ::core::mem::size_of::<UpVal>());
}

unsafe fn freeobj(g: *const Lua, mut o: *mut GCObject) {
    match (*o).tt {
        10 => luaF_freeproto(g, o as *mut Proto),
        9 => freeupval(g, o as *mut UpVal),
        6 => {
            let mut cl: *mut LClosure = o as *mut LClosure;

            luaM_free_(
                g,
                cl as *mut libc::c_void,
                (32 as libc::c_ulong as libc::c_int
                    + ::core::mem::size_of::<*mut TValue>() as libc::c_ulong as libc::c_int
                        * (*cl).nupvalues as libc::c_int) as usize,
            );
        }
        38 => {
            let mut cl_0: *mut CClosure = o as *mut CClosure;

            luaM_free_(
                g,
                cl_0 as *mut libc::c_void,
                (32 as libc::c_ulong as libc::c_int
                    + ::core::mem::size_of::<TValue>() as libc::c_ulong as libc::c_int
                        * (*cl_0).nupvalues as libc::c_int) as usize,
            );
        }
        5 => luaH_free(g, o as *mut Table),
        8 => luaE_freethread(g, o as *mut lua_State),
        7 => {
            let mut u: *mut Udata = o as *mut Udata;

            luaM_free_(
                g,
                o as *mut libc::c_void,
                (if (*u).nuvalue == 0 {
                    32
                } else {
                    40usize.wrapping_add(
                        (::core::mem::size_of::<UValue>()).wrapping_mul((*u).nuvalue.into()),
                    )
                })
                .wrapping_add((*u).len),
            );
        }
        4 => {
            let mut ts: *mut TString = o as *mut TString;

            luaS_remove(g, ts);
            luaM_free_(
                g,
                ts as *mut libc::c_void,
                24usize.wrapping_add(usize::from((*ts).shrlen) + 1),
            );
        }
        20 => {
            let mut ts_0: *mut TString = o as *mut TString;

            luaM_free_(
                g,
                ts_0 as *mut libc::c_void,
                24usize.wrapping_add(((*ts_0).u.lnglen).wrapping_add(1)),
            );
        }
        _ => unreachable!(),
    }
}

unsafe fn sweeplist(
    mut L: *mut lua_State,
    mut p: *mut *mut GCObject,
    mut countin: libc::c_int,
    mut countout: *mut libc::c_int,
) -> *mut *mut GCObject {
    let g = (*L).l_G;
    let mut ow: libc::c_int = (*g).currentwhite.get() as libc::c_int
        ^ ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int);
    let mut i: libc::c_int = 0;
    let mut white: libc::c_int = ((*g).currentwhite.get() as libc::c_int
        & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int))
        as u8 as libc::c_int;
    i = 0 as libc::c_int;
    while !(*p).is_null() && i < countin {
        let mut curr: *mut GCObject = *p;
        let mut marked: libc::c_int = (*curr).marked as libc::c_int;
        if marked & ow != 0 {
            *p = (*curr).next;
            freeobj(g, curr);
        } else {
            (*curr).marked = (marked
                & !((1 as libc::c_int) << 5 as libc::c_int
                    | ((1 as libc::c_int) << 3 as libc::c_int
                        | (1 as libc::c_int) << 4 as libc::c_int)
                    | 7 as libc::c_int)
                | white) as u8;
            p = &mut (*curr).next;
        }
        i += 1;
    }
    if !countout.is_null() {
        *countout = i;
    }
    return if (*p).is_null() {
        0 as *mut *mut GCObject
    } else {
        p
    };
}

unsafe fn sweeptolive(mut L: *mut lua_State, mut p: *mut *mut GCObject) -> *mut *mut GCObject {
    let mut old: *mut *mut GCObject = p;
    loop {
        p = sweeplist(L, p, 1 as libc::c_int, 0 as *mut libc::c_int);
        if !(p == old) {
            break;
        }
    }
    return p;
}

unsafe fn checkSizes(mut L: *mut lua_State, g: *const Lua) {
    if (*g).gcemergency.get() == 0 {
        if (*(*g).strt.get()).nuse < (*(*g).strt.get()).size / 4 {
            let mut olddebt: isize = (*g).GCdebt.get();
            luaS_resize(L, (*(*g).strt.get()).size / 2 as libc::c_int);
            (*g).GCestimate
                .set(((*g).GCestimate.get()).wrapping_add(((*g).GCdebt.get() - olddebt) as usize));
        }
    }
}

unsafe fn findlast(mut p: *mut *mut GCObject) -> *mut *mut GCObject {
    while !(*p).is_null() {
        p = &mut (**p).next;
    }
    return p;
}

unsafe fn checkpointer(mut p: *mut *mut GCObject, mut o: *mut GCObject) {
    if o == *p {
        *p = (*o).next;
    }
}

unsafe fn correctpointers(g: &Lua, mut o: *mut GCObject) {
    checkpointer((*g).survival.as_ptr(), o);
    checkpointer((*g).old1.as_ptr(), o);
    checkpointer((*g).reallyold.as_ptr(), o);
    checkpointer((*g).firstold1.as_ptr(), o);
}

unsafe fn setpause(g: *const Lua) {
    let mut threshold: isize = 0;
    let mut debt: isize = 0;
    let mut pause: libc::c_int = (*g).gcpause.get() as libc::c_int * 4 as libc::c_int;
    let mut estimate: isize = ((*g).GCestimate.get() / 100) as isize;

    threshold = if (pause as isize)
        < (!(0 as libc::c_int as usize) >> 1 as libc::c_int) as isize / estimate
    {
        estimate * pause as isize
    } else {
        (!(0 as libc::c_int as usize) >> 1 as libc::c_int) as isize
    };

    debt = (((*g).totalbytes.get() + (*g).GCdebt.get()) as usize).wrapping_sub(threshold as usize)
        as isize;

    if debt > 0 as libc::c_int as isize {
        debt = 0 as libc::c_int as isize;
    }
    luaE_setdebt(g, debt);
}

unsafe fn sweep2old(mut L: *mut lua_State, mut p: *mut *mut GCObject) {
    let mut curr: *mut GCObject = 0 as *mut GCObject;
    let g = (*L).l_G;
    loop {
        curr = *p;
        if curr.is_null() {
            break;
        }
        if (*curr).marked & (1 << 3 | 1 << 4) != 0 {
            *p = (*curr).next;
            freeobj(g, curr);
        } else {
            (*curr).marked =
                ((*curr).marked as libc::c_int & !(7 as libc::c_int) | 4 as libc::c_int) as u8;
            if (*curr).tt as libc::c_int
                == 8 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
            {
                let mut th: *mut lua_State = curr as *mut lua_State;
                linkgclist_(
                    th as *mut GCObject,
                    &mut (*th).gclist,
                    (*g).grayagain.as_ptr(),
                );
            } else if (*curr).tt as libc::c_int
                == 9 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                && (*(curr as *mut UpVal)).v.p
                    != &mut (*(curr as *mut UpVal)).u.value as *mut TValue
            {
                (*curr).marked = ((*curr).marked as libc::c_int
                    & !((1 as libc::c_int) << 5 as libc::c_int
                        | ((1 as libc::c_int) << 3 as libc::c_int
                            | (1 as libc::c_int) << 4 as libc::c_int)) as u8
                        as libc::c_int) as u8;
            } else {
                (*curr).marked =
                    ((*curr).marked as libc::c_int | (1 as libc::c_int) << 5 as libc::c_int) as u8;
            }
            p = &mut (*curr).next;
        }
    }
}

unsafe fn sweepgen(
    g: *const Lua,
    mut p: *mut *mut GCObject,
    mut limit: *mut GCObject,
    mut pfirstold1: *mut *mut GCObject,
) -> *mut *mut GCObject {
    static mut nextage: [u8; 7] = [
        1 as libc::c_int as u8,
        3 as libc::c_int as u8,
        3 as libc::c_int as u8,
        4 as libc::c_int as u8,
        4 as libc::c_int as u8,
        5 as libc::c_int as u8,
        6 as libc::c_int as u8,
    ];
    let mut white: libc::c_int = ((*g).currentwhite.get() as libc::c_int
        & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int))
        as u8 as libc::c_int;
    let mut curr: *mut GCObject = 0 as *mut GCObject;
    loop {
        curr = *p;
        if !(curr != limit) {
            break;
        }
        if (*curr).marked & (1 << 3 | 1 << 4) != 0 {
            *p = (*curr).next;
            freeobj(g, curr);
        } else {
            if (*curr).marked as libc::c_int & 7 as libc::c_int == 0 as libc::c_int {
                let mut marked: libc::c_int = (*curr).marked as libc::c_int
                    & !((1 as libc::c_int) << 5 as libc::c_int
                        | ((1 as libc::c_int) << 3 as libc::c_int
                            | (1 as libc::c_int) << 4 as libc::c_int)
                        | 7 as libc::c_int);
                (*curr).marked = (marked | 1 as libc::c_int | white) as u8;
            } else {
                (*curr).marked = ((*curr).marked as libc::c_int & !(7 as libc::c_int)
                    | nextage[((*curr).marked as libc::c_int & 7 as libc::c_int) as usize]
                        as libc::c_int) as u8;
                if (*curr).marked as libc::c_int & 7 as libc::c_int == 3 as libc::c_int
                    && (*pfirstold1).is_null()
                {
                    *pfirstold1 = curr;
                }
            }
            p = &mut (*curr).next;
        }
    }
    return p;
}

unsafe fn whitelist(g: *const Lua, mut p: *mut GCObject) {
    let white = (*g).currentwhite.get() & (1 << 3 | 1 << 4);

    while !p.is_null() {
        (*p).marked = (*p).marked & !(1 << 5 | (1 << 3 | 1 << 4) | 7) | white;
        p = (*p).next;
    }
}

unsafe fn correctgraylist(mut p: *mut *mut GCObject) -> *mut *mut GCObject {
    let mut current_block: u64;
    let mut curr: *mut GCObject = 0 as *mut GCObject;
    loop {
        curr = *p;
        if curr.is_null() {
            break;
        }
        let mut next: *mut *mut GCObject = getgclist(curr);
        if !((*curr).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            != 0)
        {
            if (*curr).marked as libc::c_int & 7 as libc::c_int == 5 as libc::c_int {
                (*curr).marked =
                    ((*curr).marked as libc::c_int | (1 as libc::c_int) << 5 as libc::c_int) as u8;
                (*curr).marked =
                    ((*curr).marked as libc::c_int ^ (5 as libc::c_int ^ 6 as libc::c_int)) as u8;
                current_block = 1342372514885429604;
            } else if (*curr).tt as libc::c_int
                == 8 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
            {
                current_block = 1342372514885429604;
            } else {
                if (*curr).marked as libc::c_int & 7 as libc::c_int == 6 as libc::c_int {
                    (*curr).marked = ((*curr).marked as libc::c_int
                        ^ (6 as libc::c_int ^ 4 as libc::c_int))
                        as u8;
                }
                (*curr).marked =
                    ((*curr).marked as libc::c_int | (1 as libc::c_int) << 5 as libc::c_int) as u8;
                current_block = 10379503568882611001;
            }
            match current_block {
                10379503568882611001 => {}
                _ => {
                    p = next;
                    continue;
                }
            }
        }
        *p = *next;
    }
    return p;
}

unsafe fn correctgraylists(g: *const Lua) {
    let mut list: *mut *mut GCObject = correctgraylist((*g).grayagain.as_ptr());
    *list = (*g).weak.get();
    (*g).weak.set(0 as *mut GCObject);
    list = correctgraylist(list);
    *list = (*g).allweak.get();
    (*g).allweak.set(0 as *mut GCObject);
    list = correctgraylist(list);
    *list = (*g).ephemeron.get();
    (*g).ephemeron.set(0 as *mut GCObject);
    correctgraylist(list);
}

unsafe fn markold(g: *const Lua, mut from: *mut GCObject, mut to: *mut GCObject) {
    let mut p: *mut GCObject = 0 as *mut GCObject;
    p = from;
    while p != to {
        if (*p).marked as libc::c_int & 7 as libc::c_int == 3 as libc::c_int {
            (*p).marked =
                ((*p).marked as libc::c_int ^ (3 as libc::c_int ^ 4 as libc::c_int)) as u8;
            if (*p).marked as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0 {
                reallymarkobject(g, p);
            }
        }
        p = (*p).next;
    }
}

unsafe fn finishgencycle(mut L: *mut lua_State, g: *const Lua) {
    correctgraylists(g);
    checkSizes(L, g);
    (*g).gcstate.set(0);
}

unsafe fn youngcollection(mut L: *mut lua_State, g: *const Lua) {
    let mut psurvival: *mut *mut GCObject = 0 as *mut *mut GCObject;
    if !((*g).firstold1.get()).is_null() {
        markold(g, (*g).firstold1.get(), (*g).reallyold.get());
        (*g).firstold1.set(0 as *mut GCObject);
    }

    atomic(L);
    (*g).gcstate.set(3);
    psurvival = sweepgen(
        g,
        (*g).allgc.as_ptr(),
        (*g).survival.get(),
        (*g).firstold1.as_ptr(),
    );
    sweepgen(g, psurvival, (*g).old1.get(), (*g).firstold1.as_ptr());
    (*g).reallyold.set((*g).old1.get());
    (*g).old1.set(*psurvival);
    (*g).survival.set((*g).allgc.get());

    finishgencycle(L, g);
}

unsafe fn atomic2gen(mut L: *mut lua_State, g: *const Lua) {
    cleargraylists(g);
    (*g).gcstate.set(3);
    sweep2old(L, (*g).allgc.as_ptr());
    (*g).survival.set((*g).allgc.get());
    (*g).old1.set((*g).survival.get());
    (*g).reallyold.set((*g).old1.get());
    (*g).firstold1.set(0 as *mut GCObject);
    (*g).gckind.set(1);
    (*g).lastatomic.set(0);
    (*g).GCestimate
        .set(((*g).totalbytes.get() + (*g).GCdebt.get()) as usize);
    finishgencycle(L, g);
}

unsafe fn setminordebt(g: *const Lua) {
    luaE_setdebt(
        g,
        -((((*g).totalbytes.get() + (*g).GCdebt.get()) as usize / 100) as isize
            * (*g).genminormul.get() as isize),
    );
}

unsafe fn entergen(mut L: *mut lua_State, g: *const Lua) -> usize {
    let mut numobjs: usize = 0;
    luaC_runtilstate(L, (1 as libc::c_int) << 8 as libc::c_int);
    luaC_runtilstate(L, (1 as libc::c_int) << 0 as libc::c_int);
    numobjs = atomic(L);
    atomic2gen(L, g);
    setminordebt(g);
    return numobjs;
}

unsafe fn enterinc(g: *const Lua) {
    whitelist(g, (*g).allgc.get());
    (*g).survival.set(0 as *mut GCObject);
    (*g).old1.set((*g).survival.get());
    (*g).reallyold.set((*g).old1.get());
    (*g).gcstate.set(8);
    (*g).gckind.set(0);
    (*g).lastatomic.set(0);
}

pub unsafe fn luaC_changemode(mut L: *mut lua_State, mut newmode: libc::c_int) {
    let g = (*L).l_G;
    if newmode != (*g).gckind.get() as libc::c_int {
        if newmode == 1 {
            entergen(L, g);
        } else {
            enterinc(g);
        }
    }
    (*g).lastatomic.set(0);
}

unsafe fn fullgen(mut L: *mut lua_State, g: *const Lua) -> usize {
    enterinc(g);
    return entergen(L, g);
}

unsafe fn stepgenfull(mut L: *mut lua_State, g: *const Lua) {
    let mut newatomic: usize = 0;
    let mut lastatomic: usize = (*g).lastatomic.get();

    if (*g).gckind.get() == 1 {
        enterinc(g);
    }

    luaC_runtilstate(L, (1 as libc::c_int) << 0 as libc::c_int);
    newatomic = atomic(L);
    if newatomic < lastatomic.wrapping_add(lastatomic >> 3 as libc::c_int) {
        atomic2gen(L, g);
        setminordebt(g);
    } else {
        (*g).GCestimate
            .set(((*g).totalbytes.get() + (*g).GCdebt.get()) as usize);
        entersweep(L);
        luaC_runtilstate(L, (1 as libc::c_int) << 8 as libc::c_int);
        setpause(g);
        (*g).lastatomic.set(newatomic);
    };
}

unsafe fn genstep(mut L: *mut lua_State, g: *const Lua) {
    if (*g).lastatomic.get() != 0 {
        stepgenfull(L, g);
    } else {
        let mut majorbase: usize = (*g).GCestimate.get();
        let mut majorinc: usize =
            majorbase / 100 as usize * ((*g).genmajormul.get() as libc::c_int * 4) as usize;

        if (*g).GCdebt.get() > 0
            && ((*g).totalbytes.get() + (*g).GCdebt.get()) as usize
                > majorbase.wrapping_add(majorinc)
        {
            let mut numobjs: usize = fullgen(L, g);
            if !((((*g).totalbytes.get() + (*g).GCdebt.get()) as usize)
                < majorbase.wrapping_add(majorinc / 2 as libc::c_int as usize))
            {
                (*g).lastatomic.set(numobjs);
                setpause(g);
            }
        } else {
            youngcollection(L, g);
            setminordebt(g);
            (*g).GCestimate.set(majorbase);
        }
    };
}

unsafe fn entersweep(mut L: *mut lua_State) {
    let g = (*L).l_G;
    (*g).gcstate.set(3);
    (*g).sweepgc.set(sweeptolive(L, (*g).allgc.as_ptr()));
}

unsafe fn deletelist(g: &Lua, mut p: *mut GCObject, mut limit: *mut GCObject) {
    while p != limit {
        let mut next: *mut GCObject = (*p).next;
        freeobj(g, p);
        p = next;
    }
}

pub unsafe fn luaC_freeallobjects(g: &Lua) {
    g.gcstp.set(4);

    if g.gckind.get() != 0 {
        enterinc(g);
    }

    (*g).lastatomic.set(0);

    deletelist(g, (*g).allgc.get(), null_mut());
    deletelist(g, (*g).fixedgc.get(), null_mut());
}

unsafe fn atomic(mut L: *mut lua_State) -> usize {
    let g = (*L).l_G;
    let mut work: usize = 0 as libc::c_int as usize;
    let mut origweak: *mut GCObject = 0 as *mut GCObject;
    let mut origall: *mut GCObject = 0 as *mut GCObject;
    let mut grayagain: *mut GCObject = (*g).grayagain.get();

    (*g).grayagain.set(0 as *mut GCObject);
    (*g).gcstate.set(2);

    if (*L).marked & (1 << 3 | 1 << 4) != 0 {
        reallymarkobject(g, L as *mut GCObject);
    }

    if (*(*g).l_registry.get()).tt_ & 1 << 6 != 0
        && (*(*(*g).l_registry.get()).value_.gc).marked & (1 << 3 | 1 << 4) != 0
    {
        reallymarkobject(g, (*(*g).l_registry.get()).value_.gc);
    }

    markmt(g);
    work = work.wrapping_add(propagateall(g));
    work = work.wrapping_add(remarkupvals(g) as usize);
    work = work.wrapping_add(propagateall(g));
    (*g).gray.set(grayagain);
    work = work.wrapping_add(propagateall(g));
    convergeephemerons(g);
    clearbyvalues(g, (*g).weak.get(), 0 as *mut GCObject);
    clearbyvalues(g, (*g).allweak.get(), 0 as *mut GCObject);
    origweak = (*g).weak.get();
    origall = (*g).allweak.get();
    work = work.wrapping_add(propagateall(g));
    convergeephemerons(g);
    clearbykeys(g, (*g).ephemeron.get());
    clearbykeys(g, (*g).allweak.get());
    clearbyvalues(g, (*g).weak.get(), origweak);
    clearbyvalues(g, (*g).allweak.get(), origall);
    luaS_clearcache(g);

    (*g).currentwhite
        .set((*g).currentwhite.get() ^ (1 << 3 | 1 << 4));

    return work;
}

unsafe fn sweepstep(
    mut L: *mut lua_State,
    g: *const Lua,
    mut nextstate: libc::c_int,
    mut nextlist: *mut *mut GCObject,
) -> libc::c_int {
    if !((*g).sweepgc.get()).is_null() {
        let mut olddebt: isize = (*g).GCdebt.get();
        let mut count: libc::c_int = 0;
        (*g).sweepgc.set(sweeplist(
            L,
            (*g).sweepgc.get(),
            100 as libc::c_int,
            &mut count,
        ));
        (*g).GCestimate
            .set(((*g).GCestimate.get()).wrapping_add(((*g).GCdebt.get() - olddebt) as usize));
        return count;
    } else {
        (*g).gcstate.set(nextstate as u8);
        (*g).sweepgc.set(nextlist);
        return 0 as libc::c_int;
    };
}

unsafe fn singlestep(mut L: *mut lua_State) -> usize {
    let g = (*L).l_G;
    let mut work: usize = 0;

    (*g).gcstopem.set(1 as libc::c_int as u8);

    match (*g).gcstate.get() {
        8 => {
            restartcollection(g);
            (*g).gcstate.set(0 as libc::c_int as u8);
            work = 1 as libc::c_int as usize;
        }
        0 => {
            if ((*g).gray.get()).is_null() {
                (*g).gcstate.set(1);
                work = 0 as libc::c_int as usize;
            } else {
                work = propagatemark(g);
            }
        }
        1 => {
            work = atomic(L);
            entersweep(L);
            (*g).GCestimate
                .set(((*g).totalbytes.get() + (*g).GCdebt.get()) as usize);
        }
        3 => work = sweepstep(L, g, 6, 0 as *mut *mut GCObject) as usize,
        6 => {
            checkSizes(L, g);
            (*g).gcstate.set(7 as libc::c_int as u8);
            work = 0 as libc::c_int as usize;
        }
        7 => {
            (*g).gcstate.set(8 as libc::c_int as u8);
            work = 0 as libc::c_int as usize;
        }
        _ => return 0 as libc::c_int as usize,
    }

    (*g).gcstopem.set(0 as libc::c_int as u8);

    return work;
}

pub unsafe fn luaC_runtilstate(mut L: *mut lua_State, mut statesmask: libc::c_int) {
    let g = (*L).l_G;

    while statesmask & (1 as libc::c_int) << (*g).gcstate.get() == 0 {
        singlestep(L);
    }
}

unsafe fn incstep(mut L: *mut lua_State, g: *const Lua) {
    let mut stepmul: libc::c_int =
        (*g).gcstepmul.get() as libc::c_int * 4 as libc::c_int | 1 as libc::c_int;
    let mut debt: isize = ((*g).GCdebt.get() as libc::c_ulong)
        .wrapping_div(::core::mem::size_of::<TValue>() as libc::c_ulong)
        .wrapping_mul(stepmul as libc::c_ulong) as isize;
    let mut stepsize: isize = (if (*g).gcstepsize.get() as libc::c_ulong
        <= (::core::mem::size_of::<isize>() as libc::c_ulong)
            .wrapping_mul(8 as libc::c_int as libc::c_ulong)
            .wrapping_sub(2 as libc::c_int as libc::c_ulong)
    {
        (((1 as libc::c_int as isize) << (*g).gcstepsize.get() as libc::c_int) as libc::c_ulong)
            .wrapping_div(::core::mem::size_of::<TValue>() as libc::c_ulong)
            .wrapping_mul(stepmul as libc::c_ulong)
    } else {
        (!(0 as libc::c_int as usize) >> 1 as libc::c_int) as isize as libc::c_ulong
    }) as isize;
    loop {
        let mut work: usize = singlestep(L);
        debt = (debt as usize).wrapping_sub(work) as isize as isize;

        if !(debt > -stepsize && (*g).gcstate.get() != 8) {
            break;
        }
    }

    if (*g).gcstate.get() == 8 {
        setpause(g);
    } else {
        debt = ((debt / stepmul as isize) as libc::c_ulong)
            .wrapping_mul(::core::mem::size_of::<TValue>() as libc::c_ulong)
            as isize;
        luaE_setdebt(g, debt);
    };
}

pub unsafe fn luaC_step(mut L: *mut lua_State) {
    let g = (*L).l_G;

    if !((*g).gcstp.get() == 0) {
        luaE_setdebt(g, -(2000 as libc::c_int) as isize);
    } else if (*g).gckind.get() == 1 || (*g).lastatomic.get() != 0 {
        genstep(L, g);
    } else {
        incstep(L, g);
    };
}

unsafe fn fullinc(mut L: *mut lua_State, g: *const Lua) {
    if (*g).gcstate.get() <= 2 {
        entersweep(L);
    }

    luaC_runtilstate(L, (1 as libc::c_int) << 8 as libc::c_int);
    luaC_runtilstate(L, (1 as libc::c_int) << 0 as libc::c_int);
    (*g).gcstate.set(1 as libc::c_int as u8);
    luaC_runtilstate(L, (1 as libc::c_int) << 7 as libc::c_int);
    luaC_runtilstate(L, (1 as libc::c_int) << 8 as libc::c_int);
    setpause(g);
}

pub unsafe fn luaC_fullgc(mut L: *mut lua_State, mut isemergency: libc::c_int) {
    let g = (*L).l_G;

    (*g).gcemergency.set(isemergency as u8);

    if (*g).gckind.get() == 0 {
        fullinc(L, g);
    } else {
        fullgen(L, g);
    }

    (*g).gcemergency.set(0 as libc::c_int as u8);
}
