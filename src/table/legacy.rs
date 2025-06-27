#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments
)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::gc::{Object, luaC_barrierback_};
use crate::ldebug::luaG_runerror;
use crate::lmem::{luaM_free_, luaM_malloc_, luaM_realloc_};
use crate::lobject::{StkId, luaO_ceillog2};
use crate::lstring::{luaS_eqlngstr, luaS_hashlongstr};
use crate::value::{UnsafeValue, UntaggedValue};
use crate::vm::{F2Ieq, luaV_flttointeger};
use crate::{Node, NodeKey, Str, Table, TableError, Thread};
use alloc::boxed::Box;
use core::alloc::Layout;
use core::cell::Cell;
use libm::frexp;

type c_int = i32;
type c_uint = u32;
type c_longlong = i64;
type c_double = f64;

pub(super) static mut dummynode_: Node = Node {
    u: {
        let init = NodeKey {
            value_: UntaggedValue {
                gc: 0 as *const Object as *mut Object,
            },
            tt_: (0 as c_int | (1 as c_int) << 4 as c_int) as u8,
            key_tt: (0 as c_int | (0 as c_int) << 4 as c_int) as u8,
            next: 0 as c_int,
            key_val: UntaggedValue {
                gc: 0 as *const Object as *mut Object,
            },
        };
        init
    },
};

static mut absentkey: UnsafeValue = {
    let init = UnsafeValue {
        value_: UntaggedValue {
            gc: 0 as *const Object as *mut Object,
        },
        tt_: (0 as c_int | (2 as c_int) << 4 as c_int) as u8,
    };
    init
};

unsafe fn hashint(t: *const Table, i: i64) -> *mut Node {
    let ui: u64 = i as u64;
    if ui <= 2147483647 as c_int as c_uint as u64 {
        return ((*t).node.get()).offset(
            (ui as c_int
                % (((1 as c_int) << (*t).lsizenode.get() as c_int) - 1 as c_int | 1 as c_int))
                as isize,
        ) as *mut Node;
    } else {
        return ((*t).node.get()).offset(
            (ui % (((1 as c_int) << (*t).lsizenode.get() as c_int) - 1 as c_int | 1 as c_int)
                as u64) as isize,
        ) as *mut Node;
    };
}

unsafe fn l_hashfloat(mut n: f64) -> c_int {
    let mut i: c_int = 0;
    let mut ni: i64 = 0;
    (n, i) = frexp(n);
    n = n * -((-(2147483647 as c_int) - 1 as c_int) as f64);
    if !(n >= (-(0x7fffffffffffffff as c_longlong) - 1 as c_int as c_longlong) as c_double
        && n < -((-(0x7fffffffffffffff as c_longlong) - 1 as c_int as c_longlong) as c_double)
        && {
            ni = n as c_longlong;
            1 as c_int != 0
        })
    {
        return 0 as c_int;
    } else {
        let u: c_uint = (i as c_uint).wrapping_add(ni as c_uint);
        return (if u <= 2147483647 as c_int as c_uint {
            u
        } else {
            !u
        }) as c_int;
    };
}

unsafe fn mainpositionTV(t: *const Table, key: *const UnsafeValue) -> *mut Node {
    match (*key).tt_ as c_int & 0x3f as c_int {
        3 => {
            let i: i64 = (*key).value_.i;
            return hashint(t, i);
        }
        19 => {
            let n: f64 = (*key).value_.n;
            return ((*t).node.get()).offset(
                (l_hashfloat(n)
                    % (((1 as c_int) << (*t).lsizenode.get() as c_int) - 1 as c_int | 1 as c_int))
                    as isize,
            ) as *mut Node;
        }
        4 => {
            let ts: *mut Str = (*key).value_.gc as *mut Str;
            return ((*t).node.get()).offset(
                ((*ts).hash.get()
                    & (((1 as c_int) << (*t).lsizenode.get() as c_int) - 1 as c_int) as c_uint)
                    as c_int as isize,
            ) as *mut Node;
        }
        20 => {
            let ts_0: *mut Str = (*key).value_.gc as *mut Str;
            return ((*t).node.get()).offset(
                (luaS_hashlongstr(ts_0)
                    & (((1 as c_int) << (*t).lsizenode.get() as c_int) - 1 as c_int) as c_uint)
                    as c_int as isize,
            ) as *mut Node;
        }
        1 => {
            return ((*t).node.get()).offset(
                (0 as c_int & ((1 as c_int) << (*t).lsizenode.get() as c_int) - 1 as c_int)
                    as isize,
            ) as *mut Node;
        }
        17 => {
            return ((*t).node.get()).offset(
                (1 as c_int & ((1 as c_int) << (*t).lsizenode.get() as c_int) - 1 as c_int)
                    as isize,
            ) as *mut Node;
        }
        2 => {
            let f = (*key).value_.f;

            (*t).node
                .get()
                .offset((((f as usize) & 0xffffffff) as c_uint).wrapping_rem(
                    (((1 as c_int) << (*t).lsizenode.get() as c_int) - 1 as c_int | 1 as c_int)
                        as c_uint,
                ) as isize)
        }
        18 | 34 | 50 => todo!(),
        _ => {
            let o = (*key).value_.gc;
            return ((*t).node.get()).offset(
                ((o as usize & 0xffffffff as c_uint as usize) as c_uint).wrapping_rem(
                    (((1 as c_int) << (*t).lsizenode.get() as c_int) - 1 as c_int | 1 as c_int)
                        as c_uint,
                ) as isize,
            ) as *mut Node;
        }
    }
}

unsafe fn mainpositionfromnode(t: *const Table, nd: *mut Node) -> *mut Node {
    let mut key: UnsafeValue = UnsafeValue {
        value_: UntaggedValue {
            gc: 0 as *mut Object,
        },
        tt_: 0,
    };
    let io_: *mut UnsafeValue = &mut key;
    let n_: *const Node = nd;
    (*io_).value_ = (*n_).u.key_val;
    (*io_).tt_ = (*n_).u.key_tt;
    return mainpositionTV(t, &mut key);
}

unsafe fn equalkey(k1: *const UnsafeValue, n2: *const Node, deadok: c_int) -> c_int {
    if (*k1).tt_ != (*n2).u.key_tt
        && !(deadok != 0
            && (*n2).u.key_tt as c_int == 9 as c_int + 2 as c_int
            && (*k1).tt_ as c_int & (1 as c_int) << 6 as c_int != 0)
    {
        return 0 as c_int;
    }

    match (*n2).u.key_tt {
        0 | 1 | 17 => 1,
        3 => ((*k1).value_.i == (*n2).u.key_val.i) as c_int,
        19 => ((*k1).value_.n == (*n2).u.key_val.n) as c_int,
        2 | 18 | 34 | 50 => core::ptr::fn_addr_eq((*k1).value_.f, (*n2).u.key_val.f) as c_int,
        84 => luaS_eqlngstr((*k1).value_.gc as *mut Str, (*n2).u.key_val.gc as *mut Str),
        _ => ((*k1).value_.gc == (*n2).u.key_val.gc) as c_int,
    }
}

pub unsafe fn luaH_realasize(t: *const Table) -> c_uint {
    if (*t).flags.get() as c_int & (1 as c_int) << 7 as c_int == 0
        || (*t).alimit.get() & ((*t).alimit.get()).wrapping_sub(1 as c_int as c_uint)
            == 0 as c_int as c_uint
    {
        return (*t).alimit.get();
    } else {
        let mut size: c_uint = (*t).alimit.get();
        size |= size >> 1 as c_int;
        size |= size >> 2 as c_int;
        size |= size >> 4 as c_int;
        size |= size >> 8 as c_int;
        size |= size >> 16 as c_int;
        size = size.wrapping_add(1);
        return size;
    };
}

unsafe fn ispow2realasize(t: *const Table) -> c_int {
    return ((*t).flags.get() as c_int & (1 as c_int) << 7 as c_int != 0
        || (*t).alimit.get() & ((*t).alimit.get()).wrapping_sub(1 as c_int as c_uint)
            == 0 as c_int as c_uint) as c_int;
}

unsafe fn setlimittosize(t: *const Table) -> c_uint {
    (*t).alimit.set(luaH_realasize(t));
    (*t).flags
        .set(((*t).flags.get() as c_int & !((1 as c_int) << 7) as u8 as c_int) as u8);
    return (*t).alimit.get();
}

unsafe fn getgeneric(
    t: *const Table,
    key: *const UnsafeValue,
    deadok: c_int,
) -> *const UnsafeValue {
    let mut n: *mut Node = mainpositionTV(t, key);
    loop {
        if equalkey(key, n, deadok) != 0 {
            return &mut (*n).i_val;
        } else {
            let nx: c_int = (*n).u.next;
            if nx == 0 as c_int {
                return &raw const absentkey;
            }
            n = n.offset(nx as isize);
        }
    }
}

unsafe fn arrayindex(k: i64) -> c_uint {
    if (k as u64).wrapping_sub(1 as c_uint as u64)
        < (if ((1 as c_uint)
            << (::core::mem::size_of::<c_int>() as libc::c_ulong)
                .wrapping_mul(8 as c_int as libc::c_ulong)
                .wrapping_sub(1 as c_int as libc::c_ulong) as c_int) as usize
            <= (!(0 as c_int as usize)).wrapping_div(::core::mem::size_of::<UnsafeValue>())
        {
            (1 as c_uint)
                << (::core::mem::size_of::<c_int>() as libc::c_ulong)
                    .wrapping_mul(8 as c_int as libc::c_ulong)
                    .wrapping_sub(1 as c_int as libc::c_ulong) as c_int
        } else {
            (!(0 as c_int as usize)).wrapping_div(::core::mem::size_of::<UnsafeValue>()) as c_uint
        }) as u64
    {
        return k as c_uint;
    } else {
        return 0 as c_int as c_uint;
    };
}

unsafe fn findindex(
    L: *const Thread,
    t: *mut Table,
    key: *mut UnsafeValue,
    asize: c_uint,
) -> Result<c_uint, Box<dyn core::error::Error>> {
    let mut i: c_uint = 0;
    if (*key).tt_ as c_int & 0xf as c_int == 0 as c_int {
        return Ok(0 as c_int as c_uint);
    }
    i = if (*key).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
        arrayindex((*key).value_.i)
    } else {
        0 as c_int as c_uint
    };
    if i.wrapping_sub(1 as c_uint) < asize {
        return Ok(i);
    } else {
        let n: *const UnsafeValue = getgeneric(t, key, 1 as c_int);

        if (*n).tt_ == 0 | 2 << 4 {
            luaG_runerror(L, "invalid key to 'next'")?;
        }

        i = (n as *mut Node).offset_from(((*t).node.get()).offset(0 as c_int as isize) as *mut Node)
            as libc::c_long as c_int as c_uint;
        return Ok(i.wrapping_add(1 as c_int as c_uint).wrapping_add(asize));
    };
}

pub unsafe fn luaH_next(
    L: *const Thread,
    t: *mut Table,
    key: StkId,
) -> Result<c_int, Box<dyn core::error::Error>> {
    let asize: c_uint = luaH_realasize(t);
    let mut i: c_uint = findindex(L, t, &mut (*key).val, asize)?;
    while i < asize {
        if !((*(*t).array.get().offset(i as isize)).tt_ as c_int & 0xf as c_int == 0 as c_int) {
            let io: *mut UnsafeValue = &mut (*key).val;
            (*io).value_.i = i.wrapping_add(1 as c_int as c_uint) as i64;
            (*io).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
            let io1: *mut UnsafeValue = &mut (*key.offset(1 as c_int as isize)).val;
            let io2: *const UnsafeValue = ((*t).array.get()).offset(i as isize) as *mut UnsafeValue;
            (*io1).value_ = (*io2).value_;
            (*io1).tt_ = (*io2).tt_;
            return Ok(1 as c_int);
        }
        i = i.wrapping_add(1);
    }
    i = i.wrapping_sub(asize);
    while (i as c_int) < (1 as c_int) << (*t).lsizenode.get() as c_int {
        if !((*((*t).node.get()).offset(i as isize)).i_val.tt_ as c_int & 0xf as c_int
            == 0 as c_int)
        {
            let n: *mut Node = ((*t).node.get()).offset(i as isize) as *mut Node;
            let io_: *mut UnsafeValue = &mut (*key).val;
            let n_: *const Node = n;
            (*io_).value_ = (*n_).u.key_val;
            (*io_).tt_ = (*n_).u.key_tt;
            let io1_0: *mut UnsafeValue = &mut (*key.offset(1 as c_int as isize)).val;
            let io2_0: *const UnsafeValue = &mut (*n).i_val;
            (*io1_0).value_ = (*io2_0).value_;
            (*io1_0).tt_ = (*io2_0).tt_;
            return Ok(1 as c_int);
        }
        i = i.wrapping_add(1);
    }
    return Ok(0 as c_int);
}

unsafe fn freehash(t: *const Table) {
    if !((*t).lastfree.get()).is_null() {
        luaM_free_(
            (*t).hdr.global,
            (*t).node.get() as *mut libc::c_void,
            (((1 as c_int) << (*t).lsizenode.get() as c_int) as usize)
                .wrapping_mul(size_of::<Node>()),
        );
    }
}

unsafe fn computesizes(nums: *mut c_uint, pna: *mut c_uint) -> c_uint {
    let mut i: c_int = 0;
    let mut twotoi: c_uint = 0;
    let mut a: c_uint = 0 as c_int as c_uint;
    let mut na: c_uint = 0 as c_int as c_uint;
    let mut optimal: c_uint = 0 as c_int as c_uint;
    i = 0 as c_int;
    twotoi = 1 as c_int as c_uint;
    while twotoi > 0 as c_int as c_uint && *pna > twotoi.wrapping_div(2 as c_int as c_uint) {
        a = a.wrapping_add(*nums.offset(i as isize));
        if a > twotoi.wrapping_div(2 as c_int as c_uint) {
            optimal = twotoi;
            na = a;
        }
        i += 1;
        twotoi = twotoi.wrapping_mul(2 as c_int as c_uint);
    }
    *pna = na;
    return optimal;
}

unsafe fn countint(key: i64, nums: *mut c_uint) -> c_int {
    let k: c_uint = arrayindex(key);
    if k != 0 as c_int as c_uint {
        let ref mut fresh0 = *nums.offset(luaO_ceillog2(k) as isize);
        *fresh0 = (*fresh0).wrapping_add(1);
        return 1 as c_int;
    } else {
        return 0 as c_int;
    };
}

unsafe fn numusearray(t: *const Table, nums: *mut c_uint) -> c_uint {
    let mut lg: c_int = 0;
    let mut ttlg: c_uint = 0;
    let mut ause: c_uint = 0 as c_int as c_uint;
    let mut i: c_uint = 1 as c_int as c_uint;
    let asize: c_uint = (*t).alimit.get();
    lg = 0 as c_int;
    ttlg = 1 as c_int as c_uint;
    while lg
        <= (::core::mem::size_of::<c_int>() as libc::c_ulong)
            .wrapping_mul(8 as c_int as libc::c_ulong)
            .wrapping_sub(1 as c_int as libc::c_ulong) as c_int
    {
        let mut lc: c_uint = 0 as c_int as c_uint;
        let mut lim: c_uint = ttlg;
        if lim > asize {
            lim = asize;
            if i > lim {
                break;
            }
        }
        while i <= lim {
            if !((*((*t).array.get()).offset(i.wrapping_sub(1 as c_int as c_uint) as isize)).tt_
                as c_int
                & 0xf as c_int
                == 0 as c_int)
            {
                lc = lc.wrapping_add(1);
            }
            i = i.wrapping_add(1);
        }
        let ref mut fresh1 = *nums.offset(lg as isize);
        *fresh1 = (*fresh1).wrapping_add(lc);
        ause = ause.wrapping_add(lc);
        lg += 1;
        ttlg = ttlg.wrapping_mul(2 as c_int as c_uint);
    }
    return ause;
}

unsafe fn numusehash(t: *const Table, nums: *mut c_uint, pna: *mut c_uint) -> c_int {
    let mut totaluse: c_int = 0 as c_int;
    let mut ause: c_int = 0 as c_int;
    let mut i: c_int = (1 as c_int) << (*t).lsizenode.get() as c_int;
    loop {
        let fresh2 = i;
        i = i - 1;
        if !(fresh2 != 0) {
            break;
        }
        let n: *mut Node = ((*t).node.get()).offset(i as isize) as *mut Node;
        if !((*n).i_val.tt_ as c_int & 0xf as c_int == 0 as c_int) {
            if (*n).u.key_tt as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                ause += countint((*n).u.key_val.i, nums);
            }
            totaluse += 1;
        }
    }
    *pna = (*pna).wrapping_add(ause as c_uint);
    return totaluse;
}

unsafe fn setnodevector(t: *const Table, mut size: c_uint) {
    if size == 0 as c_int as c_uint {
        (*t).node.set(&raw mut dummynode_);
        (*t).lsizenode.set(0 as c_int as u8);
        (*t).lastfree.set(0 as *mut Node);
    } else {
        let mut i: c_int = 0;
        let lsize: c_int = luaO_ceillog2(size);
        if lsize
            > (::core::mem::size_of::<c_int>() as libc::c_ulong)
                .wrapping_mul(8 as c_int as libc::c_ulong)
                .wrapping_sub(1 as c_int as libc::c_ulong) as c_int
                - 1 as c_int
            || (1 as c_uint) << lsize
                > (if ((1 as c_uint)
                    << (::core::mem::size_of::<c_int>() as libc::c_ulong)
                        .wrapping_mul(8 as c_int as libc::c_ulong)
                        .wrapping_sub(1 as c_int as libc::c_ulong) as c_int
                        - 1 as c_int) as usize
                    <= (!(0 as c_int as usize)).wrapping_div(::core::mem::size_of::<Node>())
                {
                    (1 as c_uint)
                        << (::core::mem::size_of::<c_int>() as libc::c_ulong)
                            .wrapping_mul(8 as c_int as libc::c_ulong)
                            .wrapping_sub(1 as c_int as libc::c_ulong)
                            as c_int
                            - 1 as c_int
                } else {
                    (!(0 as c_int as usize)).wrapping_div(::core::mem::size_of::<Node>()) as c_uint
                })
        {
            panic!("table overflow");
        }

        size = ((1 as c_int) << lsize) as c_uint;
        (*t).node
            .set(luaM_malloc_((*t).hdr.global, (size as usize) * size_of::<Node>()) as *mut Node);

        i = 0 as c_int;
        while i < size as c_int {
            let n: *mut Node = ((*t).node.get()).offset(i as isize) as *mut Node;
            (*n).u.next = 0 as c_int;
            (*n).u.key_tt = 0 as c_int as u8;
            (*n).i_val.tt_ = (0 as c_int | (1 as c_int) << 4 as c_int) as u8;
            i += 1;
        }
        (*t).lsizenode.set(lsize as u8);
        (*t).lastfree
            .set((*t).node.get().offset(size as isize) as *mut Node);
    }
}

unsafe fn reinsert(ot: *const Table, t: *const Table) {
    let mut j: c_int = 0;
    let size: c_int = (1 as c_int) << (*ot).lsizenode.get() as c_int;
    j = 0 as c_int;

    while j < size {
        let old: *mut Node = ((*ot).node.get()).offset(j as isize) as *mut Node;
        if !((*old).i_val.tt_ as c_int & 0xf as c_int == 0 as c_int) {
            let mut k: UnsafeValue = UnsafeValue {
                value_: UntaggedValue {
                    gc: 0 as *mut Object,
                },
                tt_: 0,
            };
            let io_: *mut UnsafeValue = &mut k;
            let n_: *const Node = old;
            (*io_).value_ = (*n_).u.key_val;
            (*io_).tt_ = (*n_).u.key_tt;

            // Key already valid so this should never fails.
            luaH_set(t, &raw const k, &raw const (*old).i_val).unwrap();
        }
        j += 1;
    }
}

unsafe fn exchangehashpart(t1: *const Table, t2: *mut Table) {
    let lsizenode: u8 = (*t1).lsizenode.get();
    let node: *mut Node = (*t1).node.get();
    let lastfree: *mut Node = (*t1).lastfree.get();
    (*t1).lsizenode.set((*t2).lsizenode.get());
    (*t1).node.set((*t2).node.get());
    (*t1).lastfree.set((*t2).lastfree.get());
    (*t2).lsizenode.set(lsizenode);
    (*t2).node.set(node);
    (*t2).lastfree.set(lastfree);
}

pub unsafe fn luaH_resize(t: *const Table, newasize: c_uint, nhsize: c_uint) {
    let mut i: c_uint = 0;
    let mut newt = Table {
        hdr: Object::default(),
        flags: Cell::new(0),
        lsizenode: Cell::new(0),
        alimit: Cell::new(0),
        array: Cell::new(0 as *mut UnsafeValue),
        node: Cell::new(0 as *mut Node),
        lastfree: Cell::new(0 as *mut Node),
        metatable: Cell::new(0 as *mut Table),
    };
    let oldasize: c_uint = setlimittosize(t);
    let mut newarray: *mut UnsafeValue = 0 as *mut UnsafeValue;

    newt.hdr.global = (*t).hdr.global;

    setnodevector(&raw const newt, nhsize);

    if newasize < oldasize {
        (*t).alimit.set(newasize);
        exchangehashpart(t, &mut newt);
        i = newasize;
        while i < oldasize {
            if !((*((*t).array.get()).offset(i as isize)).tt_ as c_int & 0xf as c_int == 0 as c_int)
            {
                luaH_setint(
                    t,
                    i.wrapping_add(1 as c_int as c_uint) as i64,
                    ((*t).array.get()).offset(i as isize),
                );
            }
            i = i.wrapping_add(1);
        }
        (*t).alimit.set(oldasize);
        exchangehashpart(t, &mut newt);
    }

    newarray = luaM_realloc_(
        (*t).hdr.global,
        (*t).array.get() as *mut libc::c_void,
        (oldasize as usize).wrapping_mul(::core::mem::size_of::<UnsafeValue>()),
        (newasize as usize).wrapping_mul(::core::mem::size_of::<UnsafeValue>()),
    ) as *mut UnsafeValue;

    if ((newarray.is_null() && newasize > 0 as c_int as c_uint) as c_int != 0 as c_int) as c_int
        as libc::c_long
        != 0
    {
        freehash(&raw const newt);
        todo!("invoke handle_alloc_error");
    }

    exchangehashpart(t, &raw mut newt);
    (*t).array.set(newarray);
    (*t).alimit.set(newasize);
    i = oldasize;

    while i < newasize {
        (*((*t).array.get()).offset(i as isize)).tt_ =
            (0 as c_int | (1 as c_int) << 4 as c_int) as u8;
        i = i.wrapping_add(1);
    }

    reinsert(&raw const newt, t);
    freehash(&raw const newt);
}

pub unsafe fn luaH_resizearray(t: *const Table, nasize: c_uint) {
    let nsize: c_int = if ((*t).lastfree.get()).is_null() {
        0 as c_int
    } else {
        (1 as c_int) << (*t).lsizenode.get() as c_int
    };

    luaH_resize(t, nasize, nsize as c_uint)
}

unsafe fn rehash(t: *const Table, ek: *const UnsafeValue) {
    let mut asize: c_uint = 0;
    let mut na: c_uint = 0;
    let mut nums: [c_uint; 32] = [0; 32];
    let mut i: c_int = 0;
    let mut totaluse: c_int = 0;
    i = 0 as c_int;
    while i
        <= (::core::mem::size_of::<c_int>() as libc::c_ulong)
            .wrapping_mul(8 as c_int as libc::c_ulong)
            .wrapping_sub(1 as c_int as libc::c_ulong) as c_int
    {
        nums[i as usize] = 0 as c_int as c_uint;
        i += 1;
    }
    setlimittosize(t);
    na = numusearray(t, nums.as_mut_ptr());
    totaluse = na as c_int;
    totaluse += numusehash(t, nums.as_mut_ptr(), &mut na);
    if (*ek).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
        na = na.wrapping_add(countint((*ek).value_.i, nums.as_mut_ptr()) as c_uint);
    }
    totaluse += 1;
    asize = computesizes(nums.as_mut_ptr(), &mut na);
    luaH_resize(t, asize, (totaluse as c_uint).wrapping_sub(na))
}

pub unsafe fn luaH_free(t: *mut Table) {
    let g = (*t).hdr.global;
    let layout = Layout::new::<Table>();

    freehash(t);
    luaM_free_(
        g,
        (*t).array.get() as *mut libc::c_void,
        (luaH_realasize(t) as usize).wrapping_mul(size_of::<UnsafeValue>()),
    );

    (*g).gc.dealloc(t.cast(), layout);
}

unsafe fn getfreepos(t: *const Table) -> *mut Node {
    if !((*t).lastfree.get()).is_null() {
        while (*t).lastfree.get() > (*t).node.get() {
            (*t).lastfree.set(((*t).lastfree.get()).offset(-1));

            if (*(*t).lastfree.get()).u.key_tt as c_int == 0 as c_int {
                return (*t).lastfree.get();
            }
        }
    }
    return 0 as *mut Node;
}

unsafe fn luaH_newkey(
    t: *const Table,
    mut key: *const UnsafeValue,
    value: *const UnsafeValue,
) -> Result<(), TableError> {
    let mut mp: *mut Node = 0 as *mut Node;
    let mut aux: UnsafeValue = UnsafeValue {
        value_: UntaggedValue {
            gc: 0 as *mut Object,
        },
        tt_: 0,
    };

    if (*key).tt_ & 0xf == 0 {
        return Err(TableError::NilKey);
    } else if (*key).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 {
        let f: f64 = (*key).value_.n;
        let mut k: i64 = 0;

        if luaV_flttointeger(f, &mut k, F2Ieq) != 0 {
            let io: *mut UnsafeValue = &raw mut aux;
            (*io).value_.i = k;
            (*io).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
            key = &mut aux;
        } else if !(f == f) {
            return Err(TableError::NanKey);
        }
    }

    if (*value).tt_ as c_int & 0xf as c_int == 0 as c_int {
        return Ok(());
    }

    mp = mainpositionTV(t, key);

    if !((*mp).i_val.tt_ as c_int & 0xf as c_int == 0 as c_int) || ((*t).lastfree.get()).is_null() {
        let mut othern: *mut Node = 0 as *mut Node;
        let f_0: *mut Node = getfreepos(t);
        if f_0.is_null() {
            rehash(t, key);
            luaH_set(t, key, value)?;
            return Ok(());
        }
        othern = mainpositionfromnode(t, mp);
        if othern != mp {
            while othern.offset((*othern).u.next as isize) != mp {
                othern = othern.offset((*othern).u.next as isize);
            }
            (*othern).u.next = f_0.offset_from(othern) as libc::c_long as c_int;
            *f_0 = *mp;
            if (*mp).u.next != 0 as c_int {
                (*f_0).u.next += mp.offset_from(f_0) as libc::c_long as c_int;
                (*mp).u.next = 0 as c_int;
            }
            (*mp).i_val.tt_ = (0 as c_int | (1 as c_int) << 4 as c_int) as u8;
        } else {
            if (*mp).u.next != 0 as c_int {
                (*f_0).u.next =
                    mp.offset((*mp).u.next as isize).offset_from(f_0) as libc::c_long as c_int;
            }
            (*mp).u.next = f_0.offset_from(mp) as libc::c_long as c_int;
            mp = f_0;
        }
    }
    let n_: *mut Node = mp;
    let io_: *const UnsafeValue = key;
    (*n_).u.key_val = (*io_).value_;
    (*n_).u.key_tt = (*io_).tt_;

    if (*key).tt_ as c_int & (1 as c_int) << 6 as c_int != 0 {
        if (*t).hdr.marked.get() as c_int & (1 as c_int) << 5 as c_int != 0
            && (*(*key).value_.gc).marked.get() as c_int
                & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                != 0
        {
            luaC_barrierback_(t.cast());
        }
    }

    let io1: *mut UnsafeValue = &raw mut (*mp).i_val;
    let io2: *const UnsafeValue = value;
    (*io1).value_ = (*io2).value_;
    (*io1).tt_ = (*io2).tt_;
    Ok(())
}

pub unsafe fn luaH_getint(t: *const Table, key: i64) -> *const UnsafeValue {
    let alimit: u64 = (*t).alimit.get() as u64;
    if (key as u64).wrapping_sub(1 as c_uint as u64) < alimit {
        return ((*t).array.get()).offset((key - 1) as isize) as *mut UnsafeValue;
    } else if (*t).flags.get() as c_int & (1 as c_int) << 7 as c_int != 0
        && (key as u64).wrapping_sub(1 as c_uint as u64) & !alimit.wrapping_sub(1 as c_uint as u64)
            < alimit
    {
        (*t).alimit.set(key as c_uint);
        return ((*t).array.get()).offset((key - 1 as c_int as i64) as isize) as *mut UnsafeValue;
    } else {
        let mut n: *mut Node = hashint(t, key);
        loop {
            if (*n).u.key_tt as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                && (*n).u.key_val.i == key
            {
                return &mut (*n).i_val;
            } else {
                let nx: c_int = (*n).u.next;
                if nx == 0 as c_int {
                    break;
                }
                n = n.offset(nx as isize);
            }
        }
        return &raw const absentkey;
    };
}

pub unsafe fn luaH_getshortstr(t: *const Table, key: *const Str) -> *const UnsafeValue {
    let mut n =
        ((*t).node.get()).offset(((*key).hash.get() & ((1 << (*t).lsizenode.get()) - 1)) as isize);

    loop {
        if (*n).u.key_tt as c_int == 4 | 0 << 4 | 1 << 6 && (*n).u.key_val.gc.cast() == key {
            return &mut (*n).i_val;
        } else {
            let nx: c_int = (*n).u.next;
            if nx == 0 as c_int {
                return &raw const absentkey;
            }
            n = n.offset(nx as isize);
        }
    }
}

pub unsafe fn luaH_getstr(t: *const Table, key: *const Str) -> *const UnsafeValue {
    if (*key).hdr.tt as c_int == 4 as c_int | (0 as c_int) << 4 as c_int {
        return luaH_getshortstr(t, key);
    } else {
        let mut ko: UnsafeValue = UnsafeValue {
            value_: UntaggedValue {
                gc: 0 as *mut Object,
            },
            tt_: 0,
        };
        let io: *mut UnsafeValue = &mut ko;

        (*io).value_.gc = key.cast();
        (*io).tt_ = ((*key).hdr.tt as c_int | (1 as c_int) << 6 as c_int) as u8;

        return getgeneric(t, &mut ko, 0 as c_int);
    };
}

pub unsafe fn luaH_get(t: *const Table, key: *const UnsafeValue) -> *const UnsafeValue {
    match (*key).tt_ as c_int & 0x3f as c_int {
        4 => return luaH_getshortstr(t, (*key).value_.gc as *mut Str),
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
    return getgeneric(t, key, 0 as c_int);
}

pub unsafe fn luaH_finishset(
    t: *const Table,
    key: *const UnsafeValue,
    slot: *const UnsafeValue,
    value: *const UnsafeValue,
) -> Result<(), TableError> {
    if (*slot).tt_ as c_int == 0 as c_int | (2 as c_int) << 4 as c_int {
        luaH_newkey(t, key, value)?;
    } else {
        let io1: *mut UnsafeValue = slot as *mut UnsafeValue;
        let io2: *const UnsafeValue = value;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
    };
    Ok(())
}

pub unsafe fn luaH_set(
    t: *const Table,
    key: *const UnsafeValue,
    value: *const UnsafeValue,
) -> Result<(), TableError> {
    let slot: *const UnsafeValue = luaH_get(t, key);

    luaH_finishset(t, key, slot, value)
}

pub unsafe fn luaH_setint(t: *const Table, key: i64, value: *const UnsafeValue) {
    let p: *const UnsafeValue = luaH_getint(t, key);

    if (*p).tt_ as c_int == 0 as c_int | (2 as c_int) << 4 as c_int {
        let mut k: UnsafeValue = UnsafeValue {
            value_: UntaggedValue {
                gc: 0 as *mut Object,
            },
            tt_: 0,
        };
        let io: *mut UnsafeValue = &raw mut k;
        (*io).value_.i = key;
        (*io).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
        luaH_newkey(t, &raw const k, value).unwrap(); // Integer key never fails.
    } else {
        let io1: *mut UnsafeValue = p as *mut UnsafeValue;
        let io2: *const UnsafeValue = value;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
    }
}

unsafe fn hash_search(t: *mut Table, mut j: u64) -> u64 {
    let mut i: u64 = 0;
    if j == 0 as c_int as u64 {
        j = j.wrapping_add(1);
    }
    loop {
        i = j;
        if j <= 0x7fffffffffffffff as c_longlong as u64 / 2 as c_int as u64 {
            j = j * 2 as c_int as u64;
            if (*luaH_getint(t, j as i64)).tt_ as c_int & 0xf as c_int == 0 as c_int {
                break;
            }
        } else {
            j = 0x7fffffffffffffff as c_longlong as u64;
            if (*luaH_getint(t, j as i64)).tt_ as c_int & 0xf as c_int == 0 as c_int {
                break;
            }
            return j;
        }
    }
    while j.wrapping_sub(i) > 1 as c_uint as u64 {
        let m: u64 = i.wrapping_add(j) / 2 as c_int as u64;
        if (*luaH_getint(t, m as i64)).tt_ as c_int & 0xf as c_int == 0 as c_int {
            j = m;
        } else {
            i = m;
        }
    }
    return i;
}

unsafe fn binsearch(array: *const UnsafeValue, mut i: c_uint, mut j: c_uint) -> c_uint {
    while j.wrapping_sub(i) > 1 as c_uint {
        let m: c_uint = i.wrapping_add(j).wrapping_div(2 as c_int as c_uint);
        if (*array.offset(m.wrapping_sub(1 as c_int as c_uint) as isize)).tt_ as c_int
            & 0xf as c_int
            == 0 as c_int
        {
            j = m;
        } else {
            i = m;
        }
    }
    return i;
}

pub unsafe fn luaH_getn(t: *mut Table) -> u64 {
    let mut limit: c_uint = (*t).alimit.get();
    if limit > 0 as c_int as c_uint
        && (*((*t).array.get()).offset(limit.wrapping_sub(1 as c_int as c_uint) as isize)).tt_
            as c_int
            & 0xf as c_int
            == 0 as c_int
    {
        if limit >= 2 as c_int as c_uint
            && !((*((*t).array.get()).offset(limit.wrapping_sub(2 as c_int as c_uint) as isize)).tt_
                as c_int
                & 0xf as c_int
                == 0 as c_int)
        {
            if ispow2realasize(t) != 0
                && !(limit.wrapping_sub(1 as c_int as c_uint)
                    & limit
                        .wrapping_sub(1 as c_int as c_uint)
                        .wrapping_sub(1 as c_int as c_uint)
                    == 0 as c_int as c_uint)
            {
                (*t).alimit.set(limit.wrapping_sub(1 as c_int as c_uint));
                (*t).flags
                    .set(((*t).flags.get() as c_int | (1 as c_int) << 7) as u8);
            }
            return limit.wrapping_sub(1 as c_int as c_uint) as u64;
        } else {
            let boundary: c_uint = binsearch((*t).array.get(), 0 as c_int as c_uint, limit);
            if ispow2realasize(t) != 0
                && boundary > (luaH_realasize(t)).wrapping_div(2 as c_int as c_uint)
            {
                (*t).alimit.set(boundary);
                (*t).flags
                    .set(((*t).flags.get() as c_int | (1 as c_int) << 7) as u8);
            }
            return boundary as u64;
        }
    }
    if !((*t).flags.get() as c_int & (1 as c_int) << 7 as c_int == 0
        || (*t).alimit.get() & (*t).alimit.get().wrapping_sub(1 as c_int as c_uint)
            == 0 as c_int as c_uint)
    {
        if (*((*t).array.get()).offset(limit as isize)).tt_ as c_int & 0xf as c_int == 0 as c_int {
            return limit as u64;
        }
        limit = luaH_realasize(t);
        if (*((*t).array.get()).offset(limit.wrapping_sub(1 as c_int as c_uint) as isize)).tt_
            as c_int
            & 0xf as c_int
            == 0 as c_int
        {
            let boundary_0: c_uint = binsearch((*t).array.get(), (*t).alimit.get(), limit);
            (*t).alimit.set(boundary_0);
            return boundary_0 as u64;
        }
    }
    if ((*t).lastfree.get()).is_null()
        || (*luaH_getint(t, limit.wrapping_add(1 as c_int as c_uint) as i64)).tt_ as c_int
            & 0xf as c_int
            == 0 as c_int
    {
        return limit as u64;
    } else {
        return hash_search(t, limit as u64);
    };
}
