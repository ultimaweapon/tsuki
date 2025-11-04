#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments
)]
#![allow(unsafe_op_in_unsafe_fn)]

pub use self::interpreter::*;
pub use self::opcode::*;

use crate::ldebug::{luaG_forerror, luaG_typeerror};
use crate::lfunc::{luaF_findupval, luaF_newLclosure};
use crate::lobject::{Proto, UpVal, luaO_str2num};
use crate::lstate::CallInfo;
use crate::ltm::{
    TM_EQ, TM_INDEX, TM_LE, TM_LEN, TM_LT, TM_NEWINDEX, luaT_callTM, luaT_callTMres,
    luaT_callorderTM, luaT_gettm, luaT_gettmbyobj, luaT_tryconcatTM,
};
use crate::table::{luaH_finishset, luaH_get, luaH_getn};
use crate::value::UnsafeValue;
use crate::{ContentType, Float, Nil, StackValue, Str, Table, Thread, UserData};
use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::any::Any;
use core::cell::Cell;
use core::cmp::Ordering;
use core::convert::identity;
use core::ptr::null;
use libm::fmod;

pub type F2Imod = c_uint;

pub const F2Iceil: F2Imod = 2;
pub const F2Ifloor: F2Imod = 1;
pub const F2Ieq: F2Imod = 0;

type c_int = i32;
type c_uint = u32;
type c_long = i64;
type c_ulong = u64;
type c_longlong = i64;

mod interpreter;
mod opcode;

unsafe fn l_strton<D>(obj: *const UnsafeValue<D>) -> Option<UnsafeValue<D>> {
    if !((*obj).tt_ as c_int & 0xf as c_int == 4 as c_int) {
        None
    } else {
        let st = (*obj).value_.gc.cast::<Str<D>>();

        luaO_str2num((*st).as_bytes()).map(|v| v.into())
    }
}

#[inline(never)]
pub unsafe fn luaV_tonumber_<A>(obj: *const UnsafeValue<A>) -> Option<Float> {
    if (*obj).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
        Some(((*obj).value_.i as f64).into())
    } else if let Some(v) = l_strton(obj) {
        Some(if v.tt_ == 3 | 0 << 4 {
            (v.value_.i as f64).into()
        } else {
            v.value_.n
        })
    } else {
        None
    }
}

#[inline(always)]
pub fn luaV_flttointeger(n: Float, mode: F2Imod) -> Option<i64> {
    let mut f = n.floor();

    if n != f {
        if mode as c_uint == F2Ieq as c_int as c_uint {
            return None;
        } else if mode as c_uint == F2Iceil as c_int as c_uint {
            f += 1f64;
        }
    }

    match f >= i64::MIN as f64 && f < -(i64::MIN as f64) {
        true => Some(f64::from(f) as i64),
        false => None,
    }
}

#[inline(always)]
pub unsafe fn luaV_tointegerns<A>(obj: *const UnsafeValue<A>, mode: F2Imod) -> Option<i64> {
    if (*obj).tt_ == 3 | 1 << 4 {
        luaV_flttointeger((*obj).value_.n, mode)
    } else if (*obj).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
        Some((*obj).value_.i)
    } else {
        None
    }
}

#[inline(never)]
pub unsafe fn luaV_tointeger<A>(obj: *const UnsafeValue<A>, mode: F2Imod) -> Option<i64> {
    match l_strton(obj) {
        Some(v) => luaV_tointegerns(&v, mode),
        None => luaV_tointegerns(obj, mode),
    }
}

unsafe fn forlimit<D>(
    L: *const Thread<D>,
    init: i64,
    lim: *const UnsafeValue<D>,
    p: *mut i64,
    step: i64,
) -> Result<c_int, Box<dyn core::error::Error>> {
    match luaV_tointeger(lim, if step < 0 { F2Iceil } else { F2Ifloor }) {
        Some(v) => p.write(v),
        None => {
            let mut flim = Float::default();

            if if (*lim).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                flim = (*lim).value_.n;
                1 as c_int
            } else if let Some(v) = luaV_tonumber_(lim) {
                flim = v;
                1
            } else {
                0
            } == 0
            {
                luaG_forerror(L, lim, "limit")?;
            }

            if flim > 0f64 {
                if step < 0 as c_int as i64 {
                    return Ok(1 as c_int);
                }
                *p = 0x7fffffffffffffff as c_longlong;
            } else {
                if step > 0 as c_int as i64 {
                    return Ok(1 as c_int);
                }
                *p = -(0x7fffffffffffffff as c_longlong) - 1 as c_int as c_longlong;
            }
        }
    }

    return if step > 0 as c_int as i64 {
        Ok((init > *p) as c_int)
    } else {
        Ok((init < *p) as c_int)
    };
}

#[inline(never)]
unsafe fn forprep<D>(
    L: *const Thread<D>,
    ra: *mut StackValue<D>,
) -> Result<c_int, Box<dyn core::error::Error>> {
    let pinit = ra;
    let plimit = ra.offset(1 as c_int as isize);
    let pstep = ra.offset(2 as c_int as isize);

    if (*pinit).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
        && (*pstep).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
    {
        let init: i64 = (*pinit).value_.i;
        let step: i64 = (*pstep).value_.i;
        let mut limit: i64 = 0;
        if step == 0 as c_int as i64 {
            return Err("'for' step is zero".into());
        }
        let io = ra.offset(3 as c_int as isize);

        (*io).value_.i = init;
        (*io).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
        if forlimit(L, init, plimit.cast(), &mut limit, step)? != 0 {
            return Ok(1 as c_int);
        } else {
            let mut count: u64 = 0;
            if step > 0 as c_int as i64 {
                count = (limit as u64).wrapping_sub(init as u64);
                if step != 1 as c_int as i64 {
                    count = count / step as u64;
                }
            } else {
                count = (init as u64).wrapping_sub(limit as u64);
                count =
                    count / (-(step + 1 as c_int as i64) as u64).wrapping_add(1 as c_uint as u64);
            }
            let io_0 = plimit;

            (*io_0).value_.i = count as i64;
            (*io_0).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
        }
    } else {
        let mut init_0 = Float::default();
        let mut limit_0 = Float::default();
        let mut step_0 = Float::default();

        if (if (*plimit).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
            limit_0 = (*plimit).value_.n;
            1 as c_int
        } else if let Some(v) = luaV_tonumber_::<D>(plimit.cast()) {
            limit_0 = v;
            1
        } else {
            0
        }) == 0
        {
            luaG_forerror(L, plimit.cast(), "limit")?;
        }

        if (if (*pstep).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
            step_0 = (*pstep).value_.n;
            1
        } else if let Some(v) = luaV_tonumber_::<D>(pstep.cast()) {
            step_0 = v;
            1
        } else {
            0
        }) == 0
        {
            luaG_forerror(L, pstep.cast(), "step")?;
        }

        if (if (*pinit).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
            init_0 = (*pinit).value_.n;
            1
        } else if let Some(v) = luaV_tonumber_::<D>(pinit.cast()) {
            init_0 = v;
            1
        } else {
            0
        }) == 0
        {
            luaG_forerror(L, pinit.cast(), "initial value")?;
        }

        if step_0 == 0 as c_int as f64 {
            return Err("'for' step is zero".into());
        }

        if if step_0 > 0f64 {
            (limit_0 < init_0) as c_int
        } else {
            (init_0 < limit_0) as c_int
        } != 0
        {
            return Ok(1 as c_int);
        } else {
            let io_1 = plimit;
            (*io_1).value_.n = limit_0;
            (*io_1).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
            let io_2 = pstep;
            (*io_2).value_.n = step_0;
            (*io_2).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
            let io_3 = ra;
            (*io_3).value_.n = init_0;
            (*io_3).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
            let io_4 = ra.offset(3 as c_int as isize);
            (*io_4).value_.n = init_0;
            (*io_4).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
        }
    }
    return Ok(0 as c_int);
}

#[inline(always)]
unsafe fn floatforloop<D>(ra: *mut StackValue<D>) -> c_int {
    let step = (*ra.offset(2)).value_.n;
    let limit = (*ra.offset(1)).value_.n;
    let mut idx = (*ra).value_.n;

    idx = idx + step;

    if if step > 0f64 {
        idx <= limit
    } else {
        limit <= idx
    } {
        let io = ra;

        (*io).value_.n = idx;
        let io_0 = ra.offset(3 as c_int as isize);

        (*io_0).value_.n = idx;
        (*io_0).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
        return 1 as c_int;
    } else {
        return 0 as c_int;
    };
}

#[inline(never)]
pub unsafe fn luaV_finishget<A>(
    L: &Thread<A>,
    mut t: *const UnsafeValue<A>,
    key: *const UnsafeValue<A>,
    mut props_tried: bool,
) -> Result<UnsafeValue<A>, Box<dyn core::error::Error>> {
    for _ in 0..2000 {
        // Check type.
        let mt = match (*t).tt_ & 0xf {
            5 => Some((*(*t).value_.gc.cast::<Table<A>>()).metatable.get()),
            7 => 'b: {
                // Check for properties.
                let ud = (*t).value_.gc.cast::<UserData<A, dyn Any>>();
                let props = (*ud).props.get();

                if props.is_null() {
                    break 'b None;
                } else if core::mem::take(&mut props_tried) {
                    break 'b Some((*ud).mt);
                }

                // Get property.
                let v = luaH_get(props, key);

                if (*v).tt_ & 0xf != 0 {
                    return Ok(v.read());
                }

                // Return nil in case of property not found and no metatable instead of error.
                Some((*ud).mt)
            }
            _ => None,
        };

        // Get __index.
        let index = match mt {
            Some(v) => {
                let v = if v.is_null() {
                    null()
                } else if (*v).flags.get() & 1 << TM_INDEX != 0 {
                    null()
                } else {
                    luaT_gettm(v, TM_INDEX)
                };

                if v.is_null() {
                    return Ok(Nil.into());
                }

                v
            }
            None => {
                let v = luaT_gettmbyobj(L, t, TM_INDEX);

                if (*v).tt_ & 0xf == 0 {
                    return Err(luaG_typeerror(L, t, "index"));
                }

                v
            }
        };

        // Check __index type.
        match (*index).tt_ & 0xf {
            2 | 6 => {
                if let Err(e) = luaT_callTMres(L, index, t, key) {
                    return Err(e); // Requires unsized coercion.
                }

                (*L).top.sub(1);

                return Ok((*L).top.read(0));
            }
            5 => {
                let v = luaH_get((*index).value_.gc.cast(), key);

                if (*v).tt_ & 0xf != 0 {
                    return Ok(v.read());
                }
            }
            _ => (),
        }

        t = index;
    }

    Err("'__index' chain too long; possible loop".into())
}

#[inline(never)]
pub unsafe fn luaV_finishset<A>(
    L: &Thread<A>,
    mut t: *const UnsafeValue<A>,
    key: *const UnsafeValue<A>,
    val: *const UnsafeValue<A>,
    mut slot: *const UnsafeValue<A>,
) -> Result<(), Box<dyn core::error::Error>> {
    let mut loop_0: c_int = 0;
    loop_0 = 0 as c_int;
    while loop_0 < 2000 as c_int {
        let mut tm = null();

        if !slot.is_null() {
            let h = (*t).value_.gc.cast::<Table<A>>();

            tm = if ((*h).metatable.get()).is_null() {
                null()
            } else if (*(*h).metatable.get()).flags.get() as c_uint
                & (1 as c_uint) << TM_NEWINDEX as c_int
                != 0
            {
                null()
            } else {
                luaT_gettm((*h).metatable.get(), TM_NEWINDEX)
            };

            if tm.is_null() {
                (*L).top.write_table(&*h);
                (*L).top.add(1);

                if let Err(e) = luaH_finishset(h, key, slot, val) {
                    (*L).top.sub(1);
                    return Err(Box::new(e));
                }

                (*L).top.sub(1);
                (*h).flags
                    .set(((*h).flags.get() as c_uint & !!(!0 << TM_EQ + 1)) as u8);

                if (*val).tt_ & 1 << 6 != 0 {
                    if (*h).hdr.marked.get() & 1 << 5 != 0
                        && (*(*val).value_.gc).marked.get() & (1 << 3 | 1 << 4) != 0
                    {
                        (*L).hdr.global().gc.barrier_back(h.cast());
                    }
                }

                return Ok(());
            }
        } else {
            tm = luaT_gettmbyobj(L, t, TM_NEWINDEX);

            if (*tm).tt_ & 0xf == 0 {
                return Err(luaG_typeerror(L, t, "index"));
            }
        }

        if ((*tm).tt_ & 0xf) == 2 || ((*tm).tt_ & 0xf) == 6 {
            if let Err(e) = luaT_callTM(L, tm, t, key, val) {
                return Err(e); // Requires unsized coercion.
            }

            return Ok(());
        }

        t = tm;
        if if !((*t).tt_ as c_int
            == 5 as c_int | (0 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int)
        {
            slot = null();
            0 as c_int
        } else {
            slot = luaH_get((*t).value_.gc.cast(), key);
            !((*slot).tt_ as c_int & 0xf as c_int == 0 as c_int) as c_int
        } != 0
        {
            let io1 = slot.cast_mut();
            let io2 = val;

            (*io1).value_ = (*io2).value_;
            (*io1).tt_ = (*io2).tt_;
            if (*val).tt_ as c_int & (1 as c_int) << 6 as c_int != 0 {
                if (*(*t).value_.gc).marked.get() as c_int & (1 as c_int) << 5 as c_int != 0
                    && (*(*val).value_.gc).marked.get() as c_int
                        & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                        != 0
                {
                    (*L).hdr.global().gc.barrier_back((*t).value_.gc);
                }
            }
            return Ok(());
        }
        loop_0 += 1;
    }

    Err("'__newindex' chain too long; possible loop".into())
}

#[inline(always)]
unsafe fn l_strcmp<D>(ts1: *const Str<D>, ts2: *const Str<D>) -> c_int {
    let s1 = (*ts1).as_bytes();
    let s2 = (*ts2).as_bytes();

    match s1.cmp(s2) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

#[inline(always)]
unsafe fn LTintfloat(i: i64, f: Float) -> c_int {
    if ((1 as c_int as u64) << 53 as c_int).wrapping_add(i as u64)
        <= 2 as c_int as u64 * ((1 as c_int as u64) << 53 as c_int)
    {
        (f > (i as f64)) as c_int
    } else {
        match luaV_flttointeger(f, F2Iceil) {
            Some(fi) => (i < fi) as c_int,
            None => (f > 0 as c_int as f64) as c_int,
        }
    }
}

#[inline(always)]
unsafe fn LEintfloat(i: i64, f: Float) -> c_int {
    if ((1 as c_int as u64) << 53 as c_int).wrapping_add(i as u64)
        <= 2 as c_int as u64 * ((1 as c_int as u64) << 53 as c_int)
    {
        (f >= (i as f64)) as c_int
    } else {
        match luaV_flttointeger(f, F2Ifloor) {
            Some(fi) => (i <= fi) as c_int,
            None => (f > 0 as c_int as f64) as c_int,
        }
    }
}

#[inline(always)]
unsafe fn LTfloatint(f: Float, i: i64) -> c_int {
    if ((1 as c_int as u64) << 53 as c_int).wrapping_add(i as u64)
        <= 2 as c_int as u64 * ((1 as c_int as u64) << 53 as c_int)
    {
        return (f < i as f64) as c_int;
    } else {
        match luaV_flttointeger(f, F2Ifloor) {
            Some(fi) => (fi < i) as c_int,
            None => (f < 0 as c_int as f64) as c_int,
        }
    }
}

#[inline(always)]
unsafe fn LEfloatint(f: Float, i: i64) -> c_int {
    if ((1 as c_int as u64) << 53 as c_int).wrapping_add(i as u64)
        <= 2 as c_int as u64 * ((1 as c_int as u64) << 53 as c_int)
    {
        return (f <= i as f64) as c_int;
    } else {
        match luaV_flttointeger(f, F2Iceil) {
            Some(fi) => (fi <= i) as c_int,
            None => (f < 0 as c_int as f64) as c_int,
        }
    }
}

#[inline(always)]
unsafe fn LTnum<A>(l: *const UnsafeValue<A>, r: *const UnsafeValue<A>) -> c_int {
    if (*l).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
        let li: i64 = (*l).value_.i;
        if (*r).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
            return (li < (*r).value_.i) as c_int;
        } else {
            return LTintfloat(li, (*r).value_.n);
        }
    } else {
        let lf = (*l).value_.n;

        if (*r).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
            return (lf < (*r).value_.n) as c_int;
        } else {
            return LTfloatint(lf, (*r).value_.i);
        }
    };
}

#[inline(always)]
unsafe fn LEnum<D>(l: *const UnsafeValue<D>, r: *const UnsafeValue<D>) -> c_int {
    if (*l).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
        let li: i64 = (*l).value_.i;
        if (*r).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
            return (li <= (*r).value_.i) as c_int;
        } else {
            return LEintfloat(li, (*r).value_.n);
        }
    } else {
        let lf = (*l).value_.n;

        if (*r).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
            return (lf <= (*r).value_.n) as c_int;
        } else {
            return LEfloatint(lf, (*r).value_.i);
        }
    };
}

#[inline(always)]
unsafe fn lessthanothers<A>(
    L: &Thread<A>,
    l: *const UnsafeValue<A>,
    r: *const UnsafeValue<A>,
) -> Result<c_int, Box<dyn core::error::Error>> {
    if (*l).tt_ as c_int & 0xf as c_int == 4 as c_int
        && (*r).tt_ as c_int & 0xf as c_int == 4 as c_int
    {
        return Ok(
            (l_strcmp::<A>((*l).value_.gc.cast(), (*r).value_.gc.cast()) < 0 as c_int) as c_int,
        );
    } else {
        return luaT_callorderTM(L, l, r, TM_LT);
    };
}

#[inline(never)]
pub unsafe fn luaV_lessthan<A>(
    L: &Thread<A>,
    l: *const UnsafeValue<A>,
    r: *const UnsafeValue<A>,
) -> Result<c_int, Box<dyn core::error::Error>> {
    if (*l).tt_ as c_int & 0xf as c_int == 3 as c_int
        && (*r).tt_ as c_int & 0xf as c_int == 3 as c_int
    {
        return Ok(LTnum(l, r));
    } else {
        return lessthanothers(L, l, r);
    };
}

#[inline(always)]
unsafe fn lessequalothers<A>(
    L: &Thread<A>,
    l: *const UnsafeValue<A>,
    r: *const UnsafeValue<A>,
) -> Result<c_int, Box<dyn core::error::Error>> {
    if (*l).tt_ as c_int & 0xf as c_int == 4 as c_int
        && (*r).tt_ as c_int & 0xf as c_int == 4 as c_int
    {
        return Ok(
            (l_strcmp::<A>((*l).value_.gc.cast(), (*r).value_.gc.cast()) <= 0 as c_int) as c_int,
        );
    } else {
        return luaT_callorderTM(L, l, r, TM_LE);
    };
}

pub unsafe fn luaV_lessequal<A>(
    L: &Thread<A>,
    l: *const UnsafeValue<A>,
    r: *const UnsafeValue<A>,
) -> Result<c_int, Box<dyn core::error::Error>> {
    if (*l).tt_ as c_int & 0xf as c_int == 3 as c_int
        && (*r).tt_ as c_int & 0xf as c_int == 3 as c_int
    {
        return Ok(LEnum(l, r));
    } else {
        return lessequalothers(L, l, r);
    };
}

#[inline(never)]
pub unsafe fn luaV_equalobj<A>(
    L: Option<&Thread<A>>,
    t1: *const UnsafeValue<A>,
    t2: *const UnsafeValue<A>,
) -> Result<bool, Box<dyn core::error::Error>> {
    // Check if same type.
    if (*t1).tt_ & 0x3f != (*t2).tt_ & 0x3f {
        if (*t1).tt_ & 0xf != (*t2).tt_ & 0xf || (*t1).tt_ & 0xf != 3 {
            return Ok(false);
        } else {
            let r = match (luaV_tointegerns(t1, F2Ieq), luaV_tointegerns(t2, F2Ieq)) {
                (Some(i1), Some(i2)) => i1 == i2,
                _ => false,
            };

            return Ok(r);
        }
    }

    // Compare.
    let mut tm = null();
    let th = match (*t1).tt_ & 0x3f {
        0x00 | 0x01 | 0x11 => return Ok(true),
        0x02 => return Ok(core::ptr::fn_addr_eq((*t1).value_.f, (*t2).value_.f)),
        0x12 => todo!(),
        0x22 => return Ok(core::ptr::fn_addr_eq((*t1).value_.a, (*t2).value_.a)),
        0x32 => todo!(),
        0x03 => return Ok((*t1).value_.i == (*t2).value_.i),
        0x13 => return Ok((*t1).value_.n == (*t2).value_.n),
        0x04 => {
            let t1 = (*t1).value_.gc.cast::<Str<A>>();
            let t2 = (*t2).value_.gc.cast::<Str<A>>();

            return if t1 == t2 {
                Ok(true)
            } else if (*t1).is_short() || (*t2).is_short() {
                Ok(false)
            } else {
                Ok((*t1).as_bytes() == (*t2).as_bytes())
            };
        }
        0x05 => {
            if (*t1).value_.gc == (*t2).value_.gc {
                return Ok(true);
            }

            let th = match L {
                Some(v) => v,
                None => return Ok(false),
            };

            tm = if ((*((*t1).value_.gc as *mut Table<A>)).metatable.get()).is_null() {
                null()
            } else if (*(*((*t1).value_.gc as *mut Table<A>)).metatable.get())
                .flags
                .get() as c_uint
                & (1 as c_uint) << TM_EQ as c_int
                != 0
            {
                null()
            } else {
                luaT_gettm((*((*t1).value_.gc as *mut Table<A>)).metatable.get(), TM_EQ)
            };

            if tm.is_null() {
                tm = if ((*((*t2).value_.gc as *mut Table<A>)).metatable.get()).is_null() {
                    null()
                } else if (*(*((*t2).value_.gc as *mut Table<A>)).metatable.get())
                    .flags
                    .get() as c_uint
                    & (1 as c_uint) << TM_EQ as c_int
                    != 0
                {
                    null()
                } else {
                    luaT_gettm((*((*t2).value_.gc as *mut Table<A>)).metatable.get(), TM_EQ)
                };
            }

            th
        }
        0x07 => {
            if ((*t1).value_.gc) == ((*t2).value_.gc) {
                return Ok(true);
            }

            let th = match L {
                Some(v) => v,
                None => return Ok(false),
            };

            tm = if (*(*t1).value_.gc.cast::<UserData<A, ()>>()).mt.is_null() {
                null()
            } else if (*(*(*t1).value_.gc.cast::<UserData<A, ()>>()).mt)
                .flags
                .get() as c_uint
                & (1 as c_uint) << TM_EQ as c_int
                != 0
            {
                null()
            } else {
                luaT_gettm((*(*t1).value_.gc.cast::<UserData<A, ()>>()).mt, TM_EQ)
            };

            if tm.is_null() {
                tm = if (*(*t2).value_.gc.cast::<UserData<A, ()>>()).mt.is_null() {
                    null()
                } else if (*(*(*t2).value_.gc.cast::<UserData<A, ()>>()).mt)
                    .flags
                    .get() as c_uint
                    & (1 as c_uint) << TM_EQ as c_int
                    != 0
                {
                    null()
                } else {
                    luaT_gettm((*(*t2).value_.gc.cast::<UserData<A, ()>>()).mt, TM_EQ)
                };
            }

            th
        }
        _ => return Ok((*t1).value_.gc == (*t2).value_.gc),
    };

    if tm.is_null() {
        return Ok(false);
    }

    // Invoke __eq.
    let r = match luaT_callTMres(th, tm, t1, t2) {
        Ok(_) => {
            th.top.sub(1);
            th.top.read(0)
        }
        Err(e) => return Err(e), // Requires unsized coercion.
    };

    Ok(!(r.tt_ == 1 | 0 << 4 || r.tt_ & 0xf == 0))
}

unsafe fn copy2buff<A>(
    th: *const Thread<A>,
    top: *mut StackValue<A>,
    mut n: c_int,
    len: usize,
) -> *const Str<A> {
    let mut buf = Vec::with_capacity(len);
    let mut bytes = false;

    loop {
        let st = (*top.offset(-(n as isize))).value_.gc.cast::<Str<A>>();

        buf.extend_from_slice((*st).as_bytes());
        bytes |= match (*st).ty.get() {
            Some(ContentType::Binary) => true,
            Some(ContentType::Utf8) => false,
            None => true,
        };

        n -= 1;

        if !(n > 0) {
            break;
        }
    }

    match bytes {
        true => Str::from_bytes((*th).hdr.global, buf),
        false => Str::from_str((*th).hdr.global, String::from_utf8_unchecked(buf)),
    }
    .unwrap_or_else(identity)
}

#[inline(never)]
pub unsafe fn luaV_concat<A>(
    L: &Thread<A>,
    mut total: c_int,
) -> Result<(), Box<dyn core::error::Error>> {
    if total == 1 {
        return Ok(());
    }

    loop {
        let top = (*L).top.get();
        let mut n: c_int = 2 as c_int;

        if !((*top.offset(-2)).tt_ & 0xf == 4 || (*top.offset(-2)).tt_ & 0xf == 3)
            || !((*top.offset(-1)).tt_ & 0xf == 4
                || (*top.offset(-1)).tt_ & 0xf == 3 && {
                    let v = top.offset(-1);
                    let s = if (*v).tt_ & 0x3f == 0x03 {
                        (*v).value_.i.to_string()
                    } else {
                        (*v).value_.n.to_string()
                    };
                    let s = Str::from_str((*L).hdr.global, s).unwrap_or_else(identity);

                    (*v).tt_ = (*s).hdr.tt | 1 << 6;
                    (*v).value_.gc = s.cast();

                    true
                })
        {
            luaT_tryconcatTM(L)?;
        } else if (*top.offset(-(1 as c_int as isize))).tt_ as c_int
            == 4 as c_int | (0 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int
            && (*((*top.offset(-(1 as c_int as isize))).value_.gc as *mut Str<A>)).len as c_int
                == 0 as c_int
        {
            ((*top.offset(-(2 as c_int as isize))).tt_ as c_int & 0xf as c_int == 4 as c_int
                || (*top.offset(-2)).tt_ & 0xf == 3 && {
                    let v = top.offset(-2);
                    let s = if (*v).tt_ & 0x3f == 0x03 {
                        (*v).value_.i.to_string()
                    } else {
                        (*v).value_.n.to_string()
                    };
                    let s = Str::from_str((*L).hdr.global, s).unwrap_or_else(identity);

                    (*v).tt_ = (*s).hdr.tt | 1 << 6;
                    (*v).value_.gc = s.cast();

                    true
                }) as c_int;
        } else if (*top.offset(-(2 as c_int as isize))).tt_ as c_int
            == 4 as c_int | (0 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int
            && (*((*top.offset(-(2 as c_int as isize))).value_.gc as *mut Str<A>)).len as c_int
                == 0 as c_int
        {
            let io1 = top.offset(-(2 as c_int as isize));
            let io2 = top.offset(-(1 as c_int as isize));

            (*io1).value_ = (*io2).value_;
            (*io1).tt_ = (*io2).tt_;
        } else {
            let mut tl = (*((*top.offset(-1)).value_.gc as *mut Str<A>)).len;

            n = 1 as c_int;
            while n < total
                && ((*top.offset(-(n as isize)).offset(-(1 as c_int as isize))).tt_ as c_int
                    & 0xf as c_int
                    == 4 as c_int
                    || (*top.offset(-(n as isize)).offset(-1)).tt_ & 0xf == 3 && {
                        let v = top.offset(-(n as isize)).offset(-1);
                        let s = if (*v).tt_ & 0x3f == 0x03 {
                            (*v).value_.i.to_string()
                        } else {
                            (*v).value_.n.to_string()
                        };
                        let s = Str::from_str((*L).hdr.global, s).unwrap_or_else(identity);

                        (*v).tt_ = (*s).hdr.tt | 1 << 6;
                        (*v).value_.gc = s.cast();

                        true
                    })
            {
                let l = (*((*top.offset(-(n as isize)).offset(-(1 as c_int as isize)))
                    .value_
                    .gc as *mut Str<A>))
                    .len;

                if ((l
                    >= (if (::core::mem::size_of::<usize>() as c_ulong)
                        < ::core::mem::size_of::<i64>() as c_ulong
                    {
                        !(0 as c_int as usize)
                    } else {
                        0x7fffffffffffffff as c_longlong as usize
                    })
                    .wrapping_sub(::core::mem::size_of::<Str<A>>())
                    .wrapping_sub(tl)) as c_int
                    != 0 as c_int) as c_int as c_long
                    != 0
                {
                    (*L).top.set(top.offset(-(total as isize)));
                    return Err("string length overflow".into());
                }
                tl = tl.wrapping_add(l);
                n += 1;
            }

            let ts = copy2buff(L, top, n, tl);
            let io = top.offset(-(n as isize));

            (*io).value_.gc = ts.cast();
            (*io).tt_ = ((*ts).hdr.tt as c_int | (1 as c_int) << 6) as u8;
        }

        total -= n - 1 as c_int;
        (*L).top.sub((n - 1).try_into().unwrap());

        if !(total > 1 as c_int) {
            break Ok(());
        }
    }
}

#[inline(never)]
pub unsafe fn luaV_objlen<A>(
    L: &Thread<A>,
    rb: *const UnsafeValue<A>,
) -> Result<UnsafeValue<A>, Box<dyn core::error::Error>> {
    let mut tm = null();

    match (*rb).tt_ & 0xf {
        5 => {
            let h = (*rb).value_.gc as *mut Table<A>;

            tm = if ((*h).metatable.get()).is_null() {
                null()
            } else if (*(*h).metatable.get()).flags.get() as c_uint
                & (1 as c_uint) << TM_LEN as c_int
                != 0
            {
                null()
            } else {
                luaT_gettm((*h).metatable.get(), TM_LEN)
            };

            if tm.is_null() {
                return Ok(i64::try_from(luaH_getn(h)).unwrap().into());
            }
        }
        4 => {
            return Ok(i64::try_from((*(*rb).value_.gc.cast::<Str<A>>()).len)
                .unwrap()
                .into());
        }
        _ => {
            tm = luaT_gettmbyobj(L, rb, TM_LEN);

            if (*tm).tt_ & 0xf == 0 {
                return Err(luaG_typeerror(L, rb, "get length of"));
            }
        }
    }

    // Invoke metamethod.
    if let Err(e) = luaT_callTMres(L, tm, rb, rb) {
        return Err(e); // Requires unsized coercion.
    }

    (*L).top.sub(1);

    Ok((*L).top.read(0))
}

/// Returns [`None`] if `n` is zero.
#[inline(always)]
pub fn luaV_idiv(m: i64, n: i64) -> Option<i64> {
    if (((n as u64).wrapping_add(1 as c_uint as u64) <= 1 as c_uint as u64) as c_int != 0 as c_int)
        as c_int as c_long
        != 0
    {
        if n == 0 as c_int as i64 {
            return None;
        }
        return Some((0 as c_int as u64).wrapping_sub(m as u64) as i64);
    } else {
        let mut q: i64 = m / n;
        if m ^ n < 0 as c_int as i64 && m % n != 0 as c_int as i64 {
            q -= 1 as c_int as i64;
        }
        return Some(q);
    };
}

/// Returns [`None`] if `n` is zero.
#[inline(always)]
pub fn luaV_mod(m: i64, n: i64) -> Option<i64> {
    if (n as u64).wrapping_add(1) <= 1 {
        if n == 0 as c_int as i64 {
            return None;
        }
        return Some(0 as c_int as i64);
    } else {
        let mut r: i64 = m % n;
        if r != 0 as c_int as i64 && r ^ n < 0 as c_int as i64 {
            r += n;
        }
        return Some(r);
    };
}

#[inline(always)]
pub fn luaV_modf(m: Float, n: Float) -> Float {
    let mut r = fmod(m.into(), n.into());

    if if r > 0f64 {
        n < 0f64
    } else {
        r < 0f64 && n > 0f64
    } {
        r += f64::from(n);
    }

    r.into()
}

#[inline(always)]
pub fn luaV_shiftl(x: i64, y: i64) -> i64 {
    if y < 0 as c_int as i64 {
        if y <= -((::core::mem::size_of::<i64>() as c_ulong).wrapping_mul(8 as c_int as c_ulong)
            as c_int) as i64
        {
            return 0 as c_int as i64;
        } else {
            return (x as u64 >> -y as u64) as i64;
        }
    } else if y
        >= (::core::mem::size_of::<i64>() as c_ulong).wrapping_mul(8 as c_int as c_ulong) as c_int
            as i64
    {
        return 0 as c_int as i64;
    } else {
        return ((x as u64) << y as u64) as i64;
    };
}

#[inline(never)]
unsafe fn pushclosure<D>(
    L: *const Thread<D>,
    p: *mut Proto<D>,
    encup: &[Cell<*mut UpVal<D>>],
    base: *mut StackValue<D>,
    ra: *mut StackValue<D>,
) {
    let nup: c_int = (*p).sizeupvalues;
    let uv = (*p).upvalues;
    let mut i: c_int = 0;
    let ncl = luaF_newLclosure((*L).hdr.global, nup);
    (*ncl).p.set(p);
    let io = ra;
    let x_ = ncl;

    (*io).value_.gc = x_.cast();
    (*io).tt_ = (6 as c_int | (0 as c_int) << 4 as c_int | 1 << 6) as u8;
    i = 0 as c_int;

    while i < nup {
        if (*uv.offset(i as isize)).instack != 0 {
            (*ncl).upvals[i as usize].set(luaF_findupval(
                L,
                base.offset((*uv.offset(i as isize)).idx as c_int as isize),
            ));
        } else {
            (*ncl).upvals[i as usize].set(encup[usize::from((*uv.offset(i as isize)).idx)].get());
        }

        if (*ncl).hdr.marked.get() & 1 << 5 != 0
            && (*(*ncl).upvals[i as usize].get()).hdr.marked.get() & (1 << 3 | 1 << 4) != 0
        {
            (*L).hdr
                .global()
                .gc
                .barrier(ncl.cast(), (*ncl).upvals[i as usize].get().cast());
        }
        i += 1;
    }
}

pub async unsafe fn run<A>(
    th: &Thread<A>,
    mut ci: *mut CallInfo<A>,
) -> Result<(), Box<dyn core::error::Error>> {
    let g = th.hdr.global();

    loop {
        ci = g.executor.exec(th, ci).await?;

        if ci.is_null() {
            return Ok(());
        }
    }
}
