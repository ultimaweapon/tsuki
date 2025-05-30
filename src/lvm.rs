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
#![allow(unused_variables)]
#![allow(unused_parens)]

use crate::Thread;
use crate::gc::{luaC_barrier_, luaC_barrierback_, luaC_step};
use crate::ldebug::{luaG_forerror, luaG_runerror, luaG_tracecall, luaG_traceexec, luaG_typeerror};
use crate::ldo::{luaD_call, luaD_hookcall, luaD_poscall, luaD_precall, luaD_pretailcall};
use crate::lfunc::{
    luaF_close, luaF_closeupval, luaF_findupval, luaF_newLclosure, luaF_newtbcupval,
};
use crate::lobject::{
    GCObject, LClosure, Proto, StackValue, StkId, TString, TValue, Table, Udata, UpVal, Upvaldesc,
    Value, luaO_str2num, luaO_tostring,
};
use crate::lopcodes::OpCode;
use crate::lstate::CallInfo;
use crate::lstring::{luaS_createlngstrobj, luaS_eqlngstr, luaS_newlstr};
use crate::ltable::{
    luaH_finishset, luaH_get, luaH_getint, luaH_getn, luaH_getshortstr, luaH_getstr, luaH_new,
    luaH_realasize, luaH_resize, luaH_resizearray,
};
use crate::ltm::{
    TM_BNOT, TM_EQ, TM_INDEX, TM_LE, TM_LEN, TM_LT, TM_NEWINDEX, TM_UNM, TMS, luaT_adjustvarargs,
    luaT_callTM, luaT_callTMres, luaT_callorderTM, luaT_callorderiTM, luaT_gettm, luaT_gettmbyobj,
    luaT_getvarargs, luaT_trybinTM, luaT_trybinassocTM, luaT_trybiniTM, luaT_tryconcatTM,
};
use libc::{memcpy, strcoll, strlen};
use libm::{floor, fmod, pow};
use std::ffi::c_int;

pub type F2Imod = libc::c_uint;

pub const F2Iceil: F2Imod = 2;
pub const F2Ifloor: F2Imod = 1;
pub const F2Ieq: F2Imod = 0;

unsafe fn l_strton(mut obj: *const TValue, mut result: *mut TValue) -> libc::c_int {
    if !((*obj).tt_ as libc::c_int & 0xf as libc::c_int == 4 as libc::c_int) {
        return 0 as libc::c_int;
    } else {
        let mut st: *mut TString = ((*obj).value_.gc as *mut TString);
        return (luaO_str2num(((*st).contents).as_mut_ptr(), result)
            == (if (*st).shrlen as libc::c_int != 0xff as libc::c_int {
                (*st).shrlen as usize
            } else {
                (*st).u.lnglen
            })
            .wrapping_add(1 as libc::c_int as usize)) as libc::c_int;
    };
}

pub unsafe fn luaV_tonumber_(mut obj: *const TValue, mut n: *mut f64) -> libc::c_int {
    let mut v: TValue = TValue {
        value_: Value {
            gc: 0 as *mut GCObject,
        },
        tt_: 0,
    };
    if (*obj).tt_ as libc::c_int == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int {
        *n = (*obj).value_.i as f64;
        return 1 as libc::c_int;
    } else if l_strton(obj, &mut v) != 0 {
        *n = if v.tt_ as libc::c_int == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int {
            v.value_.i as f64
        } else {
            v.value_.n
        };
        return 1 as libc::c_int;
    } else {
        return 0 as libc::c_int;
    };
}

pub unsafe fn luaV_flttointeger(mut n: f64, mut p: *mut i64, mut mode: F2Imod) -> libc::c_int {
    let mut f: f64 = floor(n);
    if n != f {
        if mode as libc::c_uint == F2Ieq as libc::c_int as libc::c_uint {
            return 0 as libc::c_int;
        } else if mode as libc::c_uint == F2Iceil as libc::c_int as libc::c_uint {
            f += 1 as libc::c_int as f64;
        }
    }
    return (f
        >= (-(0x7fffffffffffffff as libc::c_longlong) - 1 as libc::c_int as libc::c_longlong)
            as libc::c_double
        && f < -((-(0x7fffffffffffffff as libc::c_longlong) - 1 as libc::c_int as libc::c_longlong)
            as libc::c_double)
        && {
            *p = f as libc::c_longlong;
            1 as libc::c_int != 0
        }) as libc::c_int;
}

pub unsafe fn luaV_tointegerns(
    mut obj: *const TValue,
    mut p: *mut i64,
    mut mode: F2Imod,
) -> libc::c_int {
    if (*obj).tt_ as libc::c_int == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int {
        return luaV_flttointeger((*obj).value_.n, p, mode);
    } else if (*obj).tt_ as libc::c_int == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
    {
        *p = (*obj).value_.i;
        return 1 as libc::c_int;
    } else {
        return 0 as libc::c_int;
    };
}

pub unsafe fn luaV_tointeger(
    mut obj: *const TValue,
    mut p: *mut i64,
    mut mode: F2Imod,
) -> libc::c_int {
    let mut v: TValue = TValue {
        value_: Value {
            gc: 0 as *mut GCObject,
        },
        tt_: 0,
    };
    if l_strton(obj, &mut v) != 0 {
        obj = &mut v;
    }
    return luaV_tointegerns(obj, p, mode);
}

unsafe fn forlimit(
    mut L: *mut Thread,
    mut init: i64,
    mut lim: *const TValue,
    mut p: *mut i64,
    mut step: i64,
) -> Result<c_int, Box<dyn std::error::Error>> {
    if luaV_tointeger(
        lim,
        p,
        (if step < 0 as libc::c_int as i64 {
            F2Iceil as libc::c_int
        } else {
            F2Ifloor as libc::c_int
        }) as F2Imod,
    ) == 0
    {
        let mut flim: f64 = 0.;
        if if (*lim).tt_ as libc::c_int == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
        {
            flim = (*lim).value_.n;
            1 as libc::c_int
        } else {
            luaV_tonumber_(lim, &mut flim)
        } == 0
        {
            luaG_forerror(L, lim, "limit")?;
        }
        if (0 as libc::c_int as f64) < flim {
            if step < 0 as libc::c_int as i64 {
                return Ok(1 as libc::c_int);
            }
            *p = 0x7fffffffffffffff as libc::c_longlong;
        } else {
            if step > 0 as libc::c_int as i64 {
                return Ok(1 as libc::c_int);
            }
            *p = -(0x7fffffffffffffff as libc::c_longlong) - 1 as libc::c_int as libc::c_longlong;
        }
    }
    return if step > 0 as libc::c_int as i64 {
        Ok((init > *p) as libc::c_int)
    } else {
        Ok((init < *p) as libc::c_int)
    };
}

unsafe fn forprep(mut L: *mut Thread, mut ra: StkId) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut pinit: *mut TValue = &mut (*ra).val;
    let mut plimit: *mut TValue = &mut (*ra.offset(1 as libc::c_int as isize)).val;
    let mut pstep: *mut TValue = &mut (*ra.offset(2 as libc::c_int as isize)).val;
    if (*pinit).tt_ as libc::c_int == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
        && (*pstep).tt_ as libc::c_int == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
    {
        let mut init: i64 = (*pinit).value_.i;
        let mut step: i64 = (*pstep).value_.i;
        let mut limit: i64 = 0;
        if step == 0 as libc::c_int as i64 {
            luaG_runerror(L, "'for' step is zero")?;
        }
        let mut io: *mut TValue = &mut (*ra.offset(3 as libc::c_int as isize)).val;
        (*io).value_.i = init;
        (*io).tt_ = (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
        if forlimit(L, init, plimit, &mut limit, step)? != 0 {
            return Ok(1 as libc::c_int);
        } else {
            let mut count: u64 = 0;
            if step > 0 as libc::c_int as i64 {
                count = (limit as u64).wrapping_sub(init as u64);
                if step != 1 as libc::c_int as i64 {
                    count = count / step as u64;
                }
            } else {
                count = (init as u64).wrapping_sub(limit as u64);
                count = count
                    / (-(step + 1 as libc::c_int as i64) as u64)
                        .wrapping_add(1 as libc::c_uint as u64);
            }
            let mut io_0: *mut TValue = plimit;
            (*io_0).value_.i = count as i64;
            (*io_0).tt_ = (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
        }
    } else {
        let mut init_0: f64 = 0.;
        let mut limit_0: f64 = 0.;
        let mut step_0: f64 = 0.;
        if (((if (*plimit).tt_ as libc::c_int
            == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
        {
            limit_0 = (*plimit).value_.n;
            1 as libc::c_int
        } else {
            luaV_tonumber_(plimit, &mut limit_0)
        }) == 0) as libc::c_int
            != 0 as libc::c_int) as libc::c_int as libc::c_long
            != 0
        {
            luaG_forerror(L, plimit, "limit")?;
        }
        if (((if (*pstep).tt_ as libc::c_int
            == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
        {
            step_0 = (*pstep).value_.n;
            1 as libc::c_int
        } else {
            luaV_tonumber_(pstep, &mut step_0)
        }) == 0) as libc::c_int
            != 0 as libc::c_int) as libc::c_int as libc::c_long
            != 0
        {
            luaG_forerror(L, pstep, "step")?;
        }
        if (((if (*pinit).tt_ as libc::c_int
            == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
        {
            init_0 = (*pinit).value_.n;
            1 as libc::c_int
        } else {
            luaV_tonumber_(pinit, &mut init_0)
        }) == 0) as libc::c_int
            != 0 as libc::c_int) as libc::c_int as libc::c_long
            != 0
        {
            luaG_forerror(L, pinit, "initial value")?;
        }
        if step_0 == 0 as libc::c_int as f64 {
            luaG_runerror(L, "'for' step is zero")?;
        }
        if if (0 as libc::c_int as f64) < step_0 {
            (limit_0 < init_0) as libc::c_int
        } else {
            (init_0 < limit_0) as libc::c_int
        } != 0
        {
            return Ok(1 as libc::c_int);
        } else {
            let mut io_1: *mut TValue = plimit;
            (*io_1).value_.n = limit_0;
            (*io_1).tt_ = (3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
            let mut io_2: *mut TValue = pstep;
            (*io_2).value_.n = step_0;
            (*io_2).tt_ = (3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
            let mut io_3: *mut TValue = &mut (*ra).val;
            (*io_3).value_.n = init_0;
            (*io_3).tt_ = (3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
            let mut io_4: *mut TValue = &mut (*ra.offset(3 as libc::c_int as isize)).val;
            (*io_4).value_.n = init_0;
            (*io_4).tt_ = (3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
        }
    }
    return Ok(0 as libc::c_int);
}

unsafe extern "C" fn floatforloop(mut ra: StkId) -> libc::c_int {
    let mut step: f64 = (*ra.offset(2 as libc::c_int as isize)).val.value_.n;
    let mut limit: f64 = (*ra.offset(1 as libc::c_int as isize)).val.value_.n;
    let mut idx: f64 = (*ra).val.value_.n;
    idx = idx + step;
    if if (0 as libc::c_int as f64) < step {
        (idx <= limit) as libc::c_int
    } else {
        (limit <= idx) as libc::c_int
    } != 0
    {
        let mut io: *mut TValue = &mut (*ra).val;
        (*io).value_.n = idx;
        let mut io_0: *mut TValue = &mut (*ra.offset(3 as libc::c_int as isize)).val;
        (*io_0).value_.n = idx;
        (*io_0).tt_ = (3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
        return 1 as libc::c_int;
    } else {
        return 0 as libc::c_int;
    };
}

pub unsafe fn luaV_finishget(
    mut L: *mut Thread,
    mut t: *const TValue,
    mut key: *mut TValue,
    mut val: StkId,
    mut slot: *const TValue,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut loop_0: libc::c_int = 0;
    let mut tm: *const TValue = 0 as *const TValue;
    loop_0 = 0 as libc::c_int;
    while loop_0 < 2000 as libc::c_int {
        if slot.is_null() {
            tm = luaT_gettmbyobj(L, t, TM_INDEX);
            if (((*tm).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int) as libc::c_int
                != 0 as libc::c_int) as libc::c_int as libc::c_long
                != 0
            {
                luaG_typeerror(L, t, "index")?;
            }
        } else {
            tm = if ((*((*t).value_.gc as *mut Table)).metatable).is_null() {
                0 as *const TValue
            } else if (*(*((*t).value_.gc as *mut Table)).metatable).flags as libc::c_uint
                & (1 as libc::c_uint) << TM_INDEX as libc::c_int
                != 0
            {
                0 as *const TValue
            } else {
                luaT_gettm(
                    (*((*t).value_.gc as *mut Table)).metatable,
                    TM_INDEX,
                    (*(*L).global).tmname[TM_INDEX as usize].get(),
                )
            };
            if tm.is_null() {
                (*val).val.tt_ = (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                return Ok(());
            }
        }
        if (*tm).tt_ as libc::c_int & 0xf as libc::c_int == 6 as libc::c_int {
            return luaT_callTMres(L, tm, t, key, val);
        }
        t = tm;
        if if !((*t).tt_ as libc::c_int
            == 5 as libc::c_int
                | (0 as libc::c_int) << 4 as libc::c_int
                | (1 as libc::c_int) << 6 as libc::c_int)
        {
            slot = 0 as *const TValue;
            0 as libc::c_int
        } else {
            slot = luaH_get(((*t).value_.gc as *mut Table), key);
            !((*slot).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int) as libc::c_int
        } != 0
        {
            let mut io1: *mut TValue = &mut (*val).val;
            let mut io2: *const TValue = slot;
            (*io1).value_ = (*io2).value_;
            (*io1).tt_ = (*io2).tt_;
            return Ok(());
        }
        loop_0 += 1;
    }
    luaG_runerror(L, "'__index' chain too long; possible loop")?;
    unreachable!("luaG_runerror always return Err");
}

pub unsafe fn luaV_finishset(
    mut L: *mut Thread,
    mut t: *const TValue,
    mut key: *mut TValue,
    mut val: *mut TValue,
    mut slot: *const TValue,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut loop_0: libc::c_int = 0;
    loop_0 = 0 as libc::c_int;
    while loop_0 < 2000 as libc::c_int {
        let mut tm: *const TValue = 0 as *const TValue;
        if !slot.is_null() {
            let mut h: *mut Table = ((*t).value_.gc as *mut Table);
            tm = if ((*h).metatable).is_null() {
                0 as *const TValue
            } else if (*(*h).metatable).flags as libc::c_uint
                & (1 as libc::c_uint) << TM_NEWINDEX as libc::c_int
                != 0
            {
                0 as *const TValue
            } else {
                luaT_gettm(
                    (*h).metatable,
                    TM_NEWINDEX,
                    (*(*L).global).tmname[TM_NEWINDEX as usize].get(),
                )
            };
            if tm.is_null() {
                luaH_finishset(L, h, key, slot, val)?;
                (*h).flags = ((*h).flags as libc::c_uint
                    & !!(!(0 as libc::c_uint) << TM_EQ as libc::c_int + 1 as libc::c_int))
                    as u8;
                if (*val).tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0 {
                    if (*h).marked as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
                        && (*(*val).value_.gc).marked as libc::c_int
                            & ((1 as libc::c_int) << 3 as libc::c_int
                                | (1 as libc::c_int) << 4 as libc::c_int)
                            != 0
                    {
                        luaC_barrierback_(L, (h as *mut GCObject));
                    } else {
                    };
                } else {
                };
                return Ok(());
            }
        } else {
            tm = luaT_gettmbyobj(L, t, TM_NEWINDEX);
            if (((*tm).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int) as libc::c_int
                != 0 as libc::c_int) as libc::c_int as libc::c_long
                != 0
            {
                luaG_typeerror(L, t, "index")?;
            }
        }
        if (*tm).tt_ as libc::c_int & 0xf as libc::c_int == 6 as libc::c_int {
            return luaT_callTM(L, tm, t, key, val);
        }
        t = tm;
        if if !((*t).tt_ as libc::c_int
            == 5 as libc::c_int
                | (0 as libc::c_int) << 4 as libc::c_int
                | (1 as libc::c_int) << 6 as libc::c_int)
        {
            slot = 0 as *const TValue;
            0 as libc::c_int
        } else {
            slot = luaH_get(((*t).value_.gc as *mut Table), key);
            !((*slot).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int) as libc::c_int
        } != 0
        {
            let mut io1: *mut TValue = slot as *mut TValue;
            let mut io2: *const TValue = val;
            (*io1).value_ = (*io2).value_;
            (*io1).tt_ = (*io2).tt_;
            if (*val).tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0 {
                if (*(*t).value_.gc).marked as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int
                    != 0
                    && (*(*val).value_.gc).marked as libc::c_int
                        & ((1 as libc::c_int) << 3 as libc::c_int
                            | (1 as libc::c_int) << 4 as libc::c_int)
                        != 0
                {
                    luaC_barrierback_(L, (*t).value_.gc);
                } else {
                };
            } else {
            };
            return Ok(());
        }
        loop_0 += 1;
    }
    luaG_runerror(L, "'__newindex' chain too long; possible loop")?;
    unreachable!("luaG_runerror always return Err");
}

unsafe extern "C" fn l_strcmp(mut ts1: *const TString, mut ts2: *const TString) -> libc::c_int {
    let mut s1: *const libc::c_char = ((*ts1).contents).as_ptr();
    let mut rl1: usize = if (*ts1).shrlen as libc::c_int != 0xff as libc::c_int {
        (*ts1).shrlen as usize
    } else {
        (*ts1).u.lnglen
    };
    let mut s2: *const libc::c_char = ((*ts2).contents).as_ptr();
    let mut rl2: usize = if (*ts2).shrlen as libc::c_int != 0xff as libc::c_int {
        (*ts2).shrlen as usize
    } else {
        (*ts2).u.lnglen
    };
    loop {
        let mut temp: libc::c_int = strcoll(s1, s2);
        if temp != 0 as libc::c_int {
            return temp;
        } else {
            let mut zl1: usize = strlen(s1);
            let mut zl2: usize = strlen(s2);
            if zl2 == rl2 {
                return if zl1 == rl1 {
                    0 as libc::c_int
                } else {
                    1 as libc::c_int
                };
            } else if zl1 == rl1 {
                return -(1 as libc::c_int);
            }
            zl1 = zl1.wrapping_add(1);
            zl2 = zl2.wrapping_add(1);
            s1 = s1.offset(zl1 as isize);
            rl1 = rl1.wrapping_sub(zl1);
            s2 = s2.offset(zl2 as isize);
            rl2 = rl2.wrapping_sub(zl2);
        }
    }
}

#[inline]
unsafe extern "C" fn LTintfloat(mut i: i64, mut f: f64) -> libc::c_int {
    if ((1 as libc::c_int as u64) << 53 as libc::c_int).wrapping_add(i as u64)
        <= 2 as libc::c_int as u64 * ((1 as libc::c_int as u64) << 53 as libc::c_int)
    {
        return ((i as f64) < f) as libc::c_int;
    } else {
        let mut fi: i64 = 0;
        if luaV_flttointeger(f, &mut fi, F2Iceil) != 0 {
            return (i < fi) as libc::c_int;
        } else {
            return (f > 0 as libc::c_int as f64) as libc::c_int;
        }
    };
}

#[inline]
unsafe extern "C" fn LEintfloat(mut i: i64, mut f: f64) -> libc::c_int {
    if ((1 as libc::c_int as u64) << 53 as libc::c_int).wrapping_add(i as u64)
        <= 2 as libc::c_int as u64 * ((1 as libc::c_int as u64) << 53 as libc::c_int)
    {
        return (i as f64 <= f) as libc::c_int;
    } else {
        let mut fi: i64 = 0;
        if luaV_flttointeger(f, &mut fi, F2Ifloor) != 0 {
            return (i <= fi) as libc::c_int;
        } else {
            return (f > 0 as libc::c_int as f64) as libc::c_int;
        }
    };
}

#[inline]
unsafe extern "C" fn LTfloatint(mut f: f64, mut i: i64) -> libc::c_int {
    if ((1 as libc::c_int as u64) << 53 as libc::c_int).wrapping_add(i as u64)
        <= 2 as libc::c_int as u64 * ((1 as libc::c_int as u64) << 53 as libc::c_int)
    {
        return (f < i as f64) as libc::c_int;
    } else {
        let mut fi: i64 = 0;
        if luaV_flttointeger(f, &mut fi, F2Ifloor) != 0 {
            return (fi < i) as libc::c_int;
        } else {
            return (f < 0 as libc::c_int as f64) as libc::c_int;
        }
    };
}

#[inline]
unsafe extern "C" fn LEfloatint(mut f: f64, mut i: i64) -> libc::c_int {
    if ((1 as libc::c_int as u64) << 53 as libc::c_int).wrapping_add(i as u64)
        <= 2 as libc::c_int as u64 * ((1 as libc::c_int as u64) << 53 as libc::c_int)
    {
        return (f <= i as f64) as libc::c_int;
    } else {
        let mut fi: i64 = 0;
        if luaV_flttointeger(f, &mut fi, F2Iceil) != 0 {
            return (fi <= i) as libc::c_int;
        } else {
            return (f < 0 as libc::c_int as f64) as libc::c_int;
        }
    };
}

#[inline]
unsafe extern "C" fn LTnum(mut l: *const TValue, mut r: *const TValue) -> libc::c_int {
    if (*l).tt_ as libc::c_int == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int {
        let mut li: i64 = (*l).value_.i;
        if (*r).tt_ as libc::c_int == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int {
            return (li < (*r).value_.i) as libc::c_int;
        } else {
            return LTintfloat(li, (*r).value_.n);
        }
    } else {
        let mut lf: f64 = (*l).value_.n;
        if (*r).tt_ as libc::c_int == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int {
            return (lf < (*r).value_.n) as libc::c_int;
        } else {
            return LTfloatint(lf, (*r).value_.i);
        }
    };
}

#[inline]
unsafe extern "C" fn LEnum(mut l: *const TValue, mut r: *const TValue) -> libc::c_int {
    if (*l).tt_ as libc::c_int == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int {
        let mut li: i64 = (*l).value_.i;
        if (*r).tt_ as libc::c_int == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int {
            return (li <= (*r).value_.i) as libc::c_int;
        } else {
            return LEintfloat(li, (*r).value_.n);
        }
    } else {
        let mut lf: f64 = (*l).value_.n;
        if (*r).tt_ as libc::c_int == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int {
            return (lf <= (*r).value_.n) as libc::c_int;
        } else {
            return LEfloatint(lf, (*r).value_.i);
        }
    };
}

unsafe fn lessthanothers(
    mut L: *mut Thread,
    mut l: *const TValue,
    mut r: *const TValue,
) -> Result<c_int, Box<dyn std::error::Error>> {
    if (*l).tt_ as libc::c_int & 0xf as libc::c_int == 4 as libc::c_int
        && (*r).tt_ as libc::c_int & 0xf as libc::c_int == 4 as libc::c_int
    {
        return Ok((l_strcmp(
            ((*l).value_.gc as *mut TString),
            ((*r).value_.gc as *mut TString),
        ) < 0 as libc::c_int) as libc::c_int);
    } else {
        return luaT_callorderTM(L, l, r, TM_LT);
    };
}

pub unsafe fn luaV_lessthan(
    mut L: *mut Thread,
    mut l: *const TValue,
    mut r: *const TValue,
) -> Result<c_int, Box<dyn std::error::Error>> {
    if (*l).tt_ as libc::c_int & 0xf as libc::c_int == 3 as libc::c_int
        && (*r).tt_ as libc::c_int & 0xf as libc::c_int == 3 as libc::c_int
    {
        return Ok(LTnum(l, r));
    } else {
        return lessthanothers(L, l, r);
    };
}

unsafe fn lessequalothers(
    mut L: *mut Thread,
    mut l: *const TValue,
    mut r: *const TValue,
) -> Result<c_int, Box<dyn std::error::Error>> {
    if (*l).tt_ as libc::c_int & 0xf as libc::c_int == 4 as libc::c_int
        && (*r).tt_ as libc::c_int & 0xf as libc::c_int == 4 as libc::c_int
    {
        return Ok((l_strcmp(
            ((*l).value_.gc as *mut TString),
            ((*r).value_.gc as *mut TString),
        ) <= 0 as libc::c_int) as libc::c_int);
    } else {
        return luaT_callorderTM(L, l, r, TM_LE);
    };
}

pub unsafe fn luaV_lessequal(
    mut L: *mut Thread,
    mut l: *const TValue,
    mut r: *const TValue,
) -> Result<c_int, Box<dyn std::error::Error>> {
    if (*l).tt_ as libc::c_int & 0xf as libc::c_int == 3 as libc::c_int
        && (*r).tt_ as libc::c_int & 0xf as libc::c_int == 3 as libc::c_int
    {
        return Ok(LEnum(l, r));
    } else {
        return lessequalothers(L, l, r);
    };
}

pub unsafe fn luaV_equalobj(
    mut L: *mut Thread,
    mut t1: *const TValue,
    mut t2: *const TValue,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut tm: *const TValue = 0 as *const TValue;
    if (*t1).tt_ as libc::c_int & 0x3f as libc::c_int
        != (*t2).tt_ as libc::c_int & 0x3f as libc::c_int
    {
        if (*t1).tt_ as libc::c_int & 0xf as libc::c_int
            != (*t2).tt_ as libc::c_int & 0xf as libc::c_int
            || (*t1).tt_ as libc::c_int & 0xf as libc::c_int != 3 as libc::c_int
        {
            return Ok(0 as libc::c_int);
        } else {
            let mut i1: i64 = 0;
            let mut i2: i64 = 0;
            return Ok((luaV_tointegerns(t1, &mut i1, F2Ieq) != 0
                && luaV_tointegerns(t2, &mut i2, F2Ieq) != 0
                && i1 == i2) as libc::c_int);
        }
    }
    match (*t1).tt_ as libc::c_int & 0x3f as libc::c_int {
        0 | 1 | 17 => return Ok(1 as libc::c_int),
        3 => return Ok(((*t1).value_.i == (*t2).value_.i) as libc::c_int),
        19 => return Ok(((*t1).value_.n == (*t2).value_.n) as libc::c_int),
        2 => return Ok(((*t1).value_.p == (*t2).value_.p) as libc::c_int),
        22 => return Ok(((*t1).value_.f == (*t2).value_.f) as libc::c_int),
        4 => {
            return Ok((((*t1).value_.gc as *mut TString) as *mut TString
                == ((*t2).value_.gc as *mut TString) as *mut TString)
                as libc::c_int);
        }
        20 => {
            return Ok(luaS_eqlngstr(
                ((*t1).value_.gc as *mut TString),
                ((*t2).value_.gc as *mut TString),
            ));
        }
        7 => {
            if ((*t1).value_.gc as *mut Udata) as *mut Udata
                == ((*t2).value_.gc as *mut Udata) as *mut Udata
            {
                return Ok(1 as libc::c_int);
            } else if L.is_null() {
                return Ok(0 as libc::c_int);
            }
            tm = if ((*((*t1).value_.gc as *mut Udata)).metatable).is_null() {
                0 as *const TValue
            } else if (*(*((*t1).value_.gc as *mut Udata)).metatable).flags as libc::c_uint
                & (1 as libc::c_uint) << TM_EQ as libc::c_int
                != 0
            {
                0 as *const TValue
            } else {
                luaT_gettm(
                    (*((*t1).value_.gc as *mut Udata)).metatable,
                    TM_EQ,
                    (*(*L).global).tmname[TM_EQ as usize].get(),
                )
            };
            if tm.is_null() {
                tm = if ((*((*t2).value_.gc as *mut Udata)).metatable).is_null() {
                    0 as *const TValue
                } else if (*(*((*t2).value_.gc as *mut Udata)).metatable).flags as libc::c_uint
                    & (1 as libc::c_uint) << TM_EQ as libc::c_int
                    != 0
                {
                    0 as *const TValue
                } else {
                    luaT_gettm(
                        (*((*t2).value_.gc as *mut Udata)).metatable,
                        TM_EQ,
                        (*(*L).global).tmname[TM_EQ as usize].get(),
                    )
                };
            }
        }
        5 => {
            if ((*t1).value_.gc as *mut Table) as *mut Table
                == ((*t2).value_.gc as *mut Table) as *mut Table
            {
                return Ok(1 as libc::c_int);
            } else if L.is_null() {
                return Ok(0 as libc::c_int);
            }
            tm = if ((*((*t1).value_.gc as *mut Table)).metatable).is_null() {
                0 as *const TValue
            } else if (*(*((*t1).value_.gc as *mut Table)).metatable).flags as libc::c_uint
                & (1 as libc::c_uint) << TM_EQ as libc::c_int
                != 0
            {
                0 as *const TValue
            } else {
                luaT_gettm(
                    (*((*t1).value_.gc as *mut Table)).metatable,
                    TM_EQ,
                    (*(*L).global).tmname[TM_EQ as usize].get(),
                )
            };
            if tm.is_null() {
                tm = if ((*((*t2).value_.gc as *mut Table)).metatable).is_null() {
                    0 as *const TValue
                } else if (*(*((*t2).value_.gc as *mut Table)).metatable).flags as libc::c_uint
                    & (1 as libc::c_uint) << TM_EQ as libc::c_int
                    != 0
                {
                    0 as *const TValue
                } else {
                    luaT_gettm(
                        (*((*t2).value_.gc as *mut Table)).metatable,
                        TM_EQ,
                        (*(*L).global).tmname[TM_EQ as usize].get(),
                    )
                };
            }
        }
        _ => return Ok(((*t1).value_.gc == (*t2).value_.gc) as libc::c_int),
    }
    if tm.is_null() {
        return Ok(0 as libc::c_int);
    } else {
        luaT_callTMres(L, tm, t1, t2, (*L).top)?;
        return Ok(!((*(*L).top).val.tt_ as libc::c_int
            == 1 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
            || (*(*L).top).val.tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int)
            as libc::c_int);
    };
}

unsafe extern "C" fn copy2buff(mut top: StkId, mut n: libc::c_int, mut buff: *mut libc::c_char) {
    let mut tl: usize = 0 as libc::c_int as usize;
    loop {
        let mut st: *mut TString = ((*top.offset(-(n as isize))).val.value_.gc as *mut TString);
        let mut l: usize = if (*st).shrlen as libc::c_int != 0xff as libc::c_int {
            (*st).shrlen as usize
        } else {
            (*st).u.lnglen
        };
        memcpy(
            buff.offset(tl as isize) as *mut libc::c_void,
            ((*st).contents).as_mut_ptr() as *const libc::c_void,
            l.wrapping_mul(::core::mem::size_of::<libc::c_char>()),
        );
        tl = tl.wrapping_add(l);
        n -= 1;
        if !(n > 0 as libc::c_int) {
            break;
        }
    }
}

pub unsafe fn luaV_concat(
    mut L: *mut Thread,
    mut total: c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    if total == 1 {
        return Ok(());
    }

    loop {
        let mut top: StkId = (*L).top;
        let mut n: libc::c_int = 2 as libc::c_int;

        if !((*top.offset(-2)).val.tt_ & 0xf == 4 || (*top.offset(-2)).val.tt_ & 0xf == 3)
            || !((*top.offset(-(1 as libc::c_int as isize))).val.tt_ as libc::c_int
                & 0xf as libc::c_int
                == 4 as libc::c_int
                || (*top.offset(-(1 as libc::c_int as isize))).val.tt_ as libc::c_int
                    & 0xf as libc::c_int
                    == 3 as libc::c_int
                    && {
                        luaO_tostring(L, &mut (*top.offset(-(1 as libc::c_int as isize))).val)?;
                        1 as libc::c_int != 0
                    })
        {
            luaT_tryconcatTM(L)?;
        } else if (*top.offset(-(1 as libc::c_int as isize))).val.tt_ as libc::c_int
            == 4 as libc::c_int
                | (0 as libc::c_int) << 4 as libc::c_int
                | (1 as libc::c_int) << 6 as libc::c_int
            && (*((*top.offset(-(1 as libc::c_int as isize))).val.value_.gc as *mut TString)).shrlen
                as libc::c_int
                == 0 as libc::c_int
        {
            ((*top.offset(-(2 as libc::c_int as isize))).val.tt_ as libc::c_int
                & 0xf as libc::c_int
                == 4 as libc::c_int
                || (*top.offset(-(2 as libc::c_int as isize))).val.tt_ as libc::c_int
                    & 0xf as libc::c_int
                    == 3 as libc::c_int
                    && {
                        luaO_tostring(L, &mut (*top.offset(-(2 as libc::c_int as isize))).val)?;
                        1 as libc::c_int != 0
                    }) as libc::c_int;
        } else if (*top.offset(-(2 as libc::c_int as isize))).val.tt_ as libc::c_int
            == 4 as libc::c_int
                | (0 as libc::c_int) << 4 as libc::c_int
                | (1 as libc::c_int) << 6 as libc::c_int
            && (*((*top.offset(-(2 as libc::c_int as isize))).val.value_.gc as *mut TString)).shrlen
                as libc::c_int
                == 0 as libc::c_int
        {
            let mut io1: *mut TValue = &mut (*top.offset(-(2 as libc::c_int as isize))).val;
            let mut io2: *const TValue = &mut (*top.offset(-(1 as libc::c_int as isize))).val;
            (*io1).value_ = (*io2).value_;
            (*io1).tt_ = (*io2).tt_;
        } else {
            let mut tl: usize = if (*((*top.offset(-(1 as libc::c_int as isize))).val.value_.gc
                as *mut TString))
                .shrlen as libc::c_int
                != 0xff as libc::c_int
            {
                (*((*top.offset(-(1 as libc::c_int as isize))).val.value_.gc as *mut TString))
                    .shrlen as usize
            } else {
                (*((*top.offset(-(1 as libc::c_int as isize))).val.value_.gc as *mut TString))
                    .u
                    .lnglen
            };
            let mut ts: *mut TString = 0 as *mut TString;
            n = 1 as libc::c_int;
            while n < total
                && ((*top
                    .offset(-(n as isize))
                    .offset(-(1 as libc::c_int as isize)))
                .val
                .tt_ as libc::c_int
                    & 0xf as libc::c_int
                    == 4 as libc::c_int
                    || (*top
                        .offset(-(n as isize))
                        .offset(-(1 as libc::c_int as isize)))
                    .val
                    .tt_ as libc::c_int
                        & 0xf as libc::c_int
                        == 3 as libc::c_int
                        && {
                            luaO_tostring(
                                L,
                                &mut (*top
                                    .offset(-(n as isize))
                                    .offset(-(1 as libc::c_int as isize)))
                                .val,
                            )?;
                            1 as libc::c_int != 0
                        })
            {
                let mut l: usize = if (*((*top
                    .offset(-(n as isize))
                    .offset(-(1 as libc::c_int as isize)))
                .val
                .value_
                .gc as *mut TString))
                    .shrlen as libc::c_int
                    != 0xff as libc::c_int
                {
                    (*((*top
                        .offset(-(n as isize))
                        .offset(-(1 as libc::c_int as isize)))
                    .val
                    .value_
                    .gc as *mut TString))
                        .shrlen as usize
                } else {
                    (*((*top
                        .offset(-(n as isize))
                        .offset(-(1 as libc::c_int as isize)))
                    .val
                    .value_
                    .gc as *mut TString))
                        .u
                        .lnglen
                };
                if ((l
                    >= (if (::core::mem::size_of::<usize>() as libc::c_ulong)
                        < ::core::mem::size_of::<i64>() as libc::c_ulong
                    {
                        !(0 as libc::c_int as usize)
                    } else {
                        0x7fffffffffffffff as libc::c_longlong as usize
                    })
                    .wrapping_sub(::core::mem::size_of::<TString>())
                    .wrapping_sub(tl)) as libc::c_int
                    != 0 as libc::c_int) as libc::c_int as libc::c_long
                    != 0
                {
                    (*L).top = top.offset(-(total as isize));
                    luaG_runerror(L, "string length overflow")?;
                }
                tl = tl.wrapping_add(l);
                n += 1;
            }
            if tl <= 40 as libc::c_int as usize {
                let mut buff: [libc::c_char; 40] = [0; 40];
                copy2buff(top, n, buff.as_mut_ptr());
                ts = luaS_newlstr(L, buff.as_mut_ptr(), tl)?;
            } else {
                ts = luaS_createlngstrobj(L, tl);
                copy2buff(top, n, ((*ts).contents).as_mut_ptr());
            }
            let mut io: *mut TValue = &mut (*top.offset(-(n as isize))).val;
            let mut x_: *mut TString = ts;
            (*io).value_.gc = (x_ as *mut GCObject);
            (*io).tt_ = ((*x_).tt as libc::c_int | (1 as libc::c_int) << 6 as libc::c_int) as u8;
        }
        total -= n - 1 as libc::c_int;
        (*L).top = ((*L).top).offset(-((n - 1 as libc::c_int) as isize));
        if !(total > 1 as libc::c_int) {
            break Ok(());
        }
    }
}

pub unsafe fn luaV_objlen(
    mut L: *mut Thread,
    mut ra: StkId,
    mut rb: *const TValue,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut tm: *const TValue = 0 as *const TValue;
    match (*rb).tt_ as libc::c_int & 0x3f as libc::c_int {
        5 => {
            let mut h: *mut Table = ((*rb).value_.gc as *mut Table);
            tm = if ((*h).metatable).is_null() {
                0 as *const TValue
            } else if (*(*h).metatable).flags as libc::c_uint
                & (1 as libc::c_uint) << TM_LEN as libc::c_int
                != 0
            {
                0 as *const TValue
            } else {
                luaT_gettm(
                    (*h).metatable,
                    TM_LEN,
                    (*(*L).global).tmname[TM_LEN as usize].get(),
                )
            };
            if tm.is_null() {
                let mut io: *mut TValue = &mut (*ra).val;
                (*io).value_.i = luaH_getn(h) as i64;
                (*io).tt_ = (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                return Ok(());
            }
        }
        4 => {
            let mut io_0: *mut TValue = &mut (*ra).val;
            (*io_0).value_.i = (*((*rb).value_.gc as *mut TString)).shrlen as i64;
            (*io_0).tt_ = (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
            return Ok(());
        }
        20 => {
            let mut io_1: *mut TValue = &mut (*ra).val;
            (*io_1).value_.i = (*((*rb).value_.gc as *mut TString)).u.lnglen as i64;
            (*io_1).tt_ = (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
            return Ok(());
        }
        _ => {
            tm = luaT_gettmbyobj(L, rb, TM_LEN);
            if (((*tm).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int) as libc::c_int
                != 0 as libc::c_int) as libc::c_int as libc::c_long
                != 0
            {
                luaG_typeerror(L, rb, "get length of")?;
            }
        }
    }
    luaT_callTMres(L, tm, rb, rb, ra)
}

pub unsafe fn luaV_idiv(
    mut L: *mut Thread,
    mut m: i64,
    mut n: i64,
) -> Result<i64, Box<dyn std::error::Error>> {
    if (((n as u64).wrapping_add(1 as libc::c_uint as u64) <= 1 as libc::c_uint as u64)
        as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        if n == 0 as libc::c_int as i64 {
            luaG_runerror(L, "attempt to divide by zero")?;
        }
        return Ok((0 as libc::c_int as u64).wrapping_sub(m as u64) as i64);
    } else {
        let mut q: i64 = m / n;
        if m ^ n < 0 as libc::c_int as i64 && m % n != 0 as libc::c_int as i64 {
            q -= 1 as libc::c_int as i64;
        }
        return Ok(q);
    };
}

pub unsafe fn luaV_mod(
    mut L: *mut Thread,
    mut m: i64,
    mut n: i64,
) -> Result<i64, Box<dyn std::error::Error>> {
    if (((n as u64).wrapping_add(1 as libc::c_uint as u64) <= 1 as libc::c_uint as u64)
        as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        if n == 0 as libc::c_int as i64 {
            luaG_runerror(L, "attempt to perform 'n%0'")?;
        }
        return Ok(0 as libc::c_int as i64);
    } else {
        let mut r: i64 = m % n;
        if r != 0 as libc::c_int as i64 && r ^ n < 0 as libc::c_int as i64 {
            r += n;
        }
        return Ok(r);
    };
}

pub unsafe extern "C" fn luaV_modf(mut L: *mut Thread, mut m: f64, mut n: f64) -> f64 {
    let mut r: f64 = 0.;
    r = fmod(m, n);
    if if r > 0 as libc::c_int as f64 {
        (n < 0 as libc::c_int as f64) as libc::c_int
    } else {
        (r < 0 as libc::c_int as f64 && n > 0 as libc::c_int as f64) as libc::c_int
    } != 0
    {
        r += n;
    }
    return r;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaV_shiftl(mut x: i64, mut y: i64) -> i64 {
    if y < 0 as libc::c_int as i64 {
        if y <= -((::core::mem::size_of::<i64>() as libc::c_ulong)
            .wrapping_mul(8 as libc::c_int as libc::c_ulong) as libc::c_int) as i64
        {
            return 0 as libc::c_int as i64;
        } else {
            return (x as u64 >> -y as u64) as i64;
        }
    } else if y
        >= (::core::mem::size_of::<i64>() as libc::c_ulong)
            .wrapping_mul(8 as libc::c_int as libc::c_ulong) as libc::c_int as i64
    {
        return 0 as libc::c_int as i64;
    } else {
        return ((x as u64) << y as u64) as i64;
    };
}
unsafe extern "C" fn pushclosure(
    mut L: *mut Thread,
    mut p: *mut Proto,
    mut encup: *mut *mut UpVal,
    mut base: StkId,
    mut ra: StkId,
) {
    let mut nup: libc::c_int = (*p).sizeupvalues;
    let mut uv: *mut Upvaldesc = (*p).upvalues;
    let mut i: libc::c_int = 0;
    let mut ncl: *mut LClosure = luaF_newLclosure(L, nup);
    (*ncl).p = p;
    let mut io: *mut TValue = &mut (*ra).val;
    let mut x_: *mut LClosure = ncl;
    (*io).value_.gc = (x_ as *mut GCObject);
    (*io).tt_ = (6 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 6 as libc::c_int) as u8;
    i = 0 as libc::c_int;
    while i < nup {
        if (*uv.offset(i as isize)).instack != 0 {
            let ref mut fresh0 = *((*ncl).upvals).as_mut_ptr().offset(i as isize);
            *fresh0 = luaF_findupval(
                L,
                base.offset((*uv.offset(i as isize)).idx as libc::c_int as isize),
            );
        } else {
            let ref mut fresh1 = *((*ncl).upvals).as_mut_ptr().offset(i as isize);
            *fresh1 = *encup.offset((*uv.offset(i as isize)).idx as isize);
        }
        if (*ncl).marked as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
            && (**((*ncl).upvals).as_mut_ptr().offset(i as isize)).marked as libc::c_int
                & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
                != 0
        {
            luaC_barrier_(
                L,
                (ncl as *mut GCObject),
                (*((*ncl).upvals).as_mut_ptr().offset(i as isize) as *mut GCObject),
            );
        } else {
        };
        i += 1;
    }
}

pub unsafe fn luaV_finishOp(mut L: *mut Thread) -> Result<(), Box<dyn std::error::Error>> {
    let mut ci: *mut CallInfo = (*L).ci;
    let mut base: StkId = ((*ci).func).offset(1 as libc::c_int as isize);
    let mut inst: u32 = *((*ci).u.savedpc).offset(-(1 as libc::c_int as isize));
    let mut op: OpCode = (inst >> 0 as libc::c_int
        & !(!(0 as libc::c_int as u32) << 7 as libc::c_int) << 0 as libc::c_int)
        as OpCode;
    match op as libc::c_uint {
        46 | 47 | 48 => {
            let mut io1: *mut TValue = &mut (*base.offset(
                (*((*ci).u.savedpc).offset(-(2 as libc::c_int as isize))
                    >> 0 as libc::c_int + 7 as libc::c_int
                    & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                    as libc::c_int as isize,
            ))
            .val;
            (*L).top = ((*L).top).offset(-1);
            let mut io2: *const TValue = &raw mut (*(*L).top).val;
            (*io1).value_ = (*io2).value_;
            (*io1).tt_ = (*io2).tt_;
        }
        49 | 50 | 52 | 11 | 12 | 13 | 14 | 20 => {
            let mut io1_0: *mut TValue = &mut (*base.offset(
                (inst >> 0 as libc::c_int + 7 as libc::c_int
                    & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                    as libc::c_int as isize,
            ))
            .val;
            (*L).top = ((*L).top).offset(-1);
            let mut io2_0: *const TValue = &raw mut (*(*L).top).val;
            (*io1_0).value_ = (*io2_0).value_;
            (*io1_0).tt_ = (*io2_0).tt_;
        }
        58 | 59 | 62 | 63 | 64 | 65 | 57 => {
            let mut res: libc::c_int = !((*((*L).top).offset(-(1 as libc::c_int as isize))).val.tt_
                as libc::c_int
                == 1 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                || (*((*L).top).offset(-(1 as libc::c_int as isize))).val.tt_ as libc::c_int
                    & 0xf as libc::c_int
                    == 0 as libc::c_int) as libc::c_int;
            (*L).top = ((*L).top).offset(-1);

            if res
                != (inst >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int
                    & !(!(0 as libc::c_int as u32) << 1 as libc::c_int) << 0 as libc::c_int)
                    as libc::c_int
            {
                (*ci).u.savedpc = ((*ci).u.savedpc).offset(1);
                (*ci).u.savedpc;
            }
        }
        53 => {
            let mut top: StkId = ((*L).top).offset(-(1 as libc::c_int as isize));
            let mut a: libc::c_int = (inst >> 0 as libc::c_int + 7 as libc::c_int
                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                as libc::c_int;
            let mut total: libc::c_int = top
                .offset(-(1 as libc::c_int as isize))
                .offset_from(base.offset(a as isize))
                as libc::c_long as libc::c_int;
            let mut io1_1: *mut TValue = &mut (*top.offset(-(2 as libc::c_int as isize))).val;
            let mut io2_1: *const TValue = &mut (*top).val;
            (*io1_1).value_ = (*io2_1).value_;
            (*io1_1).tt_ = (*io2_1).tt_;
            (*L).top = top.offset(-(1 as libc::c_int as isize));
            luaV_concat(L, total)?;
        }
        54 => {
            (*ci).u.savedpc = ((*ci).u.savedpc).offset(-1);
            (*ci).u.savedpc;
        }
        70 => {
            let mut ra: StkId = base.offset(
                (inst >> 0 as libc::c_int + 7 as libc::c_int
                    & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                    as libc::c_int as isize,
            );
            (*L).top = ra.offset((*ci).u2.nres as isize);
            (*ci).u.savedpc = ((*ci).u.savedpc).offset(-1);
            (*ci).u.savedpc;
        }
        _ => {}
    };

    Ok(())
}

pub unsafe fn luaV_execute(
    mut L: *mut Thread,
    mut ci: *mut CallInfo,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut i: u32 = 0;
    let mut ra_65: StkId = 0 as *mut StackValue;
    let mut newci: *mut CallInfo = 0 as *mut CallInfo;
    let mut b_4: libc::c_int = 0;
    let mut nresults: libc::c_int = 0;
    let mut current_block: u64;
    let mut cl: *mut LClosure = 0 as *mut LClosure;
    let mut k: *mut TValue = 0 as *mut TValue;
    let mut base: StkId = 0 as *mut StackValue;
    let mut pc: *const u32 = 0 as *const u32;
    let mut trap: libc::c_int = 0;
    '_startfunc: loop {
        trap = (*L).hookmask.get();
        '_returning: loop {
            cl = ((*(*ci).func).val.value_.gc as *mut LClosure);
            k = (*(*cl).p).k;
            pc = (*ci).u.savedpc;
            if (trap != 0 as libc::c_int) as libc::c_int as libc::c_long != 0 {
                trap = luaG_tracecall(L)?;
            }
            base = ((*ci).func).offset(1 as libc::c_int as isize);
            loop {
                i = 0;
                if (trap != 0 as libc::c_int) as libc::c_int as libc::c_long != 0 {
                    trap = luaG_traceexec(L, pc)?;
                    base = ((*ci).func).offset(1 as libc::c_int as isize);
                }
                let fresh2 = pc;
                pc = pc.offset(1);
                i = *fresh2;
                match (i >> 0 as libc::c_int
                    & !(!(0 as libc::c_int as u32) << 7 as libc::c_int) << 0 as libc::c_int)
                    as OpCode as libc::c_uint
                {
                    0 => {
                        let mut ra: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut io1: *mut TValue = &mut (*ra).val;
                        let mut io2: *const TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        (*io1).value_ = (*io2).value_;
                        (*io1).tt_ = (*io2).tt_;
                        continue;
                    }
                    1 => {
                        let mut ra_0: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut b: i64 =
                            ((i >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32)
                                    << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int)
                                    << 0 as libc::c_int)
                                as libc::c_int
                                - (((1 as libc::c_int)
                                    << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int)
                                    - 1 as libc::c_int
                                    >> 1 as libc::c_int)) as i64;
                        let mut io: *mut TValue = &mut (*ra_0).val;
                        (*io).value_.i = b;
                        (*io).tt_ =
                            (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                        continue;
                    }
                    2 => {
                        let mut ra_1: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut b_0: libc::c_int =
                            (i >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32)
                                    << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                - (((1 as libc::c_int)
                                    << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int)
                                    - 1 as libc::c_int
                                    >> 1 as libc::c_int);
                        let mut io_0: *mut TValue = &mut (*ra_1).val;
                        (*io_0).value_.n = b_0 as f64;
                        (*io_0).tt_ =
                            (3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
                        continue;
                    }
                    3 => {
                        let mut ra_2: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut rb: *mut TValue = k.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32)
                                    << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut io1_0: *mut TValue = &mut (*ra_2).val;
                        let mut io2_0: *const TValue = rb;
                        (*io1_0).value_ = (*io2_0).value_;
                        (*io1_0).tt_ = (*io2_0).tt_;
                        continue;
                    }
                    4 => {
                        let mut ra_3: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut rb_0: *mut TValue = 0 as *mut TValue;
                        rb_0 = k.offset(
                            (*pc >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32)
                                    << 8 as libc::c_int
                                        + 8 as libc::c_int
                                        + 1 as libc::c_int
                                        + 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        pc = pc.offset(1);
                        let mut io1_1: *mut TValue = &mut (*ra_3).val;
                        let mut io2_1: *const TValue = rb_0;
                        (*io1_1).value_ = (*io2_1).value_;
                        (*io1_1).tt_ = (*io2_1).tt_;
                        continue;
                    }
                    5 => {
                        let mut ra_4: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        (*ra_4).val.tt_ =
                            (1 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                        continue;
                    }
                    6 => {
                        let mut ra_5: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        (*ra_5).val.tt_ =
                            (1 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                        pc = pc.offset(1);
                        continue;
                    }
                    7 => {
                        let mut ra_6: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        (*ra_6).val.tt_ =
                            (1 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
                        continue;
                    }
                    8 => {
                        let mut ra_7: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut b_1: libc::c_int = (i
                            >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                            as libc::c_int;
                        loop {
                            let fresh3 = ra_7;
                            ra_7 = ra_7.offset(1);
                            (*fresh3).val.tt_ =
                                (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                            let fresh4 = b_1;
                            b_1 = b_1 - 1;
                            if !(fresh4 != 0) {
                                break;
                            }
                        }
                        continue;
                    }
                    9 => {
                        let mut ra_8: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut b_2: libc::c_int = (i
                            >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                            as libc::c_int;
                        let mut io1_2: *mut TValue = &mut (*ra_8).val;
                        let mut io2_2: *const TValue =
                            (**((*cl).upvals).as_mut_ptr().offset(b_2 as isize)).v.p;
                        (*io1_2).value_ = (*io2_2).value_;
                        (*io1_2).tt_ = (*io2_2).tt_;
                        continue;
                    }
                    10 => {
                        let mut ra_9: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut uv: *mut UpVal = *((*cl).upvals).as_mut_ptr().offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut io1_3: *mut TValue = (*uv).v.p;
                        let mut io2_3: *const TValue = &mut (*ra_9).val;
                        (*io1_3).value_ = (*io2_3).value_;
                        (*io1_3).tt_ = (*io2_3).tt_;
                        if (*ra_9).val.tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int
                            != 0
                        {
                            if (*uv).marked as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int
                                != 0
                                && (*(*ra_9).val.value_.gc).marked as libc::c_int
                                    & ((1 as libc::c_int) << 3 as libc::c_int
                                        | (1 as libc::c_int) << 4 as libc::c_int)
                                    != 0
                            {
                                luaC_barrier_(
                                    L,
                                    (uv as *mut GCObject),
                                    ((*ra_9).val.value_.gc as *mut GCObject),
                                );
                            } else {
                            };
                        } else {
                        };
                        continue;
                    }
                    11 => {
                        let mut ra_10: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut slot: *const TValue = 0 as *const TValue;
                        let mut upval: *mut TValue = (**((*cl).upvals).as_mut_ptr().offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .v
                        .p;
                        let mut rc: *mut TValue = k.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut key: *mut TString = ((*rc).value_.gc as *mut TString);
                        if if !((*upval).tt_ as libc::c_int
                            == 5 as libc::c_int
                                | (0 as libc::c_int) << 4 as libc::c_int
                                | (1 as libc::c_int) << 6 as libc::c_int)
                        {
                            slot = 0 as *const TValue;
                            0 as libc::c_int
                        } else {
                            slot = luaH_getshortstr(((*upval).value_.gc as *mut Table), key);
                            !((*slot).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int)
                                as libc::c_int
                        } != 0
                        {
                            let mut io1_4: *mut TValue = &mut (*ra_10).val;
                            let mut io2_4: *const TValue = slot;
                            (*io1_4).value_ = (*io2_4).value_;
                            (*io1_4).tt_ = (*io2_4).tt_;
                        } else {
                            (*ci).u.savedpc = pc;
                            (*L).top = (*ci).top;
                            luaV_finishget(L, upval, rc, ra_10, slot)?;
                            trap = (*ci).u.trap;
                        }
                        continue;
                    }
                    12 => {
                        let mut ra_11: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut slot_0: *const TValue = 0 as *const TValue;
                        let mut rb_1: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut rc_0: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut n: u64 = 0;
                        if if (*rc_0).tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                        {
                            n = (*rc_0).value_.i as u64;
                            (if !((*rb_1).tt_ as libc::c_int
                                == 5 as libc::c_int
                                    | (0 as libc::c_int) << 4 as libc::c_int
                                    | (1 as libc::c_int) << 6 as libc::c_int)
                            {
                                slot_0 = 0 as *const TValue;
                                0 as libc::c_int
                            } else {
                                slot_0 = (if n.wrapping_sub(1 as libc::c_uint as u64)
                                    < (*((*rb_1).value_.gc as *mut Table)).alimit as u64
                                {
                                    &mut *((*((*rb_1).value_.gc as *mut Table)).array)
                                        .offset(n.wrapping_sub(1 as libc::c_int as u64) as isize)
                                        as *mut TValue
                                        as *const TValue
                                } else {
                                    luaH_getint(((*rb_1).value_.gc as *mut Table), n as i64)
                                });
                                !((*slot_0).tt_ as libc::c_int & 0xf as libc::c_int
                                    == 0 as libc::c_int)
                                    as libc::c_int
                            })
                        } else if !((*rb_1).tt_ as libc::c_int
                            == 5 as libc::c_int
                                | (0 as libc::c_int) << 4 as libc::c_int
                                | (1 as libc::c_int) << 6 as libc::c_int)
                        {
                            slot_0 = 0 as *const TValue;
                            0 as libc::c_int
                        } else {
                            slot_0 = luaH_get(((*rb_1).value_.gc as *mut Table), rc_0);
                            !((*slot_0).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int)
                                as libc::c_int
                        } != 0
                        {
                            let mut io1_5: *mut TValue = &mut (*ra_11).val;
                            let mut io2_5: *const TValue = slot_0;
                            (*io1_5).value_ = (*io2_5).value_;
                            (*io1_5).tt_ = (*io2_5).tt_;
                        } else {
                            (*ci).u.savedpc = pc;
                            (*L).top = (*ci).top;
                            luaV_finishget(L, rb_1, rc_0, ra_11, slot_0)?;
                            trap = (*ci).u.trap;
                        }
                        continue;
                    }
                    13 => {
                        let mut ra_12: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut slot_1: *const TValue = 0 as *const TValue;
                        let mut rb_2: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut c: libc::c_int = (i
                            >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                            as libc::c_int;
                        if if !((*rb_2).tt_ as libc::c_int
                            == 5 as libc::c_int
                                | (0 as libc::c_int) << 4 as libc::c_int
                                | (1 as libc::c_int) << 6 as libc::c_int)
                        {
                            slot_1 = 0 as *const TValue;
                            0 as libc::c_int
                        } else {
                            slot_1 = (if (c as u64).wrapping_sub(1 as libc::c_uint as u64)
                                < (*((*rb_2).value_.gc as *mut Table)).alimit as u64
                            {
                                &mut *((*((*rb_2).value_.gc as *mut Table)).array)
                                    .offset((c - 1 as libc::c_int) as isize)
                                    as *mut TValue as *const TValue
                            } else {
                                luaH_getint(((*rb_2).value_.gc as *mut Table), c as i64)
                            });
                            !((*slot_1).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int)
                                as libc::c_int
                        } != 0
                        {
                            let mut io1_6: *mut TValue = &mut (*ra_12).val;
                            let mut io2_6: *const TValue = slot_1;
                            (*io1_6).value_ = (*io2_6).value_;
                            (*io1_6).tt_ = (*io2_6).tt_;
                        } else {
                            let mut key_0: TValue = TValue {
                                value_: Value {
                                    gc: 0 as *mut GCObject,
                                },
                                tt_: 0,
                            };
                            let mut io_1: *mut TValue = &mut key_0;
                            (*io_1).value_.i = c as i64;
                            (*io_1).tt_ =
                                (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                            (*ci).u.savedpc = pc;
                            (*L).top = (*ci).top;
                            luaV_finishget(L, rb_2, &mut key_0, ra_12, slot_1)?;
                            trap = (*ci).u.trap;
                        }
                        continue;
                    }
                    14 => {
                        let mut ra_13: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut slot_2: *const TValue = 0 as *const TValue;
                        let mut rb_3: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut rc_1: *mut TValue = k.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut key_1: *mut TString = ((*rc_1).value_.gc as *mut TString);
                        if if !((*rb_3).tt_ as libc::c_int
                            == 5 as libc::c_int
                                | (0 as libc::c_int) << 4 as libc::c_int
                                | (1 as libc::c_int) << 6 as libc::c_int)
                        {
                            slot_2 = 0 as *const TValue;
                            0 as libc::c_int
                        } else {
                            slot_2 = luaH_getshortstr(((*rb_3).value_.gc as *mut Table), key_1);
                            !((*slot_2).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int)
                                as libc::c_int
                        } != 0
                        {
                            let mut io1_7: *mut TValue = &mut (*ra_13).val;
                            let mut io2_7: *const TValue = slot_2;
                            (*io1_7).value_ = (*io2_7).value_;
                            (*io1_7).tt_ = (*io2_7).tt_;
                        } else {
                            (*ci).u.savedpc = pc;
                            (*L).top = (*ci).top;
                            luaV_finishget(L, rb_3, rc_1, ra_13, slot_2)?;
                            trap = (*ci).u.trap;
                        }
                        continue;
                    }
                    15 => {
                        let mut slot_3: *const TValue = 0 as *const TValue;
                        let mut upval_0: *mut TValue = (**((*cl).upvals).as_mut_ptr().offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .v
                        .p;
                        let mut rb_4: *mut TValue = k.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut rc_2: *mut TValue = if (i
                            & (1 as libc::c_uint)
                                << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int)
                            as libc::c_int
                            != 0
                        {
                            k.offset(
                                (i >> 0 as libc::c_int
                                    + 7 as libc::c_int
                                    + 8 as libc::c_int
                                    + 1 as libc::c_int
                                    + 8 as libc::c_int
                                    & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                        << 0 as libc::c_int)
                                    as libc::c_int as isize,
                            )
                        } else {
                            &mut (*base.offset(
                                (i >> 0 as libc::c_int
                                    + 7 as libc::c_int
                                    + 8 as libc::c_int
                                    + 1 as libc::c_int
                                    + 8 as libc::c_int
                                    & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                        << 0 as libc::c_int)
                                    as libc::c_int as isize,
                            ))
                            .val
                        };
                        let mut key_2: *mut TString = ((*rb_4).value_.gc as *mut TString);
                        if if !((*upval_0).tt_ as libc::c_int
                            == 5 as libc::c_int
                                | (0 as libc::c_int) << 4 as libc::c_int
                                | (1 as libc::c_int) << 6 as libc::c_int)
                        {
                            slot_3 = 0 as *const TValue;
                            0 as libc::c_int
                        } else {
                            slot_3 = luaH_getshortstr(((*upval_0).value_.gc as *mut Table), key_2);
                            !((*slot_3).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int)
                                as libc::c_int
                        } != 0
                        {
                            let mut io1_8: *mut TValue = slot_3 as *mut TValue;
                            let mut io2_8: *const TValue = rc_2;
                            (*io1_8).value_ = (*io2_8).value_;
                            (*io1_8).tt_ = (*io2_8).tt_;
                            if (*rc_2).tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int
                                != 0
                            {
                                if (*(*upval_0).value_.gc).marked as libc::c_int
                                    & (1 as libc::c_int) << 5 as libc::c_int
                                    != 0
                                    && (*(*rc_2).value_.gc).marked as libc::c_int
                                        & ((1 as libc::c_int) << 3 as libc::c_int
                                            | (1 as libc::c_int) << 4 as libc::c_int)
                                        != 0
                                {
                                    luaC_barrierback_(L, (*upval_0).value_.gc);
                                } else {
                                };
                            } else {
                            };
                        } else {
                            (*ci).u.savedpc = pc;
                            (*L).top = (*ci).top;
                            luaV_finishset(L, upval_0, rb_4, rc_2, slot_3)?;
                            trap = (*ci).u.trap;
                        }
                        continue;
                    }
                    16 => {
                        let mut ra_14: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut slot_4: *const TValue = 0 as *const TValue;
                        let mut rb_5: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut rc_3: *mut TValue = if (i
                            & (1 as libc::c_uint)
                                << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int)
                            as libc::c_int
                            != 0
                        {
                            k.offset(
                                (i >> 0 as libc::c_int
                                    + 7 as libc::c_int
                                    + 8 as libc::c_int
                                    + 1 as libc::c_int
                                    + 8 as libc::c_int
                                    & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                        << 0 as libc::c_int)
                                    as libc::c_int as isize,
                            )
                        } else {
                            &mut (*base.offset(
                                (i >> 0 as libc::c_int
                                    + 7 as libc::c_int
                                    + 8 as libc::c_int
                                    + 1 as libc::c_int
                                    + 8 as libc::c_int
                                    & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                        << 0 as libc::c_int)
                                    as libc::c_int as isize,
                            ))
                            .val
                        };
                        let mut n_0: u64 = 0;
                        if if (*rb_5).tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                        {
                            n_0 = (*rb_5).value_.i as u64;
                            (if !((*ra_14).val.tt_ as libc::c_int
                                == 5 as libc::c_int
                                    | (0 as libc::c_int) << 4 as libc::c_int
                                    | (1 as libc::c_int) << 6 as libc::c_int)
                            {
                                slot_4 = 0 as *const TValue;
                                0 as libc::c_int
                            } else {
                                slot_4 = (if n_0.wrapping_sub(1 as libc::c_uint as u64)
                                    < (*((*ra_14).val.value_.gc as *mut Table)).alimit as u64
                                {
                                    &mut *((*((*ra_14).val.value_.gc as *mut Table)).array)
                                        .offset(n_0.wrapping_sub(1 as libc::c_int as u64) as isize)
                                        as *mut TValue
                                        as *const TValue
                                } else {
                                    luaH_getint(((*ra_14).val.value_.gc as *mut Table), n_0 as i64)
                                });
                                !((*slot_4).tt_ as libc::c_int & 0xf as libc::c_int
                                    == 0 as libc::c_int)
                                    as libc::c_int
                            })
                        } else if !((*ra_14).val.tt_ as libc::c_int
                            == 5 as libc::c_int
                                | (0 as libc::c_int) << 4 as libc::c_int
                                | (1 as libc::c_int) << 6 as libc::c_int)
                        {
                            slot_4 = 0 as *const TValue;
                            0 as libc::c_int
                        } else {
                            slot_4 = luaH_get(((*ra_14).val.value_.gc as *mut Table), rb_5);
                            !((*slot_4).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int)
                                as libc::c_int
                        } != 0
                        {
                            let mut io1_9: *mut TValue = slot_4 as *mut TValue;
                            let mut io2_9: *const TValue = rc_3;
                            (*io1_9).value_ = (*io2_9).value_;
                            (*io1_9).tt_ = (*io2_9).tt_;
                            if (*rc_3).tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int
                                != 0
                            {
                                if (*(*ra_14).val.value_.gc).marked as libc::c_int
                                    & (1 as libc::c_int) << 5 as libc::c_int
                                    != 0
                                    && (*(*rc_3).value_.gc).marked as libc::c_int
                                        & ((1 as libc::c_int) << 3 as libc::c_int
                                            | (1 as libc::c_int) << 4 as libc::c_int)
                                        != 0
                                {
                                    luaC_barrierback_(L, (*ra_14).val.value_.gc);
                                } else {
                                };
                            } else {
                            };
                        } else {
                            (*ci).u.savedpc = pc;
                            (*L).top = (*ci).top;
                            luaV_finishset(L, &mut (*ra_14).val, rb_5, rc_3, slot_4)?;
                            trap = (*ci).u.trap;
                        }
                        continue;
                    }
                    17 => {
                        let mut ra_15: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut slot_5: *const TValue = 0 as *const TValue;
                        let mut c_0: libc::c_int = (i
                            >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                            as libc::c_int;
                        let mut rc_4: *mut TValue = if (i
                            & (1 as libc::c_uint)
                                << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int)
                            as libc::c_int
                            != 0
                        {
                            k.offset(
                                (i >> 0 as libc::c_int
                                    + 7 as libc::c_int
                                    + 8 as libc::c_int
                                    + 1 as libc::c_int
                                    + 8 as libc::c_int
                                    & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                        << 0 as libc::c_int)
                                    as libc::c_int as isize,
                            )
                        } else {
                            &mut (*base.offset(
                                (i >> 0 as libc::c_int
                                    + 7 as libc::c_int
                                    + 8 as libc::c_int
                                    + 1 as libc::c_int
                                    + 8 as libc::c_int
                                    & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                        << 0 as libc::c_int)
                                    as libc::c_int as isize,
                            ))
                            .val
                        };
                        if if !((*ra_15).val.tt_ as libc::c_int
                            == 5 as libc::c_int
                                | (0 as libc::c_int) << 4 as libc::c_int
                                | (1 as libc::c_int) << 6 as libc::c_int)
                        {
                            slot_5 = 0 as *const TValue;
                            0 as libc::c_int
                        } else {
                            slot_5 = (if (c_0 as u64).wrapping_sub(1 as libc::c_uint as u64)
                                < (*((*ra_15).val.value_.gc as *mut Table)).alimit as u64
                            {
                                &mut *((*((*ra_15).val.value_.gc as *mut Table)).array)
                                    .offset((c_0 - 1 as libc::c_int) as isize)
                                    as *mut TValue as *const TValue
                            } else {
                                luaH_getint(((*ra_15).val.value_.gc as *mut Table), c_0 as i64)
                            });
                            !((*slot_5).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int)
                                as libc::c_int
                        } != 0
                        {
                            let mut io1_10: *mut TValue = slot_5 as *mut TValue;
                            let mut io2_10: *const TValue = rc_4;
                            (*io1_10).value_ = (*io2_10).value_;
                            (*io1_10).tt_ = (*io2_10).tt_;
                            if (*rc_4).tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int
                                != 0
                            {
                                if (*(*ra_15).val.value_.gc).marked as libc::c_int
                                    & (1 as libc::c_int) << 5 as libc::c_int
                                    != 0
                                    && (*(*rc_4).value_.gc).marked as libc::c_int
                                        & ((1 as libc::c_int) << 3 as libc::c_int
                                            | (1 as libc::c_int) << 4 as libc::c_int)
                                        != 0
                                {
                                    luaC_barrierback_(L, (*ra_15).val.value_.gc);
                                } else {
                                };
                            } else {
                            };
                        } else {
                            let mut key_3: TValue = TValue {
                                value_: Value {
                                    gc: 0 as *mut GCObject,
                                },
                                tt_: 0,
                            };
                            let mut io_2: *mut TValue = &mut key_3;
                            (*io_2).value_.i = c_0 as i64;
                            (*io_2).tt_ =
                                (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                            (*ci).u.savedpc = pc;
                            (*L).top = (*ci).top;
                            luaV_finishset(L, &mut (*ra_15).val, &mut key_3, rc_4, slot_5)?;
                            trap = (*ci).u.trap;
                        }
                        continue;
                    }
                    18 => {
                        let mut ra_16: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut slot_6: *const TValue = 0 as *const TValue;
                        let mut rb_6: *mut TValue = k.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut rc_5: *mut TValue = if (i
                            & (1 as libc::c_uint)
                                << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int)
                            as libc::c_int
                            != 0
                        {
                            k.offset(
                                (i >> 0 as libc::c_int
                                    + 7 as libc::c_int
                                    + 8 as libc::c_int
                                    + 1 as libc::c_int
                                    + 8 as libc::c_int
                                    & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                        << 0 as libc::c_int)
                                    as libc::c_int as isize,
                            )
                        } else {
                            &mut (*base.offset(
                                (i >> 0 as libc::c_int
                                    + 7 as libc::c_int
                                    + 8 as libc::c_int
                                    + 1 as libc::c_int
                                    + 8 as libc::c_int
                                    & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                        << 0 as libc::c_int)
                                    as libc::c_int as isize,
                            ))
                            .val
                        };
                        let mut key_4: *mut TString = ((*rb_6).value_.gc as *mut TString);
                        if if !((*ra_16).val.tt_ as libc::c_int
                            == 5 as libc::c_int
                                | (0 as libc::c_int) << 4 as libc::c_int
                                | (1 as libc::c_int) << 6 as libc::c_int)
                        {
                            slot_6 = 0 as *const TValue;
                            0 as libc::c_int
                        } else {
                            slot_6 =
                                luaH_getshortstr(((*ra_16).val.value_.gc as *mut Table), key_4);
                            !((*slot_6).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int)
                                as libc::c_int
                        } != 0
                        {
                            let mut io1_11: *mut TValue = slot_6 as *mut TValue;
                            let mut io2_11: *const TValue = rc_5;
                            (*io1_11).value_ = (*io2_11).value_;
                            (*io1_11).tt_ = (*io2_11).tt_;
                            if (*rc_5).tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int
                                != 0
                            {
                                if (*(*ra_16).val.value_.gc).marked as libc::c_int
                                    & (1 as libc::c_int) << 5 as libc::c_int
                                    != 0
                                    && (*(*rc_5).value_.gc).marked as libc::c_int
                                        & ((1 as libc::c_int) << 3 as libc::c_int
                                            | (1 as libc::c_int) << 4 as libc::c_int)
                                        != 0
                                {
                                    luaC_barrierback_(L, (*ra_16).val.value_.gc);
                                } else {
                                };
                            } else {
                            };
                        } else {
                            (*ci).u.savedpc = pc;
                            (*L).top = (*ci).top;
                            luaV_finishset(L, &mut (*ra_16).val, rb_6, rc_5, slot_6)?;
                            trap = (*ci).u.trap;
                        }
                        continue;
                    }
                    19 => {
                        let mut ra_17: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut b_3: libc::c_int = (i
                            >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                            as libc::c_int;
                        let mut c_1: libc::c_int = (i
                            >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                            as libc::c_int;
                        let mut t: *mut Table = 0 as *mut Table;
                        if b_3 > 0 as libc::c_int {
                            b_3 = (1 as libc::c_int) << b_3 - 1 as libc::c_int;
                        }
                        if (i
                            & (1 as libc::c_uint)
                                << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int)
                            as libc::c_int
                            != 0
                        {
                            c_1 += (*pc >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32)
                                    << 8 as libc::c_int
                                        + 8 as libc::c_int
                                        + 1 as libc::c_int
                                        + 8 as libc::c_int)
                                    << 0 as libc::c_int)
                                as libc::c_int
                                * (((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int
                                    + 1 as libc::c_int);
                        }
                        pc = pc.offset(1);
                        (*L).top = ra_17.offset(1 as libc::c_int as isize);
                        t = luaH_new(L)?;
                        let mut io_3: *mut TValue = &mut (*ra_17).val;
                        let mut x_: *mut Table = t;
                        (*io_3).value_.gc = (x_ as *mut GCObject);
                        (*io_3).tt_ = (5 as libc::c_int
                            | (0 as libc::c_int) << 4 as libc::c_int
                            | (1 as libc::c_int) << 6 as libc::c_int)
                            as u8;

                        if b_3 != 0 as libc::c_int || c_1 != 0 as libc::c_int {
                            luaH_resize(L, t, c_1 as libc::c_uint, b_3 as libc::c_uint)?;
                        }

                        if (*(*L).global).gc.debt() > 0 {
                            (*ci).u.savedpc = pc;
                            (*L).top = ra_17.offset(1 as libc::c_int as isize);
                            luaC_step(L);
                            trap = (*ci).u.trap;
                        }

                        continue;
                    }
                    20 => {
                        let mut ra_18: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut slot_7: *const TValue = 0 as *const TValue;
                        let mut rb_7: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut rc_6: *mut TValue = if (i
                            & (1 as libc::c_uint)
                                << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int)
                            as libc::c_int
                            != 0
                        {
                            k.offset(
                                (i >> 0 as libc::c_int
                                    + 7 as libc::c_int
                                    + 8 as libc::c_int
                                    + 1 as libc::c_int
                                    + 8 as libc::c_int
                                    & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                        << 0 as libc::c_int)
                                    as libc::c_int as isize,
                            )
                        } else {
                            &mut (*base.offset(
                                (i >> 0 as libc::c_int
                                    + 7 as libc::c_int
                                    + 8 as libc::c_int
                                    + 1 as libc::c_int
                                    + 8 as libc::c_int
                                    & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                        << 0 as libc::c_int)
                                    as libc::c_int as isize,
                            ))
                            .val
                        };
                        let mut key_5: *mut TString = ((*rc_6).value_.gc as *mut TString);
                        let mut io1_12: *mut TValue =
                            &mut (*ra_18.offset(1 as libc::c_int as isize)).val;
                        let mut io2_12: *const TValue = rb_7;
                        (*io1_12).value_ = (*io2_12).value_;
                        (*io1_12).tt_ = (*io2_12).tt_;
                        if if !((*rb_7).tt_ as libc::c_int
                            == 5 as libc::c_int
                                | (0 as libc::c_int) << 4 as libc::c_int
                                | (1 as libc::c_int) << 6 as libc::c_int)
                        {
                            slot_7 = 0 as *const TValue;
                            0 as libc::c_int
                        } else {
                            slot_7 = luaH_getstr(((*rb_7).value_.gc as *mut Table), key_5);
                            !((*slot_7).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int)
                                as libc::c_int
                        } != 0
                        {
                            let mut io1_13: *mut TValue = &mut (*ra_18).val;
                            let mut io2_13: *const TValue = slot_7;
                            (*io1_13).value_ = (*io2_13).value_;
                            (*io1_13).tt_ = (*io2_13).tt_;
                        } else {
                            (*ci).u.savedpc = pc;
                            (*L).top = (*ci).top;
                            luaV_finishget(L, rb_7, rc_6, ra_18, slot_7)?;
                            trap = (*ci).u.trap;
                        }
                        continue;
                    }
                    21 => {
                        let mut ra_19: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut v1: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut imm: libc::c_int = (i
                            >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                            as libc::c_int
                            - (((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int
                                >> 1 as libc::c_int);
                        if (*v1).tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                        {
                            let mut iv1: i64 = (*v1).value_.i;
                            pc = pc.offset(1);
                            let mut io_4: *mut TValue = &mut (*ra_19).val;
                            (*io_4).value_.i = (iv1 as u64).wrapping_add(imm as u64) as i64;
                            (*io_4).tt_ =
                                (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                        } else if (*v1).tt_ as libc::c_int
                            == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                        {
                            let mut nb: f64 = (*v1).value_.n;
                            let mut fimm: f64 = imm as f64;
                            pc = pc.offset(1);
                            let mut io_5: *mut TValue = &mut (*ra_19).val;
                            (*io_5).value_.n = nb + fimm;
                            (*io_5).tt_ =
                                (3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
                        }
                        continue;
                    }
                    22 => {
                        let mut v1_0: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut v2: *mut TValue = k.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut ra_20: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        if (*v1_0).tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                            && (*v2).tt_ as libc::c_int
                                == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                        {
                            let mut i1: i64 = (*v1_0).value_.i;
                            let mut i2: i64 = (*v2).value_.i;
                            pc = pc.offset(1);
                            let mut io_6: *mut TValue = &mut (*ra_20).val;
                            (*io_6).value_.i = (i1 as u64).wrapping_add(i2 as u64) as i64;
                            (*io_6).tt_ =
                                (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                        } else {
                            let mut n1: f64 = 0.;
                            let mut n2: f64 = 0.;
                            if (if (*v1_0).tt_ as libc::c_int
                                == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                            {
                                n1 = (*v1_0).value_.n;
                                1 as libc::c_int
                            } else {
                                (if (*v1_0).tt_ as libc::c_int
                                    == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                                {
                                    n1 = (*v1_0).value_.i as f64;
                                    1 as libc::c_int
                                } else {
                                    0 as libc::c_int
                                })
                            }) != 0
                                && (if (*v2).tt_ as libc::c_int
                                    == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                                {
                                    n2 = (*v2).value_.n;
                                    1 as libc::c_int
                                } else {
                                    (if (*v2).tt_ as libc::c_int
                                        == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                                    {
                                        n2 = (*v2).value_.i as f64;
                                        1 as libc::c_int
                                    } else {
                                        0 as libc::c_int
                                    })
                                }) != 0
                            {
                                pc = pc.offset(1);
                                let mut io_7: *mut TValue = &mut (*ra_20).val;
                                (*io_7).value_.n = n1 + n2;
                                (*io_7).tt_ = (3 as libc::c_int
                                    | (1 as libc::c_int) << 4 as libc::c_int)
                                    as u8;
                            }
                        }
                        continue;
                    }
                    23 => {
                        let mut v1_1: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut v2_0: *mut TValue = k.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut ra_21: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        if (*v1_1).tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                            && (*v2_0).tt_ as libc::c_int
                                == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                        {
                            let mut i1_0: i64 = (*v1_1).value_.i;
                            let mut i2_0: i64 = (*v2_0).value_.i;
                            pc = pc.offset(1);
                            let mut io_8: *mut TValue = &mut (*ra_21).val;
                            (*io_8).value_.i = (i1_0 as u64).wrapping_sub(i2_0 as u64) as i64;
                            (*io_8).tt_ =
                                (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                        } else {
                            let mut n1_0: f64 = 0.;
                            let mut n2_0: f64 = 0.;
                            if (if (*v1_1).tt_ as libc::c_int
                                == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                            {
                                n1_0 = (*v1_1).value_.n;
                                1 as libc::c_int
                            } else {
                                (if (*v1_1).tt_ as libc::c_int
                                    == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                                {
                                    n1_0 = (*v1_1).value_.i as f64;
                                    1 as libc::c_int
                                } else {
                                    0 as libc::c_int
                                })
                            }) != 0
                                && (if (*v2_0).tt_ as libc::c_int
                                    == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                                {
                                    n2_0 = (*v2_0).value_.n;
                                    1 as libc::c_int
                                } else {
                                    (if (*v2_0).tt_ as libc::c_int
                                        == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                                    {
                                        n2_0 = (*v2_0).value_.i as f64;
                                        1 as libc::c_int
                                    } else {
                                        0 as libc::c_int
                                    })
                                }) != 0
                            {
                                pc = pc.offset(1);
                                let mut io_9: *mut TValue = &mut (*ra_21).val;
                                (*io_9).value_.n = n1_0 - n2_0;
                                (*io_9).tt_ = (3 as libc::c_int
                                    | (1 as libc::c_int) << 4 as libc::c_int)
                                    as u8;
                            }
                        }
                        continue;
                    }
                    24 => {
                        let mut v1_2: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut v2_1: *mut TValue = k.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut ra_22: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        if (*v1_2).tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                            && (*v2_1).tt_ as libc::c_int
                                == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                        {
                            let mut i1_1: i64 = (*v1_2).value_.i;
                            let mut i2_1: i64 = (*v2_1).value_.i;
                            pc = pc.offset(1);
                            let mut io_10: *mut TValue = &mut (*ra_22).val;
                            (*io_10).value_.i = (i1_1 as u64 * i2_1 as u64) as i64;
                            (*io_10).tt_ =
                                (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                        } else {
                            let mut n1_1: f64 = 0.;
                            let mut n2_1: f64 = 0.;
                            if (if (*v1_2).tt_ as libc::c_int
                                == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                            {
                                n1_1 = (*v1_2).value_.n;
                                1 as libc::c_int
                            } else {
                                (if (*v1_2).tt_ as libc::c_int
                                    == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                                {
                                    n1_1 = (*v1_2).value_.i as f64;
                                    1 as libc::c_int
                                } else {
                                    0 as libc::c_int
                                })
                            }) != 0
                                && (if (*v2_1).tt_ as libc::c_int
                                    == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                                {
                                    n2_1 = (*v2_1).value_.n;
                                    1 as libc::c_int
                                } else {
                                    (if (*v2_1).tt_ as libc::c_int
                                        == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                                    {
                                        n2_1 = (*v2_1).value_.i as f64;
                                        1 as libc::c_int
                                    } else {
                                        0 as libc::c_int
                                    })
                                }) != 0
                            {
                                pc = pc.offset(1);
                                let mut io_11: *mut TValue = &mut (*ra_22).val;
                                (*io_11).value_.n = n1_1 * n2_1;
                                (*io_11).tt_ = (3 as libc::c_int
                                    | (1 as libc::c_int) << 4 as libc::c_int)
                                    as u8;
                            }
                        }
                        continue;
                    }
                    25 => {
                        (*ci).u.savedpc = pc;
                        (*L).top = (*ci).top;
                        let mut v1_3: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut v2_2: *mut TValue = k.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut ra_23: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        if (*v1_3).tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                            && (*v2_2).tt_ as libc::c_int
                                == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                        {
                            let mut i1_2: i64 = (*v1_3).value_.i;
                            let mut i2_2: i64 = (*v2_2).value_.i;
                            pc = pc.offset(1);
                            let mut io_12: *mut TValue = &mut (*ra_23).val;
                            (*io_12).value_.i = luaV_mod(L, i1_2, i2_2)?;
                            (*io_12).tt_ =
                                (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                        } else {
                            let mut n1_2: f64 = 0.;
                            let mut n2_2: f64 = 0.;
                            if (if (*v1_3).tt_ as libc::c_int
                                == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                            {
                                n1_2 = (*v1_3).value_.n;
                                1 as libc::c_int
                            } else {
                                (if (*v1_3).tt_ as libc::c_int
                                    == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                                {
                                    n1_2 = (*v1_3).value_.i as f64;
                                    1 as libc::c_int
                                } else {
                                    0 as libc::c_int
                                })
                            }) != 0
                                && (if (*v2_2).tt_ as libc::c_int
                                    == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                                {
                                    n2_2 = (*v2_2).value_.n;
                                    1 as libc::c_int
                                } else {
                                    (if (*v2_2).tt_ as libc::c_int
                                        == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                                    {
                                        n2_2 = (*v2_2).value_.i as f64;
                                        1 as libc::c_int
                                    } else {
                                        0 as libc::c_int
                                    })
                                }) != 0
                            {
                                pc = pc.offset(1);
                                let mut io_13: *mut TValue = &mut (*ra_23).val;
                                (*io_13).value_.n = luaV_modf(L, n1_2, n2_2);
                                (*io_13).tt_ = (3 as libc::c_int
                                    | (1 as libc::c_int) << 4 as libc::c_int)
                                    as u8;
                            }
                        }
                        continue;
                    }
                    26 => {
                        let mut ra_24: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut v1_4: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut v2_3: *mut TValue = k.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut n1_3: f64 = 0.;
                        let mut n2_3: f64 = 0.;
                        if (if (*v1_4).tt_ as libc::c_int
                            == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                        {
                            n1_3 = (*v1_4).value_.n;
                            1 as libc::c_int
                        } else {
                            (if (*v1_4).tt_ as libc::c_int
                                == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                            {
                                n1_3 = (*v1_4).value_.i as f64;
                                1 as libc::c_int
                            } else {
                                0 as libc::c_int
                            })
                        }) != 0
                            && (if (*v2_3).tt_ as libc::c_int
                                == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                            {
                                n2_3 = (*v2_3).value_.n;
                                1 as libc::c_int
                            } else {
                                (if (*v2_3).tt_ as libc::c_int
                                    == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                                {
                                    n2_3 = (*v2_3).value_.i as f64;
                                    1 as libc::c_int
                                } else {
                                    0 as libc::c_int
                                })
                            }) != 0
                        {
                            pc = pc.offset(1);
                            let mut io_14: *mut TValue = &mut (*ra_24).val;
                            (*io_14).value_.n = (if n2_3 == 2 as libc::c_int as f64 {
                                n1_3 * n1_3
                            } else {
                                pow(n1_3, n2_3)
                            });
                            (*io_14).tt_ =
                                (3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
                        }
                        continue;
                    }
                    27 => {
                        let mut ra_25: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut v1_5: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut v2_4: *mut TValue = k.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut n1_4: f64 = 0.;
                        let mut n2_4: f64 = 0.;
                        if (if (*v1_5).tt_ as libc::c_int
                            == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                        {
                            n1_4 = (*v1_5).value_.n;
                            1 as libc::c_int
                        } else {
                            (if (*v1_5).tt_ as libc::c_int
                                == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                            {
                                n1_4 = (*v1_5).value_.i as f64;
                                1 as libc::c_int
                            } else {
                                0 as libc::c_int
                            })
                        }) != 0
                            && (if (*v2_4).tt_ as libc::c_int
                                == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                            {
                                n2_4 = (*v2_4).value_.n;
                                1 as libc::c_int
                            } else {
                                (if (*v2_4).tt_ as libc::c_int
                                    == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                                {
                                    n2_4 = (*v2_4).value_.i as f64;
                                    1 as libc::c_int
                                } else {
                                    0 as libc::c_int
                                })
                            }) != 0
                        {
                            pc = pc.offset(1);
                            let mut io_15: *mut TValue = &mut (*ra_25).val;
                            (*io_15).value_.n = n1_4 / n2_4;
                            (*io_15).tt_ =
                                (3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
                        }
                        continue;
                    }
                    28 => {
                        (*ci).u.savedpc = pc;
                        (*L).top = (*ci).top;
                        let mut v1_6: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut v2_5: *mut TValue = k.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut ra_26: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        if (*v1_6).tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                            && (*v2_5).tt_ as libc::c_int
                                == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                        {
                            let mut i1_3: i64 = (*v1_6).value_.i;
                            let mut i2_3: i64 = (*v2_5).value_.i;
                            pc = pc.offset(1);
                            let mut io_16: *mut TValue = &mut (*ra_26).val;
                            (*io_16).value_.i = luaV_idiv(L, i1_3, i2_3)?;
                            (*io_16).tt_ =
                                (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                        } else {
                            let mut n1_5: f64 = 0.;
                            let mut n2_5: f64 = 0.;
                            if (if (*v1_6).tt_ as libc::c_int
                                == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                            {
                                n1_5 = (*v1_6).value_.n;
                                1 as libc::c_int
                            } else {
                                (if (*v1_6).tt_ as libc::c_int
                                    == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                                {
                                    n1_5 = (*v1_6).value_.i as f64;
                                    1 as libc::c_int
                                } else {
                                    0 as libc::c_int
                                })
                            }) != 0
                                && (if (*v2_5).tt_ as libc::c_int
                                    == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                                {
                                    n2_5 = (*v2_5).value_.n;
                                    1 as libc::c_int
                                } else {
                                    (if (*v2_5).tt_ as libc::c_int
                                        == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                                    {
                                        n2_5 = (*v2_5).value_.i as f64;
                                        1 as libc::c_int
                                    } else {
                                        0 as libc::c_int
                                    })
                                }) != 0
                            {
                                pc = pc.offset(1);
                                let mut io_17: *mut TValue = &mut (*ra_26).val;
                                (*io_17).value_.n = floor(n1_5 / n2_5);
                                (*io_17).tt_ = (3 as libc::c_int
                                    | (1 as libc::c_int) << 4 as libc::c_int)
                                    as u8;
                            }
                        }
                        continue;
                    }
                    29 => {
                        let mut ra_27: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut v1_7: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut v2_6: *mut TValue = k.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut i1_4: i64 = 0;
                        let mut i2_4: i64 = (*v2_6).value_.i;
                        if if (((*v1_7).tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int)
                            as libc::c_int
                            != 0 as libc::c_int) as libc::c_int
                            as libc::c_long
                            != 0
                        {
                            i1_4 = (*v1_7).value_.i;
                            1 as libc::c_int
                        } else {
                            luaV_tointegerns(v1_7, &mut i1_4, F2Ieq)
                        } != 0
                        {
                            pc = pc.offset(1);
                            let mut io_18: *mut TValue = &mut (*ra_27).val;
                            (*io_18).value_.i = (i1_4 as u64 & i2_4 as u64) as i64;
                            (*io_18).tt_ =
                                (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                        }
                        continue;
                    }
                    30 => {
                        let mut ra_28: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut v1_8: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut v2_7: *mut TValue = k.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut i1_5: i64 = 0;
                        let mut i2_5: i64 = (*v2_7).value_.i;
                        if if (((*v1_8).tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int)
                            as libc::c_int
                            != 0 as libc::c_int) as libc::c_int
                            as libc::c_long
                            != 0
                        {
                            i1_5 = (*v1_8).value_.i;
                            1 as libc::c_int
                        } else {
                            luaV_tointegerns(v1_8, &mut i1_5, F2Ieq)
                        } != 0
                        {
                            pc = pc.offset(1);
                            let mut io_19: *mut TValue = &mut (*ra_28).val;
                            (*io_19).value_.i = (i1_5 as u64 | i2_5 as u64) as i64;
                            (*io_19).tt_ =
                                (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                        }
                        continue;
                    }
                    31 => {
                        let mut ra_29: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut v1_9: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut v2_8: *mut TValue = k.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut i1_6: i64 = 0;
                        let mut i2_6: i64 = (*v2_8).value_.i;
                        if if (((*v1_9).tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int)
                            as libc::c_int
                            != 0 as libc::c_int) as libc::c_int
                            as libc::c_long
                            != 0
                        {
                            i1_6 = (*v1_9).value_.i;
                            1 as libc::c_int
                        } else {
                            luaV_tointegerns(v1_9, &mut i1_6, F2Ieq)
                        } != 0
                        {
                            pc = pc.offset(1);
                            let mut io_20: *mut TValue = &mut (*ra_29).val;
                            (*io_20).value_.i = (i1_6 as u64 ^ i2_6 as u64) as i64;
                            (*io_20).tt_ =
                                (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                        }
                        continue;
                    }
                    32 => {
                        let mut ra_30: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut rb_8: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut ic: libc::c_int = (i
                            >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                            as libc::c_int
                            - (((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int
                                >> 1 as libc::c_int);
                        let mut ib: i64 = 0;
                        if if (((*rb_8).tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int)
                            as libc::c_int
                            != 0 as libc::c_int) as libc::c_int
                            as libc::c_long
                            != 0
                        {
                            ib = (*rb_8).value_.i;
                            1 as libc::c_int
                        } else {
                            luaV_tointegerns(rb_8, &mut ib, F2Ieq)
                        } != 0
                        {
                            pc = pc.offset(1);
                            let mut io_21: *mut TValue = &mut (*ra_30).val;
                            (*io_21).value_.i = luaV_shiftl(ib, -ic as i64);
                            (*io_21).tt_ =
                                (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                        }
                        continue;
                    }
                    33 => {
                        let mut ra_31: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut rb_9: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut ic_0: libc::c_int = (i
                            >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                            as libc::c_int
                            - (((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int
                                >> 1 as libc::c_int);
                        let mut ib_0: i64 = 0;
                        if if (((*rb_9).tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int)
                            as libc::c_int
                            != 0 as libc::c_int) as libc::c_int
                            as libc::c_long
                            != 0
                        {
                            ib_0 = (*rb_9).value_.i;
                            1 as libc::c_int
                        } else {
                            luaV_tointegerns(rb_9, &mut ib_0, F2Ieq)
                        } != 0
                        {
                            pc = pc.offset(1);
                            let mut io_22: *mut TValue = &mut (*ra_31).val;
                            (*io_22).value_.i = luaV_shiftl(ic_0 as i64, ib_0);
                            (*io_22).tt_ =
                                (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                        }
                        continue;
                    }
                    34 => {
                        let mut v1_10: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut v2_9: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut ra_32: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        if (*v1_10).tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                            && (*v2_9).tt_ as libc::c_int
                                == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                        {
                            let mut i1_7: i64 = (*v1_10).value_.i;
                            let mut i2_7: i64 = (*v2_9).value_.i;
                            pc = pc.offset(1);
                            let mut io_23: *mut TValue = &mut (*ra_32).val;
                            (*io_23).value_.i = (i1_7 as u64).wrapping_add(i2_7 as u64) as i64;
                            (*io_23).tt_ =
                                (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                        } else {
                            let mut n1_6: f64 = 0.;
                            let mut n2_6: f64 = 0.;
                            if (if (*v1_10).tt_ as libc::c_int
                                == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                            {
                                n1_6 = (*v1_10).value_.n;
                                1 as libc::c_int
                            } else {
                                (if (*v1_10).tt_ as libc::c_int
                                    == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                                {
                                    n1_6 = (*v1_10).value_.i as f64;
                                    1 as libc::c_int
                                } else {
                                    0 as libc::c_int
                                })
                            }) != 0
                                && (if (*v2_9).tt_ as libc::c_int
                                    == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                                {
                                    n2_6 = (*v2_9).value_.n;
                                    1 as libc::c_int
                                } else {
                                    (if (*v2_9).tt_ as libc::c_int
                                        == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                                    {
                                        n2_6 = (*v2_9).value_.i as f64;
                                        1 as libc::c_int
                                    } else {
                                        0 as libc::c_int
                                    })
                                }) != 0
                            {
                                pc = pc.offset(1);
                                let mut io_24: *mut TValue = &mut (*ra_32).val;
                                (*io_24).value_.n = n1_6 + n2_6;
                                (*io_24).tt_ = (3 as libc::c_int
                                    | (1 as libc::c_int) << 4 as libc::c_int)
                                    as u8;
                            }
                        }
                        continue;
                    }
                    35 => {
                        let mut v1_11: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut v2_10: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut ra_33: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        if (*v1_11).tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                            && (*v2_10).tt_ as libc::c_int
                                == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                        {
                            let mut i1_8: i64 = (*v1_11).value_.i;
                            let mut i2_8: i64 = (*v2_10).value_.i;
                            pc = pc.offset(1);
                            let mut io_25: *mut TValue = &mut (*ra_33).val;
                            (*io_25).value_.i = (i1_8 as u64).wrapping_sub(i2_8 as u64) as i64;
                            (*io_25).tt_ =
                                (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                        } else {
                            let mut n1_7: f64 = 0.;
                            let mut n2_7: f64 = 0.;
                            if (if (*v1_11).tt_ as libc::c_int
                                == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                            {
                                n1_7 = (*v1_11).value_.n;
                                1 as libc::c_int
                            } else {
                                (if (*v1_11).tt_ as libc::c_int
                                    == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                                {
                                    n1_7 = (*v1_11).value_.i as f64;
                                    1 as libc::c_int
                                } else {
                                    0 as libc::c_int
                                })
                            }) != 0
                                && (if (*v2_10).tt_ as libc::c_int
                                    == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                                {
                                    n2_7 = (*v2_10).value_.n;
                                    1 as libc::c_int
                                } else {
                                    (if (*v2_10).tt_ as libc::c_int
                                        == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                                    {
                                        n2_7 = (*v2_10).value_.i as f64;
                                        1 as libc::c_int
                                    } else {
                                        0 as libc::c_int
                                    })
                                }) != 0
                            {
                                pc = pc.offset(1);
                                let mut io_26: *mut TValue = &mut (*ra_33).val;
                                (*io_26).value_.n = n1_7 - n2_7;
                                (*io_26).tt_ = (3 as libc::c_int
                                    | (1 as libc::c_int) << 4 as libc::c_int)
                                    as u8;
                            }
                        }
                        continue;
                    }
                    36 => {
                        let mut v1_12: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut v2_11: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut ra_34: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        if (*v1_12).tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                            && (*v2_11).tt_ as libc::c_int
                                == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                        {
                            let mut i1_9: i64 = (*v1_12).value_.i;
                            let mut i2_9: i64 = (*v2_11).value_.i;
                            pc = pc.offset(1);
                            let mut io_27: *mut TValue = &mut (*ra_34).val;
                            (*io_27).value_.i = ((i1_9 as u64).wrapping_mul(i2_9 as u64)) as i64;
                            (*io_27).tt_ = (3 | 0 << 4);
                        } else {
                            let mut n1_8: f64 = 0.;
                            let mut n2_8: f64 = 0.;
                            if (if (*v1_12).tt_ as libc::c_int
                                == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                            {
                                n1_8 = (*v1_12).value_.n;
                                1 as libc::c_int
                            } else {
                                (if (*v1_12).tt_ as libc::c_int
                                    == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                                {
                                    n1_8 = (*v1_12).value_.i as f64;
                                    1 as libc::c_int
                                } else {
                                    0 as libc::c_int
                                })
                            }) != 0
                                && (if (*v2_11).tt_ as libc::c_int
                                    == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                                {
                                    n2_8 = (*v2_11).value_.n;
                                    1 as libc::c_int
                                } else {
                                    (if (*v2_11).tt_ as libc::c_int
                                        == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                                    {
                                        n2_8 = (*v2_11).value_.i as f64;
                                        1 as libc::c_int
                                    } else {
                                        0 as libc::c_int
                                    })
                                }) != 0
                            {
                                pc = pc.offset(1);
                                let mut io_28: *mut TValue = &mut (*ra_34).val;
                                (*io_28).value_.n = n1_8 * n2_8;
                                (*io_28).tt_ = (3 as libc::c_int
                                    | (1 as libc::c_int) << 4 as libc::c_int)
                                    as u8;
                            }
                        }
                        continue;
                    }
                    37 => {
                        (*ci).u.savedpc = pc;
                        (*L).top = (*ci).top;
                        let mut v1_13: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut v2_12: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut ra_35: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        if (*v1_13).tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                            && (*v2_12).tt_ as libc::c_int
                                == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                        {
                            let mut i1_10: i64 = (*v1_13).value_.i;
                            let mut i2_10: i64 = (*v2_12).value_.i;
                            pc = pc.offset(1);
                            let mut io_29: *mut TValue = &mut (*ra_35).val;
                            (*io_29).value_.i = luaV_mod(L, i1_10, i2_10)?;
                            (*io_29).tt_ =
                                (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                        } else {
                            let mut n1_9: f64 = 0.;
                            let mut n2_9: f64 = 0.;
                            if (if (*v1_13).tt_ as libc::c_int
                                == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                            {
                                n1_9 = (*v1_13).value_.n;
                                1 as libc::c_int
                            } else {
                                (if (*v1_13).tt_ as libc::c_int
                                    == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                                {
                                    n1_9 = (*v1_13).value_.i as f64;
                                    1 as libc::c_int
                                } else {
                                    0 as libc::c_int
                                })
                            }) != 0
                                && (if (*v2_12).tt_ as libc::c_int
                                    == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                                {
                                    n2_9 = (*v2_12).value_.n;
                                    1 as libc::c_int
                                } else {
                                    (if (*v2_12).tt_ as libc::c_int
                                        == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                                    {
                                        n2_9 = (*v2_12).value_.i as f64;
                                        1 as libc::c_int
                                    } else {
                                        0 as libc::c_int
                                    })
                                }) != 0
                            {
                                pc = pc.offset(1);
                                let mut io_30: *mut TValue = &mut (*ra_35).val;
                                (*io_30).value_.n = luaV_modf(L, n1_9, n2_9);
                                (*io_30).tt_ = (3 as libc::c_int
                                    | (1 as libc::c_int) << 4 as libc::c_int)
                                    as u8;
                            }
                        }
                        continue;
                    }
                    38 => {
                        let mut ra_36: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut v1_14: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut v2_13: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut n1_10: f64 = 0.;
                        let mut n2_10: f64 = 0.;
                        if (if (*v1_14).tt_ as libc::c_int
                            == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                        {
                            n1_10 = (*v1_14).value_.n;
                            1 as libc::c_int
                        } else {
                            (if (*v1_14).tt_ as libc::c_int
                                == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                            {
                                n1_10 = (*v1_14).value_.i as f64;
                                1 as libc::c_int
                            } else {
                                0 as libc::c_int
                            })
                        }) != 0
                            && (if (*v2_13).tt_ as libc::c_int
                                == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                            {
                                n2_10 = (*v2_13).value_.n;
                                1 as libc::c_int
                            } else {
                                (if (*v2_13).tt_ as libc::c_int
                                    == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                                {
                                    n2_10 = (*v2_13).value_.i as f64;
                                    1 as libc::c_int
                                } else {
                                    0 as libc::c_int
                                })
                            }) != 0
                        {
                            pc = pc.offset(1);
                            let mut io_31: *mut TValue = &mut (*ra_36).val;
                            (*io_31).value_.n = (if n2_10 == 2 as libc::c_int as f64 {
                                n1_10 * n1_10
                            } else {
                                pow(n1_10, n2_10)
                            });
                            (*io_31).tt_ =
                                (3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
                        }
                        continue;
                    }
                    39 => {
                        let mut ra_37: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut v1_15: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut v2_14: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut n1_11: f64 = 0.;
                        let mut n2_11: f64 = 0.;
                        if (if (*v1_15).tt_ as libc::c_int
                            == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                        {
                            n1_11 = (*v1_15).value_.n;
                            1 as libc::c_int
                        } else {
                            (if (*v1_15).tt_ as libc::c_int
                                == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                            {
                                n1_11 = (*v1_15).value_.i as f64;
                                1 as libc::c_int
                            } else {
                                0 as libc::c_int
                            })
                        }) != 0
                            && (if (*v2_14).tt_ as libc::c_int
                                == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                            {
                                n2_11 = (*v2_14).value_.n;
                                1 as libc::c_int
                            } else {
                                (if (*v2_14).tt_ as libc::c_int
                                    == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                                {
                                    n2_11 = (*v2_14).value_.i as f64;
                                    1 as libc::c_int
                                } else {
                                    0 as libc::c_int
                                })
                            }) != 0
                        {
                            pc = pc.offset(1);
                            let mut io_32: *mut TValue = &mut (*ra_37).val;
                            (*io_32).value_.n = n1_11 / n2_11;
                            (*io_32).tt_ =
                                (3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
                        }
                        continue;
                    }
                    40 => {
                        (*ci).u.savedpc = pc;
                        (*L).top = (*ci).top;
                        let mut v1_16: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut v2_15: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut ra_38: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        if (*v1_16).tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                            && (*v2_15).tt_ as libc::c_int
                                == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                        {
                            let mut i1_11: i64 = (*v1_16).value_.i;
                            let mut i2_11: i64 = (*v2_15).value_.i;
                            pc = pc.offset(1);
                            let mut io_33: *mut TValue = &mut (*ra_38).val;
                            (*io_33).value_.i = luaV_idiv(L, i1_11, i2_11)?;
                            (*io_33).tt_ =
                                (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                        } else {
                            let mut n1_12: f64 = 0.;
                            let mut n2_12: f64 = 0.;
                            if (if (*v1_16).tt_ as libc::c_int
                                == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                            {
                                n1_12 = (*v1_16).value_.n;
                                1 as libc::c_int
                            } else {
                                (if (*v1_16).tt_ as libc::c_int
                                    == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                                {
                                    n1_12 = (*v1_16).value_.i as f64;
                                    1 as libc::c_int
                                } else {
                                    0 as libc::c_int
                                })
                            }) != 0
                                && (if (*v2_15).tt_ as libc::c_int
                                    == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                                {
                                    n2_12 = (*v2_15).value_.n;
                                    1 as libc::c_int
                                } else {
                                    (if (*v2_15).tt_ as libc::c_int
                                        == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                                    {
                                        n2_12 = (*v2_15).value_.i as f64;
                                        1 as libc::c_int
                                    } else {
                                        0 as libc::c_int
                                    })
                                }) != 0
                            {
                                pc = pc.offset(1);
                                let mut io_34: *mut TValue = &mut (*ra_38).val;
                                (*io_34).value_.n = floor(n1_12 / n2_12);
                                (*io_34).tt_ = (3 as libc::c_int
                                    | (1 as libc::c_int) << 4 as libc::c_int)
                                    as u8;
                            }
                        }
                        continue;
                    }
                    41 => {
                        let mut ra_39: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut v1_17: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut v2_16: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut i1_12: i64 = 0;
                        let mut i2_12: i64 = 0;
                        if (if (((*v1_17).tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int)
                            as libc::c_int
                            != 0 as libc::c_int) as libc::c_int
                            as libc::c_long
                            != 0
                        {
                            i1_12 = (*v1_17).value_.i;
                            1 as libc::c_int
                        } else {
                            luaV_tointegerns(v1_17, &mut i1_12, F2Ieq)
                        }) != 0
                            && (if (((*v2_16).tt_ as libc::c_int
                                == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int)
                                as libc::c_int
                                != 0 as libc::c_int)
                                as libc::c_int as libc::c_long
                                != 0
                            {
                                i2_12 = (*v2_16).value_.i;
                                1 as libc::c_int
                            } else {
                                luaV_tointegerns(v2_16, &mut i2_12, F2Ieq)
                            }) != 0
                        {
                            pc = pc.offset(1);
                            let mut io_35: *mut TValue = &mut (*ra_39).val;
                            (*io_35).value_.i = (i1_12 as u64 & i2_12 as u64) as i64;
                            (*io_35).tt_ =
                                (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                        }
                        continue;
                    }
                    42 => {
                        let mut ra_40: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut v1_18: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut v2_17: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut i1_13: i64 = 0;
                        let mut i2_13: i64 = 0;
                        if (if (((*v1_18).tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int)
                            as libc::c_int
                            != 0 as libc::c_int) as libc::c_int
                            as libc::c_long
                            != 0
                        {
                            i1_13 = (*v1_18).value_.i;
                            1 as libc::c_int
                        } else {
                            luaV_tointegerns(v1_18, &mut i1_13, F2Ieq)
                        }) != 0
                            && (if (((*v2_17).tt_ as libc::c_int
                                == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int)
                                as libc::c_int
                                != 0 as libc::c_int)
                                as libc::c_int as libc::c_long
                                != 0
                            {
                                i2_13 = (*v2_17).value_.i;
                                1 as libc::c_int
                            } else {
                                luaV_tointegerns(v2_17, &mut i2_13, F2Ieq)
                            }) != 0
                        {
                            pc = pc.offset(1);
                            let mut io_36: *mut TValue = &mut (*ra_40).val;
                            (*io_36).value_.i = (i1_13 as u64 | i2_13 as u64) as i64;
                            (*io_36).tt_ =
                                (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                        }
                        continue;
                    }
                    43 => {
                        let mut ra_41: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut v1_19: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut v2_18: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut i1_14: i64 = 0;
                        let mut i2_14: i64 = 0;
                        if (if (((*v1_19).tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int)
                            as libc::c_int
                            != 0 as libc::c_int) as libc::c_int
                            as libc::c_long
                            != 0
                        {
                            i1_14 = (*v1_19).value_.i;
                            1 as libc::c_int
                        } else {
                            luaV_tointegerns(v1_19, &mut i1_14, F2Ieq)
                        }) != 0
                            && (if (((*v2_18).tt_ as libc::c_int
                                == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int)
                                as libc::c_int
                                != 0 as libc::c_int)
                                as libc::c_int as libc::c_long
                                != 0
                            {
                                i2_14 = (*v2_18).value_.i;
                                1 as libc::c_int
                            } else {
                                luaV_tointegerns(v2_18, &mut i2_14, F2Ieq)
                            }) != 0
                        {
                            pc = pc.offset(1);
                            let mut io_37: *mut TValue = &mut (*ra_41).val;
                            (*io_37).value_.i = (i1_14 as u64 ^ i2_14 as u64) as i64;
                            (*io_37).tt_ =
                                (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                        }
                        continue;
                    }
                    45 => {
                        let mut ra_42: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut v1_20: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut v2_19: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut i1_15: i64 = 0;
                        let mut i2_15: i64 = 0;
                        if (if (((*v1_20).tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int)
                            as libc::c_int
                            != 0 as libc::c_int) as libc::c_int
                            as libc::c_long
                            != 0
                        {
                            i1_15 = (*v1_20).value_.i;
                            1 as libc::c_int
                        } else {
                            luaV_tointegerns(v1_20, &mut i1_15, F2Ieq)
                        }) != 0
                            && (if (((*v2_19).tt_ as libc::c_int
                                == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int)
                                as libc::c_int
                                != 0 as libc::c_int)
                                as libc::c_int as libc::c_long
                                != 0
                            {
                                i2_15 = (*v2_19).value_.i;
                                1 as libc::c_int
                            } else {
                                luaV_tointegerns(v2_19, &mut i2_15, F2Ieq)
                            }) != 0
                        {
                            pc = pc.offset(1);
                            let mut io_38: *mut TValue = &mut (*ra_42).val;
                            (*io_38).value_.i = luaV_shiftl(
                                i1_15,
                                (0 as libc::c_int as u64).wrapping_sub(i2_15 as u64) as i64,
                            );
                            (*io_38).tt_ =
                                (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                        }
                        continue;
                    }
                    44 => {
                        let mut ra_43: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut v1_21: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut v2_20: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut i1_16: i64 = 0;
                        let mut i2_16: i64 = 0;
                        if (if (((*v1_21).tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int)
                            as libc::c_int
                            != 0 as libc::c_int) as libc::c_int
                            as libc::c_long
                            != 0
                        {
                            i1_16 = (*v1_21).value_.i;
                            1 as libc::c_int
                        } else {
                            luaV_tointegerns(v1_21, &mut i1_16, F2Ieq)
                        }) != 0
                            && (if (((*v2_20).tt_ as libc::c_int
                                == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int)
                                as libc::c_int
                                != 0 as libc::c_int)
                                as libc::c_int as libc::c_long
                                != 0
                            {
                                i2_16 = (*v2_20).value_.i;
                                1 as libc::c_int
                            } else {
                                luaV_tointegerns(v2_20, &mut i2_16, F2Ieq)
                            }) != 0
                        {
                            pc = pc.offset(1);
                            let mut io_39: *mut TValue = &mut (*ra_43).val;
                            (*io_39).value_.i = luaV_shiftl(i1_16, i2_16);
                            (*io_39).tt_ =
                                (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                        }
                        continue;
                    }
                    46 => {
                        let mut ra_44: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut pi: u32 = *pc.offset(-(2 as libc::c_int as isize));
                        let mut rb_10: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut tm: TMS = (i
                            >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                            as libc::c_int as TMS;
                        let mut result: StkId = base.offset(
                            (pi >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        (*ci).u.savedpc = pc;
                        (*L).top = (*ci).top;
                        luaT_trybinTM(L, &mut (*ra_44).val, rb_10, result, tm)?;
                        trap = (*ci).u.trap;
                        continue;
                    }
                    47 => {
                        let mut ra_45: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut pi_0: u32 = *pc.offset(-(2 as libc::c_int as isize));
                        let mut imm_0: libc::c_int = (i
                            >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                            as libc::c_int
                            - (((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int
                                >> 1 as libc::c_int);
                        let mut tm_0: TMS = (i
                            >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                            as libc::c_int as TMS;
                        let mut flip: libc::c_int = (i
                            >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int
                            & !(!(0 as libc::c_int as u32) << 1 as libc::c_int) << 0 as libc::c_int)
                            as libc::c_int;
                        let mut result_0: StkId = base.offset(
                            (pi_0 >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        (*ci).u.savedpc = pc;
                        (*L).top = (*ci).top;
                        luaT_trybiniTM(L, &mut (*ra_45).val, imm_0 as i64, flip, result_0, tm_0)?;
                        trap = (*ci).u.trap;
                        continue;
                    }
                    48 => {
                        let mut ra_46: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut pi_1: u32 = *pc.offset(-(2 as libc::c_int as isize));
                        let mut imm_1: *mut TValue = k.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut tm_1: TMS = (i
                            >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                            as libc::c_int as TMS;
                        let mut flip_0: libc::c_int = (i
                            >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int
                            & !(!(0 as libc::c_int as u32) << 1 as libc::c_int) << 0 as libc::c_int)
                            as libc::c_int;
                        let mut result_1: StkId = base.offset(
                            (pi_1 >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        (*ci).u.savedpc = pc;
                        (*L).top = (*ci).top;
                        luaT_trybinassocTM(L, &mut (*ra_46).val, imm_1, flip_0, result_1, tm_1)?;
                        trap = (*ci).u.trap;
                        continue;
                    }
                    49 => {
                        let mut ra_47: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut rb_11: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut nb_0: f64 = 0.;
                        if (*rb_11).tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                        {
                            let mut ib_1: i64 = (*rb_11).value_.i;
                            let mut io_40: *mut TValue = &mut (*ra_47).val;
                            (*io_40).value_.i =
                                (0 as libc::c_int as u64).wrapping_sub(ib_1 as u64) as i64;
                            (*io_40).tt_ =
                                (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                        } else if if (*rb_11).tt_ as libc::c_int
                            == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                        {
                            nb_0 = (*rb_11).value_.n;
                            1 as libc::c_int
                        } else if (*rb_11).tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                        {
                            nb_0 = (*rb_11).value_.i as f64;
                            1 as libc::c_int
                        } else {
                            0 as libc::c_int
                        } != 0
                        {
                            let mut io_41: *mut TValue = &mut (*ra_47).val;
                            (*io_41).value_.n = -nb_0;
                            (*io_41).tt_ =
                                (3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
                        } else {
                            (*ci).u.savedpc = pc;
                            (*L).top = (*ci).top;
                            luaT_trybinTM(L, rb_11, rb_11, ra_47, TM_UNM)?;
                            trap = (*ci).u.trap;
                        }
                        continue;
                    }
                    50 => {
                        let mut ra_48: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut rb_12: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        let mut ib_2: i64 = 0;
                        if if (((*rb_12).tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int)
                            as libc::c_int
                            != 0 as libc::c_int) as libc::c_int
                            as libc::c_long
                            != 0
                        {
                            ib_2 = (*rb_12).value_.i;
                            1 as libc::c_int
                        } else {
                            luaV_tointegerns(rb_12, &mut ib_2, F2Ieq)
                        } != 0
                        {
                            let mut io_42: *mut TValue = &mut (*ra_48).val;
                            (*io_42).value_.i = (!(0 as libc::c_int as u64) ^ ib_2 as u64) as i64;
                            (*io_42).tt_ =
                                (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                        } else {
                            (*ci).u.savedpc = pc;
                            (*L).top = (*ci).top;
                            luaT_trybinTM(L, rb_12, rb_12, ra_48, TM_BNOT)?;
                            trap = (*ci).u.trap;
                        }
                        continue;
                    }
                    51 => {
                        let mut ra_49: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut rb_13: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        if (*rb_13).tt_ as libc::c_int
                            == 1 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                            || (*rb_13).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int
                        {
                            (*ra_49).val.tt_ =
                                (1 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
                        } else {
                            (*ra_49).val.tt_ =
                                (1 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
                        }
                        continue;
                    }
                    52 => {
                        let mut ra_50: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        (*ci).u.savedpc = pc;
                        (*L).top = (*ci).top;
                        luaV_objlen(
                            L,
                            ra_50,
                            &mut (*base.offset(
                                (i >> 0 as libc::c_int
                                    + 7 as libc::c_int
                                    + 8 as libc::c_int
                                    + 1 as libc::c_int
                                    & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                        << 0 as libc::c_int)
                                    as libc::c_int as isize,
                            ))
                            .val,
                        )?;
                        trap = (*ci).u.trap;
                        continue;
                    }
                    53 => {
                        let mut ra_51: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut n_1: libc::c_int = (i
                            >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                            as libc::c_int;
                        (*L).top = ra_51.offset(n_1 as isize);
                        (*ci).u.savedpc = pc;
                        luaV_concat(L, n_1)?;
                        trap = (*ci).u.trap;

                        if (*(*L).global).gc.debt() > 0 {
                            (*ci).u.savedpc = pc;
                            (*L).top = (*L).top;
                            luaC_step(L);
                            trap = (*ci).u.trap;
                        }

                        continue;
                    }
                    54 => {
                        let mut ra_52: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        (*ci).u.savedpc = pc;
                        (*L).top = (*ci).top;
                        luaF_close(L, ra_52)?;
                        trap = (*ci).u.trap;
                        continue;
                    }
                    55 => {
                        let mut ra_53: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        (*ci).u.savedpc = pc;
                        (*L).top = (*ci).top;
                        luaF_newtbcupval(L, ra_53)?;
                        continue;
                    }
                    56 => {
                        pc = pc.offset(
                            ((i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32)
                                    << 8 as libc::c_int
                                        + 8 as libc::c_int
                                        + 1 as libc::c_int
                                        + 8 as libc::c_int)
                                    << 0 as libc::c_int)
                                as libc::c_int
                                - (((1 as libc::c_int)
                                    << 8 as libc::c_int
                                        + 8 as libc::c_int
                                        + 1 as libc::c_int
                                        + 8 as libc::c_int)
                                    - 1 as libc::c_int
                                    >> 1 as libc::c_int)
                                + 0 as libc::c_int) as isize,
                        );
                        trap = (*ci).u.trap;
                        continue;
                    }
                    57 => {
                        let mut ra_54: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut cond: libc::c_int = 0;
                        let mut rb_14: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        (*ci).u.savedpc = pc;
                        (*L).top = (*ci).top;
                        cond = luaV_equalobj(L, &mut (*ra_54).val, rb_14)?;
                        trap = (*ci).u.trap;
                        if cond
                            != (i >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 1 as libc::c_int)
                                    << 0 as libc::c_int)
                                as libc::c_int
                        {
                            pc = pc.offset(1);
                        } else {
                            let mut ni: u32 = *pc;
                            pc = pc.offset(
                                ((ni >> 0 as libc::c_int + 7 as libc::c_int
                                    & !(!(0 as libc::c_int as u32)
                                        << 8 as libc::c_int
                                            + 8 as libc::c_int
                                            + 1 as libc::c_int
                                            + 8 as libc::c_int)
                                        << 0 as libc::c_int)
                                    as libc::c_int
                                    - (((1 as libc::c_int)
                                        << 8 as libc::c_int
                                            + 8 as libc::c_int
                                            + 1 as libc::c_int
                                            + 8 as libc::c_int)
                                        - 1 as libc::c_int
                                        >> 1 as libc::c_int)
                                    + 1 as libc::c_int) as isize,
                            );
                            trap = (*ci).u.trap;
                        }
                        continue;
                    }
                    58 => {
                        let mut ra_55: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut cond_0: libc::c_int = 0;
                        let mut rb_15: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        if (*ra_55).val.tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                            && (*rb_15).tt_ as libc::c_int
                                == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                        {
                            let mut ia: i64 = (*ra_55).val.value_.i;
                            let mut ib_3: i64 = (*rb_15).value_.i;
                            cond_0 = (ia < ib_3) as libc::c_int;
                        } else if (*ra_55).val.tt_ as libc::c_int & 0xf as libc::c_int
                            == 3 as libc::c_int
                            && (*rb_15).tt_ as libc::c_int & 0xf as libc::c_int == 3 as libc::c_int
                        {
                            cond_0 = LTnum(&mut (*ra_55).val, rb_15);
                        } else {
                            (*ci).u.savedpc = pc;
                            (*L).top = (*ci).top;
                            cond_0 = lessthanothers(L, &mut (*ra_55).val, rb_15)?;
                            trap = (*ci).u.trap;
                        }
                        if cond_0
                            != (i >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 1 as libc::c_int)
                                    << 0 as libc::c_int)
                                as libc::c_int
                        {
                            pc = pc.offset(1);
                        } else {
                            let mut ni_0: u32 = *pc;
                            pc = pc.offset(
                                ((ni_0 >> 0 as libc::c_int + 7 as libc::c_int
                                    & !(!(0 as libc::c_int as u32)
                                        << 8 as libc::c_int
                                            + 8 as libc::c_int
                                            + 1 as libc::c_int
                                            + 8 as libc::c_int)
                                        << 0 as libc::c_int)
                                    as libc::c_int
                                    - (((1 as libc::c_int)
                                        << 8 as libc::c_int
                                            + 8 as libc::c_int
                                            + 1 as libc::c_int
                                            + 8 as libc::c_int)
                                        - 1 as libc::c_int
                                        >> 1 as libc::c_int)
                                    + 1 as libc::c_int) as isize,
                            );
                            trap = (*ci).u.trap;
                        }
                        continue;
                    }
                    59 => {
                        let mut ra_56: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut cond_1: libc::c_int = 0;
                        let mut rb_16: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        if (*ra_56).val.tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                            && (*rb_16).tt_ as libc::c_int
                                == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                        {
                            let mut ia_0: i64 = (*ra_56).val.value_.i;
                            let mut ib_4: i64 = (*rb_16).value_.i;
                            cond_1 = (ia_0 <= ib_4) as libc::c_int;
                        } else if (*ra_56).val.tt_ as libc::c_int & 0xf as libc::c_int
                            == 3 as libc::c_int
                            && (*rb_16).tt_ as libc::c_int & 0xf as libc::c_int == 3 as libc::c_int
                        {
                            cond_1 = LEnum(&mut (*ra_56).val, rb_16);
                        } else {
                            (*ci).u.savedpc = pc;
                            (*L).top = (*ci).top;
                            cond_1 = lessequalothers(L, &mut (*ra_56).val, rb_16)?;
                            trap = (*ci).u.trap;
                        }
                        if cond_1
                            != (i >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 1 as libc::c_int)
                                    << 0 as libc::c_int)
                                as libc::c_int
                        {
                            pc = pc.offset(1);
                        } else {
                            let mut ni_1: u32 = *pc;
                            pc = pc.offset(
                                ((ni_1 >> 0 as libc::c_int + 7 as libc::c_int
                                    & !(!(0 as libc::c_int as u32)
                                        << 8 as libc::c_int
                                            + 8 as libc::c_int
                                            + 1 as libc::c_int
                                            + 8 as libc::c_int)
                                        << 0 as libc::c_int)
                                    as libc::c_int
                                    - (((1 as libc::c_int)
                                        << 8 as libc::c_int
                                            + 8 as libc::c_int
                                            + 1 as libc::c_int
                                            + 8 as libc::c_int)
                                        - 1 as libc::c_int
                                        >> 1 as libc::c_int)
                                    + 1 as libc::c_int) as isize,
                            );
                            trap = (*ci).u.trap;
                        }
                        continue;
                    }
                    60 => {
                        let mut ra_57: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut rb_17: *mut TValue = k.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut cond_2: libc::c_int =
                            luaV_equalobj(0 as *mut Thread, &mut (*ra_57).val, rb_17)?;
                        if cond_2
                            != (i >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 1 as libc::c_int)
                                    << 0 as libc::c_int)
                                as libc::c_int
                        {
                            pc = pc.offset(1);
                        } else {
                            let mut ni_2: u32 = *pc;
                            pc = pc.offset(
                                ((ni_2 >> 0 as libc::c_int + 7 as libc::c_int
                                    & !(!(0 as libc::c_int as u32)
                                        << 8 as libc::c_int
                                            + 8 as libc::c_int
                                            + 1 as libc::c_int
                                            + 8 as libc::c_int)
                                        << 0 as libc::c_int)
                                    as libc::c_int
                                    - (((1 as libc::c_int)
                                        << 8 as libc::c_int
                                            + 8 as libc::c_int
                                            + 1 as libc::c_int
                                            + 8 as libc::c_int)
                                        - 1 as libc::c_int
                                        >> 1 as libc::c_int)
                                    + 1 as libc::c_int) as isize,
                            );
                            trap = (*ci).u.trap;
                        }
                        continue;
                    }
                    61 => {
                        let mut ra_58: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut cond_3: libc::c_int = 0;
                        let mut im: libc::c_int = (i
                            >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                            as libc::c_int
                            - (((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int
                                >> 1 as libc::c_int);
                        if (*ra_58).val.tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                        {
                            cond_3 = ((*ra_58).val.value_.i == im as i64) as libc::c_int;
                        } else if (*ra_58).val.tt_ as libc::c_int
                            == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                        {
                            cond_3 = ((*ra_58).val.value_.n == im as f64) as libc::c_int;
                        } else {
                            cond_3 = 0 as libc::c_int;
                        }
                        if cond_3
                            != (i >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 1 as libc::c_int)
                                    << 0 as libc::c_int)
                                as libc::c_int
                        {
                            pc = pc.offset(1);
                        } else {
                            let mut ni_3: u32 = *pc;
                            pc = pc.offset(
                                ((ni_3 >> 0 as libc::c_int + 7 as libc::c_int
                                    & !(!(0 as libc::c_int as u32)
                                        << 8 as libc::c_int
                                            + 8 as libc::c_int
                                            + 1 as libc::c_int
                                            + 8 as libc::c_int)
                                        << 0 as libc::c_int)
                                    as libc::c_int
                                    - (((1 as libc::c_int)
                                        << 8 as libc::c_int
                                            + 8 as libc::c_int
                                            + 1 as libc::c_int
                                            + 8 as libc::c_int)
                                        - 1 as libc::c_int
                                        >> 1 as libc::c_int)
                                    + 1 as libc::c_int) as isize,
                            );
                            trap = (*ci).u.trap;
                        }
                        continue;
                    }
                    62 => {
                        let mut ra_59: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut cond_4: libc::c_int = 0;
                        let mut im_0: libc::c_int = (i
                            >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                            as libc::c_int
                            - (((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int
                                >> 1 as libc::c_int);
                        if (*ra_59).val.tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                        {
                            cond_4 = ((*ra_59).val.value_.i < im_0 as i64) as libc::c_int;
                        } else if (*ra_59).val.tt_ as libc::c_int
                            == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                        {
                            let mut fa: f64 = (*ra_59).val.value_.n;
                            let mut fim: f64 = im_0 as f64;
                            cond_4 = (fa < fim) as libc::c_int;
                        } else {
                            let mut isf: libc::c_int = (i
                                >> 0 as libc::c_int
                                    + 7 as libc::c_int
                                    + 8 as libc::c_int
                                    + 1 as libc::c_int
                                    + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int)
                                as libc::c_int;
                            (*ci).u.savedpc = pc;
                            (*L).top = (*ci).top;
                            cond_4 = luaT_callorderiTM(
                                L,
                                &mut (*ra_59).val,
                                im_0,
                                0 as libc::c_int,
                                isf,
                                TM_LT,
                            )?;
                            trap = (*ci).u.trap;
                        }
                        if cond_4
                            != (i >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 1 as libc::c_int)
                                    << 0 as libc::c_int)
                                as libc::c_int
                        {
                            pc = pc.offset(1);
                        } else {
                            let mut ni_4: u32 = *pc;
                            pc = pc.offset(
                                ((ni_4 >> 0 as libc::c_int + 7 as libc::c_int
                                    & !(!(0 as libc::c_int as u32)
                                        << 8 as libc::c_int
                                            + 8 as libc::c_int
                                            + 1 as libc::c_int
                                            + 8 as libc::c_int)
                                        << 0 as libc::c_int)
                                    as libc::c_int
                                    - (((1 as libc::c_int)
                                        << 8 as libc::c_int
                                            + 8 as libc::c_int
                                            + 1 as libc::c_int
                                            + 8 as libc::c_int)
                                        - 1 as libc::c_int
                                        >> 1 as libc::c_int)
                                    + 1 as libc::c_int) as isize,
                            );
                            trap = (*ci).u.trap;
                        }
                        continue;
                    }
                    63 => {
                        let mut ra_60: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut cond_5: libc::c_int = 0;
                        let mut im_1: libc::c_int = (i
                            >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                            as libc::c_int
                            - (((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int
                                >> 1 as libc::c_int);
                        if (*ra_60).val.tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                        {
                            cond_5 = ((*ra_60).val.value_.i <= im_1 as i64) as libc::c_int;
                        } else if (*ra_60).val.tt_ as libc::c_int
                            == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                        {
                            let mut fa_0: f64 = (*ra_60).val.value_.n;
                            let mut fim_0: f64 = im_1 as f64;
                            cond_5 = (fa_0 <= fim_0) as libc::c_int;
                        } else {
                            let mut isf_0: libc::c_int = (i
                                >> 0 as libc::c_int
                                    + 7 as libc::c_int
                                    + 8 as libc::c_int
                                    + 1 as libc::c_int
                                    + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int)
                                as libc::c_int;
                            (*ci).u.savedpc = pc;
                            (*L).top = (*ci).top;
                            cond_5 = luaT_callorderiTM(
                                L,
                                &mut (*ra_60).val,
                                im_1,
                                0 as libc::c_int,
                                isf_0,
                                TM_LE,
                            )?;
                            trap = (*ci).u.trap;
                        }
                        if cond_5
                            != (i >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 1 as libc::c_int)
                                    << 0 as libc::c_int)
                                as libc::c_int
                        {
                            pc = pc.offset(1);
                        } else {
                            let mut ni_5: u32 = *pc;
                            pc = pc.offset(
                                ((ni_5 >> 0 as libc::c_int + 7 as libc::c_int
                                    & !(!(0 as libc::c_int as u32)
                                        << 8 as libc::c_int
                                            + 8 as libc::c_int
                                            + 1 as libc::c_int
                                            + 8 as libc::c_int)
                                        << 0 as libc::c_int)
                                    as libc::c_int
                                    - (((1 as libc::c_int)
                                        << 8 as libc::c_int
                                            + 8 as libc::c_int
                                            + 1 as libc::c_int
                                            + 8 as libc::c_int)
                                        - 1 as libc::c_int
                                        >> 1 as libc::c_int)
                                    + 1 as libc::c_int) as isize,
                            );
                            trap = (*ci).u.trap;
                        }
                        continue;
                    }
                    64 => {
                        let mut ra_61: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut cond_6: libc::c_int = 0;
                        let mut im_2: libc::c_int = (i
                            >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                            as libc::c_int
                            - (((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int
                                >> 1 as libc::c_int);
                        if (*ra_61).val.tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                        {
                            cond_6 = ((*ra_61).val.value_.i > im_2 as i64) as libc::c_int;
                        } else if (*ra_61).val.tt_ as libc::c_int
                            == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                        {
                            let mut fa_1: f64 = (*ra_61).val.value_.n;
                            let mut fim_1: f64 = im_2 as f64;
                            cond_6 = (fa_1 > fim_1) as libc::c_int;
                        } else {
                            let mut isf_1: libc::c_int = (i
                                >> 0 as libc::c_int
                                    + 7 as libc::c_int
                                    + 8 as libc::c_int
                                    + 1 as libc::c_int
                                    + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int)
                                as libc::c_int;
                            (*ci).u.savedpc = pc;
                            (*L).top = (*ci).top;
                            cond_6 = luaT_callorderiTM(
                                L,
                                &mut (*ra_61).val,
                                im_2,
                                1 as libc::c_int,
                                isf_1,
                                TM_LT,
                            )?;
                            trap = (*ci).u.trap;
                        }
                        if cond_6
                            != (i >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 1 as libc::c_int)
                                    << 0 as libc::c_int)
                                as libc::c_int
                        {
                            pc = pc.offset(1);
                        } else {
                            let mut ni_6: u32 = *pc;
                            pc = pc.offset(
                                ((ni_6 >> 0 as libc::c_int + 7 as libc::c_int
                                    & !(!(0 as libc::c_int as u32)
                                        << 8 as libc::c_int
                                            + 8 as libc::c_int
                                            + 1 as libc::c_int
                                            + 8 as libc::c_int)
                                        << 0 as libc::c_int)
                                    as libc::c_int
                                    - (((1 as libc::c_int)
                                        << 8 as libc::c_int
                                            + 8 as libc::c_int
                                            + 1 as libc::c_int
                                            + 8 as libc::c_int)
                                        - 1 as libc::c_int
                                        >> 1 as libc::c_int)
                                    + 1 as libc::c_int) as isize,
                            );
                            trap = (*ci).u.trap;
                        }
                        continue;
                    }
                    65 => {
                        let mut ra_62: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut cond_7: libc::c_int = 0;
                        let mut im_3: libc::c_int = (i
                            >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                            as libc::c_int
                            - (((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int
                                >> 1 as libc::c_int);
                        if (*ra_62).val.tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                        {
                            cond_7 = ((*ra_62).val.value_.i >= im_3 as i64) as libc::c_int;
                        } else if (*ra_62).val.tt_ as libc::c_int
                            == 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int
                        {
                            let mut fa_2: f64 = (*ra_62).val.value_.n;
                            let mut fim_2: f64 = im_3 as f64;
                            cond_7 = (fa_2 >= fim_2) as libc::c_int;
                        } else {
                            let mut isf_2: libc::c_int = (i
                                >> 0 as libc::c_int
                                    + 7 as libc::c_int
                                    + 8 as libc::c_int
                                    + 1 as libc::c_int
                                    + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int)
                                as libc::c_int;
                            (*ci).u.savedpc = pc;
                            (*L).top = (*ci).top;
                            cond_7 = luaT_callorderiTM(
                                L,
                                &mut (*ra_62).val,
                                im_3,
                                1 as libc::c_int,
                                isf_2,
                                TM_LE,
                            )?;
                            trap = (*ci).u.trap;
                        }
                        if cond_7
                            != (i >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 1 as libc::c_int)
                                    << 0 as libc::c_int)
                                as libc::c_int
                        {
                            pc = pc.offset(1);
                        } else {
                            let mut ni_7: u32 = *pc;
                            pc = pc.offset(
                                ((ni_7 >> 0 as libc::c_int + 7 as libc::c_int
                                    & !(!(0 as libc::c_int as u32)
                                        << 8 as libc::c_int
                                            + 8 as libc::c_int
                                            + 1 as libc::c_int
                                            + 8 as libc::c_int)
                                        << 0 as libc::c_int)
                                    as libc::c_int
                                    - (((1 as libc::c_int)
                                        << 8 as libc::c_int
                                            + 8 as libc::c_int
                                            + 1 as libc::c_int
                                            + 8 as libc::c_int)
                                        - 1 as libc::c_int
                                        >> 1 as libc::c_int)
                                    + 1 as libc::c_int) as isize,
                            );
                            trap = (*ci).u.trap;
                        }
                        continue;
                    }
                    66 => {
                        let mut ra_63: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut cond_8: libc::c_int = !((*ra_63).val.tt_ as libc::c_int
                            == 1 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                            || (*ra_63).val.tt_ as libc::c_int & 0xf as libc::c_int
                                == 0 as libc::c_int)
                            as libc::c_int;
                        if cond_8
                            != (i >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 1 as libc::c_int)
                                    << 0 as libc::c_int)
                                as libc::c_int
                        {
                            pc = pc.offset(1);
                        } else {
                            let mut ni_8: u32 = *pc;
                            pc = pc.offset(
                                ((ni_8 >> 0 as libc::c_int + 7 as libc::c_int
                                    & !(!(0 as libc::c_int as u32)
                                        << 8 as libc::c_int
                                            + 8 as libc::c_int
                                            + 1 as libc::c_int
                                            + 8 as libc::c_int)
                                        << 0 as libc::c_int)
                                    as libc::c_int
                                    - (((1 as libc::c_int)
                                        << 8 as libc::c_int
                                            + 8 as libc::c_int
                                            + 1 as libc::c_int
                                            + 8 as libc::c_int)
                                        - 1 as libc::c_int
                                        >> 1 as libc::c_int)
                                    + 1 as libc::c_int) as isize,
                            );
                            trap = (*ci).u.trap;
                        }
                        continue;
                    }
                    67 => {
                        let mut ra_64: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut rb_18: *mut TValue = &mut (*base.offset(
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        ))
                        .val;
                        if ((*rb_18).tt_ as libc::c_int
                            == 1 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                            || (*rb_18).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int)
                            as libc::c_int
                            == (i >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 1 as libc::c_int)
                                    << 0 as libc::c_int)
                                as libc::c_int
                        {
                            pc = pc.offset(1);
                        } else {
                            let mut io1_14: *mut TValue = &mut (*ra_64).val;
                            let mut io2_14: *const TValue = rb_18;
                            (*io1_14).value_ = (*io2_14).value_;
                            (*io1_14).tt_ = (*io2_14).tt_;
                            let mut ni_9: u32 = *pc;
                            pc = pc.offset(
                                ((ni_9 >> 0 as libc::c_int + 7 as libc::c_int
                                    & !(!(0 as libc::c_int as u32)
                                        << 8 as libc::c_int
                                            + 8 as libc::c_int
                                            + 1 as libc::c_int
                                            + 8 as libc::c_int)
                                        << 0 as libc::c_int)
                                    as libc::c_int
                                    - (((1 as libc::c_int)
                                        << 8 as libc::c_int
                                            + 8 as libc::c_int
                                            + 1 as libc::c_int
                                            + 8 as libc::c_int)
                                        - 1 as libc::c_int
                                        >> 1 as libc::c_int)
                                    + 1 as libc::c_int) as isize,
                            );
                            trap = (*ci).u.trap;
                        }
                        continue;
                    }
                    68 => {
                        ra_65 = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        newci = 0 as *mut CallInfo;
                        b_4 = (i
                            >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                            as libc::c_int;
                        nresults = (i
                            >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                            as libc::c_int
                            - 1 as libc::c_int;
                        if b_4 != 0 as libc::c_int {
                            (*L).top = ra_65.offset(b_4 as isize);
                        }
                        (*ci).u.savedpc = pc;
                        newci = luaD_precall(L, ra_65, nresults)?;
                        if !newci.is_null() {
                            break '_returning;
                        }
                        trap = (*ci).u.trap;
                        continue;
                    }
                    69 => {
                        let mut ra_66: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut b_5: libc::c_int = (i
                            >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                            as libc::c_int;
                        let mut n_2: libc::c_int = 0;
                        let mut nparams1: libc::c_int = (i
                            >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                            as libc::c_int;
                        let mut delta: libc::c_int = if nparams1 != 0 {
                            (*ci).u.nextraargs + nparams1
                        } else {
                            0 as libc::c_int
                        };
                        if b_5 != 0 as libc::c_int {
                            (*L).top = ra_66.offset(b_5 as isize);
                        } else {
                            b_5 = ((*L).top).offset_from(ra_66) as libc::c_long as libc::c_int;
                        }
                        (*ci).u.savedpc = pc;
                        if (i
                            & (1 as libc::c_uint)
                                << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int)
                            as libc::c_int
                            != 0
                        {
                            luaF_closeupval(L, base);
                        }
                        n_2 = luaD_pretailcall(L, ci, ra_66, b_5, delta)?;
                        if n_2 < 0 {
                            continue '_startfunc;
                        }
                        (*ci).func = ((*ci).func).offset(-(delta as isize));
                        luaD_poscall(L, ci, n_2)?;
                        trap = (*ci).u.trap;
                        break;
                    }
                    70 => {
                        let mut ra_67: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut n_3: libc::c_int = (i
                            >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                            as libc::c_int
                            - 1 as libc::c_int;
                        let mut nparams1_0: libc::c_int = (i
                            >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                            as libc::c_int;
                        if n_3 < 0 as libc::c_int {
                            n_3 = ((*L).top).offset_from(ra_67) as libc::c_long as libc::c_int;
                        }
                        (*ci).u.savedpc = pc;
                        if (i
                            & (1 as libc::c_uint)
                                << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int)
                            as libc::c_int
                            != 0
                        {
                            (*ci).u2.nres = n_3;
                            if (*L).top < (*ci).top {
                                (*L).top = (*ci).top;
                            }
                            luaF_close(L, base)?;
                            trap = (*ci).u.trap;
                            if (trap != 0 as libc::c_int) as libc::c_int as libc::c_long != 0 {
                                base = ((*ci).func).offset(1 as libc::c_int as isize);
                                ra_67 = base.offset(
                                    (i >> 0 as libc::c_int + 7 as libc::c_int
                                        & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                            << 0 as libc::c_int)
                                        as libc::c_int as isize,
                                );
                            }
                        }
                        if nparams1_0 != 0 {
                            (*ci).func =
                                ((*ci).func).offset(-(((*ci).u.nextraargs + nparams1_0) as isize));
                        }
                        (*L).top = ra_67.offset(n_3 as isize);
                        luaD_poscall(L, ci, n_3)?;
                        trap = (*ci).u.trap;
                        break;
                    }
                    71 => {
                        if ((*L).hookmask.get() != 0) as libc::c_int as libc::c_long != 0 {
                            let mut ra_68: StkId = base.offset(
                                (i >> 0 as libc::c_int + 7 as libc::c_int
                                    & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                        << 0 as libc::c_int)
                                    as libc::c_int as isize,
                            );
                            (*L).top = ra_68;
                            (*ci).u.savedpc = pc;
                            luaD_poscall(L, ci, 0 as libc::c_int)?;
                            trap = 1 as libc::c_int;
                        } else {
                            let mut nres: libc::c_int = 0;
                            (*L).ci = (*ci).previous;
                            (*L).top = base.offset(-(1 as libc::c_int as isize));
                            nres = (*ci).nresults as libc::c_int;
                            while ((nres > 0 as libc::c_int) as libc::c_int != 0 as libc::c_int)
                                as libc::c_int as libc::c_long
                                != 0
                            {
                                let fresh5 = (*L).top;
                                (*L).top = ((*L).top).offset(1);
                                (*fresh5).val.tt_ = (0 as libc::c_int
                                    | (0 as libc::c_int) << 4 as libc::c_int)
                                    as u8;
                                nres -= 1;
                            }
                        }
                        break;
                    }
                    72 => {
                        if ((*L).hookmask.get() != 0) as libc::c_int as libc::c_long != 0 {
                            let mut ra_69: StkId = base.offset(
                                (i >> 0 as libc::c_int + 7 as libc::c_int
                                    & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                        << 0 as libc::c_int)
                                    as libc::c_int as isize,
                            );
                            (*L).top = ra_69.offset(1 as libc::c_int as isize);
                            (*ci).u.savedpc = pc;
                            luaD_poscall(L, ci, 1 as libc::c_int)?;
                            trap = 1 as libc::c_int;
                        } else {
                            let mut nres_0: libc::c_int = (*ci).nresults as libc::c_int;
                            (*L).ci = (*ci).previous;
                            if nres_0 == 0 as libc::c_int {
                                (*L).top = base.offset(-(1 as libc::c_int as isize));
                            } else {
                                let mut ra_70: StkId = base.offset(
                                    (i >> 0 as libc::c_int + 7 as libc::c_int
                                        & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                            << 0 as libc::c_int)
                                        as libc::c_int as isize,
                                );
                                let mut io1_15: *mut TValue =
                                    &mut (*base.offset(-(1 as libc::c_int as isize))).val;
                                let mut io2_15: *const TValue = &mut (*ra_70).val;
                                (*io1_15).value_ = (*io2_15).value_;
                                (*io1_15).tt_ = (*io2_15).tt_;
                                (*L).top = base;
                                while ((nres_0 > 1 as libc::c_int) as libc::c_int
                                    != 0 as libc::c_int)
                                    as libc::c_int
                                    as libc::c_long
                                    != 0
                                {
                                    let fresh6 = (*L).top;
                                    (*L).top = ((*L).top).offset(1);
                                    (*fresh6).val.tt_ = (0 as libc::c_int
                                        | (0 as libc::c_int) << 4 as libc::c_int)
                                        as u8;
                                    nres_0 -= 1;
                                }
                            }
                        }
                        break;
                    }
                    73 => {
                        let mut ra_71: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        if (*ra_71.offset(2 as libc::c_int as isize)).val.tt_ as libc::c_int
                            == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                        {
                            let mut count: u64 =
                                (*ra_71.offset(1 as libc::c_int as isize)).val.value_.i as u64;
                            if count > 0 as libc::c_int as u64 {
                                let mut step: i64 =
                                    (*ra_71.offset(2 as libc::c_int as isize)).val.value_.i;
                                let mut idx: i64 = (*ra_71).val.value_.i;
                                let mut io_43: *mut TValue =
                                    &mut (*ra_71.offset(1 as libc::c_int as isize)).val;
                                (*io_43).value_.i =
                                    count.wrapping_sub(1 as libc::c_int as u64) as i64;
                                idx = (idx as u64).wrapping_add(step as u64) as i64;
                                let mut io_44: *mut TValue = &mut (*ra_71).val;
                                (*io_44).value_.i = idx;
                                let mut io_45: *mut TValue =
                                    &mut (*ra_71.offset(3 as libc::c_int as isize)).val;
                                (*io_45).value_.i = idx;
                                (*io_45).tt_ = (3 as libc::c_int
                                    | (0 as libc::c_int) << 4 as libc::c_int)
                                    as u8;
                                pc = pc.offset(
                                    -((i >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int
                                        & !(!(0 as libc::c_int as u32)
                                            << 8 as libc::c_int
                                                + 8 as libc::c_int
                                                + 1 as libc::c_int)
                                            << 0 as libc::c_int)
                                        as libc::c_int
                                        as isize),
                                );
                            }
                        } else if floatforloop(ra_71) != 0 {
                            pc = pc.offset(
                                -((i >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int
                                    & !(!(0 as libc::c_int as u32)
                                        << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int)
                                        << 0 as libc::c_int)
                                    as libc::c_int as isize),
                            );
                        }
                        trap = (*ci).u.trap;
                        continue;
                    }
                    74 => {
                        let mut ra_72: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        (*ci).u.savedpc = pc;
                        (*L).top = (*ci).top;
                        if forprep(L, ra_72)? != 0 {
                            pc = pc.offset(
                                ((i >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int
                                    & !(!(0 as libc::c_int as u32)
                                        << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int)
                                        << 0 as libc::c_int)
                                    as libc::c_int
                                    + 1 as libc::c_int) as isize,
                            );
                        }
                        continue;
                    }
                    75 => {
                        let mut ra_73: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        (*ci).u.savedpc = pc;
                        (*L).top = (*ci).top;
                        luaF_newtbcupval(L, ra_73.offset(3 as libc::c_int as isize))?;
                        pc = pc.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32)
                                    << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let fresh7 = pc;
                        pc = pc.offset(1);
                        i = *fresh7;
                        current_block = 13973394567113199817;
                    }
                    76 => {
                        current_block = 13973394567113199817;
                    }
                    77 => {
                        current_block = 15611964311717037170;
                    }
                    78 => {
                        let mut ra_76: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut n_4: libc::c_int = (i
                            >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                            as libc::c_int;
                        let mut last: libc::c_uint = (i
                            >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                            as libc::c_int
                            as libc::c_uint;
                        let mut h: *mut Table = ((*ra_76).val.value_.gc as *mut Table);
                        if n_4 == 0 as libc::c_int {
                            n_4 = ((*L).top).offset_from(ra_76) as libc::c_long as libc::c_int
                                - 1 as libc::c_int;
                        } else {
                            (*L).top = (*ci).top;
                        }
                        last = last.wrapping_add(n_4 as libc::c_uint);
                        if (i
                            & (1 as libc::c_uint)
                                << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int)
                            as libc::c_int
                            != 0
                        {
                            last = last.wrapping_add(
                                ((*pc >> 0 as libc::c_int + 7 as libc::c_int
                                    & !(!(0 as libc::c_int as u32)
                                        << 8 as libc::c_int
                                            + 8 as libc::c_int
                                            + 1 as libc::c_int
                                            + 8 as libc::c_int)
                                        << 0 as libc::c_int)
                                    as libc::c_int
                                    * (((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int
                                        + 1 as libc::c_int))
                                    as libc::c_uint,
                            );
                            pc = pc.offset(1);
                        }
                        if last > luaH_realasize(h) {
                            luaH_resizearray(L, h, last)?;
                        }
                        while n_4 > 0 as libc::c_int {
                            let mut val: *mut TValue = &mut (*ra_76.offset(n_4 as isize)).val;
                            let mut io1_17: *mut TValue =
                                &mut *((*h).array)
                                    .offset(last.wrapping_sub(1 as libc::c_int as libc::c_uint)
                                        as isize) as *mut TValue;
                            let mut io2_17: *const TValue = val;
                            (*io1_17).value_ = (*io2_17).value_;
                            (*io1_17).tt_ = (*io2_17).tt_;
                            last = last.wrapping_sub(1);
                            if (*val).tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int
                                != 0
                            {
                                if (*h).marked as libc::c_int
                                    & (1 as libc::c_int) << 5 as libc::c_int
                                    != 0
                                    && (*(*val).value_.gc).marked as libc::c_int
                                        & ((1 as libc::c_int) << 3 as libc::c_int
                                            | (1 as libc::c_int) << 4 as libc::c_int)
                                        != 0
                                {
                                    luaC_barrierback_(L, h as *mut GCObject);
                                } else {
                                };
                            } else {
                            };
                            n_4 -= 1;
                        }
                        continue;
                    }
                    79 => {
                        let mut ra_77: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut p: *mut Proto = *((*(*cl).p).p).offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32)
                                    << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        (*ci).u.savedpc = pc;
                        (*L).top = (*ci).top;
                        pushclosure(L, p, ((*cl).upvals).as_mut_ptr(), base, ra_77);

                        if (*(*L).global).gc.debt() > 0 {
                            (*ci).u.savedpc = pc;
                            (*L).top = ra_77.offset(1 as libc::c_int as isize);
                            luaC_step(L);
                            trap = (*ci).u.trap;
                        }

                        continue;
                    }
                    80 => {
                        let mut ra_78: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        let mut n_5: libc::c_int = (i
                            >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                            as libc::c_int
                            - 1 as libc::c_int;
                        (*ci).u.savedpc = pc;
                        (*L).top = (*ci).top;
                        luaT_getvarargs(L, ci, ra_78, n_5)?;
                        trap = (*ci).u.trap;
                        continue;
                    }
                    81 => {
                        (*ci).u.savedpc = pc;
                        luaT_adjustvarargs(
                            L,
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int,
                            ci,
                            (*cl).p,
                        )?;
                        trap = (*ci).u.trap;
                        if (trap != 0 as libc::c_int) as libc::c_int as libc::c_long != 0 {
                            luaD_hookcall(L, ci)?;
                            (*L).oldpc.set(1);
                        }
                        base = ((*ci).func).offset(1 as libc::c_int as isize);
                        continue;
                    }
                    82 | _ => {
                        continue;
                    }
                }
                match current_block {
                    13973394567113199817 => {
                        let mut ra_74: StkId = base.offset(
                            (i >> 0 as libc::c_int + 7 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int
                                as isize,
                        );
                        memcpy(
                            ra_74.offset(4 as libc::c_int as isize) as *mut libc::c_void,
                            ra_74 as *const libc::c_void,
                            3usize.wrapping_mul(::core::mem::size_of::<StackValue>()),
                        );
                        (*L).top = ra_74
                            .offset(4 as libc::c_int as isize)
                            .offset(3 as libc::c_int as isize);
                        (*ci).u.savedpc = pc;
                        luaD_call(
                            L,
                            ra_74.offset(4 as libc::c_int as isize),
                            (i >> 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                    << 0 as libc::c_int) as libc::c_int,
                        )?;
                        trap = (*ci).u.trap;
                        if (trap != 0 as libc::c_int) as libc::c_int as libc::c_long != 0 {
                            base = ((*ci).func).offset(1 as libc::c_int as isize);
                            ra_74 = base.offset(
                                (i >> 0 as libc::c_int + 7 as libc::c_int
                                    & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                        << 0 as libc::c_int)
                                    as libc::c_int as isize,
                            );
                        }
                        let fresh8 = pc;
                        pc = pc.offset(1);
                        i = *fresh8;
                    }
                    _ => {}
                }
                let mut ra_75: StkId = base.offset(
                    (i >> 0 as libc::c_int + 7 as libc::c_int
                        & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                        as libc::c_int as isize,
                );
                if !((*ra_75.offset(4 as libc::c_int as isize)).val.tt_ as libc::c_int
                    & 0xf as libc::c_int
                    == 0 as libc::c_int)
                {
                    let mut io1_16: *mut TValue =
                        &mut (*ra_75.offset(2 as libc::c_int as isize)).val;
                    let mut io2_16: *const TValue =
                        &mut (*ra_75.offset(4 as libc::c_int as isize)).val;
                    (*io1_16).value_ = (*io2_16).value_;
                    (*io1_16).tt_ = (*io2_16).tt_;
                    pc = pc.offset(
                        -((i >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int
                            & !(!(0 as libc::c_int as u32)
                                << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int)
                                << 0 as libc::c_int) as libc::c_int
                            as isize),
                    );
                }
            }
            if (*ci).callstatus as libc::c_int & (1 as libc::c_int) << 2 as libc::c_int != 0 {
                break '_startfunc Ok(());
            }
            ci = (*ci).previous;
        }
        ci = newci;
    }
}
