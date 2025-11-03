use super::{
    F2Ieq, LEnum, LTnum, OP_ADDI, OP_CALL, OP_CLOSURE, OP_CONCAT, OP_GETTABLE, OP_GETTABUP, OP_LEN,
    OP_LOADF, OP_LOADI, OP_MOVE, OP_NEWTABLE, OP_SETLIST, OP_SETTABLE, OP_TAILCALL, OP_TFORCALL,
    floatforloop, forprep, lessequalothers, lessthanothers, luaV_concat, luaV_equalobj,
    luaV_finishget, luaV_finishset, luaV_idiv, luaV_mod, luaV_modf, luaV_objlen, luaV_shiftl,
    luaV_tointegerns, pushclosure,
};
use crate::ldebug::luaG_runerror;
use crate::ldo::{luaD_call, luaD_poscall, luaD_precall, luaD_pretailcall};
use crate::lfunc::{luaF_close, luaF_closeupval, luaF_newtbcupval};
use crate::lstate::CallInfo;
use crate::ltm::{
    TM_BNOT, TM_LE, TM_LT, TM_UNM, TMS, luaT_adjustvarargs, luaT_callorderiTM, luaT_getvarargs,
    luaT_trybinTM, luaT_trybinassocTM, luaT_trybiniTM,
};
use crate::value::UnsafeValue;
use crate::{
    ArithError, Float, LuaFn, NON_YIELDABLE_WAKER, StackValue, Str, Table, Thread, UserData,
    luaH_get, luaH_getint, luaH_getshortstr, luaH_getstr, luaH_realasize, luaH_resize,
    luaH_resizearray,
};
use alloc::boxed::Box;
use core::any::Any;
use core::hint::unreachable_unchecked;
use core::pin::pin;
use core::ptr::{null, null_mut};
use core::task::{Context, Poll, Waker};
use libc::memcpy;

type c_int = i32;
type c_uint = u32;

pub async unsafe fn exec<A>(
    th: &Thread<A>,
    ci: *mut CallInfo<A>,
) -> Result<*mut CallInfo<A>, Box<dyn core::error::Error>> {
    let cl = (*(*ci).func).value_.gc.cast::<LuaFn<A>>();
    let k = (*(*cl).p.get()).k;
    let mut pc = (*ci).u.savedpc;
    let mut base = (*ci).func.add(1);
    let mut i = pc.read();
    let mut tab = null_mut();
    let mut key = UnsafeValue::default();

    pc = pc.offset(1);

    loop {
        let current_block: u64;

        macro_rules! next {
            () => {
                i = pc.read();
                pc = pc.offset(1);
                continue;
            };
        }

        match i & 0x7F {
            OP_MOVE => {
                let ra = base.add((i >> 7 & 0xFF) as usize);
                let rb = base.add((i >> 7 + 8 + 1 & 0xFF) as usize);

                (*ra).tt_ = (*rb).tt_;
                (*ra).value_ = (*rb).value_;

                next!();
            }
            OP_LOADI => {
                let ra = base.add((i >> 7 & 0xFF) as usize);
                let b: i64 = ((i >> 0 as c_int + 7 as c_int + 8 as c_int
                    & !(!(0 as c_int as u32) << 8 as c_int + 8 as c_int + 1 as c_int) << 0 as c_int)
                    as c_int
                    - (((1 as c_int) << 8 as c_int + 8 as c_int + 1 as c_int) - 1 as c_int
                        >> 1 as c_int)) as i64;

                (*ra).tt_ = 3 | 0 << 4;
                (*ra).value_.i = b;

                next!();
            }
            OP_LOADF => {
                let ra = base.add((i >> 7 & 0xFF) as usize);
                let b_0: c_int = (i >> 0 as c_int + 7 as c_int + 8 as c_int
                    & !(!(0 as c_int as u32) << 8 as c_int + 8 as c_int + 1 as c_int) << 0 as c_int)
                    as c_int
                    - (((1 as c_int) << 8 as c_int + 8 as c_int + 1 as c_int) - 1 as c_int
                        >> 1 as c_int);

                (*ra).tt_ = 3 | 1 << 4;
                (*ra).value_.n = (b_0 as f64).into();

                next!();
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
                let io1_0 = ra_2;
                let io2_0 = rb;

                (*io1_0).value_ = (*io2_0).value_;
                (*io1_0).tt_ = (*io2_0).tt_;
                next!();
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
                let io1_1 = ra_3;
                let io2_1 = rb_0;

                (*io1_1).value_ = (*io2_1).value_;
                (*io1_1).tt_ = (*io2_1).tt_;
                next!();
            }
            5 => {
                let ra_4 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                (*ra_4).tt_ = (1 as c_int | (0 as c_int) << 4 as c_int) as u8;
                next!();
            }
            6 => {
                let ra_5 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                (*ra_5).tt_ = (1 as c_int | (0 as c_int) << 4 as c_int) as u8;
                pc = pc.offset(1);
                next!();
            }
            7 => {
                let ra_6 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                (*ra_6).tt_ = (1 as c_int | (1 as c_int) << 4 as c_int) as u8;
                next!();
            }
            8 => {
                let mut ra_7 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let mut b_1: c_int = (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                    as c_int;
                loop {
                    let fresh3 = ra_7;
                    ra_7 = ra_7.offset(1);
                    (*fresh3).tt_ = (0 as c_int | (0 as c_int) << 4 as c_int) as u8;
                    let fresh4 = b_1;
                    b_1 = b_1 - 1;
                    if !(fresh4 != 0) {
                        break;
                    }
                }
                next!();
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
                let io1_2 = ra_8;
                let io2_2 = (*(*cl).upvals[b_2 as usize].get()).v.get();

                (*io1_2).value_ = (*io2_2).value_;
                (*io1_2).tt_ = (*io2_2).tt_;
                next!();
            }
            10 => {
                let ra_9 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let uv = (*cl).upvals[(i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                    as c_int as usize]
                    .get();
                let io1_3 = (*uv).v.get();
                let io2_3 = ra_9;

                (*io1_3).value_ = (*io2_3).value_;
                (*io1_3).tt_ = (*io2_3).tt_;
                if (*ra_9).tt_ as c_int & (1 as c_int) << 6 as c_int != 0 {
                    if (*uv).hdr.marked.get() as c_int & (1 as c_int) << 5 as c_int != 0
                        && (*(*ra_9).value_.gc).marked.get() as c_int
                            & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                            != 0
                    {
                        (*th).hdr.global().gc.barrier(uv.cast(), (*ra_9).value_.gc);
                    }
                }
                next!();
            }
            OP_GETTABUP => {
                let ra = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );

                tab = (*(*cl).upvals[(i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                    & !(!(0 as c_int as u32) << 8) << 0)
                    as c_int as usize]
                    .get())
                .v
                .get();
                let k = k.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let v = match (*tab).tt_ & 0xf {
                    5 => luaH_getshortstr((*tab).value_.gc.cast(), (*k).value_.gc.cast()),
                    7 => 'b: {
                        let ud = (*tab).value_.gc.cast::<UserData<A, dyn Any>>();
                        let props = (*ud).props.get();

                        if props.is_null() {
                            break 'b null();
                        }

                        luaH_getshortstr(props, (*k).value_.gc.cast())
                    }
                    _ => null(),
                };

                if !v.is_null() && (*v).tt_ & 0xf != 0 {
                    (*ra).tt_ = (*v).tt_;
                    (*ra).value_ = (*v).value_;

                    next!();
                }

                key.tt_ = (*k).tt_;
                key.value_ = (*k).value_;

                current_block = 0;
            }
            OP_GETTABLE => {
                let ra = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );

                tab = base
                    .offset(
                        (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    )
                    .cast();
                let k = base
                    .offset(
                        (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    )
                    .cast::<UnsafeValue<A>>();
                let v = match (*tab).tt_ & 0xf {
                    5 => {
                        if (*k).tt_ == 3 | 0 << 4 {
                            luaH_getint((*tab).value_.gc.cast(), (*k).value_.i)
                        } else {
                            luaH_get((*tab).value_.gc.cast(), k)
                        }
                    }
                    7 => {
                        let ud = (*tab).value_.gc.cast::<UserData<A, dyn Any>>();
                        let props = (*ud).props.get();

                        if props.is_null() {
                            null()
                        } else if (*k).tt_ == 3 | 0 << 4 {
                            luaH_getint(props, (*k).value_.i)
                        } else {
                            luaH_get(props, k)
                        }
                    }
                    _ => null(),
                };

                if !v.is_null() && (*v).tt_ & 0xf != 0 {
                    (*ra).tt_ = (*v).tt_;
                    (*ra).value_ = (*v).value_;

                    next!();
                }

                key.tt_ = (*k).tt_;
                key.value_ = (*k).value_;

                current_block = 0;
            }
            13 => {
                let ra = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );

                tab = base
                    .offset(
                        (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    )
                    .cast();
                let c: c_int = (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                    as c_int;
                let v = match (*tab).tt_ & 0xf {
                    5 => luaH_getint((*tab).value_.gc.cast(), c.into()),
                    7 => {
                        let ud = (*tab).value_.gc.cast::<UserData<A, dyn Any>>();
                        let props = (*ud).props.get();

                        match props.is_null() {
                            true => null(),
                            false => luaH_getint(props, c.into()),
                        }
                    }
                    _ => null(),
                };

                if !v.is_null() && (*v).tt_ & 0xf != 0 {
                    (*ra).tt_ = (*v).tt_;
                    (*ra).value_ = (*v).value_;

                    next!();
                }

                key.tt_ = 3 | 0 << 4;
                key.value_.i = c.into();

                current_block = 0;
            }
            14 => {
                let ra = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );

                tab = base
                    .offset(
                        (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    )
                    .cast();
                let k = k.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let v = match (*tab).tt_ & 0xf {
                    5 => luaH_getshortstr((*tab).value_.gc.cast(), (*k).value_.gc.cast()),
                    7 => {
                        let ud = (*tab).value_.gc.cast::<UserData<A, dyn Any>>();
                        let props = (*ud).props.get();

                        match props.is_null() {
                            true => null(),
                            false => luaH_getshortstr(props, (*k).value_.gc.cast()),
                        }
                    }
                    _ => null(),
                };

                if !v.is_null() && (*v).tt_ & 0xf != 0 {
                    (*ra).tt_ = (*v).tt_;
                    (*ra).value_ = (*v).value_;

                    next!();
                }

                key.tt_ = (*k).tt_;
                key.value_ = (*k).value_;

                current_block = 0;
            }
            15 => {
                let mut slot_3 = null();
                let upval_0 = (*(*cl).upvals[(i >> 0 as c_int + 7 as c_int
                    & !(!(0 as c_int as u32) << 8) << 0)
                    as c_int as usize]
                    .get())
                .v
                .get();
                let rb_4 = k.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let rc_2 =
                    if (i & (1 as c_uint) << 0 as c_int + 7 as c_int + 8 as c_int) as c_int != 0 {
                        k.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        )
                    } else {
                        base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        )
                        .cast()
                    };
                let key_2 = (*rb_4).value_.gc.cast::<Str<A>>();

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
                            (*th).hdr.global().gc.barrier_back((*upval_0).value_.gc);
                        }
                    }
                } else {
                    (*ci).u.savedpc = pc;
                    (*th).top.set((*ci).top);
                    luaV_finishset(th, upval_0, rb_4, rc_2, slot_3)?;

                    base = (*ci).func.add(1);
                }
                next!();
            }
            OP_SETTABLE => {
                let tab = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let key = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let val =
                    if (i & (1 as c_uint) << 0 as c_int + 7 as c_int + 8 as c_int) as c_int != 0 {
                        k.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        )
                    } else {
                        base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        )
                        .cast()
                    };
                let slot = match (*tab).tt_ == 5 | 0 << 4 | 1 << 6 {
                    true => {
                        if (*key).tt_ == 3 | 0 << 4 {
                            luaH_getint((*tab).value_.gc.cast(), (*key).value_.i)
                        } else {
                            luaH_get((*tab).value_.gc.cast(), key.cast())
                        }
                    }
                    false => null(),
                };

                if !slot.is_null() && (*slot).tt_ & 0xf != 0 {
                    let slot = slot.cast_mut();

                    (*slot).tt_ = (*val).tt_;
                    (*slot).value_ = (*val).value_;

                    if (*val).tt_ as c_int & (1 as c_int) << 6 as c_int != 0 {
                        if (*(*tab).value_.gc).marked.get() as c_int & (1 as c_int) << 5 as c_int
                            != 0
                            && (*(*val).value_.gc).marked.get() as c_int
                                & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                                != 0
                        {
                            (*th).hdr.global().gc.barrier_back((*tab).value_.gc);
                        }
                    }
                } else {
                    (*ci).u.savedpc = pc;
                    (*th).top.set((*ci).top);
                    luaV_finishset(th, tab.cast(), key.cast(), val, slot)?;

                    base = (*ci).func.add(1);
                }
                next!();
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
                let rc_4 =
                    if (i & (1 as c_uint) << 0 as c_int + 7 as c_int + 8 as c_int) as c_int != 0 {
                        k.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        )
                    } else {
                        base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        )
                        .cast()
                    };
                if if !((*ra_15).tt_ as c_int
                    == 5 as c_int | (0 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int)
                {
                    slot_5 = null();
                    0 as c_int
                } else {
                    slot_5 = if (c_0 as u64).wrapping_sub(1 as c_uint as u64)
                        < (*((*ra_15).value_.gc as *mut Table<A>)).alimit.get() as u64
                    {
                        (*((*ra_15).value_.gc as *mut Table<A>))
                            .array
                            .get()
                            .offset((c_0 - 1 as c_int) as isize)
                    } else {
                        luaH_getint((*ra_15).value_.gc.cast(), c_0 as i64)
                    };
                    !((*slot_5).tt_ as c_int & 0xf as c_int == 0 as c_int) as c_int
                } != 0
                {
                    let io1_10 = slot_5.cast_mut();
                    let io2_10 = rc_4;

                    (*io1_10).value_ = (*io2_10).value_;
                    (*io1_10).tt_ = (*io2_10).tt_;
                    if (*rc_4).tt_ as c_int & (1 as c_int) << 6 as c_int != 0 {
                        if (*(*ra_15).value_.gc).marked.get() as c_int & (1 as c_int) << 5 as c_int
                            != 0
                            && (*(*rc_4).value_.gc).marked.get() as c_int
                                & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                                != 0
                        {
                            (*th).hdr.global().gc.barrier_back((*ra_15).value_.gc);
                        }
                    }
                } else {
                    let mut key_3 = UnsafeValue::default();
                    let io_2 = &raw mut key_3;

                    (*io_2).value_.i = c_0 as i64;
                    (*io_2).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                    (*ci).u.savedpc = pc;
                    (*th).top.set((*ci).top);
                    luaV_finishset(th, ra_15.cast(), &mut key_3, rc_4, slot_5)?;

                    base = (*ci).func.add(1);
                }
                next!();
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
                let rc_5 =
                    if (i & (1 as c_uint) << 0 as c_int + 7 as c_int + 8 as c_int) as c_int != 0 {
                        k.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        )
                    } else {
                        base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        )
                        .cast()
                    };
                let key_4 = (*rb_6).value_.gc as *mut Str<A>;

                if if !((*ra_16).tt_ as c_int
                    == 5 as c_int | (0 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int)
                {
                    slot_6 = null();
                    0 as c_int
                } else {
                    slot_6 = luaH_getshortstr((*ra_16).value_.gc.cast(), key_4);
                    !((*slot_6).tt_ as c_int & 0xf as c_int == 0 as c_int) as c_int
                } != 0
                {
                    let io1_11 = slot_6.cast_mut();
                    let io2_11 = rc_5;

                    (*io1_11).value_ = (*io2_11).value_;
                    (*io1_11).tt_ = (*io2_11).tt_;
                    if (*rc_5).tt_ as c_int & (1 as c_int) << 6 as c_int != 0 {
                        if (*(*ra_16).value_.gc).marked.get() as c_int & (1 as c_int) << 5 as c_int
                            != 0
                            && (*(*rc_5).value_.gc).marked.get() as c_int
                                & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                                != 0
                        {
                            (*th).hdr.global().gc.barrier_back((*ra_16).value_.gc);
                        }
                    }
                } else {
                    (*ci).u.savedpc = pc;
                    (*th).top.set((*ci).top);
                    luaV_finishset(th, ra_16.cast(), rb_6, rc_5, slot_6)?;

                    base = (*ci).func.add(1);
                }
                next!();
            }
            OP_NEWTABLE => {
                let ra_17 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let mut b_3: c_int = (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                    as c_int;
                let mut c_1: c_int = (i
                    >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                    as c_int;

                if b_3 > 0 as c_int {
                    b_3 = (1 as c_int) << b_3 - 1 as c_int;
                }
                if (i & (1 as c_uint) << 0 as c_int + 7 as c_int + 8 as c_int) as c_int != 0 {
                    c_1 += (*pc >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32)
                            << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                            << 0 as c_int) as c_int
                        * (((1 as c_int) << 8 as c_int) - 1 as c_int + 1 as c_int);
                }
                pc = pc.offset(1);
                (*th).top.set(ra_17.offset(1 as c_int as isize));

                // Create table.
                let t = Table::new((*th).hdr.global);
                let io_3 = ra_17;

                (*io_3).value_.gc = t.cast();
                (*io_3).tt_ = 5 | 0 << 4 | 1 << 6;

                if b_3 != 0 as c_int || c_1 != 0 as c_int {
                    luaH_resize(t, c_1 as c_uint, b_3 as c_uint);
                }

                (*ci).u.savedpc = pc;
                (*th).top.set(ra_17.offset(1 as c_int as isize));
                (*th).hdr.global().gc.step();

                base = (*ci).func.add(1);

                next!();
            }
            20 => {
                let ra = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );

                tab = base
                    .offset(
                        (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    )
                    .cast();

                // This one need to set here otherwise it will impact the performance.
                key = if (i & (1 as c_uint) << 0 as c_int + 7 as c_int + 8 as c_int) != 0 {
                    k.offset(
                        (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    )
                    .read()
                } else {
                    base.offset(
                        (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    )
                    .cast::<UnsafeValue<A>>()
                    .read()
                };

                let io1_12 = ra.offset(1 as c_int as isize);

                (*io1_12).tt_ = (*tab).tt_;
                (*io1_12).value_ = (*tab).value_;

                let v = match (*tab).tt_ & 0xf {
                    5 => luaH_getstr((*tab).value_.gc.cast(), key.value_.gc.cast()),
                    7 => {
                        let ud = (*tab).value_.gc.cast::<UserData<A, dyn Any>>();
                        let props = (*ud).props.get();

                        match props.is_null() {
                            true => null(),
                            false => luaH_getstr(props, key.value_.gc.cast()),
                        }
                    }
                    _ => null(),
                };

                if !v.is_null() && (*v).tt_ & 0xf != 0 {
                    (*ra).tt_ = (*v).tt_;
                    (*ra).value_ = (*v).value_;

                    next!();
                }

                current_block = 0;
            }
            OP_ADDI => {
                let ra_19 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let v1 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let imm: c_int = (i
                    >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                    as c_int
                    - (((1 as c_int) << 8 as c_int) - 1 as c_int >> 1 as c_int);
                let tt = (*v1).tt_;

                if tt & 0xf != 3 {
                    next!();
                }

                (*ra_19).tt_ = tt;

                if tt as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                    let iv1: i64 = (*v1).value_.i;
                    let io_4 = ra_19;

                    (*io_4).value_.i = (iv1 as u64).wrapping_add(imm as u64) as i64;
                } else if tt as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                    let nb = (*v1).value_.n;
                    let fimm: f64 = imm as f64;
                    let io_5 = ra_19;

                    (*io_5).value_.n = nb + fimm;
                } else {
                    unreachable_unchecked();
                }

                pc = pc.offset(1);
                next!();
            }
            22 => {
                let v1_0 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
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
                    let io_6 = ra_20;

                    (*io_6).value_.i = (i1 as u64).wrapping_add(i2 as u64) as i64;
                    (*io_6).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                } else {
                    let mut n1 = Float::default();
                    let mut n2 = Float::default();

                    if (if (*v1_0).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                        n1 = (*v1_0).value_.n;
                        1 as c_int
                    } else {
                        if (*v1_0).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                            n1 = ((*v1_0).value_.i as f64).into();
                            1 as c_int
                        } else {
                            0 as c_int
                        }
                    }) != 0
                        && (if (*v2).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                            n2 = (*v2).value_.n;
                            1 as c_int
                        } else {
                            if (*v2).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                                n2 = ((*v2).value_.i as f64).into();
                                1 as c_int
                            } else {
                                0 as c_int
                            }
                        }) != 0
                    {
                        pc = pc.offset(1);
                        let io_7 = ra_20;

                        (*io_7).value_.n = n1 + n2;
                        (*io_7).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                    }
                }

                next!();
            }
            23 => {
                let v1_1 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
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
                    let io_8 = ra_21;

                    (*io_8).value_.i = (i1_0 as u64).wrapping_sub(i2_0 as u64) as i64;
                    (*io_8).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                } else {
                    let mut n1_0 = Float::default();
                    let mut n2_0 = Float::default();

                    if (if (*v1_1).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                        n1_0 = (*v1_1).value_.n;
                        1 as c_int
                    } else {
                        if (*v1_1).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                            n1_0 = ((*v1_1).value_.i as f64).into();
                            1 as c_int
                        } else {
                            0 as c_int
                        }
                    }) != 0
                        && (if (*v2_0).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                            n2_0 = (*v2_0).value_.n;
                            1 as c_int
                        } else {
                            if (*v2_0).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                                n2_0 = ((*v2_0).value_.i as f64).into();
                                1 as c_int
                            } else {
                                0 as c_int
                            }
                        }) != 0
                    {
                        pc = pc.offset(1);
                        let io_9 = ra_21;

                        (*io_9).value_.n = n1_0 - n2_0;
                        (*io_9).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                    }
                }

                next!();
            }
            24 => {
                let v1_2 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
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
                    let io_10 = ra_22;

                    (*io_10).value_.i = (i1_1 as u64 * i2_1 as u64) as i64;
                    (*io_10).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                } else {
                    let mut n1_1 = Float::default();
                    let mut n2_1 = Float::default();

                    if (if (*v1_2).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                        n1_1 = (*v1_2).value_.n;
                        1 as c_int
                    } else {
                        if (*v1_2).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                            n1_1 = ((*v1_2).value_.i as f64).into();
                            1 as c_int
                        } else {
                            0 as c_int
                        }
                    }) != 0
                        && (if (*v2_1).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                            n2_1 = (*v2_1).value_.n;
                            1 as c_int
                        } else {
                            if (*v2_1).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                                n2_1 = ((*v2_1).value_.i as f64).into();
                                1 as c_int
                            } else {
                                0 as c_int
                            }
                        }) != 0
                    {
                        pc = pc.offset(1);
                        let io_11 = ra_22;

                        (*io_11).value_.n = n1_1 * n2_1;
                        (*io_11).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                    }
                }

                next!();
            }
            25 => {
                (*ci).u.savedpc = pc;
                (*th).top.set((*ci).top);

                let v1_3 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
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
                    let io_12 = ra_23;

                    (*io_12).value_.i = match luaV_mod(i1_2, i2_2) {
                        Some(v) => v,
                        None => return Err(luaG_runerror(th, ArithError::ModZero)),
                    };
                    (*io_12).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                } else {
                    let mut n1_2 = Float::default();
                    let mut n2_2 = Float::default();

                    if (if (*v1_3).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                        n1_2 = (*v1_3).value_.n;
                        1 as c_int
                    } else {
                        if (*v1_3).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                            n1_2 = ((*v1_3).value_.i as f64).into();
                            1 as c_int
                        } else {
                            0 as c_int
                        }
                    }) != 0
                        && (if (*v2_2).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                            n2_2 = (*v2_2).value_.n;
                            1 as c_int
                        } else {
                            if (*v2_2).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                                n2_2 = ((*v2_2).value_.i as f64).into();
                                1 as c_int
                            } else {
                                0 as c_int
                            }
                        }) != 0
                    {
                        pc = pc.offset(1);
                        let io_13 = ra_23;

                        (*io_13).value_.n = luaV_modf(n1_2, n2_2);
                        (*io_13).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                    }
                }

                next!();
            }
            26 => {
                let ra_24 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let v1_4 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let v2_3 = k.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let mut n1_3 = Float::default();
                let mut n2_3 = Float::default();

                if (if (*v1_4).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                    n1_3 = (*v1_4).value_.n;
                    1 as c_int
                } else {
                    if (*v1_4).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                        n1_3 = ((*v1_4).value_.i as f64).into();
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
                            n2_3 = ((*v2_3).value_.i as f64).into();
                            1 as c_int
                        } else {
                            0 as c_int
                        }
                    }) != 0
                {
                    pc = pc.offset(1);
                    let io_14 = ra_24;
                    (*io_14).value_.n = if n2_3 == 2 as c_int as f64 {
                        n1_3 * n1_3
                    } else {
                        n1_3.pow(n2_3)
                    };
                    (*io_14).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                }

                next!();
            }
            27 => {
                let ra_25 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let v1_5 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let v2_4 = k.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let mut n1_4 = Float::default();
                let mut n2_4 = Float::default();

                if (if (*v1_5).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                    n1_4 = (*v1_5).value_.n;
                    1 as c_int
                } else {
                    if (*v1_5).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                        n1_4 = ((*v1_5).value_.i as f64).into();
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
                            n2_4 = ((*v2_4).value_.i as f64).into();
                            1 as c_int
                        } else {
                            0 as c_int
                        }
                    }) != 0
                {
                    pc = pc.offset(1);
                    let io_15 = ra_25;

                    (*io_15).value_.n = n1_4 / n2_4;
                    (*io_15).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                }
                next!();
            }
            28 => {
                (*ci).u.savedpc = pc;
                (*th).top.set((*ci).top);
                let v1_6 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
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
                    let io_16 = ra_26;

                    (*io_16).value_.i = match luaV_idiv(i1_3, i2_3) {
                        Some(v) => v,
                        None => return Err(luaG_runerror(th, ArithError::DivZero)),
                    };
                    (*io_16).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                } else {
                    let mut n1_5 = Float::default();
                    let mut n2_5 = Float::default();

                    if (if (*v1_6).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                        n1_5 = (*v1_6).value_.n;
                        1 as c_int
                    } else {
                        if (*v1_6).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                            n1_5 = ((*v1_6).value_.i as f64).into();
                            1 as c_int
                        } else {
                            0 as c_int
                        }
                    }) != 0
                        && (if (*v2_5).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                            n2_5 = (*v2_5).value_.n;
                            1 as c_int
                        } else {
                            if (*v2_5).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                                n2_5 = ((*v2_5).value_.i as f64).into();
                                1 as c_int
                            } else {
                                0 as c_int
                            }
                        }) != 0
                    {
                        pc = pc.offset(1);
                        let io_17 = ra_26;
                        (*io_17).value_.n = (n1_5 / n2_5).floor();
                        (*io_17).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                    }
                }
                next!();
            }
            29 => {
                let ra_27 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let v1_7 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let v2_6 = k.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let mut i1_4: i64 = 0;
                let i2_4: i64 = (*v2_6).value_.i;

                if if (*v1_7).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                    i1_4 = (*v1_7).value_.i;
                    1 as c_int
                } else if let Some(v) = luaV_tointegerns::<A>(v1_7.cast(), F2Ieq) {
                    i1_4 = v;
                    1
                } else {
                    0
                } != 0
                {
                    pc = pc.offset(1);
                    let io_18 = ra_27;
                    (*io_18).value_.i = (i1_4 as u64 & i2_4 as u64) as i64;
                    (*io_18).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                }

                next!();
            }
            30 => {
                let ra_28 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let v1_8 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let v2_7 = k.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let mut i1_5: i64 = 0;
                let i2_5: i64 = (*v2_7).value_.i;

                if if (*v1_8).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                    i1_5 = (*v1_8).value_.i;
                    1 as c_int
                } else if let Some(v) = luaV_tointegerns::<A>(v1_8.cast(), F2Ieq) {
                    i1_5 = v;
                    1
                } else {
                    0
                } != 0
                {
                    pc = pc.offset(1);
                    let io_19 = ra_28;

                    (*io_19).value_.i = (i1_5 as u64 | i2_5 as u64) as i64;
                    (*io_19).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                }

                next!();
            }
            31 => {
                let ra_29 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let v1_9 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let v2_8 = k.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let mut i1_6: i64 = 0;
                let i2_6: i64 = (*v2_8).value_.i;

                if if (*v1_9).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                    i1_6 = (*v1_9).value_.i;
                    1 as c_int
                } else if let Some(v) = luaV_tointegerns::<A>(v1_9.cast(), F2Ieq) {
                    i1_6 = v;
                    1
                } else {
                    0
                } != 0
                {
                    pc = pc.offset(1);
                    let io_20 = ra_29;
                    (*io_20).value_.i = (i1_6 as u64 ^ i2_6 as u64) as i64;
                    (*io_20).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                }

                next!();
            }
            32 => {
                let ra_30 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let rb_8 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let ic: c_int = (i
                    >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                    as c_int
                    - (((1 as c_int) << 8 as c_int) - 1 as c_int >> 1 as c_int);
                let mut ib: i64 = 0;

                if if (*rb_8).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                    ib = (*rb_8).value_.i;
                    1 as c_int
                } else if let Some(v) = luaV_tointegerns::<A>(rb_8.cast(), F2Ieq) {
                    ib = v;
                    1
                } else {
                    0
                } != 0
                {
                    pc = pc.offset(1);
                    let io_21 = ra_30;

                    (*io_21).value_.i = luaV_shiftl(ib, -ic as i64);
                    (*io_21).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                }

                next!();
            }
            33 => {
                let ra_31 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let rb_9 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let ic_0: c_int = (i
                    >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                    as c_int
                    - (((1 as c_int) << 8 as c_int) - 1 as c_int >> 1 as c_int);
                let mut ib_0: i64 = 0;

                if if (*rb_9).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                    ib_0 = (*rb_9).value_.i;
                    1 as c_int
                } else if let Some(v) = luaV_tointegerns::<A>(rb_9.cast(), F2Ieq) {
                    ib_0 = v;
                    1
                } else {
                    0
                } != 0
                {
                    pc = pc.offset(1);
                    let io_22 = ra_31;
                    (*io_22).value_.i = luaV_shiftl(ic_0 as i64, ib_0);
                    (*io_22).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                }

                next!();
            }
            34 => {
                let v1_10 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let v2_9 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
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
                    let io_23 = ra_32;
                    (*io_23).value_.i = (i1_7 as u64).wrapping_add(i2_7 as u64) as i64;
                    (*io_23).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                } else {
                    let mut n1_6 = Float::default();
                    let mut n2_6 = Float::default();

                    if (if (*v1_10).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                        n1_6 = (*v1_10).value_.n;
                        1 as c_int
                    } else {
                        if (*v1_10).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                            n1_6 = ((*v1_10).value_.i as f64).into();
                            1 as c_int
                        } else {
                            0 as c_int
                        }
                    }) != 0
                        && (if (*v2_9).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                            n2_6 = (*v2_9).value_.n;
                            1 as c_int
                        } else {
                            if (*v2_9).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                                n2_6 = ((*v2_9).value_.i as f64).into();
                                1 as c_int
                            } else {
                                0 as c_int
                            }
                        }) != 0
                    {
                        pc = pc.offset(1);
                        let io_24 = ra_32;
                        (*io_24).value_.n = n1_6 + n2_6;
                        (*io_24).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                    }
                }

                next!();
            }
            35 => {
                let v1_11 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let v2_10 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
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
                    let io_25 = ra_33;
                    (*io_25).value_.i = (i1_8 as u64).wrapping_sub(i2_8 as u64) as i64;
                    (*io_25).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                } else {
                    let mut n1_7 = Float::default();
                    let mut n2_7 = Float::default();

                    if (if (*v1_11).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                        n1_7 = (*v1_11).value_.n;
                        1 as c_int
                    } else {
                        if (*v1_11).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                            n1_7 = ((*v1_11).value_.i as f64).into();
                            1 as c_int
                        } else {
                            0 as c_int
                        }
                    }) != 0
                        && (if (*v2_10).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                            n2_7 = (*v2_10).value_.n;
                            1 as c_int
                        } else {
                            if (*v2_10).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                                n2_7 = ((*v2_10).value_.i as f64).into();
                                1 as c_int
                            } else {
                                0 as c_int
                            }
                        }) != 0
                    {
                        pc = pc.offset(1);
                        let io_26 = ra_33;
                        (*io_26).value_.n = n1_7 - n2_7;
                        (*io_26).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                    }
                }

                next!();
            }
            36 => {
                let v1_12 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let v2_11 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
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
                    let io_27 = ra_34;
                    (*io_27).value_.i = ((i1_9 as u64).wrapping_mul(i2_9 as u64)) as i64;
                    (*io_27).tt_ = 3 | 0 << 4;
                } else {
                    let mut n1_8 = Float::default();
                    let mut n2_8 = Float::default();

                    if (if (*v1_12).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                        n1_8 = (*v1_12).value_.n;
                        1 as c_int
                    } else {
                        if (*v1_12).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                            n1_8 = ((*v1_12).value_.i as f64).into();
                            1 as c_int
                        } else {
                            0 as c_int
                        }
                    }) != 0
                        && (if (*v2_11).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                            n2_8 = (*v2_11).value_.n;
                            1 as c_int
                        } else {
                            if (*v2_11).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                                n2_8 = ((*v2_11).value_.i as f64).into();
                                1 as c_int
                            } else {
                                0 as c_int
                            }
                        }) != 0
                    {
                        pc = pc.offset(1);
                        let io_28 = ra_34;
                        (*io_28).value_.n = n1_8 * n2_8;
                        (*io_28).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                    }
                }

                next!();
            }
            37 => {
                (*ci).u.savedpc = pc;
                (*th).top.set((*ci).top);
                let v1_13 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let v2_12 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
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
                    let io_29 = ra_35;
                    (*io_29).value_.i = match luaV_mod(i1_10, i2_10) {
                        Some(v) => v,
                        None => return Err(luaG_runerror(th, ArithError::ModZero)),
                    };
                    (*io_29).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                } else {
                    let mut n1_9 = Float::default();
                    let mut n2_9 = Float::default();

                    if (if (*v1_13).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                        n1_9 = (*v1_13).value_.n;
                        1 as c_int
                    } else {
                        if (*v1_13).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                            n1_9 = ((*v1_13).value_.i as f64).into();
                            1 as c_int
                        } else {
                            0 as c_int
                        }
                    }) != 0
                        && (if (*v2_12).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                            n2_9 = (*v2_12).value_.n;
                            1 as c_int
                        } else {
                            if (*v2_12).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                                n2_9 = ((*v2_12).value_.i as f64).into();
                                1 as c_int
                            } else {
                                0 as c_int
                            }
                        }) != 0
                    {
                        pc = pc.offset(1);
                        let io_30 = ra_35;
                        (*io_30).value_.n = luaV_modf(n1_9, n2_9);
                        (*io_30).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                    }
                }

                next!();
            }
            38 => {
                let ra_36 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let v1_14 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let v2_13 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let mut n1_10 = Float::default();
                let mut n2_10 = Float::default();

                if (if (*v1_14).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                    n1_10 = (*v1_14).value_.n;
                    1 as c_int
                } else {
                    if (*v1_14).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                        n1_10 = ((*v1_14).value_.i as f64).into();
                        1 as c_int
                    } else {
                        0 as c_int
                    }
                }) != 0
                    && (if (*v2_13).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                        n2_10 = (*v2_13).value_.n;
                        1 as c_int
                    } else {
                        if (*v2_13).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                            n2_10 = ((*v2_13).value_.i as f64).into();
                            1 as c_int
                        } else {
                            0 as c_int
                        }
                    }) != 0
                {
                    pc = pc.offset(1);
                    let io_31 = ra_36;
                    (*io_31).value_.n = if n2_10 == 2 as c_int as f64 {
                        n1_10 * n1_10
                    } else {
                        n1_10.pow(n2_10)
                    };
                    (*io_31).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                }

                next!();
            }
            39 => {
                let ra_37 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let v1_15 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let v2_14 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let mut n1_11 = Float::default();
                let mut n2_11 = Float::default();

                if (if (*v1_15).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                    n1_11 = (*v1_15).value_.n;
                    1 as c_int
                } else {
                    if (*v1_15).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                        n1_11 = ((*v1_15).value_.i as f64).into();
                        1 as c_int
                    } else {
                        0 as c_int
                    }
                }) != 0
                    && (if (*v2_14).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                        n2_11 = (*v2_14).value_.n;
                        1 as c_int
                    } else {
                        if (*v2_14).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                            n2_11 = ((*v2_14).value_.i as f64).into();
                            1 as c_int
                        } else {
                            0 as c_int
                        }
                    }) != 0
                {
                    pc = pc.offset(1);
                    let io_32 = ra_37;
                    (*io_32).value_.n = n1_11 / n2_11;
                    (*io_32).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                }

                next!();
            }
            40 => {
                (*ci).u.savedpc = pc;
                (*th).top.set((*ci).top);
                let v1_16 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let v2_15 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
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
                    let io_33 = ra_38;
                    (*io_33).value_.i = match luaV_idiv(i1_11, i2_11) {
                        Some(v) => v,
                        None => return Err(luaG_runerror(th, ArithError::DivZero)),
                    };
                    (*io_33).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                } else {
                    let mut n1_12 = Float::default();
                    let mut n2_12 = Float::default();

                    if (if (*v1_16).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                        n1_12 = (*v1_16).value_.n;
                        1 as c_int
                    } else {
                        if (*v1_16).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                            n1_12 = ((*v1_16).value_.i as f64).into();
                            1 as c_int
                        } else {
                            0 as c_int
                        }
                    }) != 0
                        && (if (*v2_15).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                            n2_12 = (*v2_15).value_.n;
                            1 as c_int
                        } else {
                            if (*v2_15).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                                n2_12 = ((*v2_15).value_.i as f64).into();
                                1 as c_int
                            } else {
                                0 as c_int
                            }
                        }) != 0
                    {
                        pc = pc.offset(1);
                        let io_34 = ra_38;
                        (*io_34).value_.n = (n1_12 / n2_12).floor();
                        (*io_34).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                    }
                }

                next!();
            }
            41 => {
                let ra_39 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let v1_17 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let v2_16 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let mut i1_12: i64 = 0;
                let mut i2_12: i64 = 0;

                if (if (*v1_17).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                    i1_12 = (*v1_17).value_.i;
                    1 as c_int
                } else if let Some(v) = luaV_tointegerns::<A>(v1_17.cast(), F2Ieq) {
                    i1_12 = v;
                    1
                } else {
                    0
                }) != 0
                    && (if (*v2_16).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                        i2_12 = (*v2_16).value_.i;
                        1 as c_int
                    } else if let Some(v) = luaV_tointegerns::<A>(v2_16.cast(), F2Ieq) {
                        i2_12 = v;
                        1
                    } else {
                        0
                    }) != 0
                {
                    pc = pc.offset(1);
                    let io_35 = ra_39;
                    (*io_35).value_.i = (i1_12 as u64 & i2_12 as u64) as i64;
                    (*io_35).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                }

                next!();
            }
            42 => {
                let ra_40 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let v1_18 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let v2_17 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let mut i1_13: i64 = 0;
                let mut i2_13: i64 = 0;

                if (if (*v1_18).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                    i1_13 = (*v1_18).value_.i;
                    1 as c_int
                } else if let Some(v) = luaV_tointegerns::<A>(v1_18.cast(), F2Ieq) {
                    i1_13 = v;
                    1
                } else {
                    0
                }) != 0
                    && (if (*v2_17).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                        i2_13 = (*v2_17).value_.i;
                        1 as c_int
                    } else if let Some(v) = luaV_tointegerns::<A>(v2_17.cast(), F2Ieq) {
                        i2_13 = v;
                        1
                    } else {
                        0
                    }) != 0
                {
                    pc = pc.offset(1);
                    let io_36 = ra_40;
                    (*io_36).value_.i = (i1_13 as u64 | i2_13 as u64) as i64;
                    (*io_36).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                }

                next!();
            }
            43 => {
                let ra_41 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let v1_19 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let v2_18 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let mut i1_14: i64 = 0;
                let mut i2_14: i64 = 0;

                if (if (*v1_19).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                    i1_14 = (*v1_19).value_.i;
                    1 as c_int
                } else if let Some(v) = luaV_tointegerns::<A>(v1_19.cast(), F2Ieq) {
                    i1_14 = v;
                    1
                } else {
                    0
                }) != 0
                    && (if (*v2_18).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                        i2_14 = (*v2_18).value_.i;
                        1 as c_int
                    } else if let Some(v) = luaV_tointegerns::<A>(v2_18.cast(), F2Ieq) {
                        i2_14 = v;
                        1
                    } else {
                        0
                    }) != 0
                {
                    pc = pc.offset(1);
                    let io_37 = ra_41;
                    (*io_37).value_.i = (i1_14 as u64 ^ i2_14 as u64) as i64;
                    (*io_37).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                }

                next!();
            }
            45 => {
                let ra_42 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let v1_20 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let v2_19 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let mut i1_15: i64 = 0;
                let mut i2_15: i64 = 0;

                if (if (*v1_20).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                    i1_15 = (*v1_20).value_.i;
                    1 as c_int
                } else if let Some(v) = luaV_tointegerns::<A>(v1_20.cast(), F2Ieq) {
                    i1_15 = v;
                    1
                } else {
                    0
                }) != 0
                    && (if (*v2_19).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                        i2_15 = (*v2_19).value_.i;
                        1 as c_int
                    } else if let Some(v) = luaV_tointegerns::<A>(v2_19.cast(), F2Ieq) {
                        i2_15 = v;
                        1
                    } else {
                        0
                    }) != 0
                {
                    pc = pc.offset(1);
                    let io_38 = ra_42;
                    (*io_38).value_.i =
                        luaV_shiftl(i1_15, (0 as c_int as u64).wrapping_sub(i2_15 as u64) as i64);
                    (*io_38).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                }

                next!();
            }
            44 => {
                let ra_43 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let v1_21 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let v2_20 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let mut i1_16: i64 = 0;
                let mut i2_16: i64 = 0;

                if (if (*v1_21).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                    i1_16 = (*v1_21).value_.i;
                    1 as c_int
                } else if let Some(v) = luaV_tointegerns::<A>(v1_21.cast(), F2Ieq) {
                    i1_16 = v;
                    1
                } else {
                    0
                }) != 0
                    && (if (*v2_20).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                        i2_16 = (*v2_20).value_.i;
                        1 as c_int
                    } else if let Some(v) = luaV_tointegerns::<A>(v2_20.cast(), F2Ieq) {
                        i2_16 = v;
                        1
                    } else {
                        0
                    }) != 0
                {
                    pc = pc.offset(1);
                    let io_39 = ra_43;
                    (*io_39).value_.i = luaV_shiftl(i1_16, i2_16);
                    (*io_39).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                }

                next!();
            }
            46 => {
                let ra_44 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let pi: u32 = *pc.offset(-(2 as c_int as isize));
                let rb_10 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let tm: TMS = (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                    as c_int as TMS;

                (*ci).u.savedpc = pc;
                (*th).top.set((*ci).top);

                let val = luaT_trybinTM(th, ra_44.cast(), rb_10.cast(), tm)?;

                base = (*ci).func.add(1);

                let result = base.offset(
                    (pi >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );

                (*result).tt_ = val.tt_;
                (*result).value_ = val.value_;

                next!();
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
                (*th).top.set((*ci).top);

                let val = luaT_trybiniTM(th, ra_45.cast(), imm_0 as i64, flip, tm_0)?;

                base = (*ci).func.add(1);

                let result = base.offset(
                    (pi_0 >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );

                (*result).tt_ = val.tt_;
                (*result).value_ = val.value_;

                next!();
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
                (*th).top.set((*ci).top);

                let val = luaT_trybinassocTM(th, ra_46.cast(), imm_1, flip_0, tm_1)?;

                base = (*ci).func.add(1);

                let result = base.offset(
                    (pi_1 >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );

                (*result).tt_ = val.tt_;
                (*result).value_ = val.value_;

                next!();
            }
            49 => {
                let mut ra_47 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let rb_11 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let mut nb_0 = Float::default();

                if (*rb_11).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                    let ib_1: i64 = (*rb_11).value_.i;
                    let io_40 = ra_47;
                    (*io_40).value_.i = (0 as c_int as u64).wrapping_sub(ib_1 as u64) as i64;
                    (*io_40).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                } else if if (*rb_11).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 {
                    nb_0 = (*rb_11).value_.n;
                    1 as c_int
                } else if (*rb_11).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                    nb_0 = ((*rb_11).value_.i as f64).into();
                    1 as c_int
                } else {
                    0 as c_int
                } != 0
                {
                    let io_41 = ra_47;
                    (*io_41).value_.n = -nb_0;
                    (*io_41).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                } else {
                    (*ci).u.savedpc = pc;
                    (*th).top.set((*ci).top);

                    let val = luaT_trybinTM(th, rb_11.cast(), rb_11.cast(), TM_UNM)?;

                    base = (*ci).func.add(1);
                    ra_47 = base.offset(
                        (i >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );

                    (*ra_47).tt_ = val.tt_;
                    (*ra_47).value_ = val.value_;
                }

                next!();
            }
            50 => {
                let mut ra_48 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let rb_12 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let mut ib_2: i64 = 0;

                if if (*rb_12).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                    ib_2 = (*rb_12).value_.i;
                    1 as c_int
                } else if let Some(v) = luaV_tointegerns::<A>(rb_12.cast(), F2Ieq) {
                    ib_2 = v;
                    1
                } else {
                    0
                } != 0
                {
                    let io_42 = ra_48;
                    (*io_42).value_.i = (!(0 as c_int as u64) ^ ib_2 as u64) as i64;
                    (*io_42).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                } else {
                    (*ci).u.savedpc = pc;
                    (*th).top.set((*ci).top);

                    let val = luaT_trybinTM(th, rb_12.cast(), rb_12.cast(), TM_BNOT)?;

                    base = (*ci).func.add(1);
                    ra_48 = base.offset(
                        (i >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );

                    (*ra_48).tt_ = val.tt_;
                    (*ra_48).value_ = val.value_;
                }

                next!();
            }
            51 => {
                let ra_49 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let rb_13 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                if (*rb_13).tt_ as c_int == 1 as c_int | (0 as c_int) << 4 as c_int
                    || (*rb_13).tt_ as c_int & 0xf as c_int == 0 as c_int
                {
                    (*ra_49).tt_ = (1 as c_int | (1 as c_int) << 4 as c_int) as u8;
                } else {
                    (*ra_49).tt_ = (1 as c_int | (0 as c_int) << 4 as c_int) as u8;
                }
                next!();
            }
            OP_LEN => {
                (*ci).u.savedpc = pc;
                (*th).top.set((*ci).top);

                let val = luaV_objlen(
                    th,
                    base.offset(
                        (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    )
                    .cast(),
                )?;

                base = (*ci).func.add(1);

                let ra = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );

                (*ra).tt_ = val.tt_;
                (*ra).value_ = val.value_;

                next!();
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

                (*th).top.set(ra_51.offset(n_1 as isize));
                (*ci).u.savedpc = pc;
                luaV_concat(th, n_1)?;

                (*ci).u.savedpc = pc;
                (*th).hdr.global().gc.step();

                base = (*ci).func.add(1);

                next!();
            }
            54 => {
                let ra_52 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                (*ci).u.savedpc = pc;
                (*th).top.set((*ci).top);

                if let Err(e) = luaF_close(th, ra_52) {
                    return Err(e); // Requires unsized coercion.
                }

                base = (*ci).func.add(1);
                next!();
            }
            55 => {
                let ra_53 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                (*ci).u.savedpc = pc;
                (*th).top.set((*ci).top);
                luaF_newtbcupval(th, ra_53)?;
                next!();
            }
            56 => {
                pc = pc.offset(
                    ((i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32)
                            << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                            << 0 as c_int) as c_int
                        - (((1 as c_int) << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                            - 1 as c_int
                            >> 1 as c_int)
                        + 0 as c_int) as isize,
                );
                next!();
            }
            57 => {
                let ra_54 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let mut cond: c_int = 0;
                let rb_14 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                (*ci).u.savedpc = pc;
                (*th).top.set((*ci).top);
                cond = luaV_equalobj(Some(th), ra_54.cast(), rb_14.cast())?.into();
                base = (*ci).func.add(1);
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
                            - (((1 as c_int) << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                - 1 as c_int
                                >> 1 as c_int)
                            + 1 as c_int) as isize,
                    );
                }
                next!();
            }
            58 => {
                let ra_55 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let mut cond_0: c_int = 0;
                let rb_15 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                if (*ra_55).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                    && (*rb_15).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                {
                    let ia: i64 = (*ra_55).value_.i;
                    let ib_3: i64 = (*rb_15).value_.i;
                    cond_0 = (ia < ib_3) as c_int;
                } else if (*ra_55).tt_ as c_int & 0xf as c_int == 3 as c_int
                    && (*rb_15).tt_ as c_int & 0xf as c_int == 3 as c_int
                {
                    cond_0 = LTnum::<A>(ra_55.cast(), rb_15.cast());
                } else {
                    (*ci).u.savedpc = pc;
                    (*th).top.set((*ci).top);
                    cond_0 = lessthanothers(th, ra_55.cast(), rb_15.cast())?;
                    base = (*ci).func.add(1);
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
                            - (((1 as c_int) << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                - 1 as c_int
                                >> 1 as c_int)
                            + 1 as c_int) as isize,
                    );
                }
                next!();
            }
            59 => {
                let ra_56 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let mut cond_1: c_int = 0;
                let rb_16 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                if (*ra_56).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                    && (*rb_16).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                {
                    let ia_0: i64 = (*ra_56).value_.i;
                    let ib_4: i64 = (*rb_16).value_.i;
                    cond_1 = (ia_0 <= ib_4) as c_int;
                } else if (*ra_56).tt_ as c_int & 0xf as c_int == 3 as c_int
                    && (*rb_16).tt_ as c_int & 0xf as c_int == 3 as c_int
                {
                    cond_1 = LEnum::<A>(ra_56.cast(), rb_16.cast());
                } else {
                    (*ci).u.savedpc = pc;
                    (*th).top.set((*ci).top);
                    cond_1 = lessequalothers(th, ra_56.cast(), rb_16.cast())?;
                    base = (*ci).func.add(1);
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
                            - (((1 as c_int) << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                - 1 as c_int
                                >> 1 as c_int)
                            + 1 as c_int) as isize,
                    );
                }
                next!();
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
                let cond_2: c_int = luaV_equalobj(None, ra_57.cast(), rb_17)?.into();

                base = (*ci).func.add(1);

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
                            - (((1 as c_int) << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                - 1 as c_int
                                >> 1 as c_int)
                            + 1 as c_int) as isize,
                    );
                }
                next!();
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
                if (*ra_58).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                    cond_3 = ((*ra_58).value_.i == im as i64) as c_int;
                } else if (*ra_58).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                    cond_3 = ((*ra_58).value_.n == im as f64) as c_int;
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
                            - (((1 as c_int) << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                - 1 as c_int
                                >> 1 as c_int)
                            + 1 as c_int) as isize,
                    );
                }
                next!();
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

                if (*ra_59).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                    cond_4 = ((*ra_59).value_.i < im_0 as i64) as c_int;
                } else if (*ra_59).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                    let fa = (*ra_59).value_.n;
                    let fim: f64 = im_0 as f64;
                    cond_4 = (fa < fim) as c_int;
                } else {
                    let isf: c_int = (i
                        >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int;
                    (*ci).u.savedpc = pc;
                    (*th).top.set((*ci).top);
                    cond_4 = luaT_callorderiTM(th, ra_59.cast(), im_0, 0, isf, TM_LT)?;
                    base = (*ci).func.add(1);
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
                            - (((1 as c_int) << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                - 1 as c_int
                                >> 1 as c_int)
                            + 1 as c_int) as isize,
                    );
                }

                next!();
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

                if (*ra_60).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                    cond_5 = ((*ra_60).value_.i <= im_1 as i64) as c_int;
                } else if (*ra_60).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                    let fa_0 = (*ra_60).value_.n;
                    let fim_0: f64 = im_1 as f64;
                    cond_5 = (fa_0 <= fim_0) as c_int;
                } else {
                    let isf_0: c_int = (i
                        >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int;
                    (*ci).u.savedpc = pc;
                    (*th).top.set((*ci).top);
                    cond_5 = luaT_callorderiTM(th, ra_60.cast(), im_1, 0, isf_0, TM_LE)?;
                    base = (*ci).func.add(1);
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
                            - (((1 as c_int) << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                - 1 as c_int
                                >> 1 as c_int)
                            + 1 as c_int) as isize,
                    );
                }

                next!();
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

                if (*ra_61).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                    cond_6 = ((*ra_61).value_.i > im_2 as i64) as c_int;
                } else if (*ra_61).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                    let fa_1 = (*ra_61).value_.n;
                    let fim_1: f64 = im_2 as f64;
                    cond_6 = (fa_1 > fim_1) as c_int;
                } else {
                    let isf_1: c_int = (i
                        >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int;
                    (*ci).u.savedpc = pc;
                    (*th).top.set((*ci).top);
                    cond_6 = luaT_callorderiTM(th, ra_61.cast(), im_2, 1, isf_1, TM_LT)?;
                    base = (*ci).func.add(1);
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
                            - (((1 as c_int) << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                - 1 as c_int
                                >> 1 as c_int)
                            + 1 as c_int) as isize,
                    );
                }

                next!();
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

                if (*ra_62).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                    cond_7 = ((*ra_62).value_.i >= im_3 as i64) as c_int;
                } else if (*ra_62).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                    let fa_2 = (*ra_62).value_.n;
                    let fim_2: f64 = im_3 as f64;
                    cond_7 = (fa_2 >= fim_2) as c_int;
                } else {
                    let isf_2: c_int = (i
                        >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int;
                    (*ci).u.savedpc = pc;
                    (*th).top.set((*ci).top);
                    cond_7 = luaT_callorderiTM(th, ra_62.cast(), im_3, 1, isf_2, TM_LE)?;
                    base = (*ci).func.add(1);
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
                            - (((1 as c_int) << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                - 1 as c_int
                                >> 1 as c_int)
                            + 1 as c_int) as isize,
                    );
                }

                next!();
            }
            66 => {
                let ra_63 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let cond_8: c_int = !((*ra_63).tt_ as c_int
                    == 1 as c_int | (0 as c_int) << 4 as c_int
                    || (*ra_63).tt_ as c_int & 0xf as c_int == 0 as c_int)
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
                            - (((1 as c_int) << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                - 1 as c_int
                                >> 1 as c_int)
                            + 1 as c_int) as isize,
                    );
                }
                next!();
            }
            67 => {
                let ra_64 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let rb_18 = base.offset(
                    (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                if ((*rb_18).tt_ as c_int == 1 as c_int | (0 as c_int) << 4 as c_int
                    || (*rb_18).tt_ as c_int & 0xf as c_int == 0 as c_int)
                    as c_int
                    == (i >> 0 as c_int + 7 as c_int + 8 as c_int
                        & !(!(0 as c_int as u32) << 1 as c_int) << 0 as c_int)
                        as c_int
                {
                    pc = pc.offset(1);
                } else {
                    let io1_14 = ra_64;
                    let io2_14 = rb_18;

                    (*io1_14).value_ = (*io2_14).value_;
                    (*io1_14).tt_ = (*io2_14).tt_;
                    let ni_9: u32 = *pc;
                    pc = pc.offset(
                        ((ni_9 >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32)
                                << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                << 0 as c_int) as c_int
                            - (((1 as c_int) << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                - 1 as c_int
                                >> 1 as c_int)
                            + 1 as c_int) as isize,
                    );
                }
                next!();
            }
            OP_CALL => {
                let ra_65 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let b_4 = (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                    as c_int;
                let nresults = (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                    as c_int
                    - 1 as c_int;
                if b_4 != 0 as c_int {
                    (*th).top.set(ra_65.offset(b_4 as isize));
                }
                (*ci).u.savedpc = pc;

                let newci = luaD_precall(th, ra_65, nresults).await?;

                if !newci.is_null() {
                    return Ok(newci);
                }

                base = (*ci).func.add(1);

                next!();
            }
            OP_TAILCALL => {
                let ra_66 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let mut b_5: c_int = (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
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
                    (*th).top.set(ra_66.offset(b_5 as isize));
                } else {
                    b_5 = ((*th).top.get()).offset_from(ra_66) as c_int;
                }
                (*ci).u.savedpc = pc;
                if (i & (1 as c_uint) << 0 as c_int + 7 as c_int + 8 as c_int) as c_int != 0 {
                    luaF_closeupval(th, base);
                }
                n_2 = luaD_pretailcall(th, ci, ra_66, b_5, delta).await?;
                if n_2 < 0 {
                    return Ok(ci);
                }
                (*ci).func = ((*ci).func).offset(-(delta as isize));
                luaD_poscall(th, ci, n_2)?;
                base = (*ci).func.add(1);
                break;
            }
            70 => {
                let mut ra_67 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let mut n_3: c_int = (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                    as c_int
                    - 1 as c_int;
                let nparams1_0: c_int = (i
                    >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                    as c_int;
                if n_3 < 0 as c_int {
                    n_3 = ((*th).top.get()).offset_from(ra_67) as c_int;
                }
                (*ci).u.savedpc = pc;
                if (i & (1 as c_uint) << 0 as c_int + 7 as c_int + 8 as c_int) as c_int != 0 {
                    (*ci).u2.nres = n_3;
                    if (*th).top.get() < (*ci).top {
                        (*th).top.set((*ci).top);
                    }

                    if let Err(e) = luaF_close(th, base) {
                        return Err(e); // Requires unsized coercion.
                    }

                    base = ((*ci).func).offset(1 as c_int as isize);
                    ra_67 = base.offset(
                        (i >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );
                }
                if nparams1_0 != 0 {
                    (*ci).func = ((*ci).func).offset(-(((*ci).u.nextraargs + nparams1_0) as isize));
                }
                (*th).top.set(ra_67.offset(n_3 as isize));
                luaD_poscall(th, ci, n_3)?;
                base = (*ci).func.add(1);
                break;
            }
            71 => {
                if (*th).hookmask.get() != 0 {
                    let ra_68 = base.offset(
                        (i >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );
                    (*th).top.set(ra_68);
                    (*ci).u.savedpc = pc;
                    luaD_poscall(th, ci, 0 as c_int)?;
                } else {
                    let mut nres: c_int = 0;
                    (*th).ci.set((*ci).previous);
                    (*th).top.set(base.offset(-(1 as c_int as isize)));
                    nres = (*ci).nresults as c_int;
                    while (nres > 0 as c_int) as c_int != 0 as c_int {
                        let fresh5 = (*th).top.get();
                        (*th).top.add(1);
                        (*fresh5).tt_ = (0 as c_int | (0 as c_int) << 4 as c_int) as u8;
                        nres -= 1;
                    }
                }
                break;
            }
            72 => {
                if (*th).hookmask.get() != 0 {
                    let ra_69 = base.offset(
                        (i >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );
                    (*th).top.set(ra_69.offset(1 as c_int as isize));
                    (*ci).u.savedpc = pc;
                    luaD_poscall(th, ci, 1 as c_int)?;
                } else {
                    let mut nres_0: c_int = (*ci).nresults as c_int;
                    (*th).ci.set((*ci).previous);
                    if nres_0 == 0 as c_int {
                        (*th).top.set(base.offset(-(1 as c_int as isize)));
                    } else {
                        let ra_70 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                        let io1_15 = base.offset(-(1 as c_int as isize));
                        let io2_15 = ra_70;

                        (*io1_15).value_ = (*io2_15).value_;
                        (*io1_15).tt_ = (*io2_15).tt_;
                        (*th).top.set(base);
                        while (nres_0 > 1 as c_int) as c_int != 0 as c_int {
                            let fresh6 = (*th).top.get();
                            (*th).top.add(1);
                            (*fresh6).tt_ = (0 as c_int | (0 as c_int) << 4 as c_int) as u8;
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
                if (*ra_71.offset(2 as c_int as isize)).tt_ as c_int
                    == 3 as c_int | (0 as c_int) << 4 as c_int
                {
                    let count: u64 = (*ra_71.offset(1 as c_int as isize)).value_.i as u64;
                    if count > 0 as c_int as u64 {
                        let step: i64 = (*ra_71.offset(2 as c_int as isize)).value_.i;
                        let mut idx: i64 = (*ra_71).value_.i;
                        let io_43 = ra_71.offset(1 as c_int as isize);
                        (*io_43).value_.i = count.wrapping_sub(1 as c_int as u64) as i64;
                        idx = (idx as u64).wrapping_add(step as u64) as i64;
                        let io_44 = ra_71;
                        (*io_44).value_.i = idx;
                        let io_45 = ra_71.offset(3 as c_int as isize);
                        (*io_45).value_.i = idx;
                        (*io_45).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                        pc = pc.offset(
                            -((i >> 0 as c_int + 7 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int + 8 as c_int + 1 as c_int)
                                    << 0 as c_int) as c_int as isize),
                        );
                    }
                } else if floatforloop(ra_71) != 0 {
                    pc = pc.offset(
                        -((i >> 0 as c_int + 7 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int + 8 as c_int + 1 as c_int)
                                << 0 as c_int) as c_int as isize),
                    );
                }
                next!();
            }
            74 => {
                let ra_72 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                (*ci).u.savedpc = pc;
                (*th).top.set((*ci).top);
                if forprep(th, ra_72)? != 0 {
                    pc = pc.offset(
                        ((i >> 0 as c_int + 7 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int + 8 as c_int + 1 as c_int)
                                << 0 as c_int) as c_int
                            + 1 as c_int) as isize,
                    );
                }
                next!();
            }
            75 => {
                let ra_73 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                (*ci).u.savedpc = pc;
                (*th).top.set((*ci).top);
                luaF_newtbcupval(th, ra_73.offset(3 as c_int as isize))?;
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
            OP_SETLIST => {
                let ra_76 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                let mut n_4: c_int = (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                    as c_int;
                let mut last: c_uint = (i
                    >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                    & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                    as c_int as c_uint;
                let h = (*ra_76).value_.gc as *mut Table<A>;

                if n_4 == 0 as c_int {
                    n_4 = ((*th).top.get()).offset_from(ra_76) as c_int - 1 as c_int;
                } else {
                    (*th).top.set((*ci).top);
                }
                last = last.wrapping_add(n_4 as c_uint);
                if (i & (1 as c_uint) << 0 as c_int + 7 as c_int + 8 as c_int) as c_int != 0 {
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
                    let val = ra_76.offset(n_4 as isize);
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
                            (*th).hdr.global().gc.barrier_back(h.cast());
                        }
                    }
                    n_4 -= 1;
                }
                next!();
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
                (*th).top.set((*ci).top);
                pushclosure(th, p, &(*cl).upvals, base, ra_77);

                (*ci).u.savedpc = pc;
                (*th).top.set(ra_77.offset(1 as c_int as isize));
                (*th).hdr.global().gc.step();

                base = (*ci).func.add(1);

                next!();
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
                (*th).top.set((*ci).top);
                luaT_getvarargs(th, ci, ra_78, n_5)?;
                base = (*ci).func.add(1);
                next!();
            }
            81 => {
                (*ci).u.savedpc = pc;
                luaT_adjustvarargs(
                    th,
                    (i >> 0 as c_int + 7 as c_int & !(!(0 as c_int as u32) << 8 as c_int) << 0)
                        as c_int,
                    ci,
                    (*cl).p.get(),
                )?;

                base = ((*ci).func).offset(1 as c_int as isize);
                next!();
            }
            82 => {
                next!();
            }
            _ => unreachable_unchecked(), // TODO: Remove this once we converted to enum.
        }
        match current_block {
            0 => {
                (*ci).u.savedpc = pc;
                (*th).top.set((*ci).top);

                let val = luaV_finishget(th, tab, &raw const key, true)?;

                base = (*ci).func.add(1);

                let ra = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );

                (*ra).tt_ = val.tt_;
                (*ra).value_ = val.value_;

                next!();
            }
            13973394567113199817 => {
                let ra_74 = base.offset(
                    (i >> 0 as c_int + 7 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as isize,
                );
                memcpy(
                    ra_74.offset(4).cast(),
                    ra_74.cast(),
                    3usize * size_of::<StackValue<A>>(),
                );
                (*th).top.set(
                    ra_74
                        .offset(4 as c_int as isize)
                        .offset(3 as c_int as isize),
                );
                (*ci).u.savedpc = pc;

                // Invoke iterator function.
                {
                    let w = Waker::new(null(), &NON_YIELDABLE_WAKER);
                    let f = pin!(luaD_call(
                        th,
                        ra_74.offset(4),
                        (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int,
                    ));

                    match f.poll(&mut Context::from_waker(&w)) {
                        Poll::Ready(Ok(_)) => (),
                        Poll::Ready(Err(e)) => return Err(e), // Requires unsized coercion.
                        Poll::Pending => unreachable!(),
                    }
                }

                base = ((*ci).func).offset(1 as c_int as isize);

                let fresh8 = pc;
                pc = pc.offset(1);
                i = *fresh8;
            }
            _ => {}
        }
        let ra_75 = base.offset(
            (i >> 0 as c_int + 7 as c_int & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                as c_int as isize,
        );
        if !((*ra_75.offset(4 as c_int as isize)).tt_ as c_int & 0xf as c_int == 0 as c_int) {
            let io1_16 = ra_75.offset(2 as c_int as isize);
            let io2_16 = ra_75.offset(4 as c_int as isize);

            (*io1_16).value_ = (*io2_16).value_;
            (*io1_16).tt_ = (*io2_16).tt_;
            pc = pc.offset(
                -((i >> 0 as c_int + 7 as c_int + 8 as c_int
                    & !(!(0 as c_int as u32) << 8 as c_int + 8 as c_int + 1 as c_int) << 0 as c_int)
                    as c_int as isize),
            );
        }
        next!();
    }

    if (*ci).callstatus as c_int & (1 as c_int) << 2 as c_int != 0 {
        Ok(null_mut())
    } else {
        Ok((*ci).previous)
    }
}
