#![allow(
    mutable_transmutes,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments
)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::Lua;
use crate::ldo::luaD_shrinkstack;
use crate::lfunc::{luaF_freeproto, luaF_unlinkupval};
use crate::lobject::{
    CClosure, GCObject, LClosure, Node, Proto, StkId, TString, TValue, Table, UValue, Udata, UpVal,
};
use crate::lstate::{lua_State, luaE_freethread};
use crate::lstring::{luaS_clearcache, luaS_remove};
use crate::ltable::{luaH_free, luaH_realasize};
use crate::ltm::{TM_MODE, luaT_gettm};
use libc::strchr;
use std::alloc::{Layout, handle_alloc_error};
use std::cell::Cell;
use std::ffi::c_int;
use std::mem::offset_of;
use std::ptr::null_mut;

unsafe fn getgclist(o: *mut GCObject) -> *mut *mut GCObject {
    match (*o).tt {
        5 => &raw mut (*(o as *mut Table)).gclist,
        6 => &raw mut (*(o as *mut LClosure)).gclist,
        38 => &raw mut (*(o as *mut CClosure)).gclist,
        8 => &raw mut (*(o as *mut lua_State)).gclist,
        10 => &raw mut (*(o as *mut Proto)).gclist,
        7 => &raw mut (*(o as *mut Udata)).gclist,
        _ => null_mut(),
    }
}

unsafe fn linkgclist_(o: *mut GCObject, pnext: *mut *mut GCObject, list: *mut *mut GCObject) {
    *pnext = *list;
    *list = o;

    (*o).marked = (*o).marked & !(1 << 5 | (1 << 3 | 1 << 4));
}

unsafe fn clearkey(n: *mut Node) {
    if (*n).u.key_tt as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0 {
        (*n).u.key_tt = (9 as libc::c_int + 2 as libc::c_int) as u8;
    }
}

unsafe fn iscleared(g: *const Lua, o: *const GCObject) -> libc::c_int {
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

pub(crate) unsafe fn luaC_barrier_(L: *mut lua_State, o: *mut GCObject, v: *mut GCObject) {
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
            | ((*g).gc.currentwhite() as libc::c_int
                & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int))
                as u8 as libc::c_int) as u8;
    }
}

pub(crate) unsafe fn luaC_barrierback_(L: *mut lua_State, o: *mut GCObject) {
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

pub(crate) unsafe fn luaC_fix(g: &Lua, o: *mut GCObject) {
    (*o).marked = (*o).marked & !(1 << 5 | (1 << 3 | 1 << 4));
    (*o).marked = ((*o).marked as libc::c_int & !(7 as libc::c_int) | 4 as libc::c_int) as u8;
    (*g).gc.allgc.set((*o).next);
    (*o).next = (*g).fixedgc.get();
    (*g).fixedgc.set(o);
}

unsafe fn reallymarkobject(g: *const Lua, o: *mut GCObject) {
    match (*o).tt {
        4 | 20 => {
            (*o).marked = (*o).marked & !(1 << 3 | 1 << 4) | 1 << 5;
            return;
        }
        9 => {
            let uv: *mut UpVal = o as *mut UpVal;
            if (*uv).v.p != &mut (*uv).u.value as *mut TValue {
                (*uv).marked = ((*uv).marked as libc::c_int
                    & !((1 as libc::c_int) << 5 as libc::c_int
                        | ((1 as libc::c_int) << 3 as libc::c_int
                            | (1 as libc::c_int) << 4 as libc::c_int)) as u8
                        as libc::c_int) as u8;
            } else {
                (*uv).marked = (*uv).marked & !(1 << 3 | 1 << 4) | 1 << 5;
            }

            if (*(*uv).v.p).tt_ & 1 << 6 != 0
                && (*(*(*uv).v.p).value_.gc).marked & (1 << 3 | 1 << 4) != 0
            {
                reallymarkobject(g, (*(*uv).v.p).value_.gc);
            }

            return;
        }
        7 => {
            let u: *mut Udata = o as *mut Udata;

            if (*u).nuvalue == 0 {
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

unsafe fn cleargraylists(g: &Lua) {
    g.grayagain.set(null_mut());
    g.gray.set(null_mut());
    g.ephemeron.set(null_mut());
    g.allweak.set(null_mut());
    g.weak.set(null_mut());
}

unsafe fn genlink(g: *const Lua, o: *mut GCObject) {
    if (*o).marked as libc::c_int & 7 as libc::c_int == 5 as libc::c_int {
        linkgclist_(o as *mut GCObject, getgclist(o), (*g).grayagain.as_ptr());
    } else if (*o).marked as libc::c_int & 7 as libc::c_int == 6 as libc::c_int {
        (*o).marked = ((*o).marked as libc::c_int ^ (6 as libc::c_int ^ 4 as libc::c_int)) as u8;
    }
}

unsafe fn traverseweakvalue(g: *const Lua, h: *mut Table) {
    let mut n: *mut Node = 0 as *mut Node;
    let limit: *mut Node = &mut *((*h).node)
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

unsafe fn traverseephemeron(g: *const Lua, h: *mut Table, inv: libc::c_int) -> libc::c_int {
    let mut marked: libc::c_int = 0 as libc::c_int;
    let mut hasclears: libc::c_int = 0 as libc::c_int;
    let mut hasww: libc::c_int = 0 as libc::c_int;
    let mut i: libc::c_uint = 0;
    let asize: libc::c_uint = luaH_realasize(h);
    let nsize: libc::c_uint = ((1 as libc::c_int) << (*h).lsizenode as libc::c_int) as libc::c_uint;
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
        let n: *mut Node = if inv != 0 {
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

unsafe fn traversestrongtable(g: *const Lua, h: *mut Table) {
    let mut n: *mut Node = 0 as *mut Node;
    let limit: *mut Node = &mut *((*h).node)
        .offset(((1 as libc::c_int) << (*h).lsizenode as libc::c_int) as usize as isize)
        as *mut Node;
    let mut i: libc::c_uint = 0;
    let asize: libc::c_uint = luaH_realasize(h);
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

unsafe fn traversetable(g: *const Lua, h: *mut Table) -> usize {
    let mut weakkey: *const libc::c_char = 0 as *const libc::c_char;
    let mut weakvalue: *const libc::c_char = 0 as *const libc::c_char;
    let mode: *const TValue = if ((*h).metatable).is_null() {
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

unsafe fn traverseudata(g: *const Lua, u: *mut Udata) -> libc::c_int {
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

unsafe fn traverseproto(g: *const Lua, f: *mut Proto) -> libc::c_int {
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

unsafe fn traverseCclosure(g: *const Lua, cl: *mut CClosure) -> libc::c_int {
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

unsafe fn traverseLclosure(g: *const Lua, cl: *mut LClosure) -> libc::c_int {
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
        let uv: *mut UpVal = *((*cl).upvals).as_mut_ptr().offset(i as isize);
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

unsafe fn traversethread(g: *const Lua, th: *mut lua_State) -> libc::c_int {
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
        luaD_shrinkstack(th);

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

unsafe fn propagatemark(g: &Lua) -> usize {
    let o: *mut GCObject = g.gray.get();

    (*o).marked |= 1 << 5;
    (*g).gray.set(*getgclist(o));

    match (*o).tt {
        5 => traversetable(g, o as *mut Table),
        7 => traverseudata(g, o as *mut Udata) as usize,
        6 => traverseLclosure(g, o as *mut LClosure) as usize,
        38 => traverseCclosure(g, o as *mut CClosure) as usize,
        10 => traverseproto(g, o as *mut Proto) as usize,
        8 => traversethread(g, o as *mut lua_State) as usize,
        _ => 0,
    }
}

unsafe fn propagateall(g: *const Lua) -> usize {
    let mut tot: usize = 0 as libc::c_int as usize;
    while !((*g).gray.get()).is_null() {
        tot = tot.wrapping_add(propagatemark(&*g));
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
            let h: *mut Table = w as *mut Table;
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
        let h: *mut Table = l as *mut Table;
        let limit: *mut Node = &mut *((*h).node)
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

unsafe fn clearbyvalues(g: *const Lua, mut l: *mut GCObject, f: *mut GCObject) {
    while l != f {
        let h: *mut Table = l as *mut Table;
        let mut n: *mut Node = 0 as *mut Node;
        let limit: *mut Node = &mut *((*h).node)
            .offset(((1 as libc::c_int) << (*h).lsizenode as libc::c_int) as usize as isize)
            as *mut Node;
        let mut i: libc::c_uint = 0;
        let asize: libc::c_uint = luaH_realasize(h);
        i = 0 as libc::c_int as libc::c_uint;
        while i < asize {
            let o: *mut TValue = &mut *((*h).array).offset(i as isize) as *mut TValue;
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

unsafe fn freeupval(g: *const Lua, uv: *mut UpVal) {
    let layout = Layout::new::<UpVal>();

    if (*uv).v.p != &mut (*uv).u.value as *mut TValue {
        luaF_unlinkupval(uv);
    }

    (*g).gc.dealloc(uv.cast(), layout);
}

unsafe fn freeobj(g: *const Lua, o: *mut GCObject) {
    (*g).gcstp.set((*g).gcstp.get() | 2);

    match (*o).tt {
        10 => luaF_freeproto(g, o as *mut Proto),
        9 => freeupval(g, o as *mut UpVal),
        6 => {
            let cl: *mut LClosure = o as *mut LClosure;
            let nupvalues = usize::from((*cl).nupvalues);
            let size = offset_of!(LClosure, upvals) + size_of::<*mut TValue>() * nupvalues;
            let align = align_of::<LClosure>();
            let layout = Layout::from_size_align(size, align).unwrap().pad_to_align();

            (*g).gc.dealloc(cl.cast(), layout);
        }
        38 => {
            let cl_0: *mut CClosure = o as *mut CClosure;
            let nupvalues = usize::from((*cl_0).nupvalues);
            let size = offset_of!(CClosure, upvalue) + size_of::<TValue>() * nupvalues;
            let align = align_of::<CClosure>();
            let layout = Layout::from_size_align(size, align).unwrap().pad_to_align();

            (*g).gc.dealloc(cl_0.cast(), layout);
        }
        5 => luaH_free(g, o as *mut Table),
        8 => luaE_freethread(g, o as *mut lua_State),
        7 => {
            let u: *mut Udata = o as *mut Udata;
            let layout = Layout::from_size_align(
                (if (*u).nuvalue == 0 {
                    offset_of!(Udata, gclist)
                } else {
                    offset_of!(Udata, uv) + size_of::<UValue>().wrapping_mul((*u).nuvalue.into())
                })
                .wrapping_add((*u).len),
                align_of::<Udata>(),
            )
            .unwrap()
            .pad_to_align();

            (*g).gc.dealloc(o.cast(), layout);
        }
        4 => {
            let ts: *mut TString = o as *mut TString;
            let size = offset_of!(TString, contents) + usize::from((*ts).shrlen) + 1;
            let align = align_of::<TString>();
            let layout = Layout::from_size_align(size, align).unwrap().pad_to_align();

            luaS_remove(g, ts);
            (*g).gc.dealloc(ts.cast(), layout);
        }
        20 => {
            let ts_0: *mut TString = o as *mut TString;
            let size = offset_of!(TString, contents) + (*ts_0).u.lnglen + 1;
            let align = align_of::<TString>();
            let layout = Layout::from_size_align(size, align).unwrap().pad_to_align();

            (*g).gc.dealloc(ts_0.cast(), layout);
        }
        _ => unreachable!(),
    }

    (*g).gcstp.set((*g).gcstp.get() & !2);
}

unsafe fn sweeplist(
    L: *mut lua_State,
    mut p: *mut *mut GCObject,
    countin: libc::c_int,
    countout: *mut libc::c_int,
) -> *mut *mut GCObject {
    let g = &*(*L).l_G;
    let ow = g.gc.currentwhite() ^ (1 << 3 | 1 << 4);
    let mut i = 0;
    let white = g.gc.currentwhite() & (1 << 3 | 1 << 4);

    while !(*p).is_null() && i < countin {
        let curr: *mut GCObject = *p;
        let marked = (*curr).marked;

        if marked & ow != 0 {
            *p = (*curr).next;
            freeobj(g, curr);
        } else {
            (*curr).marked = marked & !(1 << 5 | (1 << 3 | 1 << 4) | 7) | white;
            p = &raw mut (*curr).next;
        }

        i += 1;
    }

    if !countout.is_null() {
        *countout = i;
    }

    if (*p).is_null() { null_mut() } else { p }
}

/// Sweep a list until a live object (or end of list).
unsafe fn sweeptolive(L: *mut lua_State, mut p: *mut *mut GCObject) -> *mut *mut GCObject {
    let old: *mut *mut GCObject = p;

    loop {
        p = sweeplist(L, p, 1, 0 as *mut libc::c_int);
        if p != old {
            break;
        }
    }

    return p;
}

unsafe fn setpause(g: *const Lua) {
    let mut threshold: isize = 0;
    let mut debt: isize = 0;
    let pause: libc::c_int = (*g).gcpause.get() as libc::c_int * 4 as libc::c_int;
    let estimate: isize = ((*g).GCestimate.get() / 100) as isize;

    threshold = if (pause as isize)
        < (!(0 as libc::c_int as usize) >> 1 as libc::c_int) as isize / estimate
    {
        estimate * pause as isize
    } else {
        (!(0 as libc::c_int as usize) >> 1 as libc::c_int) as isize
    };

    debt = (((*g).gc.totalbytes.get() + (*g).gc.debt.get()) as usize)
        .wrapping_sub(threshold as usize) as isize;

    if debt > 0 as libc::c_int as isize {
        debt = 0 as libc::c_int as isize;
    }

    (*g).gc.set_debt(debt);
}

unsafe fn sweep2old(L: *mut lua_State, mut p: *mut *mut GCObject) {
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
                let th: *mut lua_State = curr as *mut lua_State;
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
    limit: *mut GCObject,
    pfirstold1: *mut *mut GCObject,
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
    let white: libc::c_int = ((*g).gc.currentwhite() as libc::c_int
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
                let marked: libc::c_int = (*curr).marked as libc::c_int
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
    let white = (*g).gc.currentwhite() & (1 << 3 | 1 << 4);

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
        let next: *mut *mut GCObject = getgclist(curr);
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

unsafe fn markold(g: *const Lua, from: *mut GCObject, to: *mut GCObject) {
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

unsafe fn finishgencycle(g: *const Lua) {
    correctgraylists(g);
    (*g).gcstate.set(0);
}

unsafe fn youngcollection(L: *mut lua_State, g: *const Lua) {
    let mut psurvival: *mut *mut GCObject = 0 as *mut *mut GCObject;
    if !((*g).firstold1.get()).is_null() {
        markold(g, (*g).firstold1.get(), (*g).reallyold.get());
        (*g).firstold1.set(0 as *mut GCObject);
    }

    atomic(L);
    (*g).gcstate.set(3);
    psurvival = sweepgen(
        g,
        (*g).gc.allgc.as_ptr(),
        (*g).survival.get(),
        (*g).firstold1.as_ptr(),
    );
    sweepgen(g, psurvival, (*g).old1.get(), (*g).firstold1.as_ptr());
    (*g).reallyold.set((*g).old1.get());
    (*g).old1.set(*psurvival);
    (*g).survival.set((*g).gc.allgc.get());

    finishgencycle(g);
}

unsafe fn atomic2gen(L: *mut lua_State, g: *const Lua) {
    cleargraylists(&*g);
    (*g).gcstate.set(3);
    sweep2old(L, (*g).gc.allgc.as_ptr());
    (*g).survival.set((*g).gc.allgc.get());
    (*g).old1.set((*g).survival.get());
    (*g).reallyold.set((*g).old1.get());
    (*g).firstold1.set(0 as *mut GCObject);
    (*g).gckind.set(1);
    (*g).lastatomic.set(0);
    (*g).GCestimate
        .set(((*g).gc.totalbytes.get() + (*g).gc.debt.get()) as usize);
    finishgencycle(g);
}

unsafe fn setminordebt(g: *const Lua) {
    (*g).gc.set_debt(
        -((((*g).gc.totalbytes.get() + (*g).gc.debt.get()) as usize / 100) as isize
            * (*g).genminormul.get() as isize),
    );
}

unsafe fn entergen(L: *mut lua_State, g: *const Lua) -> usize {
    let mut numobjs: usize = 0;
    luaC_runtilstate(L, (1 as libc::c_int) << 8 as libc::c_int);
    luaC_runtilstate(L, (1 as libc::c_int) << 0 as libc::c_int);
    numobjs = atomic(L);
    atomic2gen(L, g);
    setminordebt(g);
    return numobjs;
}

unsafe fn enterinc(g: *const Lua) {
    whitelist(g, (*g).gc.allgc.get());
    (*g).survival.set(0 as *mut GCObject);
    (*g).old1.set((*g).survival.get());
    (*g).reallyold.set((*g).old1.get());
    (*g).gcstate.set(8);
    (*g).gckind.set(0);
    (*g).lastatomic.set(0);
}

pub(crate) unsafe fn luaC_changemode(L: *mut lua_State, newmode: libc::c_int) {
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

unsafe fn fullgen(L: *mut lua_State, g: *const Lua) -> usize {
    enterinc(g);
    return entergen(L, g);
}

unsafe fn stepgenfull(L: *mut lua_State, g: *const Lua) {
    let mut newatomic: usize = 0;
    let lastatomic: usize = (*g).lastatomic.get();

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
            .set(((*g).gc.totalbytes.get() + (*g).gc.debt.get()) as usize);
        entersweep(L);
        luaC_runtilstate(L, (1 as libc::c_int) << 8 as libc::c_int);
        setpause(g);
        (*g).lastatomic.set(newatomic);
    };
}

unsafe fn genstep(L: *mut lua_State, g: *const Lua) {
    if (*g).lastatomic.get() != 0 {
        stepgenfull(L, g);
    } else {
        let majorbase: usize = (*g).GCestimate.get();
        let majorinc: usize =
            majorbase / 100 as usize * ((*g).genmajormul.get() as libc::c_int * 4) as usize;

        if (*g).gc.debt.get() > 0
            && ((*g).gc.totalbytes.get() + (*g).gc.debt.get()) as usize
                > majorbase.wrapping_add(majorinc)
        {
            let numobjs: usize = fullgen(L, g);
            if !((((*g).gc.totalbytes.get() + (*g).gc.debt.get()) as usize)
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

unsafe fn entersweep(L: *mut lua_State) {
    let g = (*L).l_G;
    (*g).gcstate.set(3);
    (*g).sweepgc.set(sweeptolive(L, (*g).gc.allgc.as_ptr()));
}

unsafe fn deletelist(g: &Lua, mut p: *mut GCObject, limit: *mut GCObject) {
    while p != limit {
        let next: *mut GCObject = (*p).next;
        freeobj(g, p);
        p = next;
    }
}

pub(crate) unsafe fn luaC_freeallobjects(g: &Lua) {
    g.gcstp.set(4);

    if g.gckind.get() != 0 {
        enterinc(g);
    }

    (*g).lastatomic.set(0);

    deletelist(g, (*g).gc.allgc.get(), null_mut());
    deletelist(g, (*g).fixedgc.get(), null_mut());
}

unsafe fn atomic(L: *mut lua_State) -> usize {
    let g = &*(*L).l_G;
    let mut work: usize = 0 as libc::c_int as usize;
    let mut origweak: *mut GCObject = 0 as *mut GCObject;
    let mut origall: *mut GCObject = 0 as *mut GCObject;
    let grayagain: *mut GCObject = g.grayagain.get();

    g.grayagain.set(null_mut());
    g.gcstate.set(2);

    if (*L).marked & (1 << 3 | 1 << 4) != 0 {
        reallymarkobject(g, L as *mut GCObject);
    }

    if (*g.l_registry.get()).tt_ & 1 << 6 != 0
        && (*(*g.l_registry.get()).value_.gc).marked & (1 << 3 | 1 << 4) != 0
    {
        reallymarkobject(g, (*g.l_registry.get()).value_.gc);
    }

    markmt(g);
    work = work.wrapping_add(propagateall(g));
    work = work.wrapping_add(remarkupvals(g) as usize);
    work = work.wrapping_add(propagateall(g));
    g.gray.set(grayagain);
    work = work.wrapping_add(propagateall(g));
    convergeephemerons(g);
    clearbyvalues(g, g.weak.get(), 0 as *mut GCObject);
    clearbyvalues(g, g.allweak.get(), 0 as *mut GCObject);
    origweak = g.weak.get();
    origall = g.allweak.get();
    work = work.wrapping_add(propagateall(g));
    convergeephemerons(g);
    clearbykeys(g, g.ephemeron.get());
    clearbykeys(g, g.allweak.get());
    clearbyvalues(g, g.weak.get(), origweak);
    clearbyvalues(g, g.allweak.get(), origall);
    luaS_clearcache(g);

    g.gc.currentwhite
        .set(g.gc.currentwhite() ^ (1 << 3 | 1 << 4));

    work
}

unsafe fn sweepstep(
    L: *mut lua_State,
    g: *const Lua,
    nextstate: libc::c_int,
    nextlist: *mut *mut GCObject,
) -> libc::c_int {
    if !((*g).sweepgc.get()).is_null() {
        let olddebt: isize = (*g).gc.debt.get();
        let mut count: libc::c_int = 0;
        (*g).sweepgc.set(sweeplist(
            L,
            (*g).sweepgc.get(),
            100 as libc::c_int,
            &mut count,
        ));
        (*g).GCestimate
            .set(((*g).GCestimate.get()).wrapping_add(((*g).gc.debt.get() - olddebt) as usize));
        return count;
    } else {
        (*g).gcstate.set(nextstate as u8);
        (*g).sweepgc.set(nextlist);
        return 0 as libc::c_int;
    };
}

unsafe fn singlestep(L: *mut lua_State) -> usize {
    let g = &*(*L).l_G;
    let mut work: usize = 0;

    g.gcstopem.set(1);

    match g.gcstate.get() {
        8 => {
            cleargraylists(g);
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
            work = atomic(L);
            entersweep(L);
            g.GCestimate
                .set((g.gc.totalbytes.get() + g.gc.debt.get()) as usize);
        }
        3 => work = sweepstep(L, g, 6, null_mut()) as usize,
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

pub(crate) unsafe fn luaC_runtilstate(L: *mut lua_State, statesmask: libc::c_int) {
    let g = (*L).l_G;

    while statesmask & (1 as libc::c_int) << (*g).gcstate.get() == 0 {
        singlestep(L);
    }
}

unsafe fn incstep(L: *mut lua_State, g: *const Lua) {
    let stepmul: libc::c_int =
        (*g).gcstepmul.get() as libc::c_int * 4 as libc::c_int | 1 as libc::c_int;
    let mut debt: isize = ((*g).gc.debt.get() as libc::c_ulong)
        .wrapping_div(::core::mem::size_of::<TValue>() as libc::c_ulong)
        .wrapping_mul(stepmul as libc::c_ulong) as isize;
    let stepsize: isize = (if (*g).gcstepsize.get() as libc::c_ulong
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
        let work: usize = singlestep(L);
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
        (*g).gc.set_debt(debt);
    };
}

pub(crate) unsafe fn luaC_step(L: *mut lua_State) {
    let g = (*L).l_G;

    if !((*g).gcstp.get() == 0) {
        (*g).gc.set_debt(-2000);
    } else if (*g).gckind.get() == 1 || (*g).lastatomic.get() != 0 {
        genstep(L, g);
    } else {
        incstep(L, g);
    };
}

unsafe fn fullinc(L: *mut lua_State, g: *const Lua) {
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

pub(crate) unsafe fn luaC_fullgc(L: *mut lua_State) {
    let g = (*L).l_G;

    if (*g).gckind.get() == 0 {
        fullinc(L, g);
    } else {
        fullgen(L, g);
    }
}

/// Garbage Collector for Lua objects.
pub struct Gc {
    currentwhite: Cell<u8>,
    totalbytes: Cell<isize>,
    debt: Cell<isize>,
    allgc: Cell<*mut GCObject>,
}

impl Gc {
    pub(super) fn new(totalbytes: usize) -> Self {
        Self {
            currentwhite: Cell::new(1 << 3),
            totalbytes: Cell::new(totalbytes.try_into().unwrap()),
            debt: Cell::new(0),
            allgc: Cell::new(null_mut()),
        }
    }

    #[inline(always)]
    pub(crate) fn currentwhite(&self) -> u8 {
        self.currentwhite.get()
    }

    #[inline(always)]
    pub(crate) fn totalbytes(&self) -> isize {
        self.totalbytes.get()
    }

    #[inline(always)]
    pub(crate) fn debt(&self) -> isize {
        self.debt.get()
    }

    pub(crate) fn set_debt(&self, mut debt: isize) {
        let tb: isize = self.totalbytes.get() + self.debt.get();

        if debt < tb - (!(0 as libc::c_int as usize) >> 1) as isize {
            debt = tb - (!(0 as libc::c_int as usize) >> 1 as libc::c_int) as isize;
        }

        self.totalbytes.set(tb - debt);
        self.debt.set(debt);
    }

    pub(crate) unsafe fn alloc(&self, tt: u8, layout: Layout) -> *mut GCObject {
        let o = unsafe { std::alloc::alloc(layout) as *mut GCObject };

        if o.is_null() {
            handle_alloc_error(layout);
        }

        unsafe { (*o).marked = self.currentwhite.get() & (1 << 3 | 1 << 4) };
        unsafe { (*o).tt = tt };
        unsafe { (*o).next = self.allgc.get() };

        self.allgc.set(o);
        self.debt
            .set(self.debt.get().checked_add_unsigned(layout.size()).unwrap());

        o
    }

    pub(crate) unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { std::alloc::dealloc(ptr, layout) };
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

/// Command to control the garbage collector.
pub enum GcCommand {
    Stop,
    Restart,
    Collect,
    Count,
    CountByte,
    Step(c_int),
    SetPause(c_int),
    SetStepMul(c_int),
    GetRunning,
    SetGen(c_int, c_int),
    SetInc(c_int, c_int, c_int),
}
