#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments
)]
#![allow(unsafe_op_in_unsafe_fn)]

pub use self::opcode::*;

use crate::ldebug::{luaG_forerror, luaG_runerror, luaG_tracecall, luaG_traceexec, luaG_typeerror};
use crate::ldo::{luaD_call, luaD_hookcall, luaD_poscall, luaD_precall, luaD_pretailcall};
use crate::lfunc::{
    luaF_close, luaF_closeupval, luaF_findupval, luaF_newLclosure, luaF_newtbcupval,
};
use crate::lobject::{Proto, UpVal, luaO_str2num, luaO_tostring};
use crate::lstate::CallInfo;
use crate::lstring::luaS_eqlngstr;
use crate::ltm::{
    TM_BNOT, TM_EQ, TM_INDEX, TM_LE, TM_LEN, TM_LT, TM_NEWINDEX, TM_UNM, TMS, luaT_adjustvarargs,
    luaT_callTM, luaT_callTMres, luaT_callorderTM, luaT_callorderiTM, luaT_gettm, luaT_gettmbyobj,
    luaT_getvarargs, luaT_trybinTM, luaT_trybinassocTM, luaT_trybiniTM, luaT_tryconcatTM,
};
use crate::table::{
    luaH_finishset, luaH_get, luaH_getint, luaH_getn, luaH_getshortstr, luaH_getstr,
    luaH_realasize, luaH_resize, luaH_resizearray,
};
use crate::value::UnsafeValue;
use crate::{
    ArithError, LuaFn, NON_YIELDABLE_WAKER, Nil, StackValue, Str, Table, Thread, UserData,
};
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::Cell;
use core::cmp::Ordering;
use core::ffi::c_void;
use core::hint::unreachable_unchecked;
use core::pin::pin;
use core::ptr::{null, null_mut};
use core::task::{Context, Poll, Waker};
use libc::memcpy;
use libm::{floor, fmod, pow};

pub type F2Imod = c_uint;

pub const F2Iceil: F2Imod = 2;
pub const F2Ifloor: F2Imod = 1;
pub const F2Ieq: F2Imod = 0;

type c_int = i32;
type c_uint = u32;
type c_long = i64;
type c_ulong = u64;
type c_longlong = i64;
type c_double = f64;

mod opcode;

unsafe fn l_strton<D>(obj: *const UnsafeValue<D>) -> Option<UnsafeValue<D>> {
    if !((*obj).tt_ as c_int & 0xf as c_int == 4 as c_int) {
        None
    } else {
        let st = (*obj).value_.gc.cast::<Str<D>>();

        luaO_str2num((*st).contents.as_ptr()).map(|v| v.into())
    }
}

#[inline(never)]
pub unsafe fn luaV_tonumber_<D>(obj: *const UnsafeValue<D>, n: *mut f64) -> c_int {
    if (*obj).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
        *n = (*obj).value_.i as f64;
        return 1 as c_int;
    } else if let Some(v) = l_strton(obj) {
        *n = if v.tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
            v.value_.i as f64
        } else {
            v.value_.n
        };
        return 1 as c_int;
    } else {
        return 0 as c_int;
    };
}

pub unsafe fn luaV_flttointeger(n: f64, p: *mut i64, mode: F2Imod) -> c_int {
    let mut f: f64 = floor(n);
    if n != f {
        if mode as c_uint == F2Ieq as c_int as c_uint {
            return 0 as c_int;
        } else if mode as c_uint == F2Iceil as c_int as c_uint {
            f += 1 as c_int as f64;
        }
    }
    return (f >= (-(0x7fffffffffffffff as c_longlong) - 1 as c_int as c_longlong) as c_double
        && f < -((-(0x7fffffffffffffff as c_longlong) - 1 as c_int as c_longlong) as c_double)
        && {
            *p = f as c_longlong;
            1 as c_int != 0
        }) as c_int;
}

pub unsafe fn luaV_tointegerns<D>(obj: *const UnsafeValue<D>, p: *mut i64, mode: F2Imod) -> c_int {
    if (*obj).tt_ == 3 | 1 << 4 {
        luaV_flttointeger((*obj).value_.n, p, mode)
    } else if (*obj).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
        *p = (*obj).value_.i;
        return 1 as c_int;
    } else {
        return 0 as c_int;
    }
}

#[inline(never)]
pub unsafe fn luaV_tointeger<D>(obj: *const UnsafeValue<D>, p: *mut i64, mode: F2Imod) -> c_int {
    match l_strton(obj) {
        Some(v) => luaV_tointegerns(&v, p, mode),
        None => luaV_tointegerns(obj, p, mode),
    }
}

unsafe fn forlimit<D>(
    L: *const Thread<D>,
    init: i64,
    lim: *const UnsafeValue<D>,
    p: *mut i64,
    step: i64,
) -> Result<c_int, Box<dyn core::error::Error>> {
    if luaV_tointeger(
        lim,
        p,
        (if step < 0 as c_int as i64 {
            F2Iceil as c_int
        } else {
            F2Ifloor as c_int
        }) as F2Imod,
    ) == 0
    {
        let mut flim: f64 = 0.;
        if if (*lim).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
            flim = (*lim).value_.n;
            1 as c_int
        } else {
            luaV_tonumber_(lim, &mut flim)
        } == 0
        {
            luaG_forerror(L, lim, "limit")?;
        }
        if (0 as c_int as f64) < flim {
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
    return if step > 0 as c_int as i64 {
        Ok((init > *p) as c_int)
    } else {
        Ok((init < *p) as c_int)
    };
}

unsafe fn forprep<D>(
    L: *const Thread<D>,
    ra: *mut StackValue<D>,
) -> Result<c_int, Box<dyn core::error::Error>> {
    let pinit = &raw mut (*ra).val;
    let plimit = &raw mut (*ra.offset(1 as c_int as isize)).val;
    let pstep = &raw mut (*ra.offset(2 as c_int as isize)).val;

    if (*pinit).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
        && (*pstep).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
    {
        let init: i64 = (*pinit).value_.i;
        let step: i64 = (*pstep).value_.i;
        let mut limit: i64 = 0;
        if step == 0 as c_int as i64 {
            return Err(luaG_runerror(L, "'for' step is zero"));
        }
        let io = &raw mut (*ra.offset(3 as c_int as isize)).val;

        (*io).value_.i = init;
        (*io).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
        if forlimit(L, init, plimit, &mut limit, step)? != 0 {
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
        let mut init_0: f64 = 0.;
        let mut limit_0: f64 = 0.;
        let mut step_0: f64 = 0.;
        if (((if (*plimit).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
            limit_0 = (*plimit).value_.n;
            1 as c_int
        } else {
            luaV_tonumber_(plimit, &mut limit_0)
        }) == 0) as c_int
            != 0 as c_int) as c_int as c_long
            != 0
        {
            luaG_forerror(L, plimit, "limit")?;
        }
        if (((if (*pstep).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
            step_0 = (*pstep).value_.n;
            1 as c_int
        } else {
            luaV_tonumber_(pstep, &mut step_0)
        }) == 0) as c_int
            != 0 as c_int) as c_int as c_long
            != 0
        {
            luaG_forerror(L, pstep, "step")?;
        }
        if (((if (*pinit).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
            init_0 = (*pinit).value_.n;
            1 as c_int
        } else {
            luaV_tonumber_(pinit, &mut init_0)
        }) == 0) as c_int
            != 0 as c_int) as c_int as c_long
            != 0
        {
            luaG_forerror(L, pinit, "initial value")?;
        }
        if step_0 == 0 as c_int as f64 {
            return Err(luaG_runerror(L, "'for' step is zero"));
        }
        if if (0 as c_int as f64) < step_0 {
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
            let io_3 = &raw mut (*ra).val;
            (*io_3).value_.n = init_0;
            (*io_3).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
            let io_4 = &raw mut (*ra.offset(3 as c_int as isize)).val;
            (*io_4).value_.n = init_0;
            (*io_4).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
        }
    }
    return Ok(0 as c_int);
}

unsafe fn floatforloop<D>(ra: *mut StackValue<D>) -> c_int {
    let step: f64 = (*ra.offset(2 as c_int as isize)).val.value_.n;
    let limit: f64 = (*ra.offset(1 as c_int as isize)).val.value_.n;
    let mut idx: f64 = (*ra).val.value_.n;
    idx = idx + step;
    if if (0 as c_int as f64) < step {
        (idx <= limit) as c_int
    } else {
        (limit <= idx) as c_int
    } != 0
    {
        let io = &raw mut (*ra).val;

        (*io).value_.n = idx;
        let io_0 = &raw mut (*ra.offset(3 as c_int as isize)).val;

        (*io_0).value_.n = idx;
        (*io_0).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
        return 1 as c_int;
    } else {
        return 0 as c_int;
    };
}

pub unsafe fn luaV_finishget<D>(
    L: *const Thread<D>,
    mut t: *const UnsafeValue<D>,
    key: *mut UnsafeValue<D>,
    mut slot: *const UnsafeValue<D>,
) -> Result<UnsafeValue<D>, Box<dyn core::error::Error>> {
    let mut loop_0: c_int = 0;
    let mut tm = null();

    loop_0 = 0 as c_int;
    while loop_0 < 2000 as c_int {
        if slot.is_null() {
            tm = luaT_gettmbyobj(L, t, TM_INDEX);

            if (*tm).tt_ & 0xf == 0 {
                return Err(luaG_typeerror(L, t, "index"));
            }
        } else {
            tm = if ((*((*t).value_.gc.cast::<Table<D>>())).metatable.get()).is_null() {
                null()
            } else if (*(*((*t).value_.gc.cast::<Table<D>>())).metatable.get())
                .flags
                .get() as c_uint
                & (1 as c_uint) << TM_INDEX as c_int
                != 0
            {
                null()
            } else {
                luaT_gettm(
                    (*((*t).value_.gc.cast::<Table<D>>())).metatable.get(),
                    TM_INDEX,
                )
            };
            if tm.is_null() {
                return Ok(Nil.into());
            }
        }

        if ((*tm).tt_ & 0xf) == 2 || ((*tm).tt_ & 0xf) == 6 {
            if let Err(e) = luaT_callTMres(L, tm, t, key) {
                return Err(e); // Requires unsized coercion.
            }

            (*L).top.sub(1);

            return Ok((*L).top.read(0));
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
            return Ok(slot.read());
        }
        loop_0 += 1;
    }

    Err(luaG_runerror(L, "'__index' chain too long; possible loop"))
}

pub unsafe fn luaV_finishset<D>(
    L: *const Thread<D>,
    mut t: *const UnsafeValue<D>,
    key: *mut UnsafeValue<D>,
    val: *mut UnsafeValue<D>,
    mut slot: *const UnsafeValue<D>,
) -> Result<(), Box<dyn core::error::Error>> {
    let mut loop_0: c_int = 0;
    loop_0 = 0 as c_int;
    while loop_0 < 2000 as c_int {
        let mut tm = null();

        if !slot.is_null() {
            let h = (*t).value_.gc.cast::<Table<D>>();

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

    Err(luaG_runerror(
        L,
        "'__newindex' chain too long; possible loop",
    ))
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

unsafe fn LTintfloat(i: i64, f: f64) -> c_int {
    if ((1 as c_int as u64) << 53 as c_int).wrapping_add(i as u64)
        <= 2 as c_int as u64 * ((1 as c_int as u64) << 53 as c_int)
    {
        return ((i as f64) < f) as c_int;
    } else {
        let mut fi: i64 = 0;
        if luaV_flttointeger(f, &mut fi, F2Iceil) != 0 {
            return (i < fi) as c_int;
        } else {
            return (f > 0 as c_int as f64) as c_int;
        }
    };
}

unsafe fn LEintfloat(i: i64, f: f64) -> c_int {
    if ((1 as c_int as u64) << 53 as c_int).wrapping_add(i as u64)
        <= 2 as c_int as u64 * ((1 as c_int as u64) << 53 as c_int)
    {
        return (i as f64 <= f) as c_int;
    } else {
        let mut fi: i64 = 0;
        if luaV_flttointeger(f, &mut fi, F2Ifloor) != 0 {
            return (i <= fi) as c_int;
        } else {
            return (f > 0 as c_int as f64) as c_int;
        }
    };
}

unsafe fn LTfloatint(f: f64, i: i64) -> c_int {
    if ((1 as c_int as u64) << 53 as c_int).wrapping_add(i as u64)
        <= 2 as c_int as u64 * ((1 as c_int as u64) << 53 as c_int)
    {
        return (f < i as f64) as c_int;
    } else {
        let mut fi: i64 = 0;
        if luaV_flttointeger(f, &mut fi, F2Ifloor) != 0 {
            return (fi < i) as c_int;
        } else {
            return (f < 0 as c_int as f64) as c_int;
        }
    };
}

unsafe fn LEfloatint(f: f64, i: i64) -> c_int {
    if ((1 as c_int as u64) << 53 as c_int).wrapping_add(i as u64)
        <= 2 as c_int as u64 * ((1 as c_int as u64) << 53 as c_int)
    {
        return (f <= i as f64) as c_int;
    } else {
        let mut fi: i64 = 0;
        if luaV_flttointeger(f, &mut fi, F2Iceil) != 0 {
            return (fi <= i) as c_int;
        } else {
            return (f < 0 as c_int as f64) as c_int;
        }
    };
}

unsafe fn LTnum<D>(l: *const UnsafeValue<D>, r: *const UnsafeValue<D>) -> c_int {
    if (*l).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
        let li: i64 = (*l).value_.i;
        if (*r).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
            return (li < (*r).value_.i) as c_int;
        } else {
            return LTintfloat(li, (*r).value_.n);
        }
    } else {
        let lf: f64 = (*l).value_.n;
        if (*r).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
            return (lf < (*r).value_.n) as c_int;
        } else {
            return LTfloatint(lf, (*r).value_.i);
        }
    };
}

unsafe fn LEnum<D>(l: *const UnsafeValue<D>, r: *const UnsafeValue<D>) -> c_int {
    if (*l).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
        let li: i64 = (*l).value_.i;
        if (*r).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
            return (li <= (*r).value_.i) as c_int;
        } else {
            return LEintfloat(li, (*r).value_.n);
        }
    } else {
        let lf: f64 = (*l).value_.n;
        if (*r).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
            return (lf <= (*r).value_.n) as c_int;
        } else {
            return LEfloatint(lf, (*r).value_.i);
        }
    };
}

unsafe fn lessthanothers<D>(
    L: *const Thread<D>,
    l: *const UnsafeValue<D>,
    r: *const UnsafeValue<D>,
) -> Result<c_int, Box<dyn core::error::Error>> {
    if (*l).tt_ as c_int & 0xf as c_int == 4 as c_int
        && (*r).tt_ as c_int & 0xf as c_int == 4 as c_int
    {
        return Ok(
            (l_strcmp::<D>((*l).value_.gc.cast(), (*r).value_.gc.cast()) < 0 as c_int) as c_int,
        );
    } else {
        return luaT_callorderTM(L, l, r, TM_LT);
    };
}

#[inline(never)]
pub unsafe fn luaV_lessthan<D>(
    L: *const Thread<D>,
    l: *const UnsafeValue<D>,
    r: *const UnsafeValue<D>,
) -> Result<c_int, Box<dyn core::error::Error>> {
    if (*l).tt_ as c_int & 0xf as c_int == 3 as c_int
        && (*r).tt_ as c_int & 0xf as c_int == 3 as c_int
    {
        return Ok(LTnum(l, r));
    } else {
        return lessthanothers(L, l, r);
    };
}

unsafe fn lessequalothers<D>(
    L: *const Thread<D>,
    l: *const UnsafeValue<D>,
    r: *const UnsafeValue<D>,
) -> Result<c_int, Box<dyn core::error::Error>> {
    if (*l).tt_ as c_int & 0xf as c_int == 4 as c_int
        && (*r).tt_ as c_int & 0xf as c_int == 4 as c_int
    {
        return Ok(
            (l_strcmp::<D>((*l).value_.gc.cast(), (*r).value_.gc.cast()) <= 0 as c_int) as c_int,
        );
    } else {
        return luaT_callorderTM(L, l, r, TM_LE);
    };
}

pub unsafe fn luaV_lessequal<D>(
    L: *const Thread<D>,
    l: *const UnsafeValue<D>,
    r: *const UnsafeValue<D>,
) -> Result<c_int, Box<dyn core::error::Error>> {
    if (*l).tt_ as c_int & 0xf as c_int == 3 as c_int
        && (*r).tt_ as c_int & 0xf as c_int == 3 as c_int
    {
        return Ok(LEnum(l, r));
    } else {
        return lessequalothers(L, l, r);
    };
}

pub unsafe fn luaV_equalobj<D>(
    L: *const Thread<D>,
    t1: *const UnsafeValue<D>,
    t2: *const UnsafeValue<D>,
) -> Result<c_int, Box<dyn core::error::Error>> {
    let mut tm = null();

    if (*t1).tt_ as c_int & 0x3f as c_int != (*t2).tt_ as c_int & 0x3f as c_int {
        if (*t1).tt_ as c_int & 0xf as c_int != (*t2).tt_ as c_int & 0xf as c_int
            || (*t1).tt_ as c_int & 0xf as c_int != 3 as c_int
        {
            return Ok(0 as c_int);
        } else {
            let mut i1: i64 = 0;
            let mut i2: i64 = 0;
            return Ok((luaV_tointegerns(t1, &mut i1, F2Ieq) != 0
                && luaV_tointegerns(t2, &mut i2, F2Ieq) != 0
                && i1 == i2) as c_int);
        }
    }
    match (*t1).tt_ as c_int & 0x3f as c_int {
        0 | 1 | 17 => return Ok(1 as c_int),
        3 => return Ok(((*t1).value_.i == (*t2).value_.i) as c_int),
        19 => return Ok(((*t1).value_.n == (*t2).value_.n) as c_int),
        2 | 18 | 50 => {
            return Ok(core::ptr::fn_addr_eq((*t1).value_.f, (*t2).value_.f) as c_int);
        }
        34 => return Ok(core::ptr::fn_addr_eq((*t1).value_.a, (*t2).value_.a) as c_int),
        4 => return Ok((((*t1).value_.gc) == ((*t2).value_.gc)) as c_int),
        20 => {
            return Ok(luaS_eqlngstr::<D>(
                (*t1).value_.gc.cast(),
                (*t2).value_.gc.cast(),
            ));
        }
        7 => {
            if ((*t1).value_.gc) == ((*t2).value_.gc) {
                return Ok(1 as c_int);
            } else if L.is_null() {
                return Ok(0 as c_int);
            }

            tm = if (*(*t1).value_.gc.cast::<UserData<D, ()>>()).mt.is_null() {
                null()
            } else if (*(*(*t1).value_.gc.cast::<UserData<D, ()>>()).mt)
                .flags
                .get() as c_uint
                & (1 as c_uint) << TM_EQ as c_int
                != 0
            {
                null()
            } else {
                luaT_gettm((*(*t1).value_.gc.cast::<UserData<D, ()>>()).mt, TM_EQ)
            };
            if tm.is_null() {
                tm = if (*(*t2).value_.gc.cast::<UserData<D, ()>>()).mt.is_null() {
                    null()
                } else if (*(*(*t2).value_.gc.cast::<UserData<D, ()>>()).mt)
                    .flags
                    .get() as c_uint
                    & (1 as c_uint) << TM_EQ as c_int
                    != 0
                {
                    null()
                } else {
                    luaT_gettm((*(*t2).value_.gc.cast::<UserData<D, ()>>()).mt, TM_EQ)
                };
            }
        }
        5 => {
            if ((*t1).value_.gc) == ((*t2).value_.gc) {
                return Ok(1 as c_int);
            } else if L.is_null() {
                return Ok(0 as c_int);
            }
            tm = if ((*((*t1).value_.gc as *mut Table<D>)).metatable.get()).is_null() {
                null()
            } else if (*(*((*t1).value_.gc as *mut Table<D>)).metatable.get())
                .flags
                .get() as c_uint
                & (1 as c_uint) << TM_EQ as c_int
                != 0
            {
                null()
            } else {
                luaT_gettm((*((*t1).value_.gc as *mut Table<D>)).metatable.get(), TM_EQ)
            };
            if tm.is_null() {
                tm = if ((*((*t2).value_.gc as *mut Table<D>)).metatable.get()).is_null() {
                    null()
                } else if (*(*((*t2).value_.gc as *mut Table<D>)).metatable.get())
                    .flags
                    .get() as c_uint
                    & (1 as c_uint) << TM_EQ as c_int
                    != 0
                {
                    null()
                } else {
                    luaT_gettm((*((*t2).value_.gc as *mut Table<D>)).metatable.get(), TM_EQ)
                };
            }
        }
        _ => return Ok(((*t1).value_.gc == (*t2).value_.gc) as c_int),
    }
    if tm.is_null() {
        return Ok(0 as c_int);
    } else {
        if let Err(e) = luaT_callTMres(L, tm, t1, t2) {
            return Err(e); // Requires unsized coercion.
        }

        (*L).top.sub(1);

        return Ok(
            !((*(*L).top.get()).val.tt_ as c_int == 1 as c_int | (0 as c_int) << 4 as c_int
                || (*(*L).top.get()).val.tt_ as c_int & 0xf as c_int == 0 as c_int)
                as c_int,
        );
    };
}

unsafe fn copy2buff<D>(
    th: *const Thread<D>,
    top: *mut StackValue<D>,
    mut n: c_int,
    len: usize,
) -> *const Str<D> {
    let mut buf = Vec::with_capacity(len);
    let mut bytes = false;

    loop {
        let st = (*top.offset(-(n as isize))).val.value_.gc.cast::<Str<D>>();

        buf.extend_from_slice((*st).as_bytes());
        bytes |= (*st).is_utf8() == false;

        n -= 1;

        if !(n > 0 as c_int) {
            break;
        }
    }

    match bytes {
        true => Str::from_bytes((*th).hdr.global, buf),
        false => Str::from_str((*th).hdr.global, String::from_utf8_unchecked(buf)),
    }
}

pub unsafe fn luaV_concat<D>(
    L: *const Thread<D>,
    mut total: c_int,
) -> Result<(), Box<dyn core::error::Error>> {
    if total == 1 {
        return Ok(());
    }

    loop {
        let top = (*L).top.get();
        let mut n: c_int = 2 as c_int;

        if !((*top.offset(-2)).val.tt_ & 0xf == 4 || (*top.offset(-2)).val.tt_ & 0xf == 3)
            || !((*top.offset(-(1 as c_int as isize))).val.tt_ as c_int & 0xf as c_int
                == 4 as c_int
                || (*top.offset(-(1 as c_int as isize))).val.tt_ as c_int & 0xf as c_int
                    == 3 as c_int
                    && {
                        luaO_tostring((*L).hdr.global, &raw mut (*top.offset(-1)).val);
                        1 as c_int != 0
                    })
        {
            luaT_tryconcatTM(L)?;
        } else if (*top.offset(-(1 as c_int as isize))).val.tt_ as c_int
            == 4 as c_int | (0 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int
            && (*((*top.offset(-(1 as c_int as isize))).val.value_.gc as *mut Str<D>)).len as c_int
                == 0 as c_int
        {
            ((*top.offset(-(2 as c_int as isize))).val.tt_ as c_int & 0xf as c_int == 4 as c_int
                || (*top.offset(-(2 as c_int as isize))).val.tt_ as c_int & 0xf as c_int
                    == 3 as c_int
                    && {
                        luaO_tostring((*L).hdr.global, &raw mut (*top.offset(-2)).val);
                        1 as c_int != 0
                    }) as c_int;
        } else if (*top.offset(-(2 as c_int as isize))).val.tt_ as c_int
            == 4 as c_int | (0 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int
            && (*((*top.offset(-(2 as c_int as isize))).val.value_.gc as *mut Str<D>)).len as c_int
                == 0 as c_int
        {
            let io1 = &raw mut (*top.offset(-(2 as c_int as isize))).val;
            let io2 = &raw mut (*top.offset(-(1 as c_int as isize))).val;

            (*io1).value_ = (*io2).value_;
            (*io1).tt_ = (*io2).tt_;
        } else {
            let mut tl = (*((*top.offset(-1)).val.value_.gc as *mut Str<D>)).len;

            n = 1 as c_int;
            while n < total
                && ((*top.offset(-(n as isize)).offset(-(1 as c_int as isize)))
                    .val
                    .tt_ as c_int
                    & 0xf as c_int
                    == 4 as c_int
                    || (*top.offset(-(n as isize)).offset(-(1 as c_int as isize)))
                        .val
                        .tt_ as c_int
                        & 0xf as c_int
                        == 3 as c_int
                        && {
                            luaO_tostring(
                                (*L).hdr.global,
                                &raw mut (*top.offset(-(n as isize)).offset(-1)).val,
                            );
                            1 as c_int != 0
                        })
            {
                let l = (*((*top.offset(-(n as isize)).offset(-(1 as c_int as isize)))
                    .val
                    .value_
                    .gc as *mut Str<D>))
                    .len;

                if ((l
                    >= (if (::core::mem::size_of::<usize>() as c_ulong)
                        < ::core::mem::size_of::<i64>() as c_ulong
                    {
                        !(0 as c_int as usize)
                    } else {
                        0x7fffffffffffffff as c_longlong as usize
                    })
                    .wrapping_sub(::core::mem::size_of::<Str<D>>())
                    .wrapping_sub(tl)) as c_int
                    != 0 as c_int) as c_int as c_long
                    != 0
                {
                    (*L).top.set(top.offset(-(total as isize)));
                    return Err(luaG_runerror(L, "string length overflow"));
                }
                tl = tl.wrapping_add(l);
                n += 1;
            }

            let ts = copy2buff(L, top, n, tl);
            let io = &raw mut (*top.offset(-(n as isize))).val;

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

pub unsafe fn luaV_objlen<D>(
    L: *const Thread<D>,
    rb: *const UnsafeValue<D>,
) -> Result<UnsafeValue<D>, Box<dyn core::error::Error>> {
    let mut tm = null();

    match (*rb).tt_ as c_int & 0x3f as c_int {
        5 => {
            let h = (*rb).value_.gc as *mut Table<D>;

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
        4 | 20 => {
            return Ok(i64::try_from((*(*rb).value_.gc.cast::<Str<D>>()).len)
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
pub fn luaV_mod(m: i64, n: i64) -> Option<i64> {
    if (((n as u64).wrapping_add(1 as c_uint as u64) <= 1 as c_uint as u64) as c_int != 0 as c_int)
        as c_int as c_long
        != 0
    {
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

pub fn luaV_modf(m: f64, n: f64) -> f64 {
    let mut r: f64 = 0.;
    r = fmod(m, n);
    if if r > 0 as c_int as f64 {
        (n < 0 as c_int as f64) as c_int
    } else {
        (r < 0 as c_int as f64 && n > 0 as c_int as f64) as c_int
    } != 0
    {
        r += n;
    }
    return r;
}

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
    let io = &raw mut (*ra).val;
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

pub async unsafe fn luaV_execute<D>(
    L: *const Thread<D>,
    mut ci: *mut CallInfo<D>,
) -> Result<(), Box<dyn core::error::Error>> {
    let mut ra_65 = null_mut();
    let mut newci = null_mut();
    let mut b_4: c_int = 0;
    let mut nresults: c_int = 0;
    let mut current_block: u64;
    let mut cl = null_mut();
    let mut pc: *const u32 = 0 as *const u32;
    let mut trap: c_int = 0;

    '_startfunc: loop {
        trap = (*L).hookmask.get();

        '_returning: loop {
            cl = (*(*ci).func).val.value_.gc as *mut LuaFn<D>;
            let k = (*(*cl).p.get()).k;
            pc = (*ci).u.savedpc;

            if (trap != 0 as c_int) as c_int as c_long != 0 {
                trap = luaG_tracecall(L)?;
            }

            let mut base = ((*ci).func).add(1);
            let mut i = pc.read();
            let mut tab = null_mut();
            let mut key = null_mut();
            let mut slot = null();

            if trap != 0 {
                trap = luaG_traceexec(L, pc)?;
                base = (*ci).func.add(1);
            }

            pc = pc.offset(1);

            loop {
                // We need to do this at the end of each instruction instead of begining of the loop
                // otherwise it will slow down almost 2x.
                macro_rules! vmbreak {
                    () => {
                        if trap != 0 {
                            trap = luaG_traceexec(L, pc)?;
                            base = (*ci).func.add(1);
                        }

                        i = pc.read();
                        pc = pc.offset(1);
                        continue;
                    };
                }

                match i & 0x7F {
                    OP_MOVE => {
                        let ra = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let io1 = &raw mut (*ra).val;
                        let io2 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        (*io1).tt_ = (*io2).tt_;
                        (*io1).value_ = (*io2).value_;
                        vmbreak!();
                    }
                    1 => {
                        let ra_0 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let b: i64 = ((i >> 0 as c_int + 7 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int + 8 as c_int + 1 as c_int)
                                << 0 as c_int) as c_int
                            - (((1 as c_int) << 8 as c_int + 8 as c_int + 1 as c_int) - 1 as c_int
                                >> 1 as c_int)) as i64;
                        let io = &raw mut (*ra_0).val;

                        (*io).value_.i = b;
                        (*io).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                        vmbreak!();
                    }
                    2 => {
                        let ra_1 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let b_0: c_int = (i >> 0 as c_int + 7 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int + 8 as c_int + 1 as c_int)
                                << 0 as c_int) as c_int
                            - (((1 as c_int) << 8 as c_int + 8 as c_int + 1 as c_int) - 1 as c_int
                                >> 1 as c_int);
                        let io_0 = &raw mut (*ra_1).val;

                        (*io_0).value_.n = b_0 as f64;
                        (*io_0).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                        vmbreak!();
                    }
                    3 => {
                        let ra_2 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let rb = k.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int + 8 as c_int + 1 as c_int)
                                    << 0 as c_int) as c_int as isize,
                        );
                        let io1_0 = &raw mut (*ra_2).val;
                        let io2_0 = rb;

                        (*io1_0).value_ = (*io2_0).value_;
                        (*io1_0).tt_ = (*io2_0).tt_;
                        vmbreak!();
                    }
                    4 => {
                        let ra_3 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let rb_0 = k.offset(
                            (*pc >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32)
                                    << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                    << 0 as c_int) as c_int as isize,
                        );
                        pc = pc.offset(1);
                        let io1_1 = &raw mut (*ra_3).val;
                        let io2_1 = rb_0;

                        (*io1_1).value_ = (*io2_1).value_;
                        (*io1_1).tt_ = (*io2_1).tt_;
                        vmbreak!();
                    }
                    5 => {
                        let ra_4 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        (*ra_4).val.tt_ = (1 as c_int | (0 as c_int) << 4 as c_int) as u8;
                        vmbreak!();
                    }
                    6 => {
                        let ra_5 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        (*ra_5).val.tt_ = (1 as c_int | (0 as c_int) << 4 as c_int) as u8;
                        pc = pc.offset(1);
                        vmbreak!();
                    }
                    7 => {
                        let ra_6 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        (*ra_6).val.tt_ = (1 as c_int | (1 as c_int) << 4 as c_int) as u8;
                        vmbreak!();
                    }
                    8 => {
                        let mut ra_7 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let mut b_1: c_int = (i
                            >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int;
                        loop {
                            let fresh3 = ra_7;
                            ra_7 = ra_7.offset(1);
                            (*fresh3).val.tt_ = (0 as c_int | (0 as c_int) << 4 as c_int) as u8;
                            let fresh4 = b_1;
                            b_1 = b_1 - 1;
                            if !(fresh4 != 0) {
                                break;
                            }
                        }
                        vmbreak!();
                    }
                    9 => {
                        let ra_8 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let b_2: c_int = (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int;
                        let io1_2 = &raw mut (*ra_8).val;
                        let io2_2 = (*(*cl).upvals[b_2 as usize].get()).v.get();

                        (*io1_2).value_ = (*io2_2).value_;
                        (*io1_2).tt_ = (*io2_2).tt_;
                        vmbreak!();
                    }
                    10 => {
                        let ra_9 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let uv = (*cl).upvals[(i
                            >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as usize]
                            .get();
                        let io1_3 = (*uv).v.get();
                        let io2_3 = &raw mut (*ra_9).val;

                        (*io1_3).value_ = (*io2_3).value_;
                        (*io1_3).tt_ = (*io2_3).tt_;
                        if (*ra_9).val.tt_ as c_int & (1 as c_int) << 6 as c_int != 0 {
                            if (*uv).hdr.marked.get() as c_int & (1 as c_int) << 5 as c_int != 0
                                && (*(*ra_9).val.value_.gc).marked.get() as c_int
                                    & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                                    != 0
                            {
                                (*L).hdr
                                    .global()
                                    .gc
                                    .barrier(uv.cast(), (*ra_9).val.value_.gc);
                            }
                        }
                        vmbreak!();
                    }
                    11 => {
                        let ra_10 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        tab = (*(*cl).upvals[(i
                            >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                            & !(!(0 as c_int as u32) << 8) << 0)
                            as c_int as usize]
                            .get())
                        .v
                        .get();
                        key = k.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );

                        if if !((*tab).tt_ as c_int
                            == 5 as c_int | (0 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int)
                        {
                            slot = null();
                            0 as c_int
                        } else {
                            slot =
                                luaH_getshortstr((*tab).value_.gc.cast(), (*key).value_.gc.cast());
                            !((*slot).tt_ as c_int & 0xf as c_int == 0 as c_int) as c_int
                        } != 0
                        {
                            let io1_4 = &raw mut (*ra_10).val;
                            let io2_4 = slot;

                            (*io1_4).value_ = (*io2_4).value_;
                            (*io1_4).tt_ = (*io2_4).tt_;
                            vmbreak!();
                        } else {
                            current_block = 0;
                        }
                    }
                    OP_GETTABLE => {
                        let ra_11 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );

                        tab = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        key = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        slot = match (*tab).tt_ == 5 | 0 << 4 | 1 << 6 {
                            true => {
                                if (*key).tt_ == 3 | 0 << 4 {
                                    luaH_getint((*tab).value_.gc.cast(), (*key).value_.i)
                                } else {
                                    luaH_get((*tab).value_.gc.cast(), key)
                                }
                            }
                            false => null(),
                        };

                        if !slot.is_null() && (*slot).tt_ & 0xf != 0 {
                            let io1_5 = &raw mut (*ra_11).val;

                            (*io1_5).tt_ = (*slot).tt_;
                            (*io1_5).value_ = (*slot).value_;

                            vmbreak!();
                        }

                        current_block = 0;
                    }
                    13 => {
                        let ra_12 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let mut slot_1 = null();
                        let rb_2 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let c: c_int = (i
                            >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int;
                        if if !((*rb_2).tt_ as c_int
                            == 5 as c_int | (0 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int)
                        {
                            slot_1 = null();
                            0 as c_int
                        } else {
                            slot_1 = if (c as u64).wrapping_sub(1 as c_uint as u64)
                                < (*((*rb_2).value_.gc as *mut Table<D>)).alimit.get() as u64
                            {
                                (*((*rb_2).value_.gc as *mut Table<D>))
                                    .array
                                    .get()
                                    .offset((c - 1 as c_int) as isize)
                            } else {
                                luaH_getint((*rb_2).value_.gc.cast(), c as i64)
                            };
                            !((*slot_1).tt_ as c_int & 0xf as c_int == 0 as c_int) as c_int
                        } != 0
                        {
                            let io1_6 = &raw mut (*ra_12).val;
                            let io2_6 = slot_1;

                            (*io1_6).value_ = (*io2_6).value_;
                            (*io1_6).tt_ = (*io2_6).tt_;
                        } else {
                            let mut key_0 = UnsafeValue::default();
                            let io_1 = &raw mut key_0;

                            (*io_1).value_.i = c as i64;
                            (*io_1).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                            (*ci).u.savedpc = pc;
                            (*L).top.set((*ci).top);

                            let val = luaV_finishget(L, rb_2, &mut key_0, slot_1)?;

                            (*ci)
                                .func
                                .add(1)
                                .offset(
                                    (i >> 0 as c_int + 7 as c_int
                                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                        as c_int as isize,
                                )
                                .write(StackValue { val });

                            trap = (*ci).u.trap;
                        }
                        vmbreak!();
                    }
                    14 => {
                        let ra_13 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        tab = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        key = k.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let key_1 = (*key).value_.gc as *mut Str<D>;

                        if if !((*tab).tt_ as c_int
                            == 5 as c_int | (0 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int)
                        {
                            slot = null();
                            0 as c_int
                        } else {
                            slot = luaH_getshortstr((*tab).value_.gc.cast(), key_1);
                            !((*slot).tt_ as c_int & 0xf as c_int == 0 as c_int) as c_int
                        } != 0
                        {
                            let io1_7 = &raw mut (*ra_13).val;
                            let io2_7 = slot;

                            (*io1_7).value_ = (*io2_7).value_;
                            (*io1_7).tt_ = (*io2_7).tt_;
                            vmbreak!();
                        } else {
                            current_block = 0;
                        }
                    }
                    15 => {
                        let mut slot_3 = null();
                        let upval_0 = (*(*cl).upvals[(i >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8) << 0)
                            as c_int
                            as usize]
                            .get())
                        .v
                        .get();
                        let rb_4 = k.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let rc_2 = if (i & (1 as c_uint) << 0 as c_int + 7 as c_int + 8 as c_int)
                            as c_int
                            != 0
                        {
                            k.offset(
                                (i >> 0 as c_int
                                    + 7 as c_int
                                    + 8 as c_int
                                    + 1 as c_int
                                    + 8 as c_int
                                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                    as c_int as isize,
                            )
                        } else {
                            &mut (*base.offset(
                                (i >> 0 as c_int
                                    + 7 as c_int
                                    + 8 as c_int
                                    + 1 as c_int
                                    + 8 as c_int
                                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                    as c_int as isize,
                            ))
                            .val
                        };
                        let key_2 = (*rb_4).value_.gc as *mut Str<D>;

                        if if !((*upval_0).tt_ as c_int
                            == 5 as c_int | (0 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int)
                        {
                            slot_3 = null();
                            0 as c_int
                        } else {
                            slot_3 = luaH_getshortstr((*upval_0).value_.gc.cast(), key_2);
                            !((*slot_3).tt_ as c_int & 0xf as c_int == 0 as c_int) as c_int
                        } != 0
                        {
                            let io1_8 = slot_3.cast_mut();
                            let io2_8 = rc_2;

                            (*io1_8).value_ = (*io2_8).value_;
                            (*io1_8).tt_ = (*io2_8).tt_;
                            if (*rc_2).tt_ as c_int & (1 as c_int) << 6 as c_int != 0 {
                                if (*(*upval_0).value_.gc).marked.get() as c_int
                                    & (1 as c_int) << 5 as c_int
                                    != 0
                                    && (*(*rc_2).value_.gc).marked.get() as c_int
                                        & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                                        != 0
                                {
                                    (*L).hdr.global().gc.barrier_back((*upval_0).value_.gc);
                                }
                            }
                        } else {
                            (*ci).u.savedpc = pc;
                            (*L).top.set((*ci).top);
                            luaV_finishset(L, upval_0, rb_4, rc_2, slot_3)?;
                            trap = (*ci).u.trap;
                        }
                        vmbreak!();
                    }
                    OP_SETTABLE => {
                        let tab = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let key = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let val = if (i & (1 as c_uint) << 0 as c_int + 7 as c_int + 8 as c_int)
                            as c_int
                            != 0
                        {
                            k.offset(
                                (i >> 0 as c_int
                                    + 7 as c_int
                                    + 8 as c_int
                                    + 1 as c_int
                                    + 8 as c_int
                                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                    as c_int as isize,
                            )
                        } else {
                            &raw mut (*base.offset(
                                (i >> 0 as c_int
                                    + 7 as c_int
                                    + 8 as c_int
                                    + 1 as c_int
                                    + 8 as c_int
                                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                    as c_int as isize,
                            ))
                            .val
                        };
                        let slot = match (*tab).val.tt_ == 5 | 0 << 4 | 1 << 6 {
                            true => {
                                if (*key).tt_ == 3 | 0 << 4 {
                                    luaH_getint((*tab).val.value_.gc.cast(), (*key).value_.i)
                                } else {
                                    luaH_get((*tab).val.value_.gc.cast(), key)
                                }
                            }
                            false => null(),
                        };

                        if !slot.is_null() && (*slot).tt_ & 0xf != 0 {
                            let slot = slot.cast_mut();

                            (*slot).tt_ = (*val).tt_;
                            (*slot).value_ = (*val).value_;

                            if (*val).tt_ as c_int & (1 as c_int) << 6 as c_int != 0 {
                                if (*(*tab).val.value_.gc).marked.get() as c_int
                                    & (1 as c_int) << 5 as c_int
                                    != 0
                                    && (*(*val).value_.gc).marked.get() as c_int
                                        & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                                        != 0
                                {
                                    (*L).hdr.global().gc.barrier_back((*tab).val.value_.gc);
                                }
                            }
                        } else {
                            (*ci).u.savedpc = pc;
                            (*L).top.set((*ci).top);
                            luaV_finishset(L, &raw mut (*tab).val, key, val, slot)?;
                            trap = (*ci).u.trap;
                        }
                        vmbreak!();
                    }
                    17 => {
                        let ra_15 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let mut slot_5 = null();
                        let c_0: c_int = (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int;
                        let rc_4 = if (i & (1 as c_uint) << 0 as c_int + 7 as c_int + 8 as c_int)
                            as c_int
                            != 0
                        {
                            k.offset(
                                (i >> 0 as c_int
                                    + 7 as c_int
                                    + 8 as c_int
                                    + 1 as c_int
                                    + 8 as c_int
                                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                    as c_int as isize,
                            )
                        } else {
                            &mut (*base.offset(
                                (i >> 0 as c_int
                                    + 7 as c_int
                                    + 8 as c_int
                                    + 1 as c_int
                                    + 8 as c_int
                                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                    as c_int as isize,
                            ))
                            .val
                        };
                        if if !((*ra_15).val.tt_ as c_int
                            == 5 as c_int | (0 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int)
                        {
                            slot_5 = null();
                            0 as c_int
                        } else {
                            slot_5 = if (c_0 as u64).wrapping_sub(1 as c_uint as u64)
                                < (*((*ra_15).val.value_.gc as *mut Table<D>)).alimit.get() as u64
                            {
                                (*((*ra_15).val.value_.gc as *mut Table<D>))
                                    .array
                                    .get()
                                    .offset((c_0 - 1 as c_int) as isize)
                            } else {
                                luaH_getint((*ra_15).val.value_.gc.cast(), c_0 as i64)
                            };
                            !((*slot_5).tt_ as c_int & 0xf as c_int == 0 as c_int) as c_int
                        } != 0
                        {
                            let io1_10 = slot_5.cast_mut();
                            let io2_10 = rc_4;

                            (*io1_10).value_ = (*io2_10).value_;
                            (*io1_10).tt_ = (*io2_10).tt_;
                            if (*rc_4).tt_ as c_int & (1 as c_int) << 6 as c_int != 0 {
                                if (*(*ra_15).val.value_.gc).marked.get() as c_int
                                    & (1 as c_int) << 5 as c_int
                                    != 0
                                    && (*(*rc_4).value_.gc).marked.get() as c_int
                                        & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                                        != 0
                                {
                                    (*L).hdr.global().gc.barrier_back((*ra_15).val.value_.gc);
                                }
                            }
                        } else {
                            let mut key_3 = UnsafeValue::default();
                            let io_2 = &raw mut key_3;

                            (*io_2).value_.i = c_0 as i64;
                            (*io_2).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                            (*ci).u.savedpc = pc;
                            (*L).top.set((*ci).top);
                            luaV_finishset(L, &mut (*ra_15).val, &mut key_3, rc_4, slot_5)?;
                            trap = (*ci).u.trap;
                        }
                        vmbreak!();
                    }
                    18 => {
                        let ra_16 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let mut slot_6 = null();
                        let rb_6 = k.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let rc_5 = if (i & (1 as c_uint) << 0 as c_int + 7 as c_int + 8 as c_int)
                            as c_int
                            != 0
                        {
                            k.offset(
                                (i >> 0 as c_int
                                    + 7 as c_int
                                    + 8 as c_int
                                    + 1 as c_int
                                    + 8 as c_int
                                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                    as c_int as isize,
                            )
                        } else {
                            &mut (*base.offset(
                                (i >> 0 as c_int
                                    + 7 as c_int
                                    + 8 as c_int
                                    + 1 as c_int
                                    + 8 as c_int
                                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                    as c_int as isize,
                            ))
                            .val
                        };
                        let key_4 = (*rb_6).value_.gc as *mut Str<D>;

                        if if !((*ra_16).val.tt_ as c_int
                            == 5 as c_int | (0 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int)
                        {
                            slot_6 = null();
                            0 as c_int
                        } else {
                            slot_6 = luaH_getshortstr((*ra_16).val.value_.gc.cast(), key_4);
                            !((*slot_6).tt_ as c_int & 0xf as c_int == 0 as c_int) as c_int
                        } != 0
                        {
                            let io1_11 = slot_6.cast_mut();
                            let io2_11 = rc_5;

                            (*io1_11).value_ = (*io2_11).value_;
                            (*io1_11).tt_ = (*io2_11).tt_;
                            if (*rc_5).tt_ as c_int & (1 as c_int) << 6 as c_int != 0 {
                                if (*(*ra_16).val.value_.gc).marked.get() as c_int
                                    & (1 as c_int) << 5 as c_int
                                    != 0
                                    && (*(*rc_5).value_.gc).marked.get() as c_int
                                        & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                                        != 0
                                {
                                    (*L).hdr.global().gc.barrier_back((*ra_16).val.value_.gc);
                                }
                            }
                        } else {
                            (*ci).u.savedpc = pc;
                            (*L).top.set((*ci).top);
                            luaV_finishset(L, &mut (*ra_16).val, rb_6, rc_5, slot_6)?;
                            trap = (*ci).u.trap;
                        }
                        vmbreak!();
                    }
                    OP_NEWTABLE => {
                        let ra_17 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let mut b_3: c_int = (i
                            >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int;
                        let mut c_1: c_int = (i
                            >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int;

                        if b_3 > 0 as c_int {
                            b_3 = (1 as c_int) << b_3 - 1 as c_int;
                        }
                        if (i & (1 as c_uint) << 0 as c_int + 7 as c_int + 8 as c_int) as c_int != 0
                        {
                            c_1 += (*pc >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32)
                                    << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                    << 0 as c_int) as c_int
                                * (((1 as c_int) << 8 as c_int) - 1 as c_int + 1 as c_int);
                        }
                        pc = pc.offset(1);
                        (*L).top.set(ra_17.offset(1 as c_int as isize));

                        // Create table.
                        let t = Table::new((*L).hdr.global);
                        let io_3 = &raw mut (*ra_17).val;

                        (*io_3).value_.gc = t.cast();
                        (*io_3).tt_ = 5 | 0 << 4 | 1 << 6;

                        if b_3 != 0 as c_int || c_1 != 0 as c_int {
                            luaH_resize(t, c_1 as c_uint, b_3 as c_uint);
                        }

                        (*ci).u.savedpc = pc;
                        (*L).top.set(ra_17.offset(1 as c_int as isize));
                        (*L).hdr.global().gc.step();
                        trap = (*ci).u.trap;

                        vmbreak!();
                    }
                    20 => {
                        let ra_18 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        tab = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        key = if (i & (1 as c_uint) << 0 as c_int + 7 as c_int + 8 as c_int)
                            as c_int
                            != 0
                        {
                            k.offset(
                                (i >> 0 as c_int
                                    + 7 as c_int
                                    + 8 as c_int
                                    + 1 as c_int
                                    + 8 as c_int
                                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                    as c_int as isize,
                            )
                        } else {
                            &mut (*base.offset(
                                (i >> 0 as c_int
                                    + 7 as c_int
                                    + 8 as c_int
                                    + 1 as c_int
                                    + 8 as c_int
                                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                    as c_int as isize,
                            ))
                            .val
                        };
                        let key_5 = (*key).value_.gc as *mut Str<D>;
                        let io1_12 = &raw mut (*ra_18.offset(1 as c_int as isize)).val;
                        let io2_12 = tab;

                        (*io1_12).value_ = (*io2_12).value_;
                        (*io1_12).tt_ = (*io2_12).tt_;
                        if if !((*tab).tt_ as c_int
                            == 5 as c_int | (0 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int)
                        {
                            slot = null();
                            0 as c_int
                        } else {
                            slot = luaH_getstr((*tab).value_.gc.cast(), key_5);
                            !((*slot).tt_ as c_int & 0xf as c_int == 0 as c_int) as c_int
                        } != 0
                        {
                            let io1_13 = &raw mut (*ra_18).val;
                            let io2_13 = slot;

                            (*io1_13).value_ = (*io2_13).value_;
                            (*io1_13).tt_ = (*io2_13).tt_;
                            vmbreak!();
                        } else {
                            current_block = 0;
                        }
                    }
                    OP_ADDI => {
                        let ra_19 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let v1 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let imm: c_int = (i
                            >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int
                            - (((1 as c_int) << 8 as c_int) - 1 as c_int >> 1 as c_int);
                        let tt = (*v1).tt_;

                        if tt & 0xf != 3 {
                            vmbreak!();
                        }

                        (*ra_19).val.tt_ = tt;

                        if tt as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                            let iv1: i64 = (*v1).value_.i;
                            let io_4 = &raw mut (*ra_19).val;

                            (*io_4).value_.i = (iv1 as u64).wrapping_add(imm as u64) as i64;
                        } else if tt as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                            let nb: f64 = (*v1).value_.n;
                            let fimm: f64 = imm as f64;
                            let io_5 = &raw mut (*ra_19).val;

                            (*io_5).value_.n = nb + fimm;
                        } else {
                            unreachable_unchecked();
                        }

                        pc = pc.offset(1);
                        vmbreak!();
                    }
                    22 => {
                        let v1_0 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let v2 = k.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let ra_20 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        if (*v1_0).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                            && (*v2).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                        {
                            let i1: i64 = (*v1_0).value_.i;
                            let i2: i64 = (*v2).value_.i;
                            pc = pc.offset(1);
                            let io_6 = &raw mut (*ra_20).val;

                            (*io_6).value_.i = (i1 as u64).wrapping_add(i2 as u64) as i64;
                            (*io_6).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                        } else {
                            let mut n1: f64 = 0.;
                            let mut n2: f64 = 0.;
                            if (if (*v1_0).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                                n1 = (*v1_0).value_.n;
                                1 as c_int
                            } else {
                                if (*v1_0).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                                    n1 = (*v1_0).value_.i as f64;
                                    1 as c_int
                                } else {
                                    0 as c_int
                                }
                            }) != 0
                                && (if (*v2).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int
                                {
                                    n2 = (*v2).value_.n;
                                    1 as c_int
                                } else {
                                    if (*v2).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                                    {
                                        n2 = (*v2).value_.i as f64;
                                        1 as c_int
                                    } else {
                                        0 as c_int
                                    }
                                }) != 0
                            {
                                pc = pc.offset(1);
                                let io_7 = &raw mut (*ra_20).val;

                                (*io_7).value_.n = n1 + n2;
                                (*io_7).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                            }
                        }
                        vmbreak!();
                    }
                    23 => {
                        let v1_1 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let v2_0 = k.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let ra_21 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        if (*v1_1).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                            && (*v2_0).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                        {
                            let i1_0: i64 = (*v1_1).value_.i;
                            let i2_0: i64 = (*v2_0).value_.i;
                            pc = pc.offset(1);
                            let io_8 = &raw mut (*ra_21).val;

                            (*io_8).value_.i = (i1_0 as u64).wrapping_sub(i2_0 as u64) as i64;
                            (*io_8).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                        } else {
                            let mut n1_0: f64 = 0.;
                            let mut n2_0: f64 = 0.;
                            if (if (*v1_1).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                                n1_0 = (*v1_1).value_.n;
                                1 as c_int
                            } else {
                                if (*v1_1).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                                    n1_0 = (*v1_1).value_.i as f64;
                                    1 as c_int
                                } else {
                                    0 as c_int
                                }
                            }) != 0
                                && (if (*v2_0).tt_ as c_int
                                    == 3 as c_int | (1 as c_int) << 4 as c_int
                                {
                                    n2_0 = (*v2_0).value_.n;
                                    1 as c_int
                                } else {
                                    if (*v2_0).tt_ as c_int
                                        == 3 as c_int | (0 as c_int) << 4 as c_int
                                    {
                                        n2_0 = (*v2_0).value_.i as f64;
                                        1 as c_int
                                    } else {
                                        0 as c_int
                                    }
                                }) != 0
                            {
                                pc = pc.offset(1);
                                let io_9 = &raw mut (*ra_21).val;

                                (*io_9).value_.n = n1_0 - n2_0;
                                (*io_9).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                            }
                        }
                        vmbreak!();
                    }
                    24 => {
                        let v1_2 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let v2_1 = k.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let ra_22 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        if (*v1_2).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                            && (*v2_1).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                        {
                            let i1_1: i64 = (*v1_2).value_.i;
                            let i2_1: i64 = (*v2_1).value_.i;
                            pc = pc.offset(1);
                            let io_10 = &raw mut (*ra_22).val;

                            (*io_10).value_.i = (i1_1 as u64 * i2_1 as u64) as i64;
                            (*io_10).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                        } else {
                            let mut n1_1: f64 = 0.;
                            let mut n2_1: f64 = 0.;
                            if (if (*v1_2).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                                n1_1 = (*v1_2).value_.n;
                                1 as c_int
                            } else {
                                if (*v1_2).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                                    n1_1 = (*v1_2).value_.i as f64;
                                    1 as c_int
                                } else {
                                    0 as c_int
                                }
                            }) != 0
                                && (if (*v2_1).tt_ as c_int
                                    == 3 as c_int | (1 as c_int) << 4 as c_int
                                {
                                    n2_1 = (*v2_1).value_.n;
                                    1 as c_int
                                } else {
                                    if (*v2_1).tt_ as c_int
                                        == 3 as c_int | (0 as c_int) << 4 as c_int
                                    {
                                        n2_1 = (*v2_1).value_.i as f64;
                                        1 as c_int
                                    } else {
                                        0 as c_int
                                    }
                                }) != 0
                            {
                                pc = pc.offset(1);
                                let io_11 = &raw mut (*ra_22).val;

                                (*io_11).value_.n = n1_1 * n2_1;
                                (*io_11).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                            }
                        }
                        vmbreak!();
                    }
                    25 => {
                        (*ci).u.savedpc = pc;
                        (*L).top.set((*ci).top);

                        let v1_3 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let v2_2 = k.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let ra_23 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        if (*v1_3).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                            && (*v2_2).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                        {
                            let i1_2: i64 = (*v1_3).value_.i;
                            let i2_2: i64 = (*v2_2).value_.i;
                            pc = pc.offset(1);
                            let io_12 = &raw mut (*ra_23).val;

                            (*io_12).value_.i = match luaV_mod(i1_2, i2_2) {
                                Some(v) => v,
                                None => return Err(luaG_runerror(L, ArithError::ModZero)),
                            };
                            (*io_12).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                        } else {
                            let mut n1_2: f64 = 0.;
                            let mut n2_2: f64 = 0.;
                            if (if (*v1_3).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                                n1_2 = (*v1_3).value_.n;
                                1 as c_int
                            } else {
                                if (*v1_3).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                                    n1_2 = (*v1_3).value_.i as f64;
                                    1 as c_int
                                } else {
                                    0 as c_int
                                }
                            }) != 0
                                && (if (*v2_2).tt_ as c_int
                                    == 3 as c_int | (1 as c_int) << 4 as c_int
                                {
                                    n2_2 = (*v2_2).value_.n;
                                    1 as c_int
                                } else {
                                    if (*v2_2).tt_ as c_int
                                        == 3 as c_int | (0 as c_int) << 4 as c_int
                                    {
                                        n2_2 = (*v2_2).value_.i as f64;
                                        1 as c_int
                                    } else {
                                        0 as c_int
                                    }
                                }) != 0
                            {
                                pc = pc.offset(1);
                                let io_13 = &raw mut (*ra_23).val;

                                (*io_13).value_.n = luaV_modf(n1_2, n2_2);
                                (*io_13).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                            }
                        }
                        vmbreak!();
                    }
                    26 => {
                        let ra_24 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let v1_4 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let v2_3 = k.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let mut n1_3: f64 = 0.;
                        let mut n2_3: f64 = 0.;
                        if (if (*v1_4).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                            n1_3 = (*v1_4).value_.n;
                            1 as c_int
                        } else {
                            if (*v1_4).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                                n1_3 = (*v1_4).value_.i as f64;
                                1 as c_int
                            } else {
                                0 as c_int
                            }
                        }) != 0
                            && (if (*v2_3).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                                n2_3 = (*v2_3).value_.n;
                                1 as c_int
                            } else {
                                if (*v2_3).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                                    n2_3 = (*v2_3).value_.i as f64;
                                    1 as c_int
                                } else {
                                    0 as c_int
                                }
                            }) != 0
                        {
                            pc = pc.offset(1);
                            let io_14 = &raw mut (*ra_24).val;
                            (*io_14).value_.n = if n2_3 == 2 as c_int as f64 {
                                n1_3 * n1_3
                            } else {
                                pow(n1_3, n2_3)
                            };
                            (*io_14).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                        }
                        vmbreak!();
                    }
                    27 => {
                        let ra_25 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let v1_5 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let v2_4 = k.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let mut n1_4: f64 = 0.;
                        let mut n2_4: f64 = 0.;
                        if (if (*v1_5).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                            n1_4 = (*v1_5).value_.n;
                            1 as c_int
                        } else {
                            if (*v1_5).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                                n1_4 = (*v1_5).value_.i as f64;
                                1 as c_int
                            } else {
                                0 as c_int
                            }
                        }) != 0
                            && (if (*v2_4).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                                n2_4 = (*v2_4).value_.n;
                                1 as c_int
                            } else {
                                if (*v2_4).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                                    n2_4 = (*v2_4).value_.i as f64;
                                    1 as c_int
                                } else {
                                    0 as c_int
                                }
                            }) != 0
                        {
                            pc = pc.offset(1);
                            let io_15 = &raw mut (*ra_25).val;

                            (*io_15).value_.n = n1_4 / n2_4;
                            (*io_15).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                        }
                        vmbreak!();
                    }
                    28 => {
                        (*ci).u.savedpc = pc;
                        (*L).top.set((*ci).top);
                        let v1_6 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let v2_5 = k.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let ra_26 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        if (*v1_6).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                            && (*v2_5).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                        {
                            let i1_3: i64 = (*v1_6).value_.i;
                            let i2_3: i64 = (*v2_5).value_.i;
                            pc = pc.offset(1);
                            let io_16 = &raw mut (*ra_26).val;

                            (*io_16).value_.i = match luaV_idiv(i1_3, i2_3) {
                                Some(v) => v,
                                None => return Err(luaG_runerror(L, ArithError::DivZero)),
                            };
                            (*io_16).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                        } else {
                            let mut n1_5: f64 = 0.;
                            let mut n2_5: f64 = 0.;
                            if (if (*v1_6).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                                n1_5 = (*v1_6).value_.n;
                                1 as c_int
                            } else {
                                if (*v1_6).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                                    n1_5 = (*v1_6).value_.i as f64;
                                    1 as c_int
                                } else {
                                    0 as c_int
                                }
                            }) != 0
                                && (if (*v2_5).tt_ as c_int
                                    == 3 as c_int | (1 as c_int) << 4 as c_int
                                {
                                    n2_5 = (*v2_5).value_.n;
                                    1 as c_int
                                } else {
                                    if (*v2_5).tt_ as c_int
                                        == 3 as c_int | (0 as c_int) << 4 as c_int
                                    {
                                        n2_5 = (*v2_5).value_.i as f64;
                                        1 as c_int
                                    } else {
                                        0 as c_int
                                    }
                                }) != 0
                            {
                                pc = pc.offset(1);
                                let io_17 = &raw mut (*ra_26).val;
                                (*io_17).value_.n = floor(n1_5 / n2_5);
                                (*io_17).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                            }
                        }
                        vmbreak!();
                    }
                    29 => {
                        let ra_27 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let v1_7 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let v2_6 = k.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let mut i1_4: i64 = 0;
                        let i2_4: i64 = (*v2_6).value_.i;
                        if if (((*v1_7).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int)
                            as c_int
                            != 0 as c_int) as c_int as c_long
                            != 0
                        {
                            i1_4 = (*v1_7).value_.i;
                            1 as c_int
                        } else {
                            luaV_tointegerns(v1_7, &mut i1_4, F2Ieq)
                        } != 0
                        {
                            pc = pc.offset(1);
                            let io_18 = &raw mut (*ra_27).val;
                            (*io_18).value_.i = (i1_4 as u64 & i2_4 as u64) as i64;
                            (*io_18).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                        }
                        vmbreak!();
                    }
                    30 => {
                        let ra_28 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let v1_8 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let v2_7 = k.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let mut i1_5: i64 = 0;
                        let i2_5: i64 = (*v2_7).value_.i;
                        if if (((*v1_8).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int)
                            as c_int
                            != 0 as c_int) as c_int as c_long
                            != 0
                        {
                            i1_5 = (*v1_8).value_.i;
                            1 as c_int
                        } else {
                            luaV_tointegerns(v1_8, &mut i1_5, F2Ieq)
                        } != 0
                        {
                            pc = pc.offset(1);
                            let io_19 = &raw mut (*ra_28).val;

                            (*io_19).value_.i = (i1_5 as u64 | i2_5 as u64) as i64;
                            (*io_19).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                        }
                        vmbreak!();
                    }
                    31 => {
                        let ra_29 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let v1_9 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let v2_8 = k.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let mut i1_6: i64 = 0;
                        let i2_6: i64 = (*v2_8).value_.i;
                        if if (((*v1_9).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int)
                            as c_int
                            != 0 as c_int) as c_int as c_long
                            != 0
                        {
                            i1_6 = (*v1_9).value_.i;
                            1 as c_int
                        } else {
                            luaV_tointegerns(v1_9, &mut i1_6, F2Ieq)
                        } != 0
                        {
                            pc = pc.offset(1);
                            let io_20 = &raw mut (*ra_29).val;
                            (*io_20).value_.i = (i1_6 as u64 ^ i2_6 as u64) as i64;
                            (*io_20).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                        }
                        vmbreak!();
                    }
                    32 => {
                        let ra_30 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let rb_8 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let ic: c_int = (i
                            >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int
                            - (((1 as c_int) << 8 as c_int) - 1 as c_int >> 1 as c_int);
                        let mut ib: i64 = 0;
                        if if (((*rb_8).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int)
                            as c_int
                            != 0 as c_int) as c_int as c_long
                            != 0
                        {
                            ib = (*rb_8).value_.i;
                            1 as c_int
                        } else {
                            luaV_tointegerns(rb_8, &mut ib, F2Ieq)
                        } != 0
                        {
                            pc = pc.offset(1);
                            let io_21 = &raw mut (*ra_30).val;

                            (*io_21).value_.i = luaV_shiftl(ib, -ic as i64);
                            (*io_21).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                        }
                        vmbreak!();
                    }
                    33 => {
                        let ra_31 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let rb_9 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let ic_0: c_int = (i
                            >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int
                            - (((1 as c_int) << 8 as c_int) - 1 as c_int >> 1 as c_int);
                        let mut ib_0: i64 = 0;
                        if if (((*rb_9).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int)
                            as c_int
                            != 0 as c_int) as c_int as c_long
                            != 0
                        {
                            ib_0 = (*rb_9).value_.i;
                            1 as c_int
                        } else {
                            luaV_tointegerns(rb_9, &mut ib_0, F2Ieq)
                        } != 0
                        {
                            pc = pc.offset(1);
                            let io_22 = &raw mut (*ra_31).val;
                            (*io_22).value_.i = luaV_shiftl(ic_0 as i64, ib_0);
                            (*io_22).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                        }
                        vmbreak!();
                    }
                    34 => {
                        let v1_10 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let v2_9 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let ra_32 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        if (*v1_10).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                            && (*v2_9).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                        {
                            let i1_7: i64 = (*v1_10).value_.i;
                            let i2_7: i64 = (*v2_9).value_.i;
                            pc = pc.offset(1);
                            let io_23 = &raw mut (*ra_32).val;
                            (*io_23).value_.i = (i1_7 as u64).wrapping_add(i2_7 as u64) as i64;
                            (*io_23).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                        } else {
                            let mut n1_6: f64 = 0.;
                            let mut n2_6: f64 = 0.;
                            if (if (*v1_10).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int
                            {
                                n1_6 = (*v1_10).value_.n;
                                1 as c_int
                            } else {
                                if (*v1_10).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                                {
                                    n1_6 = (*v1_10).value_.i as f64;
                                    1 as c_int
                                } else {
                                    0 as c_int
                                }
                            }) != 0
                                && (if (*v2_9).tt_ as c_int
                                    == 3 as c_int | (1 as c_int) << 4 as c_int
                                {
                                    n2_6 = (*v2_9).value_.n;
                                    1 as c_int
                                } else {
                                    if (*v2_9).tt_ as c_int
                                        == 3 as c_int | (0 as c_int) << 4 as c_int
                                    {
                                        n2_6 = (*v2_9).value_.i as f64;
                                        1 as c_int
                                    } else {
                                        0 as c_int
                                    }
                                }) != 0
                            {
                                pc = pc.offset(1);
                                let io_24 = &raw mut (*ra_32).val;
                                (*io_24).value_.n = n1_6 + n2_6;
                                (*io_24).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                            }
                        }
                        vmbreak!();
                    }
                    35 => {
                        let v1_11 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let v2_10 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let ra_33 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        if (*v1_11).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                            && (*v2_10).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                        {
                            let i1_8: i64 = (*v1_11).value_.i;
                            let i2_8: i64 = (*v2_10).value_.i;
                            pc = pc.offset(1);
                            let io_25 = &raw mut (*ra_33).val;
                            (*io_25).value_.i = (i1_8 as u64).wrapping_sub(i2_8 as u64) as i64;
                            (*io_25).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                        } else {
                            let mut n1_7: f64 = 0.;
                            let mut n2_7: f64 = 0.;
                            if (if (*v1_11).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int
                            {
                                n1_7 = (*v1_11).value_.n;
                                1 as c_int
                            } else {
                                if (*v1_11).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                                {
                                    n1_7 = (*v1_11).value_.i as f64;
                                    1 as c_int
                                } else {
                                    0 as c_int
                                }
                            }) != 0
                                && (if (*v2_10).tt_ as c_int
                                    == 3 as c_int | (1 as c_int) << 4 as c_int
                                {
                                    n2_7 = (*v2_10).value_.n;
                                    1 as c_int
                                } else {
                                    if (*v2_10).tt_ as c_int
                                        == 3 as c_int | (0 as c_int) << 4 as c_int
                                    {
                                        n2_7 = (*v2_10).value_.i as f64;
                                        1 as c_int
                                    } else {
                                        0 as c_int
                                    }
                                }) != 0
                            {
                                pc = pc.offset(1);
                                let io_26 = &raw mut (*ra_33).val;
                                (*io_26).value_.n = n1_7 - n2_7;
                                (*io_26).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                            }
                        }
                        vmbreak!();
                    }
                    36 => {
                        let v1_12 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let v2_11 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let ra_34 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        if (*v1_12).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                            && (*v2_11).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                        {
                            let i1_9: i64 = (*v1_12).value_.i;
                            let i2_9: i64 = (*v2_11).value_.i;
                            pc = pc.offset(1);
                            let io_27 = &raw mut (*ra_34).val;
                            (*io_27).value_.i = ((i1_9 as u64).wrapping_mul(i2_9 as u64)) as i64;
                            (*io_27).tt_ = 3 | 0 << 4;
                        } else {
                            let mut n1_8: f64 = 0.;
                            let mut n2_8: f64 = 0.;
                            if (if (*v1_12).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int
                            {
                                n1_8 = (*v1_12).value_.n;
                                1 as c_int
                            } else {
                                if (*v1_12).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                                {
                                    n1_8 = (*v1_12).value_.i as f64;
                                    1 as c_int
                                } else {
                                    0 as c_int
                                }
                            }) != 0
                                && (if (*v2_11).tt_ as c_int
                                    == 3 as c_int | (1 as c_int) << 4 as c_int
                                {
                                    n2_8 = (*v2_11).value_.n;
                                    1 as c_int
                                } else {
                                    if (*v2_11).tt_ as c_int
                                        == 3 as c_int | (0 as c_int) << 4 as c_int
                                    {
                                        n2_8 = (*v2_11).value_.i as f64;
                                        1 as c_int
                                    } else {
                                        0 as c_int
                                    }
                                }) != 0
                            {
                                pc = pc.offset(1);
                                let io_28 = &raw mut (*ra_34).val;
                                (*io_28).value_.n = n1_8 * n2_8;
                                (*io_28).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                            }
                        }
                        vmbreak!();
                    }
                    37 => {
                        (*ci).u.savedpc = pc;
                        (*L).top.set((*ci).top);
                        let v1_13 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let v2_12 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let ra_35 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        if (*v1_13).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                            && (*v2_12).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                        {
                            let i1_10: i64 = (*v1_13).value_.i;
                            let i2_10: i64 = (*v2_12).value_.i;
                            pc = pc.offset(1);
                            let io_29 = &raw mut (*ra_35).val;
                            (*io_29).value_.i = match luaV_mod(i1_10, i2_10) {
                                Some(v) => v,
                                None => return Err(luaG_runerror(L, ArithError::ModZero)),
                            };
                            (*io_29).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                        } else {
                            let mut n1_9: f64 = 0.;
                            let mut n2_9: f64 = 0.;
                            if (if (*v1_13).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int
                            {
                                n1_9 = (*v1_13).value_.n;
                                1 as c_int
                            } else {
                                if (*v1_13).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                                {
                                    n1_9 = (*v1_13).value_.i as f64;
                                    1 as c_int
                                } else {
                                    0 as c_int
                                }
                            }) != 0
                                && (if (*v2_12).tt_ as c_int
                                    == 3 as c_int | (1 as c_int) << 4 as c_int
                                {
                                    n2_9 = (*v2_12).value_.n;
                                    1 as c_int
                                } else {
                                    if (*v2_12).tt_ as c_int
                                        == 3 as c_int | (0 as c_int) << 4 as c_int
                                    {
                                        n2_9 = (*v2_12).value_.i as f64;
                                        1 as c_int
                                    } else {
                                        0 as c_int
                                    }
                                }) != 0
                            {
                                pc = pc.offset(1);
                                let io_30 = &raw mut (*ra_35).val;
                                (*io_30).value_.n = luaV_modf(n1_9, n2_9);
                                (*io_30).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                            }
                        }
                        vmbreak!();
                    }
                    38 => {
                        let ra_36 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let v1_14 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let v2_13 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let mut n1_10: f64 = 0.;
                        let mut n2_10: f64 = 0.;
                        if (if (*v1_14).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                            n1_10 = (*v1_14).value_.n;
                            1 as c_int
                        } else {
                            if (*v1_14).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                                n1_10 = (*v1_14).value_.i as f64;
                                1 as c_int
                            } else {
                                0 as c_int
                            }
                        }) != 0
                            && (if (*v2_13).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int
                            {
                                n2_10 = (*v2_13).value_.n;
                                1 as c_int
                            } else {
                                if (*v2_13).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                                {
                                    n2_10 = (*v2_13).value_.i as f64;
                                    1 as c_int
                                } else {
                                    0 as c_int
                                }
                            }) != 0
                        {
                            pc = pc.offset(1);
                            let io_31 = &raw mut (*ra_36).val;
                            (*io_31).value_.n = if n2_10 == 2 as c_int as f64 {
                                n1_10 * n1_10
                            } else {
                                pow(n1_10, n2_10)
                            };
                            (*io_31).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                        }
                        vmbreak!();
                    }
                    39 => {
                        let ra_37 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let v1_15 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let v2_14 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let mut n1_11: f64 = 0.;
                        let mut n2_11: f64 = 0.;
                        if (if (*v1_15).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                            n1_11 = (*v1_15).value_.n;
                            1 as c_int
                        } else {
                            if (*v1_15).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                                n1_11 = (*v1_15).value_.i as f64;
                                1 as c_int
                            } else {
                                0 as c_int
                            }
                        }) != 0
                            && (if (*v2_14).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int
                            {
                                n2_11 = (*v2_14).value_.n;
                                1 as c_int
                            } else {
                                if (*v2_14).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                                {
                                    n2_11 = (*v2_14).value_.i as f64;
                                    1 as c_int
                                } else {
                                    0 as c_int
                                }
                            }) != 0
                        {
                            pc = pc.offset(1);
                            let io_32 = &raw mut (*ra_37).val;
                            (*io_32).value_.n = n1_11 / n2_11;
                            (*io_32).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                        }
                        vmbreak!();
                    }
                    40 => {
                        (*ci).u.savedpc = pc;
                        (*L).top.set((*ci).top);
                        let v1_16 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let v2_15 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let ra_38 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        if (*v1_16).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                            && (*v2_15).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                        {
                            let i1_11: i64 = (*v1_16).value_.i;
                            let i2_11: i64 = (*v2_15).value_.i;
                            pc = pc.offset(1);
                            let io_33 = &raw mut (*ra_38).val;
                            (*io_33).value_.i = match luaV_idiv(i1_11, i2_11) {
                                Some(v) => v,
                                None => return Err(luaG_runerror(L, ArithError::DivZero)),
                            };
                            (*io_33).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                        } else {
                            let mut n1_12: f64 = 0.;
                            let mut n2_12: f64 = 0.;
                            if (if (*v1_16).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int
                            {
                                n1_12 = (*v1_16).value_.n;
                                1 as c_int
                            } else {
                                if (*v1_16).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                                {
                                    n1_12 = (*v1_16).value_.i as f64;
                                    1 as c_int
                                } else {
                                    0 as c_int
                                }
                            }) != 0
                                && (if (*v2_15).tt_ as c_int
                                    == 3 as c_int | (1 as c_int) << 4 as c_int
                                {
                                    n2_12 = (*v2_15).value_.n;
                                    1 as c_int
                                } else {
                                    if (*v2_15).tt_ as c_int
                                        == 3 as c_int | (0 as c_int) << 4 as c_int
                                    {
                                        n2_12 = (*v2_15).value_.i as f64;
                                        1 as c_int
                                    } else {
                                        0 as c_int
                                    }
                                }) != 0
                            {
                                pc = pc.offset(1);
                                let io_34 = &raw mut (*ra_38).val;
                                (*io_34).value_.n = floor(n1_12 / n2_12);
                                (*io_34).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                            }
                        }
                        vmbreak!();
                    }
                    41 => {
                        let ra_39 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let v1_17 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let v2_16 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let mut i1_12: i64 = 0;
                        let mut i2_12: i64 = 0;
                        if (if (((*v1_17).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int)
                            as c_int
                            != 0 as c_int) as c_int as c_long
                            != 0
                        {
                            i1_12 = (*v1_17).value_.i;
                            1 as c_int
                        } else {
                            luaV_tointegerns(v1_17, &mut i1_12, F2Ieq)
                        }) != 0
                            && (if (((*v2_16).tt_ as c_int
                                == 3 as c_int | (0 as c_int) << 4 as c_int)
                                as c_int
                                != 0 as c_int) as c_int as c_long
                                != 0
                            {
                                i2_12 = (*v2_16).value_.i;
                                1 as c_int
                            } else {
                                luaV_tointegerns(v2_16, &mut i2_12, F2Ieq)
                            }) != 0
                        {
                            pc = pc.offset(1);
                            let io_35 = &raw mut (*ra_39).val;
                            (*io_35).value_.i = (i1_12 as u64 & i2_12 as u64) as i64;
                            (*io_35).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                        }
                        vmbreak!();
                    }
                    42 => {
                        let ra_40 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let v1_18 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let v2_17 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let mut i1_13: i64 = 0;
                        let mut i2_13: i64 = 0;
                        if (if (((*v1_18).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int)
                            as c_int
                            != 0 as c_int) as c_int as c_long
                            != 0
                        {
                            i1_13 = (*v1_18).value_.i;
                            1 as c_int
                        } else {
                            luaV_tointegerns(v1_18, &mut i1_13, F2Ieq)
                        }) != 0
                            && (if (((*v2_17).tt_ as c_int
                                == 3 as c_int | (0 as c_int) << 4 as c_int)
                                as c_int
                                != 0 as c_int) as c_int as c_long
                                != 0
                            {
                                i2_13 = (*v2_17).value_.i;
                                1 as c_int
                            } else {
                                luaV_tointegerns(v2_17, &mut i2_13, F2Ieq)
                            }) != 0
                        {
                            pc = pc.offset(1);
                            let io_36 = &raw mut (*ra_40).val;
                            (*io_36).value_.i = (i1_13 as u64 | i2_13 as u64) as i64;
                            (*io_36).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                        }
                        vmbreak!();
                    }
                    43 => {
                        let ra_41 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let v1_19 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let v2_18 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let mut i1_14: i64 = 0;
                        let mut i2_14: i64 = 0;
                        if (if (((*v1_19).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int)
                            as c_int
                            != 0 as c_int) as c_int as c_long
                            != 0
                        {
                            i1_14 = (*v1_19).value_.i;
                            1 as c_int
                        } else {
                            luaV_tointegerns(v1_19, &mut i1_14, F2Ieq)
                        }) != 0
                            && (if (((*v2_18).tt_ as c_int
                                == 3 as c_int | (0 as c_int) << 4 as c_int)
                                as c_int
                                != 0 as c_int) as c_int as c_long
                                != 0
                            {
                                i2_14 = (*v2_18).value_.i;
                                1 as c_int
                            } else {
                                luaV_tointegerns(v2_18, &mut i2_14, F2Ieq)
                            }) != 0
                        {
                            pc = pc.offset(1);
                            let io_37 = &raw mut (*ra_41).val;
                            (*io_37).value_.i = (i1_14 as u64 ^ i2_14 as u64) as i64;
                            (*io_37).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                        }
                        vmbreak!();
                    }
                    45 => {
                        let ra_42 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let v1_20 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let v2_19 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let mut i1_15: i64 = 0;
                        let mut i2_15: i64 = 0;
                        if (if (((*v1_20).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int)
                            as c_int
                            != 0 as c_int) as c_int as c_long
                            != 0
                        {
                            i1_15 = (*v1_20).value_.i;
                            1 as c_int
                        } else {
                            luaV_tointegerns(v1_20, &mut i1_15, F2Ieq)
                        }) != 0
                            && (if (((*v2_19).tt_ as c_int
                                == 3 as c_int | (0 as c_int) << 4 as c_int)
                                as c_int
                                != 0 as c_int) as c_int as c_long
                                != 0
                            {
                                i2_15 = (*v2_19).value_.i;
                                1 as c_int
                            } else {
                                luaV_tointegerns(v2_19, &mut i2_15, F2Ieq)
                            }) != 0
                        {
                            pc = pc.offset(1);
                            let io_38 = &raw mut (*ra_42).val;
                            (*io_38).value_.i = luaV_shiftl(
                                i1_15,
                                (0 as c_int as u64).wrapping_sub(i2_15 as u64) as i64,
                            );
                            (*io_38).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                        }
                        vmbreak!();
                    }
                    44 => {
                        let ra_43 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let v1_21 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let v2_20 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let mut i1_16: i64 = 0;
                        let mut i2_16: i64 = 0;
                        if (if (((*v1_21).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int)
                            as c_int
                            != 0 as c_int) as c_int as c_long
                            != 0
                        {
                            i1_16 = (*v1_21).value_.i;
                            1 as c_int
                        } else {
                            luaV_tointegerns(v1_21, &mut i1_16, F2Ieq)
                        }) != 0
                            && (if (((*v2_20).tt_ as c_int
                                == 3 as c_int | (0 as c_int) << 4 as c_int)
                                as c_int
                                != 0 as c_int) as c_int as c_long
                                != 0
                            {
                                i2_16 = (*v2_20).value_.i;
                                1 as c_int
                            } else {
                                luaV_tointegerns(v2_20, &mut i2_16, F2Ieq)
                            }) != 0
                        {
                            pc = pc.offset(1);
                            let io_39 = &raw mut (*ra_43).val;
                            (*io_39).value_.i = luaV_shiftl(i1_16, i2_16);
                            (*io_39).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                        }
                        vmbreak!();
                    }
                    46 => {
                        let ra_44 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let pi: u32 = *pc.offset(-(2 as c_int as isize));
                        let rb_10 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let tm: TMS = (i
                            >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as TMS;

                        (*ci).u.savedpc = pc;
                        (*L).top.set((*ci).top);

                        let val = luaT_trybinTM(L, &mut (*ra_44).val, rb_10, tm)?;

                        (*ci)
                            .func
                            .add(1)
                            .offset(
                                (pi >> 0 as c_int + 7 as c_int
                                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                    as c_int as isize,
                            )
                            .write(StackValue { val });

                        trap = (*ci).u.trap;
                        vmbreak!();
                    }
                    47 => {
                        let ra_45 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let pi_0: u32 = *pc.offset(-(2 as c_int as isize));
                        let imm_0: c_int = (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int
                            - (((1 as c_int) << 8 as c_int) - 1 as c_int >> 1 as c_int);
                        let tm_0: TMS = (i
                            >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as TMS;
                        let flip: c_int = (i >> 0 as c_int + 7 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 1 as c_int) << 0 as c_int)
                            as c_int;

                        (*ci).u.savedpc = pc;
                        (*L).top.set((*ci).top);

                        let val = luaT_trybiniTM(L, &mut (*ra_45).val, imm_0 as i64, flip, tm_0)?;

                        (*ci)
                            .func
                            .add(1)
                            .offset(
                                (pi_0 >> 0 as c_int + 7 as c_int
                                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                    as c_int as isize,
                            )
                            .write(StackValue { val });

                        trap = (*ci).u.trap;
                        vmbreak!();
                    }
                    48 => {
                        let ra_46 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let pi_1: u32 = *pc.offset(-(2 as c_int as isize));
                        let imm_1 = k.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let tm_1: TMS = (i
                            >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as TMS;
                        let flip_0: c_int = (i >> 0 as c_int + 7 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 1 as c_int) << 0 as c_int)
                            as c_int;

                        (*ci).u.savedpc = pc;
                        (*L).top.set((*ci).top);

                        let val = luaT_trybinassocTM(L, &mut (*ra_46).val, imm_1, flip_0, tm_1)?;

                        (*ci)
                            .func
                            .add(1)
                            .offset(
                                (pi_1 >> 0 as c_int + 7 as c_int
                                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                    as c_int as isize,
                            )
                            .write(StackValue { val });

                        trap = (*ci).u.trap;
                        vmbreak!();
                    }
                    49 => {
                        let ra_47 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let rb_11 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let mut nb_0: f64 = 0.;
                        if (*rb_11).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                            let ib_1: i64 = (*rb_11).value_.i;
                            let io_40 = &raw mut (*ra_47).val;
                            (*io_40).value_.i =
                                (0 as c_int as u64).wrapping_sub(ib_1 as u64) as i64;
                            (*io_40).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                        } else if if (*rb_11).tt_ as c_int
                            == 3 as c_int | (1 as c_int) << 4 as c_int
                        {
                            nb_0 = (*rb_11).value_.n;
                            1 as c_int
                        } else if (*rb_11).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                            nb_0 = (*rb_11).value_.i as f64;
                            1 as c_int
                        } else {
                            0 as c_int
                        } != 0
                        {
                            let io_41 = &raw mut (*ra_47).val;
                            (*io_41).value_.n = -nb_0;
                            (*io_41).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                        } else {
                            (*ci).u.savedpc = pc;
                            (*L).top.set((*ci).top);

                            let val = luaT_trybinTM(L, rb_11, rb_11, TM_UNM)?;

                            (*ci)
                                .func
                                .add(1)
                                .offset(
                                    (i >> 0 as c_int + 7 as c_int
                                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                        as c_int as isize,
                                )
                                .write(StackValue { val });

                            trap = (*ci).u.trap;
                        }
                        vmbreak!();
                    }
                    50 => {
                        let ra_48 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let rb_12 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        let mut ib_2: i64 = 0;
                        if if (((*rb_12).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int)
                            as c_int
                            != 0 as c_int) as c_int as c_long
                            != 0
                        {
                            ib_2 = (*rb_12).value_.i;
                            1 as c_int
                        } else {
                            luaV_tointegerns(rb_12, &mut ib_2, F2Ieq)
                        } != 0
                        {
                            let io_42 = &raw mut (*ra_48).val;
                            (*io_42).value_.i = (!(0 as c_int as u64) ^ ib_2 as u64) as i64;
                            (*io_42).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                        } else {
                            (*ci).u.savedpc = pc;
                            (*L).top.set((*ci).top);

                            let val = luaT_trybinTM(L, rb_12, rb_12, TM_BNOT)?;

                            (*ci)
                                .func
                                .add(1)
                                .offset(
                                    (i >> 0 as c_int + 7 as c_int
                                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                        as c_int as isize,
                                )
                                .write(StackValue { val });

                            trap = (*ci).u.trap;
                        }
                        vmbreak!();
                    }
                    51 => {
                        let ra_49 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let rb_13 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        if (*rb_13).tt_ as c_int == 1 as c_int | (0 as c_int) << 4 as c_int
                            || (*rb_13).tt_ as c_int & 0xf as c_int == 0 as c_int
                        {
                            (*ra_49).val.tt_ = (1 as c_int | (1 as c_int) << 4 as c_int) as u8;
                        } else {
                            (*ra_49).val.tt_ = (1 as c_int | (0 as c_int) << 4 as c_int) as u8;
                        }
                        vmbreak!();
                    }
                    OP_LEN => {
                        (*ci).u.savedpc = pc;
                        (*L).top.set((*ci).top);

                        let val = luaV_objlen(
                            L,
                            &raw mut (*base.offset(
                                (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                    as c_int as isize,
                            ))
                            .val,
                        )?;

                        (*ci)
                            .func
                            .add(1)
                            .offset(
                                (i >> 0 as c_int + 7 as c_int
                                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                    as c_int as isize,
                            )
                            .write(StackValue { val });

                        trap = (*ci).u.trap;
                        vmbreak!();
                    }
                    OP_CONCAT => {
                        let ra_51 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let n_1: c_int = (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int;

                        (*L).top.set(ra_51.offset(n_1 as isize));
                        (*ci).u.savedpc = pc;
                        luaV_concat(L, n_1)?;
                        trap = (*ci).u.trap;

                        (*ci).u.savedpc = pc;
                        (*L).hdr.global().gc.step();
                        trap = (*ci).u.trap;

                        vmbreak!();
                    }
                    54 => {
                        let ra_52 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        (*ci).u.savedpc = pc;
                        (*L).top.set((*ci).top);

                        if let Err(e) = luaF_close(L, ra_52) {
                            return Err(e); // Requires unsized coercion.
                        }

                        trap = (*ci).u.trap;
                        vmbreak!();
                    }
                    55 => {
                        let ra_53 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        (*ci).u.savedpc = pc;
                        (*L).top.set((*ci).top);
                        luaF_newtbcupval(L, ra_53)?;
                        vmbreak!();
                    }
                    56 => {
                        pc = pc.offset(
                            ((i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32)
                                    << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                    << 0 as c_int) as c_int
                                - (((1 as c_int)
                                    << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                    - 1 as c_int
                                    >> 1 as c_int)
                                + 0 as c_int) as isize,
                        );
                        trap = (*ci).u.trap;
                        vmbreak!();
                    }
                    57 => {
                        let ra_54 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let mut cond: c_int = 0;
                        let rb_14 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        (*ci).u.savedpc = pc;
                        (*L).top.set((*ci).top);
                        cond = luaV_equalobj(L, &mut (*ra_54).val, rb_14)?;
                        trap = (*ci).u.trap;
                        if cond
                            != (i >> 0 as c_int + 7 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 1 as c_int) << 0 as c_int)
                                as c_int
                        {
                            pc = pc.offset(1);
                        } else {
                            let ni: u32 = *pc;
                            pc = pc.offset(
                                ((ni >> 0 as c_int + 7 as c_int
                                    & !(!(0 as c_int as u32)
                                        << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                        << 0 as c_int) as c_int
                                    - (((1 as c_int)
                                        << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                        - 1 as c_int
                                        >> 1 as c_int)
                                    + 1 as c_int) as isize,
                            );
                            trap = (*ci).u.trap;
                        }
                        vmbreak!();
                    }
                    58 => {
                        let ra_55 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let mut cond_0: c_int = 0;
                        let rb_15 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        if (*ra_55).val.tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                            && (*rb_15).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                        {
                            let ia: i64 = (*ra_55).val.value_.i;
                            let ib_3: i64 = (*rb_15).value_.i;
                            cond_0 = (ia < ib_3) as c_int;
                        } else if (*ra_55).val.tt_ as c_int & 0xf as c_int == 3 as c_int
                            && (*rb_15).tt_ as c_int & 0xf as c_int == 3 as c_int
                        {
                            cond_0 = LTnum(&mut (*ra_55).val, rb_15);
                        } else {
                            (*ci).u.savedpc = pc;
                            (*L).top.set((*ci).top);
                            cond_0 = lessthanothers(L, &mut (*ra_55).val, rb_15)?;
                            trap = (*ci).u.trap;
                        }
                        if cond_0
                            != (i >> 0 as c_int + 7 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 1 as c_int) << 0 as c_int)
                                as c_int
                        {
                            pc = pc.offset(1);
                        } else {
                            let ni_0: u32 = *pc;
                            pc = pc.offset(
                                ((ni_0 >> 0 as c_int + 7 as c_int
                                    & !(!(0 as c_int as u32)
                                        << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                        << 0 as c_int) as c_int
                                    - (((1 as c_int)
                                        << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                        - 1 as c_int
                                        >> 1 as c_int)
                                    + 1 as c_int) as isize,
                            );
                            trap = (*ci).u.trap;
                        }
                        vmbreak!();
                    }
                    59 => {
                        let ra_56 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let mut cond_1: c_int = 0;
                        let rb_16 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        if (*ra_56).val.tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                            && (*rb_16).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                        {
                            let ia_0: i64 = (*ra_56).val.value_.i;
                            let ib_4: i64 = (*rb_16).value_.i;
                            cond_1 = (ia_0 <= ib_4) as c_int;
                        } else if (*ra_56).val.tt_ as c_int & 0xf as c_int == 3 as c_int
                            && (*rb_16).tt_ as c_int & 0xf as c_int == 3 as c_int
                        {
                            cond_1 = LEnum(&mut (*ra_56).val, rb_16);
                        } else {
                            (*ci).u.savedpc = pc;
                            (*L).top.set((*ci).top);
                            cond_1 = lessequalothers(L, &mut (*ra_56).val, rb_16)?;
                            trap = (*ci).u.trap;
                        }
                        if cond_1
                            != (i >> 0 as c_int + 7 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 1 as c_int) << 0 as c_int)
                                as c_int
                        {
                            pc = pc.offset(1);
                        } else {
                            let ni_1: u32 = *pc;
                            pc = pc.offset(
                                ((ni_1 >> 0 as c_int + 7 as c_int
                                    & !(!(0 as c_int as u32)
                                        << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                        << 0 as c_int) as c_int
                                    - (((1 as c_int)
                                        << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                        - 1 as c_int
                                        >> 1 as c_int)
                                    + 1 as c_int) as isize,
                            );
                            trap = (*ci).u.trap;
                        }
                        vmbreak!();
                    }
                    60 => {
                        let ra_57 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let rb_17 = k.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let cond_2: c_int = luaV_equalobj(null(), &raw mut (*ra_57).val, rb_17)?;
                        if cond_2
                            != (i >> 0 as c_int + 7 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 1 as c_int) << 0 as c_int)
                                as c_int
                        {
                            pc = pc.offset(1);
                        } else {
                            let ni_2: u32 = *pc;
                            pc = pc.offset(
                                ((ni_2 >> 0 as c_int + 7 as c_int
                                    & !(!(0 as c_int as u32)
                                        << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                        << 0 as c_int) as c_int
                                    - (((1 as c_int)
                                        << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                        - 1 as c_int
                                        >> 1 as c_int)
                                    + 1 as c_int) as isize,
                            );
                            trap = (*ci).u.trap;
                        }
                        vmbreak!();
                    }
                    61 => {
                        let ra_58 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let mut cond_3: c_int = 0;
                        let im: c_int = (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int
                            - (((1 as c_int) << 8 as c_int) - 1 as c_int >> 1 as c_int);
                        if (*ra_58).val.tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                            cond_3 = ((*ra_58).val.value_.i == im as i64) as c_int;
                        } else if (*ra_58).val.tt_ as c_int
                            == 3 as c_int | (1 as c_int) << 4 as c_int
                        {
                            cond_3 = ((*ra_58).val.value_.n == im as f64) as c_int;
                        } else {
                            cond_3 = 0 as c_int;
                        }
                        if cond_3
                            != (i >> 0 as c_int + 7 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 1 as c_int) << 0 as c_int)
                                as c_int
                        {
                            pc = pc.offset(1);
                        } else {
                            let ni_3: u32 = *pc;
                            pc = pc.offset(
                                ((ni_3 >> 0 as c_int + 7 as c_int
                                    & !(!(0 as c_int as u32)
                                        << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                        << 0 as c_int) as c_int
                                    - (((1 as c_int)
                                        << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                        - 1 as c_int
                                        >> 1 as c_int)
                                    + 1 as c_int) as isize,
                            );
                            trap = (*ci).u.trap;
                        }
                        vmbreak!();
                    }
                    62 => {
                        let ra_59 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let mut cond_4: c_int = 0;
                        let im_0: c_int = (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int
                            - (((1 as c_int) << 8 as c_int) - 1 as c_int >> 1 as c_int);
                        if (*ra_59).val.tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                            cond_4 = ((*ra_59).val.value_.i < im_0 as i64) as c_int;
                        } else if (*ra_59).val.tt_ as c_int
                            == 3 as c_int | (1 as c_int) << 4 as c_int
                        {
                            let fa: f64 = (*ra_59).val.value_.n;
                            let fim: f64 = im_0 as f64;
                            cond_4 = (fa < fim) as c_int;
                        } else {
                            let isf: c_int = (i
                                >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int;
                            (*ci).u.savedpc = pc;
                            (*L).top.set((*ci).top);
                            cond_4 = luaT_callorderiTM(
                                L,
                                &mut (*ra_59).val,
                                im_0,
                                0 as c_int,
                                isf,
                                TM_LT,
                            )?;
                            trap = (*ci).u.trap;
                        }
                        if cond_4
                            != (i >> 0 as c_int + 7 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 1 as c_int) << 0 as c_int)
                                as c_int
                        {
                            pc = pc.offset(1);
                        } else {
                            let ni_4: u32 = *pc;
                            pc = pc.offset(
                                ((ni_4 >> 0 as c_int + 7 as c_int
                                    & !(!(0 as c_int as u32)
                                        << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                        << 0 as c_int) as c_int
                                    - (((1 as c_int)
                                        << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                        - 1 as c_int
                                        >> 1 as c_int)
                                    + 1 as c_int) as isize,
                            );
                            trap = (*ci).u.trap;
                        }
                        vmbreak!();
                    }
                    63 => {
                        let ra_60 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let mut cond_5: c_int = 0;
                        let im_1: c_int = (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int
                            - (((1 as c_int) << 8 as c_int) - 1 as c_int >> 1 as c_int);
                        if (*ra_60).val.tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                            cond_5 = ((*ra_60).val.value_.i <= im_1 as i64) as c_int;
                        } else if (*ra_60).val.tt_ as c_int
                            == 3 as c_int | (1 as c_int) << 4 as c_int
                        {
                            let fa_0: f64 = (*ra_60).val.value_.n;
                            let fim_0: f64 = im_1 as f64;
                            cond_5 = (fa_0 <= fim_0) as c_int;
                        } else {
                            let isf_0: c_int = (i
                                >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int;
                            (*ci).u.savedpc = pc;
                            (*L).top.set((*ci).top);
                            cond_5 = luaT_callorderiTM(
                                L,
                                &mut (*ra_60).val,
                                im_1,
                                0 as c_int,
                                isf_0,
                                TM_LE,
                            )?;
                            trap = (*ci).u.trap;
                        }
                        if cond_5
                            != (i >> 0 as c_int + 7 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 1 as c_int) << 0 as c_int)
                                as c_int
                        {
                            pc = pc.offset(1);
                        } else {
                            let ni_5: u32 = *pc;
                            pc = pc.offset(
                                ((ni_5 >> 0 as c_int + 7 as c_int
                                    & !(!(0 as c_int as u32)
                                        << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                        << 0 as c_int) as c_int
                                    - (((1 as c_int)
                                        << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                        - 1 as c_int
                                        >> 1 as c_int)
                                    + 1 as c_int) as isize,
                            );
                            trap = (*ci).u.trap;
                        }
                        vmbreak!();
                    }
                    64 => {
                        let ra_61 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let mut cond_6: c_int = 0;
                        let im_2: c_int = (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int
                            - (((1 as c_int) << 8 as c_int) - 1 as c_int >> 1 as c_int);
                        if (*ra_61).val.tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                            cond_6 = ((*ra_61).val.value_.i > im_2 as i64) as c_int;
                        } else if (*ra_61).val.tt_ as c_int
                            == 3 as c_int | (1 as c_int) << 4 as c_int
                        {
                            let fa_1: f64 = (*ra_61).val.value_.n;
                            let fim_1: f64 = im_2 as f64;
                            cond_6 = (fa_1 > fim_1) as c_int;
                        } else {
                            let isf_1: c_int = (i
                                >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int;
                            (*ci).u.savedpc = pc;
                            (*L).top.set((*ci).top);
                            cond_6 = luaT_callorderiTM(
                                L,
                                &mut (*ra_61).val,
                                im_2,
                                1 as c_int,
                                isf_1,
                                TM_LT,
                            )?;
                            trap = (*ci).u.trap;
                        }
                        if cond_6
                            != (i >> 0 as c_int + 7 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 1 as c_int) << 0 as c_int)
                                as c_int
                        {
                            pc = pc.offset(1);
                        } else {
                            let ni_6: u32 = *pc;
                            pc = pc.offset(
                                ((ni_6 >> 0 as c_int + 7 as c_int
                                    & !(!(0 as c_int as u32)
                                        << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                        << 0 as c_int) as c_int
                                    - (((1 as c_int)
                                        << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                        - 1 as c_int
                                        >> 1 as c_int)
                                    + 1 as c_int) as isize,
                            );
                            trap = (*ci).u.trap;
                        }
                        vmbreak!();
                    }
                    65 => {
                        let ra_62 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let mut cond_7: c_int = 0;
                        let im_3: c_int = (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int
                            - (((1 as c_int) << 8 as c_int) - 1 as c_int >> 1 as c_int);
                        if (*ra_62).val.tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                            cond_7 = ((*ra_62).val.value_.i >= im_3 as i64) as c_int;
                        } else if (*ra_62).val.tt_ as c_int
                            == 3 as c_int | (1 as c_int) << 4 as c_int
                        {
                            let fa_2: f64 = (*ra_62).val.value_.n;
                            let fim_2: f64 = im_3 as f64;
                            cond_7 = (fa_2 >= fim_2) as c_int;
                        } else {
                            let isf_2: c_int = (i
                                >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int;
                            (*ci).u.savedpc = pc;
                            (*L).top.set((*ci).top);
                            cond_7 = luaT_callorderiTM(
                                L,
                                &mut (*ra_62).val,
                                im_3,
                                1 as c_int,
                                isf_2,
                                TM_LE,
                            )?;
                            trap = (*ci).u.trap;
                        }
                        if cond_7
                            != (i >> 0 as c_int + 7 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 1 as c_int) << 0 as c_int)
                                as c_int
                        {
                            pc = pc.offset(1);
                        } else {
                            let ni_7: u32 = *pc;
                            pc = pc.offset(
                                ((ni_7 >> 0 as c_int + 7 as c_int
                                    & !(!(0 as c_int as u32)
                                        << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                        << 0 as c_int) as c_int
                                    - (((1 as c_int)
                                        << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                        - 1 as c_int
                                        >> 1 as c_int)
                                    + 1 as c_int) as isize,
                            );
                            trap = (*ci).u.trap;
                        }
                        vmbreak!();
                    }
                    66 => {
                        let ra_63 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let cond_8: c_int = !((*ra_63).val.tt_ as c_int
                            == 1 as c_int | (0 as c_int) << 4 as c_int
                            || (*ra_63).val.tt_ as c_int & 0xf as c_int == 0 as c_int)
                            as c_int;
                        if cond_8
                            != (i >> 0 as c_int + 7 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 1 as c_int) << 0 as c_int)
                                as c_int
                        {
                            pc = pc.offset(1);
                        } else {
                            let ni_8: u32 = *pc;
                            pc = pc.offset(
                                ((ni_8 >> 0 as c_int + 7 as c_int
                                    & !(!(0 as c_int as u32)
                                        << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                        << 0 as c_int) as c_int
                                    - (((1 as c_int)
                                        << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                        - 1 as c_int
                                        >> 1 as c_int)
                                    + 1 as c_int) as isize,
                            );
                            trap = (*ci).u.trap;
                        }
                        vmbreak!();
                    }
                    67 => {
                        let ra_64 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let rb_18 = &raw mut (*base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        ))
                        .val;
                        if ((*rb_18).tt_ as c_int == 1 as c_int | (0 as c_int) << 4 as c_int
                            || (*rb_18).tt_ as c_int & 0xf as c_int == 0 as c_int)
                            as c_int
                            == (i >> 0 as c_int + 7 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 1 as c_int) << 0 as c_int)
                                as c_int
                        {
                            pc = pc.offset(1);
                        } else {
                            let io1_14 = &raw mut (*ra_64).val;
                            let io2_14 = rb_18;

                            (*io1_14).value_ = (*io2_14).value_;
                            (*io1_14).tt_ = (*io2_14).tt_;
                            let ni_9: u32 = *pc;
                            pc = pc.offset(
                                ((ni_9 >> 0 as c_int + 7 as c_int
                                    & !(!(0 as c_int as u32)
                                        << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                        << 0 as c_int) as c_int
                                    - (((1 as c_int)
                                        << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                        - 1 as c_int
                                        >> 1 as c_int)
                                    + 1 as c_int) as isize,
                            );
                            trap = (*ci).u.trap;
                        }
                        vmbreak!();
                    }
                    OP_CALL => {
                        ra_65 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        newci = null_mut();
                        b_4 = (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int;
                        nresults = (i
                            >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int
                            - 1 as c_int;
                        if b_4 != 0 as c_int {
                            (*L).top.set(ra_65.offset(b_4 as isize));
                        }
                        (*ci).u.savedpc = pc;
                        newci = luaD_precall(L, ra_65, nresults).await?;
                        if !newci.is_null() {
                            break '_returning;
                        }
                        trap = (*ci).u.trap;
                        vmbreak!();
                    }
                    OP_TAILCALL => {
                        let ra_66 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let mut b_5: c_int = (i
                            >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int;
                        let mut n_2: c_int = 0;
                        let nparams1: c_int = (i
                            >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int;
                        let delta: c_int = if nparams1 != 0 {
                            (*ci).u.nextraargs + nparams1
                        } else {
                            0 as c_int
                        };
                        if b_5 != 0 as c_int {
                            (*L).top.set(ra_66.offset(b_5 as isize));
                        } else {
                            b_5 = ((*L).top.get()).offset_from(ra_66) as c_long as c_int;
                        }
                        (*ci).u.savedpc = pc;
                        if (i & (1 as c_uint) << 0 as c_int + 7 as c_int + 8 as c_int) as c_int != 0
                        {
                            luaF_closeupval(L, base);
                        }
                        n_2 = luaD_pretailcall(L, ci, ra_66, b_5, delta).await?;
                        if n_2 < 0 {
                            continue '_startfunc;
                        }
                        (*ci).func = ((*ci).func).offset(-(delta as isize));
                        luaD_poscall(L, ci, n_2)?;
                        trap = (*ci).u.trap;
                        break;
                    }
                    70 => {
                        let mut ra_67 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let mut n_3: c_int = (i
                            >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int
                            - 1 as c_int;
                        let nparams1_0: c_int = (i
                            >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int;
                        if n_3 < 0 as c_int {
                            n_3 = ((*L).top.get()).offset_from(ra_67) as c_long as c_int;
                        }
                        (*ci).u.savedpc = pc;
                        if (i & (1 as c_uint) << 0 as c_int + 7 as c_int + 8 as c_int) as c_int != 0
                        {
                            (*ci).u2.nres = n_3;
                            if (*L).top.get() < (*ci).top {
                                (*L).top.set((*ci).top);
                            }

                            if let Err(e) = luaF_close(L, base) {
                                return Err(e); // Requires unsized coercion.
                            }

                            trap = (*ci).u.trap;
                            if (trap != 0 as c_int) as c_int as c_long != 0 {
                                base = ((*ci).func).offset(1 as c_int as isize);
                                ra_67 = base.offset(
                                    (i >> 0 as c_int + 7 as c_int
                                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                        as c_int as isize,
                                );
                            }
                        }
                        if nparams1_0 != 0 {
                            (*ci).func =
                                ((*ci).func).offset(-(((*ci).u.nextraargs + nparams1_0) as isize));
                        }
                        (*L).top.set(ra_67.offset(n_3 as isize));
                        luaD_poscall(L, ci, n_3)?;
                        trap = (*ci).u.trap;
                        break;
                    }
                    71 => {
                        if ((*L).hookmask.get() != 0) as c_int as c_long != 0 {
                            let ra_68 = base.offset(
                                (i >> 0 as c_int + 7 as c_int
                                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                    as c_int as isize,
                            );
                            (*L).top.set(ra_68);
                            (*ci).u.savedpc = pc;
                            luaD_poscall(L, ci, 0 as c_int)?;
                            trap = 1 as c_int;
                        } else {
                            let mut nres: c_int = 0;
                            (*L).ci.set((*ci).previous);
                            (*L).top.set(base.offset(-(1 as c_int as isize)));
                            nres = (*ci).nresults as c_int;
                            while ((nres > 0 as c_int) as c_int != 0 as c_int) as c_int as c_long
                                != 0
                            {
                                let fresh5 = (*L).top.get();
                                (*L).top.add(1);
                                (*fresh5).val.tt_ = (0 as c_int | (0 as c_int) << 4 as c_int) as u8;
                                nres -= 1;
                            }
                        }
                        break;
                    }
                    72 => {
                        if ((*L).hookmask.get() != 0) as c_int as c_long != 0 {
                            let ra_69 = base.offset(
                                (i >> 0 as c_int + 7 as c_int
                                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                    as c_int as isize,
                            );
                            (*L).top.set(ra_69.offset(1 as c_int as isize));
                            (*ci).u.savedpc = pc;
                            luaD_poscall(L, ci, 1 as c_int)?;
                            trap = 1 as c_int;
                        } else {
                            let mut nres_0: c_int = (*ci).nresults as c_int;
                            (*L).ci.set((*ci).previous);
                            if nres_0 == 0 as c_int {
                                (*L).top.set(base.offset(-(1 as c_int as isize)));
                            } else {
                                let ra_70 = base.offset(
                                    (i >> 0 as c_int + 7 as c_int
                                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                        as c_int as isize,
                                );
                                let io1_15 = &raw mut (*base.offset(-(1 as c_int as isize))).val;
                                let io2_15 = &raw mut (*ra_70).val;

                                (*io1_15).value_ = (*io2_15).value_;
                                (*io1_15).tt_ = (*io2_15).tt_;
                                (*L).top.set(base);
                                while ((nres_0 > 1 as c_int) as c_int != 0 as c_int) as c_int
                                    as c_long
                                    != 0
                                {
                                    let fresh6 = (*L).top.get();
                                    (*L).top.add(1);
                                    (*fresh6).val.tt_ =
                                        (0 as c_int | (0 as c_int) << 4 as c_int) as u8;
                                    nres_0 -= 1;
                                }
                            }
                        }
                        break;
                    }
                    73 => {
                        let ra_71 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        if (*ra_71.offset(2 as c_int as isize)).val.tt_ as c_int
                            == 3 as c_int | (0 as c_int) << 4 as c_int
                        {
                            let count: u64 =
                                (*ra_71.offset(1 as c_int as isize)).val.value_.i as u64;
                            if count > 0 as c_int as u64 {
                                let step: i64 = (*ra_71.offset(2 as c_int as isize)).val.value_.i;
                                let mut idx: i64 = (*ra_71).val.value_.i;
                                let io_43 = &raw mut (*ra_71.offset(1 as c_int as isize)).val;
                                (*io_43).value_.i = count.wrapping_sub(1 as c_int as u64) as i64;
                                idx = (idx as u64).wrapping_add(step as u64) as i64;
                                let io_44 = &raw mut (*ra_71).val;
                                (*io_44).value_.i = idx;
                                let io_45 = &raw mut (*ra_71.offset(3 as c_int as isize)).val;
                                (*io_45).value_.i = idx;
                                (*io_45).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                                pc = pc.offset(
                                    -((i >> 0 as c_int + 7 as c_int + 8 as c_int
                                        & !(!(0 as c_int as u32)
                                            << 8 as c_int + 8 as c_int + 1 as c_int)
                                            << 0 as c_int)
                                        as c_int as isize),
                                );
                            }
                        } else if floatforloop(ra_71) != 0 {
                            pc = pc.offset(
                                -((i >> 0 as c_int + 7 as c_int + 8 as c_int
                                    & !(!(0 as c_int as u32)
                                        << 8 as c_int + 8 as c_int + 1 as c_int)
                                        << 0 as c_int) as c_int
                                    as isize),
                            );
                        }
                        trap = (*ci).u.trap;
                        vmbreak!();
                    }
                    74 => {
                        let ra_72 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        (*ci).u.savedpc = pc;
                        (*L).top.set((*ci).top);
                        if forprep(L, ra_72)? != 0 {
                            pc = pc.offset(
                                ((i >> 0 as c_int + 7 as c_int + 8 as c_int
                                    & !(!(0 as c_int as u32)
                                        << 8 as c_int + 8 as c_int + 1 as c_int)
                                        << 0 as c_int) as c_int
                                    + 1 as c_int) as isize,
                            );
                        }
                        vmbreak!();
                    }
                    75 => {
                        let ra_73 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        (*ci).u.savedpc = pc;
                        (*L).top.set((*ci).top);
                        luaF_newtbcupval(L, ra_73.offset(3 as c_int as isize))?;
                        pc = pc.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int + 8 as c_int + 1 as c_int)
                                    << 0 as c_int) as c_int as isize,
                        );
                        let fresh7 = pc;
                        pc = pc.offset(1);
                        i = *fresh7;
                        current_block = 13973394567113199817;
                    }
                    OP_TFORCALL => {
                        current_block = 13973394567113199817;
                    }
                    77 => {
                        current_block = 15611964311717037170;
                    }
                    78 => {
                        let ra_76 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let mut n_4: c_int = (i
                            >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int;
                        let mut last: c_uint = (i
                            >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as c_uint;
                        let h = (*ra_76).val.value_.gc as *mut Table<D>;

                        if n_4 == 0 as c_int {
                            n_4 =
                                ((*L).top.get()).offset_from(ra_76) as c_long as c_int - 1 as c_int;
                        } else {
                            (*L).top.set((*ci).top);
                        }
                        last = last.wrapping_add(n_4 as c_uint);
                        if (i & (1 as c_uint) << 0 as c_int + 7 as c_int + 8 as c_int) as c_int != 0
                        {
                            last = last.wrapping_add(
                                ((*pc >> 0 as c_int + 7 as c_int
                                    & !(!(0 as c_int as u32)
                                        << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                        << 0 as c_int) as c_int
                                    * (((1 as c_int) << 8 as c_int) - 1 as c_int + 1 as c_int))
                                    as c_uint,
                            );
                            pc = pc.offset(1);
                        }

                        if last > luaH_realasize(h) {
                            luaH_resizearray(h, last);
                        }

                        while n_4 > 0 as c_int {
                            let val = &raw mut (*ra_76.offset(n_4 as isize)).val;
                            let io1_17 = (*h).array.get().offset(last.wrapping_sub(1) as isize);
                            let io2_17 = val;

                            (*io1_17).value_ = (*io2_17).value_;
                            (*io1_17).tt_ = (*io2_17).tt_;
                            last = last.wrapping_sub(1);
                            if (*val).tt_ as c_int & (1 as c_int) << 6 as c_int != 0 {
                                if (*h).hdr.marked.get() as c_int & (1 as c_int) << 5 as c_int != 0
                                    && (*(*val).value_.gc).marked.get() as c_int
                                        & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                                        != 0
                                {
                                    (*L).hdr.global().gc.barrier_back(h.cast());
                                }
                            }
                            n_4 -= 1;
                        }
                        vmbreak!();
                    }
                    OP_CLOSURE => {
                        let ra_77 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let p = *((*(*cl).p.get()).p).offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int + 8 as c_int + 1 as c_int)
                                    << 0 as c_int) as c_int as isize,
                        );
                        (*ci).u.savedpc = pc;
                        (*L).top.set((*ci).top);
                        pushclosure(L, p, &(*cl).upvals, base, ra_77);

                        (*ci).u.savedpc = pc;
                        (*L).top.set(ra_77.offset(1 as c_int as isize));
                        (*L).hdr.global().gc.step();
                        trap = (*ci).u.trap;

                        vmbreak!();
                    }
                    80 => {
                        let ra_78 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let n_5: c_int = (i
                            >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int
                            - 1 as c_int;
                        (*ci).u.savedpc = pc;
                        (*L).top.set((*ci).top);
                        luaT_getvarargs(L, ci, ra_78, n_5)?;
                        trap = (*ci).u.trap;
                        vmbreak!();
                    }
                    81 => {
                        (*ci).u.savedpc = pc;
                        luaT_adjustvarargs(
                            L,
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0)
                                as c_int,
                            ci,
                            (*cl).p.get(),
                        )?;
                        trap = (*ci).u.trap;
                        if (trap != 0 as c_int) as c_int as c_long != 0 {
                            luaD_hookcall(L, ci)?;
                            (*L).oldpc.set(1);
                        }
                        base = ((*ci).func).offset(1 as c_int as isize);
                        vmbreak!();
                    }
                    82 => {
                        vmbreak!();
                    }
                    _ => unreachable_unchecked(), // TODO: Remove this once we converted to enum.
                }
                match current_block {
                    0 => {
                        (*ci).u.savedpc = pc;
                        (*L).top.set((*ci).top);

                        let val = luaV_finishget(L, tab, key, slot)?;

                        (*ci)
                            .func
                            .add(1)
                            .offset(
                                (i >> 0 as c_int + 7 as c_int
                                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                    as c_int as isize,
                            )
                            .write(StackValue { val });

                        trap = (*ci).u.trap;
                        vmbreak!();
                    }
                    13973394567113199817 => {
                        let mut ra_74 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        memcpy(
                            ra_74.offset(4 as c_int as isize) as *mut c_void,
                            ra_74 as *const c_void,
                            3usize.wrapping_mul(::core::mem::size_of::<StackValue<D>>()),
                        );
                        (*L).top.set(
                            ra_74
                                .offset(4 as c_int as isize)
                                .offset(3 as c_int as isize),
                        );
                        (*ci).u.savedpc = pc;

                        // Invoke iterator function.
                        {
                            let w = Waker::new(null(), &NON_YIELDABLE_WAKER);
                            let f = pin!(luaD_call(
                                L,
                                ra_74.offset(4),
                                (i >> 0 as c_int
                                    + 7 as c_int
                                    + 8 as c_int
                                    + 1 as c_int
                                    + 8 as c_int
                                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                    as c_int,
                            ));

                            match f.poll(&mut Context::from_waker(&w)) {
                                Poll::Ready(Ok(_)) => (),
                                Poll::Ready(Err(e)) => return Err(e), // Requires unsized coercion.
                                Poll::Pending => unreachable!(),
                            }
                        }

                        trap = (*ci).u.trap;
                        if (trap != 0 as c_int) as c_int as c_long != 0 {
                            base = ((*ci).func).offset(1 as c_int as isize);
                            ra_74 = base.offset(
                                (i >> 0 as c_int + 7 as c_int
                                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                    as c_int as isize,
                            );
                        }
                        let fresh8 = pc;
                        pc = pc.offset(1);
                        i = *fresh8;
                    }
                    _ => {}
                }
                let ra_75 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                if !((*ra_75.offset(4 as c_int as isize)).val.tt_ as c_int & 0xf as c_int
                    == 0 as c_int)
                {
                    let io1_16 = &raw mut (*ra_75.offset(2 as c_int as isize)).val;
                    let io2_16 = &raw mut (*ra_75.offset(4 as c_int as isize)).val;

                    (*io1_16).value_ = (*io2_16).value_;
                    (*io1_16).tt_ = (*io2_16).tt_;
                    pc = pc.offset(
                        -((i >> 0 as c_int + 7 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int + 8 as c_int + 1 as c_int)
                                << 0 as c_int) as c_int as isize),
                    );
                }
                vmbreak!();
            }
            if (*ci).callstatus as c_int & (1 as c_int) << 2 as c_int != 0 {
                break '_startfunc Ok(());
            }
            ci = (*ci).previous;
        }
        ci = newci;
    }
}
