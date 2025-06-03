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

use crate::gc::{Object, luaC_barrierback_};
use crate::ldebug::luaG_runerror;
use crate::lmem::{luaM_free_, luaM_malloc_, luaM_realloc_};
use crate::lobject::{Node, NodeKey, StkId, TString, TValue, Table, Value, luaO_ceillog2};
use crate::lstate::lua_CFunction;
use crate::lstring::{luaS_eqlngstr, luaS_hashlongstr};
use crate::ltm::TM_EQ;
use crate::lvm::{F2Ieq, luaV_flttointeger};
use crate::{Lua, Mark, Thread};
use libm::frexp;
use std::alloc::Layout;
use std::cell::Cell;
use std::ffi::{c_int, c_uint};
use std::ptr::{addr_of_mut, null};

static mut dummynode_: Node = Node {
    u: {
        let mut init = NodeKey {
            value_: Value {
                gc: 0 as *const Object as *mut Object,
            },
            tt_: (0 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8,
            key_tt: (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8,
            next: 0 as libc::c_int,
            key_val: Value {
                gc: 0 as *const Object as *mut Object,
            },
        };
        init
    },
};

static mut absentkey: TValue = {
    let mut init = TValue {
        value_: Value {
            gc: 0 as *const Object as *mut Object,
        },
        tt_: (0 as libc::c_int | (2 as libc::c_int) << 4 as libc::c_int) as u8,
    };
    init
};

unsafe fn hashint(mut t: *const Table, mut i: i64) -> *mut Node {
    let mut ui: u64 = i as u64;
    if ui <= 2147483647 as libc::c_int as libc::c_uint as u64 {
        return &mut *((*t).node).offset(
            (ui as libc::c_int
                % (((1 as libc::c_int) << (*t).lsizenode as libc::c_int) - 1 as libc::c_int
                    | 1 as libc::c_int)) as isize,
        ) as *mut Node;
    } else {
        return &mut *((*t).node).offset(
            (ui % (((1 as libc::c_int) << (*t).lsizenode as libc::c_int) - 1 as libc::c_int
                | 1 as libc::c_int) as u64) as isize,
        ) as *mut Node;
    };
}

unsafe fn l_hashfloat(mut n: f64) -> libc::c_int {
    let mut i: libc::c_int = 0;
    let mut ni: i64 = 0;
    (n, i) = frexp(n);
    n = n * -((-(2147483647 as libc::c_int) - 1 as libc::c_int) as f64);
    if !(n
        >= (-(0x7fffffffffffffff as libc::c_longlong) - 1 as libc::c_int as libc::c_longlong)
            as libc::c_double
        && n < -((-(0x7fffffffffffffff as libc::c_longlong) - 1 as libc::c_int as libc::c_longlong)
            as libc::c_double)
        && {
            ni = n as libc::c_longlong;
            1 as libc::c_int != 0
        })
    {
        return 0 as libc::c_int;
    } else {
        let mut u: libc::c_uint = (i as libc::c_uint).wrapping_add(ni as libc::c_uint);
        return (if u <= 2147483647 as libc::c_int as libc::c_uint {
            u
        } else {
            !u
        }) as libc::c_int;
    };
}

unsafe fn mainpositionTV(mut t: *const Table, mut key: *const TValue) -> *mut Node {
    match (*key).tt_ as libc::c_int & 0x3f as libc::c_int {
        3 => {
            let mut i: i64 = (*key).value_.i;
            return hashint(t, i);
        }
        19 => {
            let mut n: f64 = (*key).value_.n;
            return &mut *((*t).node).offset(
                (l_hashfloat(n)
                    % (((1 as libc::c_int) << (*t).lsizenode as libc::c_int) - 1 as libc::c_int
                        | 1 as libc::c_int)) as isize,
            ) as *mut Node;
        }
        4 => {
            let mut ts: *mut TString = (*key).value_.gc as *mut TString;
            return &mut *((*t).node).offset(
                ((*ts).hash.get()
                    & (((1 as libc::c_int) << (*t).lsizenode as libc::c_int) - 1 as libc::c_int)
                        as libc::c_uint) as libc::c_int as isize,
            ) as *mut Node;
        }
        20 => {
            let mut ts_0: *mut TString = (*key).value_.gc as *mut TString;
            return &mut *((*t).node).offset(
                (luaS_hashlongstr(ts_0)
                    & (((1 as libc::c_int) << (*t).lsizenode as libc::c_int) - 1 as libc::c_int)
                        as libc::c_uint) as libc::c_int as isize,
            ) as *mut Node;
        }
        1 => {
            return &mut *((*t).node).offset(
                (0 as libc::c_int
                    & ((1 as libc::c_int) << (*t).lsizenode as libc::c_int) - 1 as libc::c_int)
                    as isize,
            ) as *mut Node;
        }
        17 => {
            return &mut *((*t).node).offset(
                (1 as libc::c_int
                    & ((1 as libc::c_int) << (*t).lsizenode as libc::c_int) - 1 as libc::c_int)
                    as isize,
            ) as *mut Node;
        }
        22 => {
            let mut f: lua_CFunction = (*key).value_.f;
            return &mut *((*t).node).offset(
                ((::core::mem::transmute::<lua_CFunction, usize>(f)
                    & 0xffffffff as libc::c_uint as usize) as libc::c_uint)
                    .wrapping_rem(
                        (((1 as libc::c_int) << (*t).lsizenode as libc::c_int) - 1 as libc::c_int
                            | 1 as libc::c_int) as libc::c_uint,
                    ) as isize,
            ) as *mut Node;
        }
        _ => {
            let mut o = (*key).value_.gc;
            return ((*t).node).offset(
                ((o as usize & 0xffffffff as libc::c_uint as usize) as libc::c_uint).wrapping_rem(
                    (((1 as libc::c_int) << (*t).lsizenode as libc::c_int) - 1 as libc::c_int
                        | 1 as libc::c_int) as libc::c_uint,
                ) as isize,
            ) as *mut Node;
        }
    };
}

unsafe fn mainpositionfromnode(mut t: *const Table, mut nd: *mut Node) -> *mut Node {
    let mut key: TValue = TValue {
        value_: Value {
            gc: 0 as *mut Object,
        },
        tt_: 0,
    };
    let mut io_: *mut TValue = &mut key;
    let mut n_: *const Node = nd;
    (*io_).value_ = (*n_).u.key_val;
    (*io_).tt_ = (*n_).u.key_tt;
    return mainpositionTV(t, &mut key);
}

unsafe fn equalkey(mut k1: *const TValue, mut n2: *const Node, mut deadok: libc::c_int) -> c_int {
    if (*k1).tt_ != (*n2).u.key_tt
        && !(deadok != 0
            && (*n2).u.key_tt as libc::c_int == 9 as libc::c_int + 2 as libc::c_int
            && (*k1).tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0)
    {
        return 0 as libc::c_int;
    }

    match (*n2).u.key_tt {
        0 | 1 | 17 => 1,
        3 => ((*k1).value_.i == (*n2).u.key_val.i) as libc::c_int,
        19 => ((*k1).value_.n == (*n2).u.key_val.n) as libc::c_int,
        22 => ((*k1).value_.f == (*n2).u.key_val.f) as libc::c_int,
        84 => luaS_eqlngstr(
            (*k1).value_.gc as *mut TString,
            (*n2).u.key_val.gc as *mut TString,
        ),
        _ => ((*k1).value_.gc == (*n2).u.key_val.gc) as libc::c_int,
    }
}

pub unsafe fn luaH_realasize(mut t: *const Table) -> libc::c_uint {
    if (*t).flags.get() as libc::c_int & (1 as libc::c_int) << 7 as libc::c_int == 0
        || (*t).alimit & ((*t).alimit).wrapping_sub(1 as libc::c_int as libc::c_uint)
            == 0 as libc::c_int as libc::c_uint
    {
        return (*t).alimit;
    } else {
        let mut size: libc::c_uint = (*t).alimit;
        size |= size >> 1 as libc::c_int;
        size |= size >> 2 as libc::c_int;
        size |= size >> 4 as libc::c_int;
        size |= size >> 8 as libc::c_int;
        size |= size >> 16 as libc::c_int;
        size = size.wrapping_add(1);
        return size;
    };
}

unsafe fn ispow2realasize(mut t: *const Table) -> libc::c_int {
    return ((*t).flags.get() as libc::c_int & (1 as libc::c_int) << 7 as libc::c_int != 0
        || (*t).alimit & ((*t).alimit).wrapping_sub(1 as libc::c_int as libc::c_uint)
            == 0 as libc::c_int as libc::c_uint) as libc::c_int;
}

unsafe fn setlimittosize(mut t: *mut Table) -> libc::c_uint {
    (*t).alimit = luaH_realasize(t);
    (*t).flags.set(
        ((*t).flags.get() as libc::c_int & !((1 as libc::c_int) << 7) as u8 as libc::c_int) as u8,
    );
    return (*t).alimit;
}

unsafe fn getgeneric(
    mut t: *mut Table,
    mut key: *const TValue,
    mut deadok: libc::c_int,
) -> *const TValue {
    let mut n: *mut Node = mainpositionTV(t, key);
    loop {
        if equalkey(key, n, deadok) != 0 {
            return &mut (*n).i_val;
        } else {
            let mut nx: libc::c_int = (*n).u.next;
            if nx == 0 as libc::c_int {
                return &raw const absentkey;
            }
            n = n.offset(nx as isize);
        }
    }
}
unsafe fn arrayindex(mut k: i64) -> libc::c_uint {
    if (k as u64).wrapping_sub(1 as libc::c_uint as u64)
        < (if ((1 as libc::c_uint)
            << (::core::mem::size_of::<libc::c_int>() as libc::c_ulong)
                .wrapping_mul(8 as libc::c_int as libc::c_ulong)
                .wrapping_sub(1 as libc::c_int as libc::c_ulong) as libc::c_int)
            as usize
            <= (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<TValue>())
        {
            (1 as libc::c_uint)
                << (::core::mem::size_of::<libc::c_int>() as libc::c_ulong)
                    .wrapping_mul(8 as libc::c_int as libc::c_ulong)
                    .wrapping_sub(1 as libc::c_int as libc::c_ulong)
                    as libc::c_int
        } else {
            (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<TValue>())
                as libc::c_uint
        }) as u64
    {
        return k as libc::c_uint;
    } else {
        return 0 as libc::c_int as libc::c_uint;
    };
}

unsafe fn findindex(
    mut L: *const Thread,
    mut t: *mut Table,
    mut key: *mut TValue,
    mut asize: libc::c_uint,
) -> Result<c_uint, Box<dyn std::error::Error>> {
    let mut i: libc::c_uint = 0;
    if (*key).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int {
        return Ok(0 as libc::c_int as libc::c_uint);
    }
    i = if (*key).tt_ as libc::c_int == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int {
        arrayindex((*key).value_.i)
    } else {
        0 as libc::c_int as libc::c_uint
    };
    if i.wrapping_sub(1 as libc::c_uint) < asize {
        return Ok(i);
    } else {
        let mut n: *const TValue = getgeneric(t, key, 1 as libc::c_int);
        if (((*n).tt_ as libc::c_int == 0 as libc::c_int | (2 as libc::c_int) << 4 as libc::c_int)
            as libc::c_int
            != 0 as libc::c_int) as libc::c_int as libc::c_long
            != 0
        {
            luaG_runerror(L, "invalid key to 'next'")?;
        }
        i = (n as *mut Node)
            .offset_from(&mut *((*t).node).offset(0 as libc::c_int as isize) as *mut Node)
            as libc::c_long as libc::c_int as libc::c_uint;
        return Ok(i
            .wrapping_add(1 as libc::c_int as libc::c_uint)
            .wrapping_add(asize));
    };
}

pub unsafe fn luaH_next(
    mut L: *const Thread,
    mut t: *mut Table,
    mut key: StkId,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut asize: libc::c_uint = luaH_realasize(t);
    let mut i: libc::c_uint = findindex(L, t, &mut (*key).val, asize)?;
    while i < asize {
        if !((*((*t).array).offset(i as isize)).tt_ as libc::c_int & 0xf as libc::c_int
            == 0 as libc::c_int)
        {
            let mut io: *mut TValue = &mut (*key).val;
            (*io).value_.i = i.wrapping_add(1 as libc::c_int as libc::c_uint) as i64;
            (*io).tt_ = (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
            let mut io1: *mut TValue = &mut (*key.offset(1 as libc::c_int as isize)).val;
            let mut io2: *const TValue = &mut *((*t).array).offset(i as isize) as *mut TValue;
            (*io1).value_ = (*io2).value_;
            (*io1).tt_ = (*io2).tt_;
            return Ok(1 as libc::c_int);
        }
        i = i.wrapping_add(1);
    }
    i = i.wrapping_sub(asize);
    while (i as libc::c_int) < (1 as libc::c_int) << (*t).lsizenode as libc::c_int {
        if !((*((*t).node).offset(i as isize)).i_val.tt_ as libc::c_int & 0xf as libc::c_int
            == 0 as libc::c_int)
        {
            let mut n: *mut Node = &mut *((*t).node).offset(i as isize) as *mut Node;
            let mut io_: *mut TValue = &mut (*key).val;
            let mut n_: *const Node = n;
            (*io_).value_ = (*n_).u.key_val;
            (*io_).tt_ = (*n_).u.key_tt;
            let mut io1_0: *mut TValue = &mut (*key.offset(1 as libc::c_int as isize)).val;
            let mut io2_0: *const TValue = &mut (*n).i_val;
            (*io1_0).value_ = (*io2_0).value_;
            (*io1_0).tt_ = (*io2_0).tt_;
            return Ok(1 as libc::c_int);
        }
        i = i.wrapping_add(1);
    }
    return Ok(0 as libc::c_int);
}

unsafe fn freehash(g: *const Lua, mut t: *mut Table) {
    if !((*t).lastfree).is_null() {
        luaM_free_(
            g,
            (*t).node as *mut libc::c_void,
            (((1 as libc::c_int) << (*t).lsizenode as libc::c_int) as usize)
                .wrapping_mul(size_of::<Node>()),
        );
    }
}

unsafe extern "C" fn computesizes(
    mut nums: *mut libc::c_uint,
    mut pna: *mut libc::c_uint,
) -> libc::c_uint {
    let mut i: libc::c_int = 0;
    let mut twotoi: libc::c_uint = 0;
    let mut a: libc::c_uint = 0 as libc::c_int as libc::c_uint;
    let mut na: libc::c_uint = 0 as libc::c_int as libc::c_uint;
    let mut optimal: libc::c_uint = 0 as libc::c_int as libc::c_uint;
    i = 0 as libc::c_int;
    twotoi = 1 as libc::c_int as libc::c_uint;
    while twotoi > 0 as libc::c_int as libc::c_uint
        && *pna > twotoi.wrapping_div(2 as libc::c_int as libc::c_uint)
    {
        a = a.wrapping_add(*nums.offset(i as isize));
        if a > twotoi.wrapping_div(2 as libc::c_int as libc::c_uint) {
            optimal = twotoi;
            na = a;
        }
        i += 1;
        twotoi = twotoi.wrapping_mul(2 as libc::c_int as libc::c_uint);
    }
    *pna = na;
    return optimal;
}
unsafe extern "C" fn countint(mut key: i64, mut nums: *mut libc::c_uint) -> libc::c_int {
    let mut k: libc::c_uint = arrayindex(key);
    if k != 0 as libc::c_int as libc::c_uint {
        let ref mut fresh0 = *nums.offset(luaO_ceillog2(k) as isize);
        *fresh0 = (*fresh0).wrapping_add(1);
        return 1 as libc::c_int;
    } else {
        return 0 as libc::c_int;
    };
}
unsafe extern "C" fn numusearray(mut t: *const Table, mut nums: *mut libc::c_uint) -> libc::c_uint {
    let mut lg: libc::c_int = 0;
    let mut ttlg: libc::c_uint = 0;
    let mut ause: libc::c_uint = 0 as libc::c_int as libc::c_uint;
    let mut i: libc::c_uint = 1 as libc::c_int as libc::c_uint;
    let mut asize: libc::c_uint = (*t).alimit;
    lg = 0 as libc::c_int;
    ttlg = 1 as libc::c_int as libc::c_uint;
    while lg
        <= (::core::mem::size_of::<libc::c_int>() as libc::c_ulong)
            .wrapping_mul(8 as libc::c_int as libc::c_ulong)
            .wrapping_sub(1 as libc::c_int as libc::c_ulong) as libc::c_int
    {
        let mut lc: libc::c_uint = 0 as libc::c_int as libc::c_uint;
        let mut lim: libc::c_uint = ttlg;
        if lim > asize {
            lim = asize;
            if i > lim {
                break;
            }
        }
        while i <= lim {
            if !((*((*t).array).offset(i.wrapping_sub(1 as libc::c_int as libc::c_uint) as isize))
                .tt_ as libc::c_int
                & 0xf as libc::c_int
                == 0 as libc::c_int)
            {
                lc = lc.wrapping_add(1);
            }
            i = i.wrapping_add(1);
        }
        let ref mut fresh1 = *nums.offset(lg as isize);
        *fresh1 = (*fresh1).wrapping_add(lc);
        ause = ause.wrapping_add(lc);
        lg += 1;
        ttlg = ttlg.wrapping_mul(2 as libc::c_int as libc::c_uint);
    }
    return ause;
}
unsafe extern "C" fn numusehash(
    mut t: *const Table,
    mut nums: *mut libc::c_uint,
    mut pna: *mut libc::c_uint,
) -> libc::c_int {
    let mut totaluse: libc::c_int = 0 as libc::c_int;
    let mut ause: libc::c_int = 0 as libc::c_int;
    let mut i: libc::c_int = (1 as libc::c_int) << (*t).lsizenode as libc::c_int;
    loop {
        let fresh2 = i;
        i = i - 1;
        if !(fresh2 != 0) {
            break;
        }
        let mut n: *mut Node = &mut *((*t).node).offset(i as isize) as *mut Node;
        if !((*n).i_val.tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int) {
            if (*n).u.key_tt as libc::c_int
                == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
            {
                ause += countint((*n).u.key_val.i, nums);
            }
            totaluse += 1;
        }
    }
    *pna = (*pna).wrapping_add(ause as libc::c_uint);
    return totaluse;
}

unsafe fn setnodevector(
    mut L: *const Thread,
    mut t: *mut Table,
    mut size: libc::c_uint,
) -> Result<(), Box<dyn std::error::Error>> {
    if size == 0 as libc::c_int as libc::c_uint {
        (*t).node = &raw mut dummynode_;
        (*t).lsizenode = 0 as libc::c_int as u8;
        (*t).lastfree = 0 as *mut Node;
    } else {
        let mut i: libc::c_int = 0;
        let mut lsize: libc::c_int = luaO_ceillog2(size);
        if lsize
            > (::core::mem::size_of::<libc::c_int>() as libc::c_ulong)
                .wrapping_mul(8 as libc::c_int as libc::c_ulong)
                .wrapping_sub(1 as libc::c_int as libc::c_ulong) as libc::c_int
                - 1 as libc::c_int
            || (1 as libc::c_uint) << lsize
                > (if ((1 as libc::c_uint)
                    << (::core::mem::size_of::<libc::c_int>() as libc::c_ulong)
                        .wrapping_mul(8 as libc::c_int as libc::c_ulong)
                        .wrapping_sub(1 as libc::c_int as libc::c_ulong)
                        as libc::c_int
                        - 1 as libc::c_int) as usize
                    <= (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<Node>())
                {
                    (1 as libc::c_uint)
                        << (::core::mem::size_of::<libc::c_int>() as libc::c_ulong)
                            .wrapping_mul(8 as libc::c_int as libc::c_ulong)
                            .wrapping_sub(1 as libc::c_int as libc::c_ulong)
                            as libc::c_int
                            - 1 as libc::c_int
                } else {
                    (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<Node>())
                        as libc::c_uint
                })
        {
            luaG_runerror(L, "table overflow")?;
        }
        size = ((1 as libc::c_int) << lsize) as libc::c_uint;
        (*t).node = luaM_malloc_(
            (*L).global,
            (size as usize).wrapping_mul(::core::mem::size_of::<Node>()),
        ) as *mut Node;
        i = 0 as libc::c_int;
        while i < size as libc::c_int {
            let mut n: *mut Node = &mut *((*t).node).offset(i as isize) as *mut Node;
            (*n).u.next = 0 as libc::c_int;
            (*n).u.key_tt = 0 as libc::c_int as u8;
            (*n).i_val.tt_ = (0 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
            i += 1;
        }
        (*t).lsizenode = lsize as u8;
        (*t).lastfree = &mut *((*t).node).offset(size as isize) as *mut Node;
    };

    Ok(())
}

unsafe fn reinsert(
    mut L: *const Thread,
    mut ot: *mut Table,
    mut t: *mut Table,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut j: libc::c_int = 0;
    let mut size: libc::c_int = (1 as libc::c_int) << (*ot).lsizenode as libc::c_int;
    j = 0 as libc::c_int;

    while j < size {
        let mut old: *mut Node = &mut *((*ot).node).offset(j as isize) as *mut Node;
        if !((*old).i_val.tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int) {
            let mut k: TValue = TValue {
                value_: Value {
                    gc: 0 as *mut Object,
                },
                tt_: 0,
            };
            let mut io_: *mut TValue = &mut k;
            let mut n_: *const Node = old;
            (*io_).value_ = (*n_).u.key_val;
            (*io_).tt_ = (*n_).u.key_tt;
            luaH_set(L, t, &mut k, &mut (*old).i_val)?;
        }
        j += 1;
    }

    Ok(())
}

unsafe extern "C" fn exchangehashpart(mut t1: *mut Table, mut t2: *mut Table) {
    let mut lsizenode: u8 = (*t1).lsizenode;
    let mut node: *mut Node = (*t1).node;
    let mut lastfree: *mut Node = (*t1).lastfree;
    (*t1).lsizenode = (*t2).lsizenode;
    (*t1).node = (*t2).node;
    (*t1).lastfree = (*t2).lastfree;
    (*t2).lsizenode = lsizenode;
    (*t2).node = node;
    (*t2).lastfree = lastfree;
}

pub unsafe fn luaH_resize(
    mut L: *const Thread,
    mut t: *mut Table,
    mut newasize: libc::c_uint,
    mut nhsize: libc::c_uint,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut i: libc::c_uint = 0;
    let mut newt = Table {
        hdr: Object {
            next: Cell::new(0 as *mut Object),
            tt: 0,
            marked: Mark::new(0),
            refs: Cell::new(0),
            refn: Cell::new(null()),
            refp: Cell::new(null()),
            gclist: Cell::new(null()),
        },
        flags: Cell::new(0),
        lsizenode: 0,
        alimit: 0,
        array: 0 as *mut TValue,
        node: 0 as *mut Node,
        lastfree: 0 as *mut Node,
        metatable: 0 as *mut Table,
    };
    let mut oldasize: libc::c_uint = setlimittosize(t);
    let mut newarray: *mut TValue = 0 as *mut TValue;
    setnodevector(L, &mut newt, nhsize)?;
    if newasize < oldasize {
        (*t).alimit = newasize;
        exchangehashpart(t, &mut newt);
        i = newasize;
        while i < oldasize {
            if !((*((*t).array).offset(i as isize)).tt_ as libc::c_int & 0xf as libc::c_int
                == 0 as libc::c_int)
            {
                luaH_setint(
                    L,
                    t,
                    i.wrapping_add(1 as libc::c_int as libc::c_uint) as i64,
                    &mut *((*t).array).offset(i as isize),
                )?;
            }
            i = i.wrapping_add(1);
        }
        (*t).alimit = oldasize;
        exchangehashpart(t, &mut newt);
    }

    newarray = luaM_realloc_(
        (*L).global,
        (*t).array as *mut libc::c_void,
        (oldasize as usize).wrapping_mul(::core::mem::size_of::<TValue>()),
        (newasize as usize).wrapping_mul(::core::mem::size_of::<TValue>()),
    ) as *mut TValue;

    if ((newarray.is_null() && newasize > 0 as libc::c_int as libc::c_uint) as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        freehash((*L).global, &mut newt);
        todo!("invoke handle_alloc_error");
    }

    exchangehashpart(t, &mut newt);
    (*t).array = newarray;
    (*t).alimit = newasize;
    i = oldasize;

    while i < newasize {
        (*((*t).array).offset(i as isize)).tt_ =
            (0 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
        i = i.wrapping_add(1);
    }

    reinsert(L, &mut newt, t)?;
    freehash((*L).global, &mut newt);

    Ok(())
}

pub unsafe fn luaH_resizearray(
    mut L: *const Thread,
    mut t: *mut Table,
    mut nasize: libc::c_uint,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut nsize: libc::c_int = if ((*t).lastfree).is_null() {
        0 as libc::c_int
    } else {
        (1 as libc::c_int) << (*t).lsizenode as libc::c_int
    };

    luaH_resize(L, t, nasize, nsize as libc::c_uint)
}

unsafe fn rehash(
    mut L: *const Thread,
    mut t: *mut Table,
    mut ek: *const TValue,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut asize: libc::c_uint = 0;
    let mut na: libc::c_uint = 0;
    let mut nums: [libc::c_uint; 32] = [0; 32];
    let mut i: libc::c_int = 0;
    let mut totaluse: libc::c_int = 0;
    i = 0 as libc::c_int;
    while i
        <= (::core::mem::size_of::<libc::c_int>() as libc::c_ulong)
            .wrapping_mul(8 as libc::c_int as libc::c_ulong)
            .wrapping_sub(1 as libc::c_int as libc::c_ulong) as libc::c_int
    {
        nums[i as usize] = 0 as libc::c_int as libc::c_uint;
        i += 1;
    }
    setlimittosize(t);
    na = numusearray(t, nums.as_mut_ptr());
    totaluse = na as libc::c_int;
    totaluse += numusehash(t, nums.as_mut_ptr(), &mut na);
    if (*ek).tt_ as libc::c_int == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int {
        na = na.wrapping_add(countint((*ek).value_.i, nums.as_mut_ptr()) as libc::c_uint);
    }
    totaluse += 1;
    asize = computesizes(nums.as_mut_ptr(), &mut na);
    luaH_resize(L, t, asize, (totaluse as libc::c_uint).wrapping_sub(na))
}

pub unsafe fn luaH_new(mut L: *const Thread) -> Result<*mut Table, Box<dyn std::error::Error>> {
    let layout = Layout::new::<Table>();
    let o = Object::new((*L).global, 5 | 0 << 4, layout).cast::<Table>();

    (*o).metatable = 0 as *mut Table;
    addr_of_mut!((*o).flags).write(Cell::new(!(!(0 as libc::c_uint) << TM_EQ + 1) as u8));
    (*o).array = 0 as *mut TValue;
    (*o).alimit = 0 as libc::c_int as libc::c_uint;
    setnodevector(L, o, 0 as libc::c_int as libc::c_uint)?;

    Ok(o)
}

pub unsafe fn luaH_free(g: *const Lua, mut t: *mut Table) {
    let layout = Layout::new::<Table>();

    freehash(g, t);
    luaM_free_(
        g,
        (*t).array as *mut libc::c_void,
        (luaH_realasize(t) as usize).wrapping_mul(size_of::<TValue>()),
    );

    (*g).gc.dealloc(t.cast(), layout);
}

unsafe extern "C" fn getfreepos(mut t: *mut Table) -> *mut Node {
    if !((*t).lastfree).is_null() {
        while (*t).lastfree > (*t).node {
            (*t).lastfree = ((*t).lastfree).offset(-1);
            (*t).lastfree;
            if (*(*t).lastfree).u.key_tt as libc::c_int == 0 as libc::c_int {
                return (*t).lastfree;
            }
        }
    }
    return 0 as *mut Node;
}

unsafe fn luaH_newkey(
    mut L: *const Thread,
    mut t: *mut Table,
    mut key: *const TValue,
    mut value: *mut TValue,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut mp: *mut Node = 0 as *mut Node;
    let mut aux: TValue = TValue {
        value_: Value {
            gc: 0 as *mut Object,
        },
        tt_: 0,
    };
    if (((*key).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int) as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        luaG_runerror(L, "table index is nil")?;
    } else if (*key).tt_ as libc::c_int == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
    {
        let mut f: f64 = (*key).value_.n;
        let mut k: i64 = 0;
        if luaV_flttointeger(f, &mut k, F2Ieq) != 0 {
            let mut io: *mut TValue = &mut aux;
            (*io).value_.i = k;
            (*io).tt_ = (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
            key = &mut aux;
        } else if (!(f == f) as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long != 0
        {
            luaG_runerror(L, "table index is NaN")?;
        }
    }
    if (*value).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int {
        return Ok(());
    }
    mp = mainpositionTV(t, key);
    if !((*mp).i_val.tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int)
        || ((*t).lastfree).is_null()
    {
        let mut othern: *mut Node = 0 as *mut Node;
        let mut f_0: *mut Node = getfreepos(t);
        if f_0.is_null() {
            rehash(L, t, key)?;
            luaH_set(L, t, key, value)?;
            return Ok(());
        }
        othern = mainpositionfromnode(t, mp);
        if othern != mp {
            while othern.offset((*othern).u.next as isize) != mp {
                othern = othern.offset((*othern).u.next as isize);
            }
            (*othern).u.next = f_0.offset_from(othern) as libc::c_long as libc::c_int;
            *f_0 = *mp;
            if (*mp).u.next != 0 as libc::c_int {
                (*f_0).u.next += mp.offset_from(f_0) as libc::c_long as libc::c_int;
                (*mp).u.next = 0 as libc::c_int;
            }
            (*mp).i_val.tt_ = (0 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
        } else {
            if (*mp).u.next != 0 as libc::c_int {
                (*f_0).u.next = mp.offset((*mp).u.next as isize).offset_from(f_0) as libc::c_long
                    as libc::c_int;
            }
            (*mp).u.next = f_0.offset_from(mp) as libc::c_long as libc::c_int;
            mp = f_0;
        }
    }
    let mut n_: *mut Node = mp;
    let mut io_: *const TValue = key;
    (*n_).u.key_val = (*io_).value_;
    (*n_).u.key_tt = (*io_).tt_;
    if (*key).tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0 {
        if (*t).hdr.marked.get() as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
            && (*(*key).value_.gc).marked.get() as libc::c_int
                & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
                != 0
        {
            luaC_barrierback_(L, t as *mut Object);
        } else {
        };
    } else {
    };
    let mut io1: *mut TValue = &mut (*mp).i_val;
    let mut io2: *const TValue = value;
    (*io1).value_ = (*io2).value_;
    (*io1).tt_ = (*io2).tt_;
    Ok(())
}

pub unsafe fn luaH_getint(mut t: *mut Table, mut key: i64) -> *const TValue {
    let mut alimit: u64 = (*t).alimit as u64;
    if (key as u64).wrapping_sub(1 as libc::c_uint as u64) < alimit {
        return &mut *((*t).array).offset((key - 1 as libc::c_int as i64) as isize) as *mut TValue;
    } else if (*t).flags.get() as libc::c_int & (1 as libc::c_int) << 7 as libc::c_int != 0
        && (key as u64).wrapping_sub(1 as libc::c_uint as u64)
            & !alimit.wrapping_sub(1 as libc::c_uint as u64)
            < alimit
    {
        (*t).alimit = key as libc::c_uint;
        return &mut *((*t).array).offset((key - 1 as libc::c_int as i64) as isize) as *mut TValue;
    } else {
        let mut n: *mut Node = hashint(t, key);
        loop {
            if (*n).u.key_tt as libc::c_int
                == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                && (*n).u.key_val.i == key
            {
                return &mut (*n).i_val;
            } else {
                let mut nx: libc::c_int = (*n).u.next;
                if nx == 0 as libc::c_int {
                    break;
                }
                n = n.offset(nx as isize);
            }
        }
        return &raw const absentkey;
    };
}

pub unsafe fn luaH_getshortstr(mut t: *mut Table, mut key: *mut TString) -> *const TValue {
    let mut n = ((*t).node).offset(((*key).hash.get() & ((1 << (*t).lsizenode) - 1)) as isize);

    loop {
        if (*n).u.key_tt as libc::c_int
            == 4 as libc::c_int
                | (0 as libc::c_int) << 4 as libc::c_int
                | (1 as libc::c_int) << 6 as libc::c_int
            && ((*n).u.key_val.gc as *mut TString) as *mut TString == key
        {
            return &mut (*n).i_val;
        } else {
            let mut nx: libc::c_int = (*n).u.next;
            if nx == 0 as libc::c_int {
                return &raw const absentkey;
            }
            n = n.offset(nx as isize);
        }
    }
}

pub unsafe fn luaH_getstr(mut t: *mut Table, mut key: *mut TString) -> *const TValue {
    if (*key).hdr.tt as libc::c_int == 4 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int {
        return luaH_getshortstr(t, key);
    } else {
        let mut ko: TValue = TValue {
            value_: Value {
                gc: 0 as *mut Object,
            },
            tt_: 0,
        };
        let mut io: *mut TValue = &mut ko;
        let mut x_: *mut TString = key;
        (*io).value_.gc = x_ as *mut Object;
        (*io).tt_ = ((*x_).hdr.tt as libc::c_int | (1 as libc::c_int) << 6 as libc::c_int) as u8;
        return getgeneric(t, &mut ko, 0 as libc::c_int);
    };
}

pub unsafe fn luaH_get(mut t: *mut Table, mut key: *const TValue) -> *const TValue {
    match (*key).tt_ as libc::c_int & 0x3f as libc::c_int {
        4 => return luaH_getshortstr(t, (*key).value_.gc as *mut TString),
        3 => return luaH_getint(t, (*key).value_.i),
        0 => return &raw const absentkey,
        19 => {
            let mut k: i64 = 0;
            if luaV_flttointeger((*key).value_.n, &mut k, F2Ieq) != 0 {
                return luaH_getint(t, k);
            }
        }
        _ => {}
    }
    return getgeneric(t, key, 0 as libc::c_int);
}

pub unsafe fn luaH_finishset(
    mut L: *const Thread,
    mut t: *mut Table,
    mut key: *const TValue,
    mut slot: *const TValue,
    mut value: *mut TValue,
) -> Result<(), Box<dyn std::error::Error>> {
    if (*slot).tt_ as libc::c_int == 0 as libc::c_int | (2 as libc::c_int) << 4 as libc::c_int {
        luaH_newkey(L, t, key, value)?;
    } else {
        let mut io1: *mut TValue = slot as *mut TValue;
        let mut io2: *const TValue = value;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
    };
    Ok(())
}

pub unsafe fn luaH_set(
    mut L: *const Thread,
    mut t: *mut Table,
    mut key: *const TValue,
    mut value: *mut TValue,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut slot: *const TValue = luaH_get(t, key);

    luaH_finishset(L, t, key, slot, value)
}

pub unsafe fn luaH_setint(
    mut L: *const Thread,
    mut t: *mut Table,
    mut key: i64,
    mut value: *mut TValue,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut p: *const TValue = luaH_getint(t, key);
    if (*p).tt_ as libc::c_int == 0 as libc::c_int | (2 as libc::c_int) << 4 as libc::c_int {
        let mut k: TValue = TValue {
            value_: Value {
                gc: 0 as *mut Object,
            },
            tt_: 0,
        };
        let mut io: *mut TValue = &mut k;
        (*io).value_.i = key;
        (*io).tt_ = (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
        luaH_newkey(L, t, &mut k, value)?;
    } else {
        let mut io1: *mut TValue = p as *mut TValue;
        let mut io2: *const TValue = value;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
    };
    Ok(())
}

unsafe extern "C" fn hash_search(mut t: *mut Table, mut j: u64) -> u64 {
    let mut i: u64 = 0;
    if j == 0 as libc::c_int as u64 {
        j = j.wrapping_add(1);
    }
    loop {
        i = j;
        if j <= 0x7fffffffffffffff as libc::c_longlong as u64 / 2 as libc::c_int as u64 {
            j = j * 2 as libc::c_int as u64;
            if (*luaH_getint(t, j as i64)).tt_ as libc::c_int & 0xf as libc::c_int
                == 0 as libc::c_int
            {
                break;
            }
        } else {
            j = 0x7fffffffffffffff as libc::c_longlong as u64;
            if (*luaH_getint(t, j as i64)).tt_ as libc::c_int & 0xf as libc::c_int
                == 0 as libc::c_int
            {
                break;
            }
            return j;
        }
    }
    while j.wrapping_sub(i) > 1 as libc::c_uint as u64 {
        let mut m: u64 = i.wrapping_add(j) / 2 as libc::c_int as u64;
        if (*luaH_getint(t, m as i64)).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int {
            j = m;
        } else {
            i = m;
        }
    }
    return i;
}
unsafe extern "C" fn binsearch(
    mut array: *const TValue,
    mut i: libc::c_uint,
    mut j: libc::c_uint,
) -> libc::c_uint {
    while j.wrapping_sub(i) > 1 as libc::c_uint {
        let mut m: libc::c_uint = i
            .wrapping_add(j)
            .wrapping_div(2 as libc::c_int as libc::c_uint);
        if (*array.offset(m.wrapping_sub(1 as libc::c_int as libc::c_uint) as isize)).tt_
            as libc::c_int
            & 0xf as libc::c_int
            == 0 as libc::c_int
        {
            j = m;
        } else {
            i = m;
        }
    }
    return i;
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaH_getn(mut t: *mut Table) -> u64 {
    let mut limit: libc::c_uint = (*t).alimit;
    if limit > 0 as libc::c_int as libc::c_uint
        && (*((*t).array).offset(limit.wrapping_sub(1 as libc::c_int as libc::c_uint) as isize)).tt_
            as libc::c_int
            & 0xf as libc::c_int
            == 0 as libc::c_int
    {
        if limit >= 2 as libc::c_int as libc::c_uint
            && !((*((*t).array)
                .offset(limit.wrapping_sub(2 as libc::c_int as libc::c_uint) as isize))
            .tt_ as libc::c_int
                & 0xf as libc::c_int
                == 0 as libc::c_int)
        {
            if ispow2realasize(t) != 0
                && !(limit.wrapping_sub(1 as libc::c_int as libc::c_uint)
                    & limit
                        .wrapping_sub(1 as libc::c_int as libc::c_uint)
                        .wrapping_sub(1 as libc::c_int as libc::c_uint)
                    == 0 as libc::c_int as libc::c_uint)
            {
                (*t).alimit = limit.wrapping_sub(1 as libc::c_int as libc::c_uint);
                (*t).flags
                    .set(((*t).flags.get() as libc::c_int | (1 as libc::c_int) << 7) as u8);
            }
            return limit.wrapping_sub(1 as libc::c_int as libc::c_uint) as u64;
        } else {
            let mut boundary: libc::c_uint =
                binsearch((*t).array, 0 as libc::c_int as libc::c_uint, limit);
            if ispow2realasize(t) != 0
                && boundary > (luaH_realasize(t)).wrapping_div(2 as libc::c_int as libc::c_uint)
            {
                (*t).alimit = boundary;
                (*t).flags
                    .set(((*t).flags.get() as libc::c_int | (1 as libc::c_int) << 7) as u8);
            }
            return boundary as u64;
        }
    }
    if !((*t).flags.get() as libc::c_int & (1 as libc::c_int) << 7 as libc::c_int == 0
        || (*t).alimit & ((*t).alimit).wrapping_sub(1 as libc::c_int as libc::c_uint)
            == 0 as libc::c_int as libc::c_uint)
    {
        if (*((*t).array).offset(limit as isize)).tt_ as libc::c_int & 0xf as libc::c_int
            == 0 as libc::c_int
        {
            return limit as u64;
        }
        limit = luaH_realasize(t);
        if (*((*t).array).offset(limit.wrapping_sub(1 as libc::c_int as libc::c_uint) as isize)).tt_
            as libc::c_int
            & 0xf as libc::c_int
            == 0 as libc::c_int
        {
            let mut boundary_0: libc::c_uint = binsearch((*t).array, (*t).alimit, limit);
            (*t).alimit = boundary_0;
            return boundary_0 as u64;
        }
    }
    if ((*t).lastfree).is_null()
        || (*luaH_getint(
            t,
            limit.wrapping_add(1 as libc::c_int as libc::c_uint) as i64,
        ))
        .tt_ as libc::c_int
            & 0xf as libc::c_int
            == 0 as libc::c_int
    {
        return limit as u64;
    } else {
        return hash_search(t, limit as u64);
    };
}
