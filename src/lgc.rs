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
#![allow(unused_parens)]
#![allow(unused_variables)]
#![allow(path_statements)]

use crate::ldo::{luaD_callnoyield, luaD_pcall, luaD_shrinkstack};
use crate::lfunc::{luaF_freeproto, luaF_unlinkupval};
use crate::lmem::{luaM_free_, luaM_malloc_};
use crate::lobject::{
    CClosure, GCObject, LClosure, Node, Proto, StkId, TString, TValue, Table, UValue, Udata, UpVal,
    Value,
};
use crate::lstate::{
    GCUnion, global_State, lua_State, luaE_freethread, luaE_setdebt, luaE_warnerror,
};
use crate::lstring::{luaS_clearcache, luaS_remove, luaS_resize};
use crate::ltable::{luaH_free, luaH_realasize};
use crate::ltm::{TM_GC, TM_MODE, luaT_gettm, luaT_gettmbyobj};
use libc::strchr;

unsafe extern "C" fn getgclist(mut o: *mut GCObject) -> *mut *mut GCObject {
    match (*o).tt as libc::c_int {
        5 => return &mut (*(o as *mut GCUnion)).h.gclist,
        6 => return &mut (*(o as *mut GCUnion)).cl.l.gclist,
        38 => return &mut (*(o as *mut GCUnion)).cl.c.gclist,
        8 => return &mut (*(o as *mut GCUnion)).th.gclist,
        10 => return &mut (*(o as *mut GCUnion)).p.gclist,
        7 => {
            let mut u: *mut Udata = &mut (*(o as *mut GCUnion)).u;
            return &mut (*u).gclist;
        }
        _ => return 0 as *mut *mut GCObject,
    };
}

unsafe extern "C" fn linkgclist_(
    mut o: *mut GCObject,
    mut pnext: *mut *mut GCObject,
    mut list: *mut *mut GCObject,
) {
    *pnext = *list;
    *list = o;
    (*o).marked = ((*o).marked as libc::c_int
        & !((1 as libc::c_int) << 5 as libc::c_int
            | ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int))
            as u8 as libc::c_int) as u8;
}

unsafe extern "C" fn clearkey(mut n: *mut Node) {
    if (*n).u.key_tt as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0 {
        (*n).u.key_tt = (9 as libc::c_int + 2 as libc::c_int) as u8;
    }
}

unsafe extern "C" fn iscleared(mut g: *mut global_State, mut o: *const GCObject) -> libc::c_int {
    if o.is_null() {
        return 0 as libc::c_int;
    } else if (*o).tt as libc::c_int & 0xf as libc::c_int == 4 as libc::c_int {
        if (*o).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            != 0
        {
            reallymarkobject(g, &mut (*(o as *mut GCUnion)).gc);
        }
        return 0 as libc::c_int;
    } else {
        return (*o).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int);
    };
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaC_barrier_(
    mut L: *mut lua_State,
    mut o: *mut GCObject,
    mut v: *mut GCObject,
) {
    let mut g: *mut global_State = (*L).l_G;
    if (*g).gcstate as libc::c_int <= 2 as libc::c_int {
        reallymarkobject(g, v);
        if (*o).marked as libc::c_int & 7 as libc::c_int > 1 as libc::c_int {
            (*v).marked =
                ((*v).marked as libc::c_int & !(7 as libc::c_int) | 2 as libc::c_int) as u8;
        }
    } else if (*g).gckind as libc::c_int == 0 as libc::c_int {
        (*o).marked = ((*o).marked as libc::c_int
            & !((1 as libc::c_int) << 5 as libc::c_int
                | ((1 as libc::c_int) << 3 as libc::c_int
                    | (1 as libc::c_int) << 4 as libc::c_int))
            | ((*g).currentwhite as libc::c_int
                & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int))
                as u8 as libc::c_int) as u8;
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaC_barrierback_(mut L: *mut lua_State, mut o: *mut GCObject) {
    let mut g: *mut global_State = (*L).l_G;
    if (*o).marked as libc::c_int & 7 as libc::c_int == 6 as libc::c_int {
        (*o).marked = ((*o).marked as libc::c_int
            & !((1 as libc::c_int) << 5 as libc::c_int
                | ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int))
                as u8 as libc::c_int) as u8;
    } else {
        linkgclist_(
            &mut (*(o as *mut GCUnion)).gc,
            getgclist(o),
            &mut (*g).grayagain,
        );
    }
    if (*o).marked as libc::c_int & 7 as libc::c_int > 1 as libc::c_int {
        (*o).marked = ((*o).marked as libc::c_int & !(7 as libc::c_int) | 5 as libc::c_int) as u8;
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaC_fix(mut L: *mut lua_State, mut o: *mut GCObject) {
    let mut g: *mut global_State = (*L).l_G;
    (*o).marked = ((*o).marked as libc::c_int
        & !((1 as libc::c_int) << 5 as libc::c_int
            | ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int))
            as u8 as libc::c_int) as u8;
    (*o).marked = ((*o).marked as libc::c_int & !(7 as libc::c_int) | 4 as libc::c_int) as u8;
    (*g).allgc = (*o).next;
    (*o).next = (*g).fixedgc;
    (*g).fixedgc = o;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaC_newobjdt(
    mut L: *mut lua_State,
    mut tt: libc::c_int,
    mut sz: usize,
    mut offset: usize,
) -> *mut GCObject {
    let mut g: *mut global_State = (*L).l_G;
    let mut p: *mut libc::c_char =
        luaM_malloc_(L, sz, tt & 0xf as libc::c_int) as *mut libc::c_char;
    let mut o: *mut GCObject = p.offset(offset as isize) as *mut GCObject;
    (*o).marked = ((*g).currentwhite as libc::c_int
        & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int))
        as u8;
    (*o).tt = tt as u8;
    (*o).next = (*g).allgc;
    (*g).allgc = o;
    return o;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaC_newobj(
    mut L: *mut lua_State,
    mut tt: libc::c_int,
    mut sz: usize,
) -> *mut GCObject {
    return luaC_newobjdt(L, tt, sz, 0 as libc::c_int as usize);
}

unsafe extern "C" fn reallymarkobject(mut g: *mut global_State, mut o: *mut GCObject) {
    let mut current_block_18: u64;
    match (*o).tt as libc::c_int {
        4 | 20 => {
            (*o).marked = ((*o).marked as libc::c_int
                & !((1 as libc::c_int) << 3 as libc::c_int
                    | (1 as libc::c_int) << 4 as libc::c_int)
                | (1 as libc::c_int) << 5 as libc::c_int) as u8;
            current_block_18 = 18317007320854588510;
        }
        9 => {
            let mut uv: *mut UpVal = &mut (*(o as *mut GCUnion)).upv;
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
            current_block_18 = 18317007320854588510;
        }
        7 => {
            let mut u: *mut Udata = &mut (*(o as *mut GCUnion)).u;
            if (*u).nuvalue as libc::c_int == 0 as libc::c_int {
                if !((*u).metatable).is_null() {
                    if (*(*u).metatable).marked as libc::c_int
                        & ((1 as libc::c_int) << 3 as libc::c_int
                            | (1 as libc::c_int) << 4 as libc::c_int)
                        != 0
                    {
                        reallymarkobject(g, &mut (*((*u).metatable as *mut GCUnion)).gc);
                    }
                }
                (*u).marked = ((*u).marked as libc::c_int
                    & !((1 as libc::c_int) << 3 as libc::c_int
                        | (1 as libc::c_int) << 4 as libc::c_int)
                    | (1 as libc::c_int) << 5 as libc::c_int) as u8;
                current_block_18 = 18317007320854588510;
            } else {
                current_block_18 = 15904375183555213903;
            }
        }
        6 | 38 | 5 | 8 | 10 => {
            current_block_18 = 15904375183555213903;
        }
        _ => {
            current_block_18 = 18317007320854588510;
        }
    }
    match current_block_18 {
        15904375183555213903 => {
            linkgclist_(&mut (*(o as *mut GCUnion)).gc, getgclist(o), &mut (*g).gray);
        }
        _ => {}
    };
}

unsafe extern "C" fn markmt(mut g: *mut global_State) {
    let mut i: libc::c_int = 0;
    i = 0 as libc::c_int;
    while i < 9 as libc::c_int {
        if !((*g).mt[i as usize]).is_null() {
            if (*(*g).mt[i as usize]).marked as libc::c_int
                & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
                != 0
            {
                reallymarkobject(
                    g,
                    &mut (*(*((*g).mt).as_mut_ptr().offset(i as isize) as *mut GCUnion)).gc,
                );
            }
        }
        i += 1;
        i;
    }
}

unsafe extern "C" fn markbeingfnz(mut g: *mut global_State) -> usize {
    let mut o: *mut GCObject = 0 as *mut GCObject;
    let mut count: usize = 0 as libc::c_int as usize;
    o = (*g).tobefnz;
    while !o.is_null() {
        count = count.wrapping_add(1);
        count;
        if (*o).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            != 0
        {
            reallymarkobject(g, &mut (*(o as *mut GCUnion)).gc);
        }
        o = (*o).next;
    }
    return count;
}

unsafe extern "C" fn remarkupvals(mut g: *mut global_State) -> libc::c_int {
    let mut thread: *mut lua_State = 0 as *mut lua_State;
    let mut p: *mut *mut lua_State = &mut (*g).twups;
    let mut work: libc::c_int = 0 as libc::c_int;
    loop {
        thread = *p;
        if thread.is_null() {
            break;
        }
        work += 1;
        work;
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
                work;
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

unsafe extern "C" fn cleargraylists(mut g: *mut global_State) {
    (*g).grayagain = 0 as *mut GCObject;
    (*g).gray = (*g).grayagain;
    (*g).ephemeron = 0 as *mut GCObject;
    (*g).allweak = (*g).ephemeron;
    (*g).weak = (*g).allweak;
}

unsafe extern "C" fn restartcollection(mut g: *mut global_State) {
    cleargraylists(g);
    if (*(*g).mainthread).marked as libc::c_int
        & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
        != 0
    {
        reallymarkobject(g, &mut (*((*g).mainthread as *mut GCUnion)).gc);
    }
    if (*g).l_registry.tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0
        && (*(*g).l_registry.value_.gc).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            != 0
    {
        reallymarkobject(g, (*g).l_registry.value_.gc);
    }
    markmt(g);
    markbeingfnz(g);
}

unsafe extern "C" fn genlink(mut g: *mut global_State, mut o: *mut GCObject) {
    if (*o).marked as libc::c_int & 7 as libc::c_int == 5 as libc::c_int {
        linkgclist_(
            &mut (*(o as *mut GCUnion)).gc,
            getgclist(o),
            &mut (*g).grayagain,
        );
    } else if (*o).marked as libc::c_int & 7 as libc::c_int == 6 as libc::c_int {
        (*o).marked = ((*o).marked as libc::c_int ^ (6 as libc::c_int ^ 4 as libc::c_int)) as u8;
    }
}

unsafe extern "C" fn traverseweakvalue(mut g: *mut global_State, mut h: *mut Table) {
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
                    (if (*n).i_val.tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0
                    {
                        (*n).i_val.value_.gc
                    } else {
                        0 as *mut GCObject
                    }),
                ) != 0
            {
                hasclears = 1 as libc::c_int;
            }
        }
        n = n.offset(1);
        n;
    }
    if (*g).gcstate as libc::c_int == 2 as libc::c_int && hasclears != 0 {
        linkgclist_(
            &mut (*(h as *mut GCUnion)).gc,
            &mut (*h).gclist,
            &mut (*g).weak,
        );
    } else {
        linkgclist_(
            &mut (*(h as *mut GCUnion)).gc,
            &mut (*h).gclist,
            &mut (*g).grayagain,
        );
    };
}

unsafe extern "C" fn traverseephemeron(
    mut g: *mut global_State,
    mut h: *mut Table,
    mut inv: libc::c_int,
) -> libc::c_int {
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
        i;
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
        i;
    }
    if (*g).gcstate as libc::c_int == 0 as libc::c_int {
        linkgclist_(
            &mut (*(h as *mut GCUnion)).gc,
            &mut (*h).gclist,
            &mut (*g).grayagain,
        );
    } else if hasww != 0 {
        linkgclist_(
            &mut (*(h as *mut GCUnion)).gc,
            &mut (*h).gclist,
            &mut (*g).ephemeron,
        );
    } else if hasclears != 0 {
        linkgclist_(
            &mut (*(h as *mut GCUnion)).gc,
            &mut (*h).gclist,
            &mut (*g).allweak,
        );
    } else {
        genlink(g, &mut (*(h as *mut GCUnion)).gc);
    }
    return marked;
}

unsafe extern "C" fn traversestrongtable(mut g: *mut global_State, mut h: *mut Table) {
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
        i;
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
        n;
    }
    genlink(g, &mut (*(h as *mut GCUnion)).gc);
}

unsafe extern "C" fn traversetable(mut g: *mut global_State, mut h: *mut Table) -> usize {
    let mut weakkey: *const libc::c_char = 0 as *const libc::c_char;
    let mut weakvalue: *const libc::c_char = 0 as *const libc::c_char;
    let mut mode: *const TValue = if ((*h).metatable).is_null() {
        0 as *const TValue
    } else if (*(*h).metatable).flags as libc::c_uint
        & (1 as libc::c_uint) << TM_MODE as libc::c_int
        != 0
    {
        0 as *const TValue
    } else {
        luaT_gettm(
            (*h).metatable,
            TM_MODE,
            (*g).tmname[TM_MODE as libc::c_int as usize],
        )
    };
    let mut smode: *mut TString = 0 as *mut TString;
    if !((*h).metatable).is_null() {
        if (*(*h).metatable).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            != 0
        {
            reallymarkobject(g, &mut (*((*h).metatable as *mut GCUnion)).gc);
        }
    }
    if !mode.is_null()
        && (*mode).tt_ as libc::c_int
            == 4 as libc::c_int
                | (0 as libc::c_int) << 4 as libc::c_int
                | (1 as libc::c_int) << 6 as libc::c_int
        && {
            smode = &mut (*((*mode).value_.gc as *mut GCUnion)).ts as *mut TString;
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
            linkgclist_(
                &mut (*(h as *mut GCUnion)).gc,
                &mut (*h).gclist,
                &mut (*g).allweak,
            );
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

unsafe extern "C" fn traverseudata(mut g: *mut global_State, mut u: *mut Udata) -> libc::c_int {
    let mut i: libc::c_int = 0;
    if !((*u).metatable).is_null() {
        if (*(*u).metatable).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            != 0
        {
            reallymarkobject(g, &mut (*((*u).metatable as *mut GCUnion)).gc);
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
        i;
    }
    genlink(g, &mut (*(u as *mut GCUnion)).gc);
    return 1 as libc::c_int + (*u).nuvalue as libc::c_int;
}

unsafe extern "C" fn traverseproto(mut g: *mut global_State, mut f: *mut Proto) -> libc::c_int {
    let mut i: libc::c_int = 0;
    if !((*f).source).is_null() {
        if (*(*f).source).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            != 0
        {
            reallymarkobject(g, &mut (*((*f).source as *mut GCUnion)).gc);
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
        i;
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
                    &mut (*((*((*f).upvalues).offset(i as isize)).name as *mut GCUnion)).gc,
                );
            }
        }
        i += 1;
        i;
    }
    i = 0 as libc::c_int;
    while i < (*f).sizep {
        if !(*((*f).p).offset(i as isize)).is_null() {
            if (**((*f).p).offset(i as isize)).marked as libc::c_int
                & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
                != 0
            {
                reallymarkobject(g, &mut (*(*((*f).p).offset(i as isize) as *mut GCUnion)).gc);
            }
        }
        i += 1;
        i;
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
                    &mut (*((*((*f).locvars).offset(i as isize)).varname as *mut GCUnion)).gc,
                );
            }
        }
        i += 1;
        i;
    }
    return 1 as libc::c_int + (*f).sizek + (*f).sizeupvalues + (*f).sizep + (*f).sizelocvars;
}

unsafe extern "C" fn traverseCclosure(
    mut g: *mut global_State,
    mut cl: *mut CClosure,
) -> libc::c_int {
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
        i;
    }
    return 1 as libc::c_int + (*cl).nupvalues as libc::c_int;
}

unsafe extern "C" fn traverseLclosure(
    mut g: *mut global_State,
    mut cl: *mut LClosure,
) -> libc::c_int {
    let mut i: libc::c_int = 0;
    if !((*cl).p).is_null() {
        if (*(*cl).p).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            != 0
        {
            reallymarkobject(g, &mut (*((*cl).p as *mut GCUnion)).gc);
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
                reallymarkobject(g, &mut (*(uv as *mut GCUnion)).gc);
            }
        }
        i += 1;
        i;
    }
    return 1 as libc::c_int + (*cl).nupvalues as libc::c_int;
}

unsafe extern "C" fn traversethread(
    mut g: *mut global_State,
    mut th: *mut lua_State,
) -> libc::c_int {
    let mut uv: *mut UpVal = 0 as *mut UpVal;
    let mut o: StkId = (*th).stack.p;
    if (*th).marked as libc::c_int & 7 as libc::c_int > 1 as libc::c_int
        || (*g).gcstate as libc::c_int == 0 as libc::c_int
    {
        linkgclist_(
            &mut (*(th as *mut GCUnion)).gc,
            &mut (*th).gclist,
            &mut (*g).grayagain,
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
        o;
    }
    uv = (*th).openupval;
    while !uv.is_null() {
        if (*uv).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            != 0
        {
            reallymarkobject(g, &mut (*(uv as *mut GCUnion)).gc);
        }
        uv = (*uv).u.open.next;
    }
    if (*g).gcstate as libc::c_int == 2 as libc::c_int {
        if (*g).gcemergency == 0 {
            luaD_shrinkstack(th);
        }
        o = (*th).top.p;
        while o < ((*th).stack_last.p).offset(5 as libc::c_int as isize) {
            (*o).val.tt_ = (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
            o = o.offset(1);
            o;
        }
        if !((*th).twups != th) && !((*th).openupval).is_null() {
            (*th).twups = (*g).twups;
            (*g).twups = th;
        }
    }
    return 1 as libc::c_int
        + ((*th).stack_last.p).offset_from((*th).stack.p) as libc::c_long as libc::c_int;
}

unsafe extern "C" fn propagatemark(mut g: *mut global_State) -> usize {
    let mut o: *mut GCObject = (*g).gray;
    (*o).marked = ((*o).marked as libc::c_int | (1 as libc::c_int) << 5 as libc::c_int) as u8;
    (*g).gray = *getgclist(o);
    match (*o).tt as libc::c_int {
        5 => return traversetable(g, &mut (*(o as *mut GCUnion)).h),
        7 => return traverseudata(g, &mut (*(o as *mut GCUnion)).u) as usize,
        6 => return traverseLclosure(g, &mut (*(o as *mut GCUnion)).cl.l) as usize,
        38 => return traverseCclosure(g, &mut (*(o as *mut GCUnion)).cl.c) as usize,
        10 => return traverseproto(g, &mut (*(o as *mut GCUnion)).p) as usize,
        8 => return traversethread(g, &mut (*(o as *mut GCUnion)).th) as usize,
        _ => return 0 as libc::c_int as usize,
    };
}

unsafe extern "C" fn propagateall(mut g: *mut global_State) -> usize {
    let mut tot: usize = 0 as libc::c_int as usize;
    while !((*g).gray).is_null() {
        tot = tot.wrapping_add(propagatemark(g));
    }
    return tot;
}

unsafe extern "C" fn convergeephemerons(mut g: *mut global_State) {
    let mut changed: libc::c_int = 0;
    let mut dir: libc::c_int = 0 as libc::c_int;
    loop {
        let mut w: *mut GCObject = 0 as *mut GCObject;
        let mut next: *mut GCObject = (*g).ephemeron;
        (*g).ephemeron = 0 as *mut GCObject;
        changed = 0 as libc::c_int;
        loop {
            w = next;
            if w.is_null() {
                break;
            }
            let mut h: *mut Table = &mut (*(w as *mut GCUnion)).h;
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

unsafe extern "C" fn clearbykeys(mut g: *mut global_State, mut l: *mut GCObject) {
    while !l.is_null() {
        let mut h: *mut Table = &mut (*(l as *mut GCUnion)).h;
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
            n;
        }
        l = (*(l as *mut GCUnion)).h.gclist;
    }
}

unsafe extern "C" fn clearbyvalues(
    mut g: *mut global_State,
    mut l: *mut GCObject,
    mut f: *mut GCObject,
) {
    while l != f {
        let mut h: *mut Table = &mut (*(l as *mut GCUnion)).h;
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
            i;
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
            n;
        }
        l = (*(l as *mut GCUnion)).h.gclist;
    }
}

unsafe extern "C" fn freeupval(mut L: *mut lua_State, mut uv: *mut UpVal) {
    if (*uv).v.p != &mut (*uv).u.value as *mut TValue {
        luaF_unlinkupval(uv);
    }
    luaM_free_(L, uv as *mut libc::c_void, ::core::mem::size_of::<UpVal>());
}

unsafe extern "C" fn freeobj(mut L: *mut lua_State, mut o: *mut GCObject) {
    match (*o).tt as libc::c_int {
        10 => {
            luaF_freeproto(L, &mut (*(o as *mut GCUnion)).p);
        }
        9 => {
            freeupval(L, &mut (*(o as *mut GCUnion)).upv);
        }
        6 => {
            let mut cl: *mut LClosure = &mut (*(o as *mut GCUnion)).cl.l;
            luaM_free_(
                L,
                cl as *mut libc::c_void,
                (32 as libc::c_ulong as libc::c_int
                    + ::core::mem::size_of::<*mut TValue>() as libc::c_ulong as libc::c_int
                        * (*cl).nupvalues as libc::c_int) as usize,
            );
        }
        38 => {
            let mut cl_0: *mut CClosure = &mut (*(o as *mut GCUnion)).cl.c;
            luaM_free_(
                L,
                cl_0 as *mut libc::c_void,
                (32 as libc::c_ulong as libc::c_int
                    + ::core::mem::size_of::<TValue>() as libc::c_ulong as libc::c_int
                        * (*cl_0).nupvalues as libc::c_int) as usize,
            );
        }
        5 => {
            luaH_free(L, &mut (*(o as *mut GCUnion)).h);
        }
        8 => {
            luaE_freethread(L, &mut (*(o as *mut GCUnion)).th);
        }
        7 => {
            let mut u: *mut Udata = &mut (*(o as *mut GCUnion)).u;
            luaM_free_(
                L,
                o as *mut libc::c_void,
                (if (*u).nuvalue as libc::c_int == 0 as libc::c_int {
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
            let mut ts: *mut TString = &mut (*(o as *mut GCUnion)).ts;
            luaS_remove(L, ts);
            luaM_free_(
                L,
                ts as *mut libc::c_void,
                24usize.wrapping_add(
                    (usize::from((*ts).shrlen) + 1)
                        .wrapping_mul(::core::mem::size_of::<libc::c_char>()),
                ),
            );
        }
        20 => {
            let mut ts_0: *mut TString = &mut (*(o as *mut GCUnion)).ts;
            luaM_free_(
                L,
                ts_0 as *mut libc::c_void,
                24usize.wrapping_add(
                    ((*ts_0).u.lnglen)
                        .wrapping_add(1)
                        .wrapping_mul(::core::mem::size_of::<libc::c_char>()),
                ),
            );
        }
        _ => {}
    };
}

unsafe extern "C" fn sweeplist(
    mut L: *mut lua_State,
    mut p: *mut *mut GCObject,
    mut countin: libc::c_int,
    mut countout: *mut libc::c_int,
) -> *mut *mut GCObject {
    let mut g: *mut global_State = (*L).l_G;
    let mut ow: libc::c_int = (*g).currentwhite as libc::c_int
        ^ ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int);
    let mut i: libc::c_int = 0;
    let mut white: libc::c_int = ((*g).currentwhite as libc::c_int
        & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int))
        as u8 as libc::c_int;
    i = 0 as libc::c_int;
    while !(*p).is_null() && i < countin {
        let mut curr: *mut GCObject = *p;
        let mut marked: libc::c_int = (*curr).marked as libc::c_int;
        if marked & ow != 0 {
            *p = (*curr).next;
            freeobj(L, curr);
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
        i;
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

unsafe extern "C" fn sweeptolive(
    mut L: *mut lua_State,
    mut p: *mut *mut GCObject,
) -> *mut *mut GCObject {
    let mut old: *mut *mut GCObject = p;
    loop {
        p = sweeplist(L, p, 1 as libc::c_int, 0 as *mut libc::c_int);
        if !(p == old) {
            break;
        }
    }
    return p;
}

unsafe extern "C" fn checkSizes(mut L: *mut lua_State, mut g: *mut global_State) {
    if (*g).gcemergency == 0 {
        if (*g).strt.nuse < (*g).strt.size / 4 as libc::c_int {
            let mut olddebt: isize = (*g).GCdebt;
            luaS_resize(L, (*g).strt.size / 2 as libc::c_int);
            (*g).GCestimate = ((*g).GCestimate).wrapping_add(((*g).GCdebt - olddebt) as usize);
        }
    }
}

unsafe extern "C" fn udata2finalize(mut g: *mut global_State) -> *mut GCObject {
    let mut o: *mut GCObject = (*g).tobefnz;
    (*g).tobefnz = (*o).next;
    (*o).next = (*g).allgc;
    (*g).allgc = o;
    (*o).marked = ((*o).marked as libc::c_int
        & !((1 as libc::c_int) << 6 as libc::c_int) as u8 as libc::c_int) as u8;
    if 3 as libc::c_int <= (*g).gcstate as libc::c_int
        && (*g).gcstate as libc::c_int <= 6 as libc::c_int
    {
        (*o).marked = ((*o).marked as libc::c_int
            & !((1 as libc::c_int) << 5 as libc::c_int
                | ((1 as libc::c_int) << 3 as libc::c_int
                    | (1 as libc::c_int) << 4 as libc::c_int))
            | ((*g).currentwhite as libc::c_int
                & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int))
                as u8 as libc::c_int) as u8;
    } else if (*o).marked as libc::c_int & 7 as libc::c_int == 3 as libc::c_int {
        (*g).firstold1 = o;
    }
    return o;
}

unsafe extern "C" fn dothecall(mut L: *mut lua_State, mut ud: *mut libc::c_void) {
    luaD_callnoyield(
        L,
        ((*L).top.p).offset(-(2 as libc::c_int as isize)),
        0 as libc::c_int,
    );
}

unsafe extern "C" fn GCTM(mut L: *mut lua_State) {
    let mut g: *mut global_State = (*L).l_G;
    let mut tm: *const TValue = 0 as *const TValue;
    let mut v: TValue = TValue {
        value_: Value {
            gc: 0 as *mut GCObject,
        },
        tt_: 0,
    };
    let mut io: *mut TValue = &mut v;
    let mut i_g: *mut GCObject = udata2finalize(g);
    (*io).value_.gc = i_g;
    (*io).tt_ = ((*i_g).tt as libc::c_int | (1 as libc::c_int) << 6 as libc::c_int) as u8;
    tm = luaT_gettmbyobj(L, &mut v, TM_GC);
    if !((*tm).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int) {
        let mut status: libc::c_int = 0;
        let mut oldah: u8 = (*L).allowhook;
        let mut oldgcstp: libc::c_int = (*g).gcstp as libc::c_int;
        (*g).gcstp = ((*g).gcstp as libc::c_int | 2 as libc::c_int) as u8;
        (*L).allowhook = 0 as libc::c_int as u8;
        let fresh0 = (*L).top.p;
        (*L).top.p = ((*L).top.p).offset(1);
        let mut io1: *mut TValue = &mut (*fresh0).val;
        let mut io2: *const TValue = tm;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        let fresh1 = (*L).top.p;
        (*L).top.p = ((*L).top.p).offset(1);
        let mut io1_0: *mut TValue = &mut (*fresh1).val;
        let mut io2_0: *const TValue = &mut v;
        (*io1_0).value_ = (*io2_0).value_;
        (*io1_0).tt_ = (*io2_0).tt_;
        (*(*L).ci).callstatus = ((*(*L).ci).callstatus as libc::c_int
            | (1 as libc::c_int) << 7 as libc::c_int)
            as libc::c_ushort;
        status = luaD_pcall(
            L,
            Some(dothecall as unsafe extern "C" fn(*mut lua_State, *mut libc::c_void) -> ()),
            0 as *mut libc::c_void,
            (((*L).top.p).offset(-(2 as libc::c_int as isize)) as *mut libc::c_char)
                .offset_from((*L).stack.p as *mut libc::c_char),
            0 as libc::c_int as isize,
        );
        (*(*L).ci).callstatus = ((*(*L).ci).callstatus as libc::c_int
            & !((1 as libc::c_int) << 7 as libc::c_int))
            as libc::c_ushort;
        (*L).allowhook = oldah;
        (*g).gcstp = oldgcstp as u8;
        if ((status != 0 as libc::c_int) as libc::c_int != 0 as libc::c_int) as libc::c_int
            as libc::c_long
            != 0
        {
            luaE_warnerror(L, b"__gc\0" as *const u8 as *const libc::c_char);
            (*L).top.p = ((*L).top.p).offset(-1);
            (*L).top.p;
        }
    }
}

unsafe extern "C" fn runafewfinalizers(mut L: *mut lua_State, mut n: libc::c_int) -> libc::c_int {
    let mut g: *mut global_State = (*L).l_G;
    let mut i: libc::c_int = 0;
    i = 0 as libc::c_int;
    while i < n && !((*g).tobefnz).is_null() {
        GCTM(L);
        i += 1;
        i;
    }
    return i;
}

unsafe extern "C" fn callallpendingfinalizers(mut L: *mut lua_State) {
    let mut g: *mut global_State = (*L).l_G;
    while !((*g).tobefnz).is_null() {
        GCTM(L);
    }
}

unsafe extern "C" fn findlast(mut p: *mut *mut GCObject) -> *mut *mut GCObject {
    while !(*p).is_null() {
        p = &mut (**p).next;
    }
    return p;
}

unsafe extern "C" fn separatetobefnz(mut g: *mut global_State, mut all: libc::c_int) {
    let mut curr: *mut GCObject = 0 as *mut GCObject;
    let mut p: *mut *mut GCObject = &mut (*g).finobj;
    let mut lastnext: *mut *mut GCObject = findlast(&mut (*g).tobefnz);
    loop {
        curr = *p;
        if !(curr != (*g).finobjold1) {
            break;
        }
        if !((*curr).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            != 0
            || all != 0)
        {
            p = &mut (*curr).next;
        } else {
            if curr == (*g).finobjsur {
                (*g).finobjsur = (*curr).next;
            }
            *p = (*curr).next;
            (*curr).next = *lastnext;
            *lastnext = curr;
            lastnext = &mut (*curr).next;
        }
    }
}

unsafe extern "C" fn checkpointer(mut p: *mut *mut GCObject, mut o: *mut GCObject) {
    if o == *p {
        *p = (*o).next;
    }
}

unsafe extern "C" fn correctpointers(mut g: *mut global_State, mut o: *mut GCObject) {
    checkpointer(&mut (*g).survival, o);
    checkpointer(&mut (*g).old1, o);
    checkpointer(&mut (*g).reallyold, o);
    checkpointer(&mut (*g).firstold1, o);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaC_checkfinalizer(
    mut L: *mut lua_State,
    mut o: *mut GCObject,
    mut mt: *mut Table,
) {
    let mut g: *mut global_State = (*L).l_G;
    if (*o).marked as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0
        || (if mt.is_null() {
            0 as *const TValue
        } else {
            (if (*mt).flags as libc::c_uint & (1 as libc::c_uint) << TM_GC as libc::c_int != 0 {
                0 as *const TValue
            } else {
                luaT_gettm(mt, TM_GC, (*g).tmname[TM_GC as libc::c_int as usize])
            })
        })
        .is_null()
        || (*g).gcstp as libc::c_int & 4 as libc::c_int != 0
    {
        return;
    } else {
        let mut p: *mut *mut GCObject = 0 as *mut *mut GCObject;
        if 3 as libc::c_int <= (*g).gcstate as libc::c_int
            && (*g).gcstate as libc::c_int <= 6 as libc::c_int
        {
            (*o).marked = ((*o).marked as libc::c_int
                & !((1 as libc::c_int) << 5 as libc::c_int
                    | ((1 as libc::c_int) << 3 as libc::c_int
                        | (1 as libc::c_int) << 4 as libc::c_int))
                | ((*g).currentwhite as libc::c_int
                    & ((1 as libc::c_int) << 3 as libc::c_int
                        | (1 as libc::c_int) << 4 as libc::c_int)) as u8
                    as libc::c_int) as u8;
            if (*g).sweepgc == &mut (*o).next as *mut *mut GCObject {
                (*g).sweepgc = sweeptolive(L, (*g).sweepgc);
            }
        } else {
            correctpointers(g, o);
        }
        p = &mut (*g).allgc;
        while *p != o {
            p = &mut (**p).next;
        }
        *p = (*o).next;
        (*o).next = (*g).finobj;
        (*g).finobj = o;
        (*o).marked = ((*o).marked as libc::c_int | (1 as libc::c_int) << 6 as libc::c_int) as u8;
    };
}

unsafe extern "C" fn setpause(mut g: *mut global_State) {
    let mut threshold: isize = 0;
    let mut debt: isize = 0;
    let mut pause: libc::c_int = (*g).gcpause as libc::c_int * 4 as libc::c_int;
    let mut estimate: isize = ((*g).GCestimate / 100 as libc::c_int as usize) as isize;
    threshold = if (pause as isize)
        < (!(0 as libc::c_int as usize) >> 1 as libc::c_int) as isize / estimate
    {
        estimate * pause as isize
    } else {
        (!(0 as libc::c_int as usize) >> 1 as libc::c_int) as isize
    };
    debt = (((*g).totalbytes + (*g).GCdebt) as usize).wrapping_sub(threshold as usize) as isize;
    if debt > 0 as libc::c_int as isize {
        debt = 0 as libc::c_int as isize;
    }
    luaE_setdebt(g, debt);
}

unsafe extern "C" fn sweep2old(mut L: *mut lua_State, mut p: *mut *mut GCObject) {
    let mut curr: *mut GCObject = 0 as *mut GCObject;
    let mut g: *mut global_State = (*L).l_G;
    loop {
        curr = *p;
        if curr.is_null() {
            break;
        }
        if (*curr).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            != 0
        {
            *p = (*curr).next;
            freeobj(L, curr);
        } else {
            (*curr).marked =
                ((*curr).marked as libc::c_int & !(7 as libc::c_int) | 4 as libc::c_int) as u8;
            if (*curr).tt as libc::c_int
                == 8 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
            {
                let mut th: *mut lua_State = &mut (*(curr as *mut GCUnion)).th;
                linkgclist_(
                    &mut (*(th as *mut GCUnion)).gc,
                    &mut (*th).gclist,
                    &mut (*g).grayagain,
                );
            } else if (*curr).tt as libc::c_int
                == 9 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                && (*(curr as *mut GCUnion)).upv.v.p
                    != &mut (*(curr as *mut GCUnion)).upv.u.value as *mut TValue
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

unsafe extern "C" fn sweepgen(
    mut L: *mut lua_State,
    mut g: *mut global_State,
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
    let mut white: libc::c_int = ((*g).currentwhite as libc::c_int
        & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int))
        as u8 as libc::c_int;
    let mut curr: *mut GCObject = 0 as *mut GCObject;
    loop {
        curr = *p;
        if !(curr != limit) {
            break;
        }
        if (*curr).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            != 0
        {
            *p = (*curr).next;
            freeobj(L, curr);
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

unsafe extern "C" fn whitelist(mut g: *mut global_State, mut p: *mut GCObject) {
    let mut white: libc::c_int = ((*g).currentwhite as libc::c_int
        & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int))
        as u8 as libc::c_int;
    while !p.is_null() {
        (*p).marked = ((*p).marked as libc::c_int
            & !((1 as libc::c_int) << 5 as libc::c_int
                | ((1 as libc::c_int) << 3 as libc::c_int
                    | (1 as libc::c_int) << 4 as libc::c_int)
                | 7 as libc::c_int)
            | white) as u8;
        p = (*p).next;
    }
}

unsafe extern "C" fn correctgraylist(mut p: *mut *mut GCObject) -> *mut *mut GCObject {
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

unsafe extern "C" fn correctgraylists(mut g: *mut global_State) {
    let mut list: *mut *mut GCObject = correctgraylist(&mut (*g).grayagain);
    *list = (*g).weak;
    (*g).weak = 0 as *mut GCObject;
    list = correctgraylist(list);
    *list = (*g).allweak;
    (*g).allweak = 0 as *mut GCObject;
    list = correctgraylist(list);
    *list = (*g).ephemeron;
    (*g).ephemeron = 0 as *mut GCObject;
    correctgraylist(list);
}

unsafe extern "C" fn markold(
    mut g: *mut global_State,
    mut from: *mut GCObject,
    mut to: *mut GCObject,
) {
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

unsafe extern "C" fn finishgencycle(mut L: *mut lua_State, mut g: *mut global_State) {
    correctgraylists(g);
    checkSizes(L, g);
    (*g).gcstate = 0 as libc::c_int as u8;
    if (*g).gcemergency == 0 {
        callallpendingfinalizers(L);
    }
}

unsafe extern "C" fn youngcollection(mut L: *mut lua_State, mut g: *mut global_State) {
    let mut psurvival: *mut *mut GCObject = 0 as *mut *mut GCObject;
    let mut dummy: *mut GCObject = 0 as *mut GCObject;
    if !((*g).firstold1).is_null() {
        markold(g, (*g).firstold1, (*g).reallyold);
        (*g).firstold1 = 0 as *mut GCObject;
    }
    markold(g, (*g).finobj, (*g).finobjrold);
    markold(g, (*g).tobefnz, 0 as *mut GCObject);
    atomic(L);
    (*g).gcstate = 3 as libc::c_int as u8;
    psurvival = sweepgen(L, g, &mut (*g).allgc, (*g).survival, &mut (*g).firstold1);
    sweepgen(L, g, psurvival, (*g).old1, &mut (*g).firstold1);
    (*g).reallyold = (*g).old1;
    (*g).old1 = *psurvival;
    (*g).survival = (*g).allgc;
    dummy = 0 as *mut GCObject;
    psurvival = sweepgen(L, g, &mut (*g).finobj, (*g).finobjsur, &mut dummy);
    sweepgen(L, g, psurvival, (*g).finobjold1, &mut dummy);
    (*g).finobjrold = (*g).finobjold1;
    (*g).finobjold1 = *psurvival;
    (*g).finobjsur = (*g).finobj;
    sweepgen(L, g, &mut (*g).tobefnz, 0 as *mut GCObject, &mut dummy);
    finishgencycle(L, g);
}

unsafe extern "C" fn atomic2gen(mut L: *mut lua_State, mut g: *mut global_State) {
    cleargraylists(g);
    (*g).gcstate = 3 as libc::c_int as u8;
    sweep2old(L, &mut (*g).allgc);
    (*g).survival = (*g).allgc;
    (*g).old1 = (*g).survival;
    (*g).reallyold = (*g).old1;
    (*g).firstold1 = 0 as *mut GCObject;
    sweep2old(L, &mut (*g).finobj);
    (*g).finobjsur = (*g).finobj;
    (*g).finobjold1 = (*g).finobjsur;
    (*g).finobjrold = (*g).finobjold1;
    sweep2old(L, &mut (*g).tobefnz);
    (*g).gckind = 1 as libc::c_int as u8;
    (*g).lastatomic = 0 as libc::c_int as usize;
    (*g).GCestimate = ((*g).totalbytes + (*g).GCdebt) as usize;
    finishgencycle(L, g);
}

unsafe extern "C" fn setminordebt(mut g: *mut global_State) {
    luaE_setdebt(
        g,
        -((((*g).totalbytes + (*g).GCdebt) as usize / 100 as libc::c_int as usize) as isize
            * (*g).genminormul as isize),
    );
}

unsafe extern "C" fn entergen(mut L: *mut lua_State, mut g: *mut global_State) -> usize {
    let mut numobjs: usize = 0;
    luaC_runtilstate(L, (1 as libc::c_int) << 8 as libc::c_int);
    luaC_runtilstate(L, (1 as libc::c_int) << 0 as libc::c_int);
    numobjs = atomic(L);
    atomic2gen(L, g);
    setminordebt(g);
    return numobjs;
}

unsafe extern "C" fn enterinc(mut g: *mut global_State) {
    whitelist(g, (*g).allgc);
    (*g).survival = 0 as *mut GCObject;
    (*g).old1 = (*g).survival;
    (*g).reallyold = (*g).old1;
    whitelist(g, (*g).finobj);
    whitelist(g, (*g).tobefnz);
    (*g).finobjsur = 0 as *mut GCObject;
    (*g).finobjold1 = (*g).finobjsur;
    (*g).finobjrold = (*g).finobjold1;
    (*g).gcstate = 8 as libc::c_int as u8;
    (*g).gckind = 0 as libc::c_int as u8;
    (*g).lastatomic = 0 as libc::c_int as usize;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaC_changemode(mut L: *mut lua_State, mut newmode: libc::c_int) {
    let mut g: *mut global_State = (*L).l_G;
    if newmode != (*g).gckind as libc::c_int {
        if newmode == 1 as libc::c_int {
            entergen(L, g);
        } else {
            enterinc(g);
        }
    }
    (*g).lastatomic = 0 as libc::c_int as usize;
}

unsafe extern "C" fn fullgen(mut L: *mut lua_State, mut g: *mut global_State) -> usize {
    enterinc(g);
    return entergen(L, g);
}

unsafe extern "C" fn stepgenfull(mut L: *mut lua_State, mut g: *mut global_State) {
    let mut newatomic: usize = 0;
    let mut lastatomic: usize = (*g).lastatomic;
    if (*g).gckind as libc::c_int == 1 as libc::c_int {
        enterinc(g);
    }
    luaC_runtilstate(L, (1 as libc::c_int) << 0 as libc::c_int);
    newatomic = atomic(L);
    if newatomic < lastatomic.wrapping_add(lastatomic >> 3 as libc::c_int) {
        atomic2gen(L, g);
        setminordebt(g);
    } else {
        (*g).GCestimate = ((*g).totalbytes + (*g).GCdebt) as usize;
        entersweep(L);
        luaC_runtilstate(L, (1 as libc::c_int) << 8 as libc::c_int);
        setpause(g);
        (*g).lastatomic = newatomic;
    };
}

unsafe extern "C" fn genstep(mut L: *mut lua_State, mut g: *mut global_State) {
    if (*g).lastatomic != 0 as libc::c_int as usize {
        stepgenfull(L, g);
    } else {
        let mut majorbase: usize = (*g).GCestimate;
        let mut majorinc: usize = majorbase / 100 as libc::c_int as usize
            * ((*g).genmajormul as libc::c_int * 4 as libc::c_int) as usize;
        if (*g).GCdebt > 0 as libc::c_int as isize
            && ((*g).totalbytes + (*g).GCdebt) as usize > majorbase.wrapping_add(majorinc)
        {
            let mut numobjs: usize = fullgen(L, g);
            if !((((*g).totalbytes + (*g).GCdebt) as usize)
                < majorbase.wrapping_add(majorinc / 2 as libc::c_int as usize))
            {
                (*g).lastatomic = numobjs;
                setpause(g);
            }
        } else {
            youngcollection(L, g);
            setminordebt(g);
            (*g).GCestimate = majorbase;
        }
    };
}

unsafe extern "C" fn entersweep(mut L: *mut lua_State) {
    let mut g: *mut global_State = (*L).l_G;
    (*g).gcstate = 3 as libc::c_int as u8;
    (*g).sweepgc = sweeptolive(L, &mut (*g).allgc);
}

unsafe extern "C" fn deletelist(
    mut L: *mut lua_State,
    mut p: *mut GCObject,
    mut limit: *mut GCObject,
) {
    while p != limit {
        let mut next: *mut GCObject = (*p).next;
        freeobj(L, p);
        p = next;
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaC_freeallobjects(mut L: *mut lua_State) {
    let mut g: *mut global_State = (*L).l_G;
    (*g).gcstp = 4 as libc::c_int as u8;
    luaC_changemode(L, 0 as libc::c_int);
    separatetobefnz(g, 1 as libc::c_int);
    callallpendingfinalizers(L);
    deletelist(L, (*g).allgc, &mut (*((*g).mainthread as *mut GCUnion)).gc);
    deletelist(L, (*g).fixedgc, 0 as *mut GCObject);
}

unsafe extern "C" fn atomic(mut L: *mut lua_State) -> usize {
    let mut g: *mut global_State = (*L).l_G;
    let mut work: usize = 0 as libc::c_int as usize;
    let mut origweak: *mut GCObject = 0 as *mut GCObject;
    let mut origall: *mut GCObject = 0 as *mut GCObject;
    let mut grayagain: *mut GCObject = (*g).grayagain;
    (*g).grayagain = 0 as *mut GCObject;
    (*g).gcstate = 2 as libc::c_int as u8;
    if (*L).marked as libc::c_int
        & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
        != 0
    {
        reallymarkobject(g, &mut (*(L as *mut GCUnion)).gc);
    }
    if (*g).l_registry.tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0
        && (*(*g).l_registry.value_.gc).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            != 0
    {
        reallymarkobject(g, (*g).l_registry.value_.gc);
    }
    markmt(g);
    work = work.wrapping_add(propagateall(g));
    work = work.wrapping_add(remarkupvals(g) as usize);
    work = work.wrapping_add(propagateall(g));
    (*g).gray = grayagain;
    work = work.wrapping_add(propagateall(g));
    convergeephemerons(g);
    clearbyvalues(g, (*g).weak, 0 as *mut GCObject);
    clearbyvalues(g, (*g).allweak, 0 as *mut GCObject);
    origweak = (*g).weak;
    origall = (*g).allweak;
    separatetobefnz(g, 0 as libc::c_int);
    work = work.wrapping_add(markbeingfnz(g));
    work = work.wrapping_add(propagateall(g));
    convergeephemerons(g);
    clearbykeys(g, (*g).ephemeron);
    clearbykeys(g, (*g).allweak);
    clearbyvalues(g, (*g).weak, origweak);
    clearbyvalues(g, (*g).allweak, origall);
    luaS_clearcache(g);
    (*g).currentwhite = ((*g).currentwhite as libc::c_int
        ^ ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int))
        as u8;
    return work;
}

unsafe extern "C" fn sweepstep(
    mut L: *mut lua_State,
    mut g: *mut global_State,
    mut nextstate: libc::c_int,
    mut nextlist: *mut *mut GCObject,
) -> libc::c_int {
    if !((*g).sweepgc).is_null() {
        let mut olddebt: isize = (*g).GCdebt;
        let mut count: libc::c_int = 0;
        (*g).sweepgc = sweeplist(L, (*g).sweepgc, 100 as libc::c_int, &mut count);
        (*g).GCestimate = ((*g).GCestimate).wrapping_add(((*g).GCdebt - olddebt) as usize);
        return count;
    } else {
        (*g).gcstate = nextstate as u8;
        (*g).sweepgc = nextlist;
        return 0 as libc::c_int;
    };
}
unsafe extern "C" fn singlestep(mut L: *mut lua_State) -> usize {
    let mut g: *mut global_State = (*L).l_G;
    let mut work: usize = 0;
    (*g).gcstopem = 1 as libc::c_int as u8;
    match (*g).gcstate as libc::c_int {
        8 => {
            restartcollection(g);
            (*g).gcstate = 0 as libc::c_int as u8;
            work = 1 as libc::c_int as usize;
        }
        0 => {
            if ((*g).gray).is_null() {
                (*g).gcstate = 1 as libc::c_int as u8;
                work = 0 as libc::c_int as usize;
            } else {
                work = propagatemark(g);
            }
        }
        1 => {
            work = atomic(L);
            entersweep(L);
            (*g).GCestimate = ((*g).totalbytes + (*g).GCdebt) as usize;
        }
        3 => {
            work = sweepstep(L, g, 4 as libc::c_int, &mut (*g).finobj) as usize;
        }
        4 => {
            work = sweepstep(L, g, 5 as libc::c_int, &mut (*g).tobefnz) as usize;
        }
        5 => {
            work = sweepstep(L, g, 6 as libc::c_int, 0 as *mut *mut GCObject) as usize;
        }
        6 => {
            checkSizes(L, g);
            (*g).gcstate = 7 as libc::c_int as u8;
            work = 0 as libc::c_int as usize;
        }
        7 => {
            if !((*g).tobefnz).is_null() && (*g).gcemergency == 0 {
                (*g).gcstopem = 0 as libc::c_int as u8;
                work = (runafewfinalizers(L, 10 as libc::c_int) * 50 as libc::c_int) as usize;
            } else {
                (*g).gcstate = 8 as libc::c_int as u8;
                work = 0 as libc::c_int as usize;
            }
        }
        _ => return 0 as libc::c_int as usize,
    }
    (*g).gcstopem = 0 as libc::c_int as u8;
    return work;
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaC_runtilstate(mut L: *mut lua_State, mut statesmask: libc::c_int) {
    let mut g: *mut global_State = (*L).l_G;
    while statesmask & (1 as libc::c_int) << (*g).gcstate as libc::c_int == 0 {
        singlestep(L);
    }
}
unsafe extern "C" fn incstep(mut L: *mut lua_State, mut g: *mut global_State) {
    let mut stepmul: libc::c_int =
        (*g).gcstepmul as libc::c_int * 4 as libc::c_int | 1 as libc::c_int;
    let mut debt: isize = ((*g).GCdebt as libc::c_ulong)
        .wrapping_div(::core::mem::size_of::<TValue>() as libc::c_ulong)
        .wrapping_mul(stepmul as libc::c_ulong) as isize;
    let mut stepsize: isize = (if (*g).gcstepsize as libc::c_ulong
        <= (::core::mem::size_of::<isize>() as libc::c_ulong)
            .wrapping_mul(8 as libc::c_int as libc::c_ulong)
            .wrapping_sub(2 as libc::c_int as libc::c_ulong)
    {
        (((1 as libc::c_int as isize) << (*g).gcstepsize as libc::c_int) as libc::c_ulong)
            .wrapping_div(::core::mem::size_of::<TValue>() as libc::c_ulong)
            .wrapping_mul(stepmul as libc::c_ulong)
    } else {
        (!(0 as libc::c_int as usize) >> 1 as libc::c_int) as isize as libc::c_ulong
    }) as isize;
    loop {
        let mut work: usize = singlestep(L);
        debt = (debt as usize).wrapping_sub(work) as isize as isize;
        if !(debt > -stepsize && (*g).gcstate as libc::c_int != 8 as libc::c_int) {
            break;
        }
    }
    if (*g).gcstate as libc::c_int == 8 as libc::c_int {
        setpause(g);
    } else {
        debt = ((debt / stepmul as isize) as libc::c_ulong)
            .wrapping_mul(::core::mem::size_of::<TValue>() as libc::c_ulong)
            as isize;
        luaE_setdebt(g, debt);
    };
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaC_step(mut L: *mut lua_State) {
    let mut g: *mut global_State = (*L).l_G;
    if !((*g).gcstp as libc::c_int == 0 as libc::c_int) {
        luaE_setdebt(g, -(2000 as libc::c_int) as isize);
    } else if (*g).gckind as libc::c_int == 1 as libc::c_int
        || (*g).lastatomic != 0 as libc::c_int as usize
    {
        genstep(L, g);
    } else {
        incstep(L, g);
    };
}
unsafe extern "C" fn fullinc(mut L: *mut lua_State, mut g: *mut global_State) {
    if (*g).gcstate as libc::c_int <= 2 as libc::c_int {
        entersweep(L);
    }
    luaC_runtilstate(L, (1 as libc::c_int) << 8 as libc::c_int);
    luaC_runtilstate(L, (1 as libc::c_int) << 0 as libc::c_int);
    (*g).gcstate = 1 as libc::c_int as u8;
    luaC_runtilstate(L, (1 as libc::c_int) << 7 as libc::c_int);
    luaC_runtilstate(L, (1 as libc::c_int) << 8 as libc::c_int);
    setpause(g);
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaC_fullgc(mut L: *mut lua_State, mut isemergency: libc::c_int) {
    let mut g: *mut global_State = (*L).l_G;
    (*g).gcemergency = isemergency as u8;
    if (*g).gckind as libc::c_int == 0 as libc::c_int {
        fullinc(L, g);
    } else {
        fullgen(L, g);
    }
    (*g).gcemergency = 0 as libc::c_int as u8;
}
