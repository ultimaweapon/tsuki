#![allow(non_camel_case_types, non_snake_case, unused_assignments)]
#![allow(unsafe_op_in_unsafe_fn)]

use super::RustId;
use crate::gc::Object;
use crate::hasher::LuaHasher;
use crate::lmem::luaM_realloc_;
use crate::lobject::luaO_ceillog2;
use crate::lstring::{luaS_eqlngstr, luaS_hashlongstr};
use crate::value::{UnsafeValue, UntaggedValue};
use crate::vm::{F2Ieq, luaV_flttointeger};
use crate::{Float, Node, Str, Table, TableError};
use alloc::alloc::handle_alloc_error;
use alloc::boxed::Box;
use core::alloc::Layout;
use core::any::TypeId;
use core::cell::Cell;
use core::ffi::c_void;
use core::hash::{Hash, Hasher};
use core::ptr::{null, null_mut};
use libm::frexp;

type c_int = i32;
type c_uint = u32;
type c_long = i64;
type c_ulong = u64;
type c_longlong = i64;
type c_double = f64;

unsafe fn hashint<D>(t: *const Table<D>, i: i64) -> *mut Node<D> {
    let ui: u64 = i as u64;
    if ui <= 2147483647 as c_int as c_uint as u64 {
        return ((*t).node.get()).offset(
            (ui as c_int
                % (((1 as c_int) << (*t).lsizenode.get() as c_int) - 1 as c_int | 1 as c_int))
                as isize,
        ) as *mut Node<D>;
    } else {
        return ((*t).node.get()).offset(
            (ui % (((1 as c_int) << (*t).lsizenode.get() as c_int) - 1 as c_int | 1 as c_int)
                as u64) as isize,
        ) as *mut Node<D>;
    };
}

unsafe fn l_hashfloat(n: Float) -> c_int {
    let mut ni: i64 = 0;
    let (mut n, i) = frexp(n.into());

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

unsafe fn mainpositionTV<D>(t: *const Table<D>, key: *const UnsafeValue<D>) -> *mut Node<D> {
    match (*key).tt_ & 0x3f {
        3 => {
            let i: i64 = (*key).value_.i;
            return hashint(t, i);
        }
        19 => {
            let n = (*key).value_.n;

            return ((*t).node.get()).offset(
                (l_hashfloat(n)
                    % (((1 as c_int) << (*t).lsizenode.get() as c_int) - 1 as c_int | 1 as c_int))
                    as isize,
            ) as *mut Node<D>;
        }
        4 => {
            let ts = (*key).value_.gc.cast::<Str<D>>();
            let h = match (*ts).is_short() {
                true => (*ts).hash.get(),
                false => luaS_hashlongstr(ts),
            };

            (*t).node
                .get()
                .add((h & (((1 as c_int) << (*t).lsizenode.get()) - 1) as c_uint) as usize)
        }
        1 => {
            return ((*t).node.get()).offset(
                (0 as c_int & ((1 as c_int) << (*t).lsizenode.get() as c_int) - 1 as c_int)
                    as isize,
            ) as *mut Node<D>;
        }
        17 => {
            return ((*t).node.get()).offset(
                (1 as c_int & ((1 as c_int) << (*t).lsizenode.get() as c_int) - 1 as c_int)
                    as isize,
            ) as *mut Node<D>;
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
        18 | 50 => todo!(),
        34 => {
            let f = (*key).value_.a;

            (*t).node
                .get()
                .offset((((f as usize) & 0xffffffff) as c_uint).wrapping_rem(
                    (((1 as c_int) << (*t).lsizenode.get()) - 1 as c_int | 1 as c_int) as c_uint,
                ) as isize)
        }
        14 => {
            // Get hash.
            let o = (*key).value_.gc.cast::<RustId<D>>();
            let mut h = LuaHasher::new((*t).hdr.global);

            (*o).value().hash(&mut h);

            // Lookup.
            let m = (1 << (*t).lsizenode.get()) - 1;
            let h = h.finish() & m;

            (*t).node.get().add(h as usize)
        }
        _ => {
            let o = (*key).value_.gc;
            return ((*t).node.get()).offset(
                ((o as usize & 0xffffffff as c_uint as usize) as c_uint).wrapping_rem(
                    (((1 as c_int) << (*t).lsizenode.get() as c_int) - 1 as c_int | 1 as c_int)
                        as c_uint,
                ) as isize,
            ) as *mut Node<D>;
        }
    }
}

unsafe fn mainpositionfromnode<D>(t: *const Table<D>, nd: *mut Node<D>) -> *mut Node<D> {
    let mut key = UnsafeValue::default();
    let io_ = &raw mut key;
    let n_ = nd;
    (*io_).value_ = (*n_).u.key_val;
    (*io_).tt_ = (*n_).u.key_tt;
    return mainpositionTV(t, &mut key);
}

unsafe fn equalkey<D>(k1: *const UnsafeValue<D>, n2: *const Node<D>, deadok: c_int) -> c_int {
    if (*k1).tt_ != (*n2).u.key_tt
        && !(deadok != 0 && (*n2).u.key_tt == 11 && (*k1).tt_ & 1 << 6 != 0)
    {
        return 0 as c_int;
    }

    match (*n2).u.key_tt {
        0 | 1 | 17 => 1,
        3 => ((*k1).value_.i == (*n2).u.key_val.i) as c_int,
        19 => ((*k1).value_.n == (*n2).u.key_val.n) as c_int,
        2 => core::ptr::fn_addr_eq((*k1).value_.f, (*n2).u.key_val.f) as c_int,
        18 => todo!(),
        34 => core::ptr::fn_addr_eq((*k1).value_.a, (*n2).u.key_val.a) as c_int,
        50 => todo!(),
        0x44 => {
            let n2 = (*n2).u.key_val.gc.cast::<Str<D>>();

            if (*n2).is_short() {
                ((*k1).value_.gc.cast() == n2) as c_int
            } else {
                luaS_eqlngstr((*k1).value_.gc.cast(), n2).into()
            }
        }
        _ => ((*k1).value_.gc == (*n2).u.key_val.gc) as c_int,
    }
}

pub unsafe fn luaH_realasize<D>(t: *const Table<D>) -> c_uint {
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

unsafe fn ispow2realasize<D>(t: *const Table<D>) -> c_int {
    return ((*t).flags.get() as c_int & (1 as c_int) << 7 as c_int != 0
        || (*t).alimit.get() & ((*t).alimit.get()).wrapping_sub(1 as c_int as c_uint)
            == 0 as c_int as c_uint) as c_int;
}

unsafe fn setlimittosize<D>(t: *const Table<D>) -> c_uint {
    (*t).alimit.set(luaH_realasize(t));
    (*t).flags
        .set(((*t).flags.get() as c_int & !((1 as c_int) << 7) as u8 as c_int) as u8);
    return (*t).alimit.get();
}

#[inline(never)]
unsafe fn getgeneric<D>(
    t: *const Table<D>,
    key: *const UnsafeValue<D>,
    deadok: c_int,
) -> *const UnsafeValue<D> {
    let mut n = mainpositionTV(t, key);
    loop {
        if equalkey::<D>(key, n, deadok) != 0 {
            return &raw const (*n).i_val;
        } else {
            let nx: c_int = (*n).u.next;
            if nx == 0 as c_int {
                return &raw const (*t).absent_key;
            }
            n = n.offset(nx as isize);
        }
    }
}

unsafe fn arrayindex<D>(k: i64) -> c_uint {
    if (k as u64).wrapping_sub(1 as c_uint as u64)
        < (if ((1 as c_uint)
            << (::core::mem::size_of::<c_int>() as c_ulong)
                .wrapping_mul(8 as c_int as c_ulong)
                .wrapping_sub(1 as c_int as c_ulong) as c_int) as usize
            <= (!(0 as c_int as usize)).wrapping_div(::core::mem::size_of::<UnsafeValue<D>>())
        {
            (1 as c_uint)
                << (::core::mem::size_of::<c_int>() as c_ulong)
                    .wrapping_mul(8 as c_int as c_ulong)
                    .wrapping_sub(1 as c_int as c_ulong) as c_int
        } else {
            (!(0 as c_int as usize)).wrapping_div(::core::mem::size_of::<UnsafeValue<D>>())
                as c_uint
        }) as u64
    {
        return k as c_uint;
    } else {
        return 0 as c_int as c_uint;
    };
}

pub(super) unsafe fn findindex<D>(
    t: *const Table<D>,
    key: *const UnsafeValue<D>,
    asize: c_uint,
) -> Result<c_uint, Box<dyn core::error::Error>> {
    let mut i: c_uint = 0;
    if (*key).tt_ as c_int & 0xf as c_int == 0 as c_int {
        return Ok(0 as c_int as c_uint);
    }
    i = if (*key).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
        arrayindex::<D>((*key).value_.i)
    } else {
        0 as c_int as c_uint
    };
    if i.wrapping_sub(1 as c_uint) < asize {
        return Ok(i);
    } else {
        let n = getgeneric(t, key, 1 as c_int);

        if (*n).tt_ == 0 | 2 << 4 {
            return Err("invalid key to 'next'".into());
        }

        i = (n as *mut Node<D>)
            .offset_from(((*t).node.get()).offset(0 as c_int as isize) as *mut Node<D>)
            as c_long as c_int as c_uint;
        return Ok(i.wrapping_add(1 as c_int as c_uint).wrapping_add(asize));
    };
}

pub(super) unsafe fn freehash<D>(t: *const Table<D>) {
    if !((*t).lastfree.get()).is_null() {
        let nodes = (*t).node.get();
        let len = 1 << (*t).lsizenode.get();
        let layout = Layout::array::<Node<D>>(len).unwrap();

        alloc::alloc::dealloc(nodes.cast(), layout);
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

unsafe fn countint<D>(key: i64, nums: *mut c_uint) -> c_int {
    let k: c_uint = arrayindex::<D>(key);
    if k != 0 as c_int as c_uint {
        let ref mut fresh0 = *nums.offset(luaO_ceillog2(k) as isize);
        *fresh0 = (*fresh0).wrapping_add(1);
        return 1 as c_int;
    } else {
        return 0 as c_int;
    };
}

unsafe fn numusearray<D>(t: *const Table<D>, nums: *mut c_uint) -> c_uint {
    let mut lg: c_int = 0;
    let mut ttlg: c_uint = 0;
    let mut ause: c_uint = 0 as c_int as c_uint;
    let mut i: c_uint = 1 as c_int as c_uint;
    let asize: c_uint = (*t).alimit.get();
    lg = 0 as c_int;
    ttlg = 1 as c_int as c_uint;
    while lg
        <= (::core::mem::size_of::<c_int>() as c_ulong)
            .wrapping_mul(8 as c_int as c_ulong)
            .wrapping_sub(1 as c_int as c_ulong) as c_int
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

unsafe fn numusehash<D>(t: *const Table<D>, nums: *mut c_uint, pna: *mut c_uint) -> c_int {
    let mut totaluse: c_int = 0 as c_int;
    let mut ause: c_int = 0 as c_int;
    let mut i: c_int = (1 as c_int) << (*t).lsizenode.get() as c_int;
    loop {
        let fresh2 = i;
        i = i - 1;
        if !(fresh2 != 0) {
            break;
        }
        let n = ((*t).node.get()).offset(i as isize) as *mut Node<D>;
        if !((*n).i_val.tt_ as c_int & 0xf as c_int == 0 as c_int) {
            if (*n).u.key_tt as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                ause += countint::<D>((*n).u.key_val.i, nums);
            }
            totaluse += 1;
        }
    }
    *pna = (*pna).wrapping_add(ause as c_uint);
    return totaluse;
}

unsafe fn setnodevector<D>(t: *const Table<D>, mut size: c_uint) {
    if size == 0 as c_int as c_uint {
        (*t).node.set(&raw const (*t).hdr.global().dummy_node as _);
        (*t).lsizenode.set(0 as c_int as u8);
        (*t).lastfree.set(null_mut());
    } else {
        let mut i: c_int = 0;
        let lsize: c_int = luaO_ceillog2(size);
        if lsize
            > (::core::mem::size_of::<c_int>() as c_ulong)
                .wrapping_mul(8 as c_int as c_ulong)
                .wrapping_sub(1 as c_int as c_ulong) as c_int
                - 1 as c_int
            || (1 as c_uint) << lsize
                > (if ((1 as c_uint)
                    << (::core::mem::size_of::<c_int>() as c_ulong)
                        .wrapping_mul(8 as c_int as c_ulong)
                        .wrapping_sub(1 as c_int as c_ulong) as c_int
                        - 1 as c_int) as usize
                    <= (!(0 as c_int as usize)).wrapping_div(::core::mem::size_of::<Node<D>>())
                {
                    (1 as c_uint)
                        << (::core::mem::size_of::<c_int>() as c_ulong)
                            .wrapping_mul(8 as c_int as c_ulong)
                            .wrapping_sub(1 as c_int as c_ulong) as c_int
                            - 1 as c_int
                } else {
                    (!(0 as c_int as usize)).wrapping_div(::core::mem::size_of::<Node<D>>())
                        as c_uint
                })
        {
            panic!("table overflow");
        }

        size = ((1 as c_int) << lsize) as c_uint;

        // Allocate nodes.
        let layout = Layout::array::<Node<D>>(size as usize).unwrap();
        let nodes = alloc::alloc::alloc(layout);

        if nodes.is_null() {
            handle_alloc_error(layout);
        }

        (*t).node.set(nodes.cast());

        i = 0 as c_int;
        while i < size as c_int {
            let n = ((*t).node.get()).offset(i as isize) as *mut Node<D>;
            (*n).u.next = 0 as c_int;
            (*n).u.key_tt = 0 as c_int as u8;
            (*n).i_val.tt_ = (0 as c_int | (1 as c_int) << 4 as c_int) as u8;
            i += 1;
        }
        (*t).lsizenode.set(lsize as u8);
        (*t).lastfree
            .set((*t).node.get().offset(size as isize) as *mut Node<D>);
    }
}

unsafe fn reinsert<D>(ot: *const Table<D>, t: *const Table<D>) {
    let mut j: c_int = 0;
    let size: c_int = (1 as c_int) << (*ot).lsizenode.get() as c_int;
    j = 0 as c_int;

    while j < size {
        let old = ((*ot).node.get()).offset(j as isize) as *mut Node<D>;
        if !((*old).i_val.tt_ as c_int & 0xf as c_int == 0 as c_int) {
            let mut k = UnsafeValue::default();
            let io_ = &raw mut k;
            let n_ = old;
            (*io_).value_ = (*n_).u.key_val;
            (*io_).tt_ = (*n_).u.key_tt;

            // Key already valid so this should never fails.
            luaH_set(t, &raw const k, &raw const (*old).i_val).unwrap();
        }
        j += 1;
    }
}

unsafe fn exchangehashpart<D>(t1: *const Table<D>, t2: *mut Table<D>) {
    let lsizenode: u8 = (*t1).lsizenode.get();
    let node = (*t1).node.get();
    let lastfree = (*t1).lastfree.get();

    (*t1).lsizenode.set((*t2).lsizenode.get());
    (*t1).node.set((*t2).node.get());
    (*t1).lastfree.set((*t2).lastfree.get());
    (*t2).lsizenode.set(lsizenode);
    (*t2).node.set(node);
    (*t2).lastfree.set(lastfree);
}

pub unsafe fn luaH_resize<D>(t: *const Table<D>, newasize: c_uint, nhsize: c_uint) {
    let mut i: c_uint = 0;
    let mut newt = Table {
        hdr: Object::default(),
        flags: Cell::new(0),
        lsizenode: Cell::new(0),
        alimit: Cell::new(0),
        array: Cell::new(null_mut()),
        node: Cell::new(null_mut()),
        lastfree: Cell::new(null_mut()),
        metatable: Cell::new(null()),
        absent_key: UnsafeValue::default(),
    };
    let oldasize: c_uint = setlimittosize(t);
    let mut newarray = null_mut();

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
        (*t).array.get() as *mut c_void,
        (oldasize as usize).wrapping_mul(::core::mem::size_of::<UnsafeValue<D>>()),
        (newasize as usize).wrapping_mul(::core::mem::size_of::<UnsafeValue<D>>()),
    ) as *mut UnsafeValue<D>;

    if ((newarray.is_null() && newasize > 0 as c_int as c_uint) as c_int != 0 as c_int) as c_int
        as c_long
        != 0
    {
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
}

pub unsafe fn luaH_resizearray<D>(t: *const Table<D>, nasize: c_uint) {
    let nsize: c_int = if ((*t).lastfree.get()).is_null() {
        0 as c_int
    } else {
        (1 as c_int) << (*t).lsizenode.get() as c_int
    };

    luaH_resize(t, nasize, nsize as c_uint)
}

unsafe fn rehash<D>(t: *const Table<D>, ek: *const UnsafeValue<D>) {
    let mut asize: c_uint = 0;
    let mut na: c_uint = 0;
    let mut nums: [c_uint; 32] = [0; 32];
    let mut i: c_int = 0;
    let mut totaluse: c_int = 0;
    i = 0 as c_int;
    while i
        <= (::core::mem::size_of::<c_int>() as c_ulong)
            .wrapping_mul(8 as c_int as c_ulong)
            .wrapping_sub(1 as c_int as c_ulong) as c_int
    {
        nums[i as usize] = 0 as c_int as c_uint;
        i += 1;
    }
    setlimittosize(t);
    na = numusearray(t, nums.as_mut_ptr());
    totaluse = na as c_int;
    totaluse += numusehash(t, nums.as_mut_ptr(), &mut na);
    if (*ek).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
        na = na.wrapping_add(countint::<D>((*ek).value_.i, nums.as_mut_ptr()) as c_uint);
    }
    totaluse += 1;
    asize = computesizes(nums.as_mut_ptr(), &mut na);
    luaH_resize(t, asize, (totaluse as c_uint).wrapping_sub(na))
}

unsafe fn getfreepos<D>(t: *const Table<D>) -> *mut Node<D> {
    if !((*t).lastfree.get()).is_null() {
        while (*t).lastfree.get() > (*t).node.get() {
            (*t).lastfree.set(((*t).lastfree.get()).offset(-1));

            if (*(*t).lastfree.get()).u.key_tt as c_int == 0 as c_int {
                return (*t).lastfree.get();
            }
        }
    }
    return null_mut();
}

#[inline(never)]
unsafe fn luaH_newkey<D>(
    t: *const Table<D>,
    mut key: *const UnsafeValue<D>,
    value: *const UnsafeValue<D>,
) -> Result<(), TableError> {
    let mut mp = null_mut();
    let mut aux = UnsafeValue::default();

    if (*key).tt_ & 0xf == 0 {
        return Err(TableError::NilKey);
    } else if (*key).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 {
        let f = (*key).value_.n;

        match luaV_flttointeger(f, F2Ieq) {
            Some(k) => {
                aux = k.into();
                key = &raw const aux;
            }
            None => {
                if !(f == f) {
                    return Err(TableError::NanKey);
                }
            }
        }
    }

    if (*value).tt_ as c_int & 0xf as c_int == 0 as c_int {
        return Ok(());
    }

    mp = mainpositionTV(t, key);

    if !((*mp).i_val.tt_ as c_int & 0xf as c_int == 0 as c_int) || ((*t).lastfree.get()).is_null() {
        let mut othern = null_mut();
        let f_0 = getfreepos(t);
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
            (*othern).u.next = f_0.offset_from(othern) as c_long as c_int;
            *f_0 = *mp;
            if (*mp).u.next != 0 as c_int {
                (*f_0).u.next += mp.offset_from(f_0) as c_long as c_int;
                (*mp).u.next = 0 as c_int;
            }
            (*mp).i_val.tt_ = (0 as c_int | (1 as c_int) << 4 as c_int) as u8;
        } else {
            if (*mp).u.next != 0 as c_int {
                (*f_0).u.next =
                    mp.offset((*mp).u.next as isize).offset_from(f_0) as c_long as c_int;
            }
            (*mp).u.next = f_0.offset_from(mp) as c_long as c_int;
            mp = f_0;
        }
    }
    let n_ = mp;
    let io_ = key;
    (*n_).u.key_val = (*io_).value_;
    (*n_).u.key_tt = (*io_).tt_;

    if (*key).tt_ as c_int & (1 as c_int) << 6 as c_int != 0 {
        if (*t).hdr.marked.get() as c_int & (1 as c_int) << 5 as c_int != 0
            && (*(*key).value_.gc).marked.get() as c_int
                & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                != 0
        {
            (*t).hdr.global().gc.barrier_back(t.cast());
        }
    }

    let io1 = &raw mut (*mp).i_val;
    let io2 = value;
    (*io1).value_ = (*io2).value_;
    (*io1).tt_ = (*io2).tt_;
    Ok(())
}

#[inline(always)]
pub unsafe fn luaH_getint<D>(t: *const Table<D>, key: i64) -> *const UnsafeValue<D> {
    // Check if key within array part.
    let alimit = u64::from((*t).alimit.get());

    if (key as u64).wrapping_sub(1) < alimit {
        return (*t).array.get().add((key - 1) as usize);
    }

    if (*t).flags.get() & 1 << 7 != 0
        && (key as u64).wrapping_sub(1) & !alimit.wrapping_sub(1) < alimit
    {
        (*t).alimit.set(key as c_uint);
        (*t).array.get().offset((key - 1) as isize)
    } else {
        luaH_searchint(t, key)
    }
}

#[inline(never)]
unsafe fn luaH_searchint<A>(t: *const Table<A>, key: i64) -> *const UnsafeValue<A> {
    let mut n = hashint(t, key);

    loop {
        if (*n).u.key_tt == 3 | 0 << 4 && (*n).u.key_val.i == key {
            return &raw const (*n).i_val;
        } else {
            let nx = (*n).u.next;

            if nx == 0 {
                break;
            }

            n = n.offset(nx as isize);
        }
    }

    &raw const (*t).absent_key
}

#[inline(never)]
pub unsafe fn luaH_getshortstr<A>(t: *const Table<A>, key: *const Str<A>) -> *const UnsafeValue<A> {
    let mut n =
        ((*t).node.get()).offset(((*key).hash.get() & ((1 << (*t).lsizenode.get()) - 1)) as isize);

    loop {
        if (*n).u.key_tt == 4 | 0 << 4 | 1 << 6 && (*n).u.key_val.gc.cast() == key {
            return &mut (*n).i_val;
        } else {
            let nx: c_int = (*n).u.next;
            if nx == 0 as c_int {
                return &raw const (*t).absent_key;
            }
            n = n.offset(nx as isize);
        }
    }
}

#[inline(always)]
pub unsafe fn luaH_getstr<A>(t: *const Table<A>, key: *const Str<A>) -> *const UnsafeValue<A> {
    if (*key).is_short() {
        luaH_getshortstr(t, key)
    } else {
        let ko = UnsafeValue {
            tt_: 4 | 0 << 4 | 1 << 6,
            value_: UntaggedValue { gc: key.cast() },
        };

        getgeneric(t, &raw const ko, 0)
    }
}

pub unsafe fn luaH_getid<A>(t: *const Table<A>, k: &TypeId) -> *const UnsafeValue<A> {
    // Get hash.
    let mut h = LuaHasher::new((*t).hdr.global);

    k.hash(&mut h);

    // Lookup.
    let m = (1 << (*t).lsizenode.get()) - 1;
    let h = h.finish() & m;
    let mut n = (*t).node.get().add(h as usize);

    loop {
        // Check key.
        if (*n).u.key_tt == 14 | 0 << 4 | 1 << 6
            && (*(*n).u.key_val.gc.cast::<RustId<A>>()).value() == k
        {
            return &raw const (*n).i_val;
        }

        // Get next node.
        let nx = (*n).u.next;

        if nx == 0 {
            break;
        }

        n = n.offset(nx as isize);
    }

    &raw const (*t).absent_key
}

#[inline(never)]
pub unsafe fn luaH_get<D>(t: *const Table<D>, key: *const UnsafeValue<D>) -> *const UnsafeValue<D> {
    match (*key).tt_ & 0x3f {
        4 => return luaH_getstr(t, (*key).value_.gc.cast()),
        3 => return luaH_getint(t, (*key).value_.i),
        0 => return &raw const (*t).absent_key,
        19 => {
            if let Some(k) = luaV_flttointeger((*key).value_.n, F2Ieq) {
                return luaH_getint(t, k);
            }
        }
        14 => return luaH_getid(t, (*(*key).value_.gc.cast::<RustId<D>>()).value()),
        _ => {}
    }

    getgeneric(t, key, 0 as c_int)
}

#[inline(always)]
pub unsafe fn luaH_finishset<D>(
    t: *const Table<D>,
    key: *const UnsafeValue<D>,
    slot: *const UnsafeValue<D>,
    value: *const UnsafeValue<D>,
) -> Result<(), TableError> {
    if (*slot).tt_ as c_int == 0 as c_int | (2 as c_int) << 4 as c_int {
        luaH_newkey(t, key, value)?;
    } else {
        let io1 = slot as *mut UnsafeValue<D>;
        let io2 = value;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
    };
    Ok(())
}

pub unsafe fn luaH_set<D>(
    t: *const Table<D>,
    key: *const UnsafeValue<D>,
    value: *const UnsafeValue<D>,
) -> Result<(), TableError> {
    let slot = luaH_get(t, key);

    luaH_finishset(t, key, slot, value)
}

pub unsafe fn luaH_setint<D>(t: *const Table<D>, key: i64, value: *const UnsafeValue<D>) {
    let p = luaH_getint(t, key);

    if (*p).tt_ as c_int == 0 as c_int | (2 as c_int) << 4 as c_int {
        let mut k = UnsafeValue::default();
        let io = &raw mut k;
        (*io).value_.i = key;
        (*io).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
        luaH_newkey(t, &raw const k, value).unwrap(); // Integer key never fails.
    } else {
        let io1 = p as *mut UnsafeValue<D>;
        let io2 = value;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
    }
}

unsafe fn hash_search<D>(t: *const Table<D>, j: u32) -> u64 {
    let mut j = u64::from(j);
    let mut i: u64 = 0;
    if j == 0 as c_int as u64 {
        j = j.wrapping_add(1);
    }
    loop {
        i = j;
        if j <= i64::MAX as u64 / 2 {
            j = j * 2 as c_int as u64;
            if (*luaH_getint(t, j as i64)).tt_ as c_int & 0xf as c_int == 0 as c_int {
                break;
            }
        } else {
            j = i64::MAX as u64;
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

unsafe fn binsearch<D>(array: *const UnsafeValue<D>, mut i: c_uint, mut j: c_uint) -> c_uint {
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

pub unsafe fn luaH_getn<D>(t: *const Table<D>) -> u64 {
    let mut limit = (*t).alimit.get();

    if limit > 0 && (*((*t).array.get()).offset((limit - 1) as isize)).tt_ & 0xf == 0 {
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

    if !((*t).flags.get() & 1 << 7 == 0
        || (*t).alimit.get() & (*t).alimit.get().wrapping_sub(1) == 0)
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
        || (*luaH_getint(t, limit.wrapping_add(1) as i64)).tt_ & 0xf == 0
    {
        return limit as u64;
    } else {
        return hash_search(t, limit);
    };
}
