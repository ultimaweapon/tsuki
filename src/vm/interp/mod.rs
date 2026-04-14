use super::{
    F2Ieq, LEnum, LTnum, OP_ADD, OP_ADDI, OP_ADDK, OP_BAND, OP_BANDK, OP_BNOT, OP_BOR, OP_BORK,
    OP_BXOR, OP_BXORK, OP_CALL, OP_CLOSE, OP_CLOSURE, OP_CONCAT, OP_DIV, OP_DIVK, OP_EQ, OP_EQI,
    OP_EQK, OP_EXTRAARG, OP_FORLOOP, OP_FORPREP, OP_GEI, OP_GETFIELD, OP_GETI, OP_GETTABLE,
    OP_GETTABUP, OP_GETUPVAL, OP_GTI, OP_IDIV, OP_IDIVK, OP_JMP, OP_LE, OP_LEI, OP_LEN,
    OP_LFALSESKIP, OP_LOADF, OP_LOADFALSE, OP_LOADI, OP_LOADK, OP_LOADKX, OP_LOADNIL, OP_LOADTRUE,
    OP_LT, OP_LTI, OP_MMBIN, OP_MMBINI, OP_MMBINK, OP_MOD, OP_MODK, OP_MOVE, OP_MUL, OP_MULK,
    OP_NEWTABLE, OP_NOT, OP_POW, OP_POWK, OP_RETURN, OP_RETURN0, OP_RETURN1, OP_SELF, OP_SETFIELD,
    OP_SETI, OP_SETLIST, OP_SETTABLE, OP_SETTABUP, OP_SETUPVAL, OP_SHL, OP_SHLI, OP_SHR, OP_SHRI,
    OP_SUB, OP_SUBK, OP_TAILCALL, OP_TBC, OP_TEST, OP_TESTSET, OP_TFORCALL, OP_TFORLOOP,
    OP_TFORPREP, OP_UNM, OP_VARARG, OP_VARARGPREP, floatforloop, forprep, lessequalothers,
    lessthanothers, luaV_concat, luaV_equalobj, luaV_finishget, luaV_finishset, luaV_idiv,
    luaV_mod, luaV_modf, luaV_objlen, luaV_shiftl, luaV_tointegerns, pushclosure,
};
use crate::ldo::{
    call_fp, luaD_call, luaD_poscall, luaD_precall, luaD_pretailcall, setup_lua_ci,
    setup_tailcall_ci,
};
use crate::lfunc::{luaF_close, luaF_closeupval, luaF_newtbcupval};
use crate::lstate::CallInfo;
use crate::ltm::{
    TM_BNOT, TM_LE, TM_LT, TM_UNM, TMS, luaT_adjustvarargs, luaT_callorderiTM, luaT_getvarargs,
    luaT_trybinTM, luaT_trybinassocTM, luaT_trybiniTM,
};
use crate::value::UnsafeValue;
use crate::{
    ArithError, Float, LuaFn, NON_YIELDABLE_WAKER, Str, Table, Thread, UserData, luaH_get,
    luaH_getint, luaH_getshortstr, luaH_getstr, luaH_realasize, luaH_resize, luaH_resizearray,
};
use alloc::boxed::Box;
use core::any::Any;
use core::hint::unreachable_unchecked;
use core::pin::pin;
use core::ptr::{null, null_mut};
use core::task::{Context, Poll, Waker};

type c_int = i32;
type c_uint = u32;

pub async unsafe fn run<A>(
    th: &Thread<A>,
    mut ci: *mut CallInfo,
) -> Result<(), Box<dyn core::error::Error>> {
    'top: loop {
        let cl = &*(*th.stack.get().add((*ci).func))
            .value_
            .gc
            .cast::<LuaFn<A>>();
        let p = (*cl).p.get();
        let k = (*p).k;
        let code = core::slice::from_raw_parts((*p).code, (*p).sizecode as usize);
        let mut base = th.stack.get().add((*ci).func + 1);
        let mut tab = null_mut();
        let mut key = UnsafeValue::default();
        let mut pc = (*ci).pc;
        let mut i = code[pc];

        pc += 1;

        loop {
            let current_block: u64;

            macro_rules! next {
                () => {
                    i = code[pc];
                    pc += 1;
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
                    let b: i64 = ((i >> 15 & 0x1FFFF) as i32 - ((1 << 17) - 1 >> 1)) as i64;

                    (*ra).tt_ = 3 | 0 << 4;
                    (*ra).value_.i = b;

                    next!();
                }
                OP_LOADF => {
                    let ra = base.add((i >> 7 & 0xFF) as usize);
                    let b_0: c_int = (i >> 0 as c_int + 7 as c_int + 8 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int + 8 as c_int + 1 as c_int)
                            << 0 as c_int) as c_int
                        - (((1 as c_int) << 8 as c_int + 8 as c_int + 1 as c_int) - 1 as c_int
                            >> 1 as c_int);

                    (*ra).tt_ = 3 | 1 << 4;
                    (*ra).value_.n = (b_0 as f64).into();

                    next!();
                }
                OP_LOADK => {
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
                OP_LOADKX => {
                    let ra = base.offset((i >> 0 + 7 & !(!(0u32) << 8) << 0) as isize);
                    let rb = code[pc];
                    let rb = k.offset((rb >> 0 + 7 & !(!(0u32) << 8 + 8 + 1 + 8) << 0) as isize);

                    pc += 1;

                    (*ra).tt_ = (*rb).tt_;
                    (*ra).value_ = (*rb).value_;

                    next!();
                }
                OP_LOADFALSE => {
                    let ra_4 = base.offset(
                        (i >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );
                    (*ra_4).tt_ = (1 as c_int | (0 as c_int) << 4 as c_int) as u8;
                    next!();
                }
                OP_LFALSESKIP => {
                    let ra_5 = base.offset(
                        (i >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );
                    (*ra_5).tt_ = (1 as c_int | (0 as c_int) << 4 as c_int) as u8;
                    pc += 1;
                    next!();
                }
                OP_LOADTRUE => {
                    let ra_6 = base.offset(
                        (i >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );
                    (*ra_6).tt_ = (1 as c_int | (1 as c_int) << 4 as c_int) as u8;
                    next!();
                }
                OP_LOADNIL => {
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
                OP_GETUPVAL => {
                    let ra = base.offset(
                        (i >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );
                    let b_2: c_int = (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int;
                    let uv = cl.upvals[b_2 as usize].get();
                    let uv = (*uv).v.get();

                    (*ra).tt_ = (*uv).tt_;
                    (*ra).value_ = (*uv).value_;

                    next!();
                }
                OP_SETUPVAL => {
                    let ra = base.offset(
                        (i >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );
                    let uv = cl.upvals[(i >> 0 + 7 + 8 + 1 & !(!(0u32) << 8) << 0) as usize].get();
                    let io1_3 = (*uv).v.get();

                    (*io1_3).value_ = (*ra).value_;
                    (*io1_3).tt_ = (*ra).tt_;

                    if (*ra).tt_ as c_int & (1 as c_int) << 6 as c_int != 0 {
                        if (*uv).hdr.marked.get() as c_int & (1 as c_int) << 5 as c_int != 0
                            && (*(*ra).value_.gc).marked.get() as c_int
                                & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                                != 0
                        {
                            (*th).hdr.global().gc.barrier(uv.cast(), (*ra).value_.gc);
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
                    let uv = cl.upvals[(i >> 0 + 7 + 8 + 1 & !(!(0u32) << 8) << 0) as usize].get();
                    let k = k.offset(
                        (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );

                    tab = (*uv).v.get();

                    // Check table type.
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
                OP_GETI => {
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
                    let c: c_int = (i
                        >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
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
                OP_GETFIELD => {
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
                OP_SETTABUP => {
                    let slot_3;
                    let uv = cl.upvals
                        [(i >> 0 + 7 & !(!(0 as c_int as u32) << 8) << 0) as c_int as usize]
                        .get();
                    let upval_0 = (*uv).v.get();
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
                        (*th).top.set(th.stack.get().add((*ci).top.get()));
                        (*ci).pc = pc;

                        luaV_finishset(th, upval_0, rb_4, rc_2, slot_3)?;

                        base = th.stack.get().add((*ci).func + 1);
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
                    let val = if (i & (1 as c_uint) << 0 as c_int + 7 as c_int + 8 as c_int)
                        as c_int
                        != 0
                    {
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
                            if (*(*tab).value_.gc).marked.get() as c_int
                                & (1 as c_int) << 5 as c_int
                                != 0
                                && (*(*val).value_.gc).marked.get() as c_int
                                    & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                                    != 0
                            {
                                (*th).hdr.global().gc.barrier_back((*tab).value_.gc);
                            }
                        }
                    } else {
                        (*th).top.set(th.stack.get().add((*ci).top.get()));
                        (*ci).pc = pc;

                        luaV_finishset(th, tab.cast(), key.cast(), val, slot)?;

                        base = th.stack.get().add((*ci).func + 1);
                    }
                    next!();
                }
                OP_SETI => {
                    let ra_15 = base.offset(
                        (i >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );
                    let slot_5;
                    let c_0: c_int = (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int;
                    let rc_4 = if (i & (1 as c_uint) << 0 as c_int + 7 as c_int + 8 as c_int)
                        as c_int
                        != 0
                    {
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
                            if (*(*ra_15).value_.gc).marked.get() as c_int
                                & (1 as c_int) << 5 as c_int
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

                        (*th).top.set(th.stack.get().add((*ci).top.get()));
                        (*ci).pc = pc;

                        luaV_finishset(th, ra_15.cast(), &mut key_3, rc_4, slot_5)?;

                        base = th.stack.get().add((*ci).func + 1);
                    }
                    next!();
                }
                OP_SETFIELD => {
                    let ra_16 = base.offset(
                        (i >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );
                    let slot_6;
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
                            if (*(*ra_16).value_.gc).marked.get() as c_int
                                & (1 as c_int) << 5 as c_int
                                != 0
                                && (*(*rc_5).value_.gc).marked.get() as c_int
                                    & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
                                    != 0
                            {
                                (*th).hdr.global().gc.barrier_back((*ra_16).value_.gc);
                            }
                        }
                    } else {
                        (*th).top.set(th.stack.get().add((*ci).top.get()));
                        (*ci).pc = pc;

                        luaV_finishset(th, ra_16.cast(), rb_6, rc_5, slot_6)?;

                        base = th.stack.get().add((*ci).func + 1);
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
                        let i = code[pc];

                        c_1 += (i >> 0 + 7 & !(!(0u32) << 8 + 8 + 1 + 8) << 0) as c_int
                            * (((1 as c_int) << 8) - 1 + 1);
                    }

                    pc += 1;
                    (*th).top.set(ra_17.offset(1 as c_int as isize));

                    // Create table.
                    let t = Table::new((*th).hdr.global);
                    let io_3 = ra_17;

                    (*io_3).value_.gc = t.cast();
                    (*io_3).tt_ = 5 | 0 << 4 | 1 << 6;

                    if b_3 != 0 as c_int || c_1 != 0 as c_int {
                        luaH_resize(t, c_1 as c_uint, b_3 as c_uint);
                    }

                    (*th).top.set(ra_17.offset(1 as c_int as isize));
                    (*th).hdr.global().gc.step();

                    base = th.stack.get().add((*ci).func + 1);

                    next!();
                }
                OP_SELF => {
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

                    let k = if (i & (1 as c_uint) << 0 as c_int + 7 as c_int + 8 as c_int) != 0 {
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
                        .cast::<UnsafeValue<A>>()
                    };

                    let io1_12 = ra.offset(1 as c_int as isize);

                    (*io1_12).tt_ = (*tab).tt_;
                    (*io1_12).value_ = (*tab).value_;

                    let v = match (*tab).tt_ & 0xf {
                        5 => luaH_getstr((*tab).value_.gc.cast(), (*k).value_.gc.cast()),
                        7 => {
                            let ud = (*tab).value_.gc.cast::<UserData<A, dyn Any>>();
                            let props = (*ud).props.get();

                            match props.is_null() {
                                true => null(),
                                false => luaH_getstr(props, (*k).value_.gc.cast()),
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

                    pc += 1;
                    next!();
                }
                OP_ADDK => {
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
                        pc += 1;
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
                            pc += 1;
                            let io_7 = ra_20;

                            (*io_7).value_.n = n1 + n2;
                            (*io_7).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                        }
                    }

                    next!();
                }
                OP_SUBK => {
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
                        pc += 1;
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
                            pc += 1;
                            let io_9 = ra_21;

                            (*io_9).value_.n = n1_0 - n2_0;
                            (*io_9).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                        }
                    }

                    next!();
                }
                OP_MULK => {
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
                        pc += 1;
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
                            pc += 1;
                            let io_11 = ra_22;

                            (*io_11).value_.n = n1_1 * n2_1;
                            (*io_11).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                        }
                    }

                    next!();
                }
                OP_MODK => {
                    (*th).top.set(th.stack.get().add((*ci).top.get()));

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
                        pc += 1;
                        let io_12 = ra_23;

                        (*io_12).value_.i = match luaV_mod(i1_2, i2_2) {
                            Some(v) => v,
                            None => {
                                (*ci).pc = pc;
                                return Err(Box::new(ArithError::ModZero));
                            }
                        };
                        (*io_12).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                    } else {
                        let mut n1_2 = 0.0;
                        let mut n2_2 = 0.0;

                        if (if (*v1_3).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                            n1_2 = (*v1_3).value_.n.into();
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
                                n2_2 = (*v2_2).value_.n.into();
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
                            pc += 1;
                            let io_13 = ra_23;

                            (*io_13).value_.n = luaV_modf(n1_2, n2_2).into();
                            (*io_13).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                        }
                    }

                    next!();
                }
                OP_POWK => {
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
                        pc += 1;
                        let io_14 = ra_24;
                        (*io_14).value_.n = if n2_3 == 2 as c_int as f64 {
                            n1_3 * n1_3
                        } else {
                            n1_3.0.powf(n2_3.0).into()
                        };
                        (*io_14).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                    }

                    next!();
                }
                OP_DIVK => {
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
                        pc += 1;
                        let io_15 = ra_25;

                        (*io_15).value_.n = n1_4 / n2_4;
                        (*io_15).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                    }
                    next!();
                }
                OP_IDIVK => {
                    (*th).top.set(th.stack.get().add((*ci).top.get()));

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
                        pc += 1;
                        let io_16 = ra_26;

                        (*io_16).value_.i = match luaV_idiv(i1_3, i2_3) {
                            Some(v) => v,
                            None => {
                                (*ci).pc = pc;
                                return Err(Box::new(ArithError::DivZero));
                            }
                        };
                        (*io_16).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                    } else {
                        let mut n1_5 = 0.0;
                        let mut n2_5 = 0.0;

                        if (if (*v1_6).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                            n1_5 = (*v1_6).value_.n.into();
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
                                n2_5 = (*v2_5).value_.n.into();
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
                            pc += 1;
                            let io_17 = ra_26;
                            (*io_17).value_.n = (n1_5 / n2_5).floor().into();
                            (*io_17).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                        }
                    }
                    next!();
                }
                OP_BANDK => {
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
                        pc += 1;
                        let io_18 = ra_27;
                        (*io_18).value_.i = (i1_4 as u64 & i2_4 as u64) as i64;
                        (*io_18).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                    }

                    next!();
                }
                OP_BORK => {
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
                        pc += 1;
                        let io_19 = ra_28;

                        (*io_19).value_.i = (i1_5 as u64 | i2_5 as u64) as i64;
                        (*io_19).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                    }

                    next!();
                }
                OP_BXORK => {
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
                        pc += 1;
                        let io_20 = ra_29;
                        (*io_20).value_.i = (i1_6 as u64 ^ i2_6 as u64) as i64;
                        (*io_20).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                    }

                    next!();
                }
                OP_SHRI => {
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
                        pc += 1;
                        let io_21 = ra_30;

                        (*io_21).value_.i = luaV_shiftl(ib, -ic as i64);
                        (*io_21).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                    }

                    next!();
                }
                OP_SHLI => {
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
                        pc += 1;
                        let io_22 = ra_31;
                        (*io_22).value_.i = luaV_shiftl(ic_0 as i64, ib_0);
                        (*io_22).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                    }

                    next!();
                }
                OP_ADD => {
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
                        pc += 1;
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
                            pc += 1;
                            let io_24 = ra_32;
                            (*io_24).value_.n = n1_6 + n2_6;
                            (*io_24).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                        }
                    }

                    next!();
                }
                OP_SUB => {
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
                        pc += 1;
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
                            && (if (*v2_10).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int
                            {
                                n2_7 = (*v2_10).value_.n;
                                1 as c_int
                            } else {
                                if (*v2_10).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                                {
                                    n2_7 = ((*v2_10).value_.i as f64).into();
                                    1 as c_int
                                } else {
                                    0 as c_int
                                }
                            }) != 0
                        {
                            pc += 1;
                            let io_26 = ra_33;
                            (*io_26).value_.n = n1_7 - n2_7;
                            (*io_26).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                        }
                    }

                    next!();
                }
                OP_MUL => {
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
                        pc += 1;
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
                            && (if (*v2_11).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int
                            {
                                n2_8 = (*v2_11).value_.n;
                                1 as c_int
                            } else {
                                if (*v2_11).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                                {
                                    n2_8 = ((*v2_11).value_.i as f64).into();
                                    1 as c_int
                                } else {
                                    0 as c_int
                                }
                            }) != 0
                        {
                            pc += 1;
                            let io_28 = ra_34;
                            (*io_28).value_.n = n1_8 * n2_8;
                            (*io_28).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                        }
                    }

                    next!();
                }
                OP_MOD => {
                    (*th).top.set(th.stack.get().add((*ci).top.get()));

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
                        pc += 1;
                        let io_29 = ra_35;
                        (*io_29).value_.i = match luaV_mod(i1_10, i2_10) {
                            Some(v) => v,
                            None => {
                                (*ci).pc = pc;
                                return Err(Box::new(ArithError::ModZero));
                            }
                        };
                        (*io_29).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                    } else {
                        let mut n1_9 = 0.0;
                        let mut n2_9 = 0.0;

                        if (if (*v1_13).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                            n1_9 = (*v1_13).value_.n.into();
                            1 as c_int
                        } else {
                            if (*v1_13).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                                n1_9 = ((*v1_13).value_.i as f64).into();
                                1 as c_int
                            } else {
                                0 as c_int
                            }
                        }) != 0
                            && (if (*v2_12).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int
                            {
                                n2_9 = (*v2_12).value_.n.into();
                                1 as c_int
                            } else {
                                if (*v2_12).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                                {
                                    n2_9 = ((*v2_12).value_.i as f64).into();
                                    1 as c_int
                                } else {
                                    0 as c_int
                                }
                            }) != 0
                        {
                            pc += 1;
                            let io_30 = ra_35;
                            (*io_30).value_.n = luaV_modf(n1_9, n2_9).into();
                            (*io_30).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                        }
                    }

                    next!();
                }
                OP_POW => {
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
                        pc += 1;
                        let io_31 = ra_36;
                        (*io_31).value_.n = if n2_10 == 2 as c_int as f64 {
                            n1_10 * n1_10
                        } else {
                            n1_10.0.powf(n2_10.0).into()
                        };
                        (*io_31).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                    }

                    next!();
                }
                OP_DIV => {
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
                        pc += 1;
                        let io_32 = ra_37;
                        (*io_32).value_.n = n1_11 / n2_11;
                        (*io_32).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                    }

                    next!();
                }
                OP_IDIV => {
                    (*th).top.set(th.stack.get().add((*ci).top.get()));

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
                        pc += 1;
                        let io_33 = ra_38;
                        (*io_33).value_.i = match luaV_idiv(i1_11, i2_11) {
                            Some(v) => v,
                            None => {
                                (*ci).pc = pc;
                                return Err(Box::new(ArithError::DivZero));
                            }
                        };
                        (*io_33).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                    } else {
                        let mut n1_12 = 0.0;
                        let mut n2_12 = 0.0;

                        if (if (*v1_16).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int {
                            n1_12 = (*v1_16).value_.n.into();
                            1 as c_int
                        } else {
                            if (*v1_16).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int {
                                n1_12 = ((*v1_16).value_.i as f64).into();
                                1 as c_int
                            } else {
                                0 as c_int
                            }
                        }) != 0
                            && (if (*v2_15).tt_ as c_int == 3 as c_int | (1 as c_int) << 4 as c_int
                            {
                                n2_12 = (*v2_15).value_.n.into();
                                1 as c_int
                            } else {
                                if (*v2_15).tt_ as c_int == 3 as c_int | (0 as c_int) << 4 as c_int
                                {
                                    n2_12 = ((*v2_15).value_.i as f64).into();
                                    1 as c_int
                                } else {
                                    0 as c_int
                                }
                            }) != 0
                        {
                            pc += 1;
                            let io_34 = ra_38;
                            (*io_34).value_.n = (n1_12 / n2_12).floor().into();
                            (*io_34).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
                        }
                    }

                    next!();
                }
                OP_BAND => {
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
                        pc += 1;
                        let io_35 = ra_39;
                        (*io_35).value_.i = (i1_12 as u64 & i2_12 as u64) as i64;
                        (*io_35).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                    }

                    next!();
                }
                OP_BOR => {
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
                        pc += 1;
                        let io_36 = ra_40;
                        (*io_36).value_.i = (i1_13 as u64 | i2_13 as u64) as i64;
                        (*io_36).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                    }

                    next!();
                }
                OP_BXOR => {
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
                        pc += 1;
                        let io_37 = ra_41;
                        (*io_37).value_.i = (i1_14 as u64 ^ i2_14 as u64) as i64;
                        (*io_37).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                    }

                    next!();
                }
                OP_SHR => {
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
                        pc += 1;
                        let io_38 = ra_42;
                        (*io_38).value_.i = luaV_shiftl(
                            i1_15,
                            (0 as c_int as u64).wrapping_sub(i2_15 as u64) as i64,
                        );
                        (*io_38).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                    }

                    next!();
                }
                OP_SHL => {
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
                        pc += 1;
                        let io_39 = ra_43;
                        (*io_39).value_.i = luaV_shiftl(i1_16, i2_16);
                        (*io_39).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
                    }

                    next!();
                }
                OP_MMBIN => {
                    let ra = base.offset(
                        (i >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );
                    let pi = code[pc - 2];
                    let rb = base.offset(
                        (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );
                    let tm: TMS = (i
                        >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int as TMS;

                    (*th).top.set(th.stack.get().add((*ci).top.get()));
                    (*ci).pc = pc;

                    let val = luaT_trybinTM(th, ra.cast(), rb.cast(), tm)?;

                    base = th.stack.get().add((*ci).func + 1);

                    let result = base.offset(
                        (pi >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );

                    (*result).tt_ = val.tt_;
                    (*result).value_ = val.value_;

                    next!();
                }
                OP_MMBINI => {
                    let ra = base.offset(
                        (i >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );
                    let pi = code[pc - 2];
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

                    (*th).top.set(th.stack.get().add((*ci).top.get()));
                    (*ci).pc = pc;

                    let val = luaT_trybiniTM(th, ra.cast(), imm_0 as i64, flip, tm_0)?;

                    base = th.stack.get().add((*ci).func + 1);

                    let result = base.offset(
                        (pi >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );

                    (*result).tt_ = val.tt_;
                    (*result).value_ = val.value_;

                    next!();
                }
                OP_MMBINK => {
                    let ra = base.offset(
                        (i >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );
                    let pi = code[pc - 2];
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

                    (*th).top.set(th.stack.get().add((*ci).top.get()));
                    (*ci).pc = pc;

                    let val = luaT_trybinassocTM(th, ra.cast(), imm_1, flip_0, tm_1)?;

                    base = th.stack.get().add((*ci).func + 1);

                    let result = base.offset(
                        (pi >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );

                    (*result).tt_ = val.tt_;
                    (*result).value_ = val.value_;

                    next!();
                }
                OP_UNM => {
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
                        (*th).top.set(th.stack.get().add((*ci).top.get()));
                        (*ci).pc = pc;

                        let val = luaT_trybinTM(th, rb_11.cast(), rb_11.cast(), TM_UNM)?;

                        base = th.stack.get().add((*ci).func + 1);
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
                OP_BNOT => {
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
                        (*th).top.set(th.stack.get().add((*ci).top.get()));
                        (*ci).pc = pc;

                        let val = luaT_trybinTM(th, rb_12.cast(), rb_12.cast(), TM_BNOT)?;

                        base = th.stack.get().add((*ci).func + 1);
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
                OP_NOT => {
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
                    (*th).top.set(th.stack.get().add((*ci).top.get()));
                    (*ci).pc = pc;

                    let val = luaV_objlen(
                        th,
                        base.offset(
                            (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        )
                        .cast(),
                    )?;

                    base = th.stack.get().add((*ci).func + 1);

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
                    (*ci).pc = pc;

                    luaV_concat(th, n_1)?;

                    (*th).hdr.global().gc.step();

                    base = th.stack.get().add((*ci).func + 1);

                    next!();
                }
                OP_CLOSE => {
                    let ra_52 = base.offset(
                        (i >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );

                    (*th).top.set(th.stack.get().add((*ci).top.get()));
                    (*ci).pc = pc;

                    if let Err(e) = luaF_close(th, ra_52) {
                        return Err(e); // Requires unsized coercion.
                    }

                    base = th.stack.get().add((*ci).func + 1);
                    next!();
                }
                OP_TBC => {
                    let ra_53 = base.offset(
                        (i >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );

                    (*th).top.set(th.stack.get().add((*ci).top.get()));
                    (*ci).pc = pc;

                    luaF_newtbcupval(th, ra_53)?;

                    next!();
                }
                OP_JMP => {
                    pc = pc.wrapping_add_signed(
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
                OP_EQ => {
                    let ra_54 = base.offset(
                        (i >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );
                    let cond: c_int;
                    let rb_14 = base.offset(
                        (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );

                    (*th).top.set(th.stack.get().add((*ci).top.get()));
                    (*ci).pc = pc;

                    cond = luaV_equalobj(Some(th), ra_54.cast(), rb_14.cast())?.into();
                    base = th.stack.get().add((*ci).func + 1);

                    if cond
                        != (i >> 0 as c_int + 7 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 1 as c_int) << 0 as c_int)
                            as c_int
                    {
                        pc += 1;
                    } else {
                        let ni = code[pc];

                        pc = pc.wrapping_add_signed(
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
                    }
                    next!();
                }
                OP_LT => {
                    let ra_55 = base.offset(
                        (i >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );
                    let cond_0: c_int;
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
                        (*th).top.set(th.stack.get().add((*ci).top.get()));
                        (*ci).pc = pc;

                        cond_0 = lessthanothers(th, ra_55.cast(), rb_15.cast())?;
                        base = th.stack.get().add((*ci).func + 1);
                    }
                    if cond_0
                        != (i >> 0 as c_int + 7 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 1 as c_int) << 0 as c_int)
                            as c_int
                    {
                        pc += 1;
                    } else {
                        let ni = code[pc];

                        pc = pc.wrapping_add_signed(
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
                    }
                    next!();
                }
                OP_LE => {
                    let ra_56 = base.offset(
                        (i >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );
                    let cond_1: c_int;
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
                        (*th).top.set(th.stack.get().add((*ci).top.get()));
                        (*ci).pc = pc;

                        cond_1 = lessequalothers(th, ra_56.cast(), rb_16.cast())?;
                        base = th.stack.get().add((*ci).func + 1);
                    }
                    if cond_1
                        != (i >> 0 as c_int + 7 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 1 as c_int) << 0 as c_int)
                            as c_int
                    {
                        pc += 1;
                    } else {
                        let ni = code[pc];

                        pc = pc.wrapping_add_signed(
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
                    }
                    next!();
                }
                OP_EQK => {
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

                    base = th.stack.get().add((*ci).func + 1);

                    if cond_2
                        != (i >> 0 as c_int + 7 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 1 as c_int) << 0 as c_int)
                            as c_int
                    {
                        pc += 1;
                    } else {
                        let ni = code[pc];

                        pc = pc.wrapping_add_signed(
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
                    }
                    next!();
                }
                OP_EQI => {
                    let ra_58 = base.offset(
                        (i >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );
                    let cond_3: c_int;
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
                        pc += 1;
                    } else {
                        let ni = code[pc];

                        pc = pc.wrapping_add_signed(
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
                    }
                    next!();
                }
                OP_LTI => {
                    let ra_59 = base.offset(
                        (i >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );
                    let cond_4: c_int;
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

                        (*th).top.set(th.stack.get().add((*ci).top.get()));
                        (*ci).pc = pc;

                        cond_4 = luaT_callorderiTM(th, ra_59.cast(), im_0, 0, isf, TM_LT)?;
                        base = th.stack.get().add((*ci).func + 1);
                    }

                    if cond_4
                        != (i >> 0 as c_int + 7 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 1 as c_int) << 0 as c_int)
                            as c_int
                    {
                        pc += 1;
                    } else {
                        let ni = code[pc];

                        pc = pc.wrapping_add_signed(
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
                    }

                    next!();
                }
                OP_LEI => {
                    let ra_60 = base.offset(
                        (i >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );
                    let cond_5: c_int;
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

                        (*th).top.set(th.stack.get().add((*ci).top.get()));
                        (*ci).pc = pc;

                        cond_5 = luaT_callorderiTM(th, ra_60.cast(), im_1, 0, isf_0, TM_LE)?;
                        base = th.stack.get().add((*ci).func + 1);
                    }

                    if cond_5
                        != (i >> 0 as c_int + 7 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 1 as c_int) << 0 as c_int)
                            as c_int
                    {
                        pc += 1;
                    } else {
                        let ni = code[pc];

                        pc = pc.wrapping_add_signed(
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
                    }

                    next!();
                }
                OP_GTI => {
                    let ra_61 = base.offset(
                        (i >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );
                    let cond_6: c_int;
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

                        (*th).top.set(th.stack.get().add((*ci).top.get()));
                        (*ci).pc = pc;

                        cond_6 = luaT_callorderiTM(th, ra_61.cast(), im_2, 1, isf_1, TM_LT)?;
                        base = th.stack.get().add((*ci).func + 1);
                    }

                    if cond_6
                        != (i >> 0 as c_int + 7 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 1 as c_int) << 0 as c_int)
                            as c_int
                    {
                        pc += 1;
                    } else {
                        let ni = code[pc];

                        pc = pc.wrapping_add_signed(
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
                    }

                    next!();
                }
                OP_GEI => {
                    let ra_62 = base.offset(
                        (i >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );
                    let cond_7: c_int;
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

                        (*th).top.set(th.stack.get().add((*ci).top.get()));
                        (*ci).pc = pc;

                        cond_7 = luaT_callorderiTM(th, ra_62.cast(), im_3, 1, isf_2, TM_LE)?;
                        base = th.stack.get().add((*ci).func + 1);
                    }

                    if cond_7
                        != (i >> 0 as c_int + 7 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 1 as c_int) << 0 as c_int)
                            as c_int
                    {
                        pc += 1;
                    } else {
                        let ni = code[pc];

                        pc = pc.wrapping_add_signed(
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
                    }

                    next!();
                }
                OP_TEST => {
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
                        pc += 1;
                    } else {
                        let ni = code[pc];

                        pc = pc.wrapping_add_signed(
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
                    }
                    next!();
                }
                OP_TESTSET => {
                    let ra = base.offset(
                        (i >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );
                    let rb = base.offset(
                        (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );
                    if ((*rb).tt_ as c_int == 1 as c_int | (0 as c_int) << 4 as c_int
                        || (*rb).tt_ as c_int & 0xf as c_int == 0 as c_int)
                        as c_int
                        == (i >> 0 as c_int + 7 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 1 as c_int) << 0 as c_int)
                            as c_int
                    {
                        pc += 1;
                    } else {
                        let ni = code[pc];

                        (*ra).tt_ = (*rb).tt_;
                        (*ra).value_ = (*rb).value_;

                        pc = pc.wrapping_add_signed(
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
                    }

                    next!();
                }
                OP_CALL => {
                    let ra = base.offset(
                        (i >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );
                    let args = (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int;
                    let nresults = (i
                        >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int
                        - 1 as c_int;

                    if args != 0 {
                        (*th).top.set(ra.offset(args as isize));
                    }

                    (*ci).pc = pc;

                    // Fast path for majority of the cases.
                    match (*ra).tt_ & 0x3f {
                        0x02 => match call_fp(th, ra, nresults, (*ra).value_.f).await {
                            Ok(_) => (),
                            Err(e) => return Err(e),
                        },
                        0x06 => match setup_lua_ci(th, ra, nresults) {
                            Ok(v) => {
                                ci = v;
                                continue 'top;
                            }
                            Err(e) => return Err(Box::new(e)),
                        },
                        _ => {
                            let newci = match luaD_precall(th, ra, nresults).await {
                                Ok(v) => v,
                                Err(e) => return Err(e),
                            };

                            if !newci.is_null() {
                                ci = newci;
                                continue 'top;
                            }
                        }
                    }

                    base = th.stack.get().add((*ci).func + 1);

                    next!();
                }
                OP_TAILCALL => {
                    let ra = base.offset(
                        (i >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );
                    let mut b_5: c_int = (i >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int;
                    let nparams1: c_int = (i
                        >> 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                        as c_int;
                    let delta: c_int = if nparams1 != 0 {
                        (*ci).nextraargs + nparams1
                    } else {
                        0 as c_int
                    };

                    if b_5 != 0 as c_int {
                        (*th).top.set(ra.offset(b_5 as isize));
                    } else {
                        b_5 = ((*th).top.get()).offset_from(ra) as c_int;
                    }

                    if (i & (1 as c_uint) << 0 as c_int + 7 as c_int + 8 as c_int) as c_int != 0 {
                        luaF_closeupval(th, base);
                    }

                    (*ci).pc = pc;

                    // Fast path for majority of the cases.
                    let n_2 = match (*ra).tt_ & 0x3f {
                        0x02 => match call_fp(th, ra, -1, (*ra).value_.f).await {
                            Ok(v) => v,
                            Err(e) => return Err(e),
                        },
                        0x06 => match setup_tailcall_ci(th, ci, ra, b_5, delta) {
                            Ok(_) => continue 'top,
                            Err(e) => return Err(Box::new(e)),
                        },
                        _ => match luaD_pretailcall(th, ci, ra, b_5, delta).await {
                            Ok(v) => v,
                            Err(e) => return Err(e),
                        },
                    };

                    if n_2 < 0 {
                        continue 'top;
                    }

                    (*ci).func = ((*ci).func).strict_sub_signed(delta as isize);
                    luaD_poscall(th, ci, n_2)?;
                    base = th.stack.get().add((*ci).func + 1);
                    break;
                }
                OP_RETURN => {
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

                    (*ci).pc = pc;

                    if (i & (1 as c_uint) << 0 as c_int + 7 as c_int + 8 as c_int) as c_int != 0 {
                        let top = th.stack.get().add((*ci).top.get());

                        (*ci).u2.nres = n_3;

                        if (*th).top.get() < top {
                            (*th).top.set(top);
                        }

                        if let Err(e) = luaF_close(th, base) {
                            return Err(e); // Requires unsized coercion.
                        }

                        base = th.stack.get().add((*ci).func + 1);
                        ra_67 = base.offset(
                            (i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                                as c_int as isize,
                        );
                    }

                    if nparams1_0 != 0 {
                        (*ci).func = (*ci)
                            .func
                            .strict_sub_signed(((*ci).nextraargs + nparams1_0) as isize);
                    }

                    (*th).top.set(ra_67.offset(n_3 as isize));
                    luaD_poscall(th, ci, n_3)?;
                    base = th.stack.get().add((*ci).func + 1);
                    break;
                }
                OP_RETURN0 => {
                    let mut nres: c_int;
                    (*th).ci.set((*ci).previous);
                    (*th).top.set(base.offset(-(1 as c_int as isize)));
                    nres = (*ci).nresults as c_int;
                    while (nres > 0 as c_int) as c_int != 0 as c_int {
                        let fresh5 = (*th).top.get();
                        (*th).top.add(1);
                        (*fresh5).tt_ = (0 as c_int | (0 as c_int) << 4 as c_int) as u8;
                        nres -= 1;
                    }

                    break;
                }
                OP_RETURN1 => {
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

                    break;
                }
                OP_FORLOOP => {
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

                            pc = pc.wrapping_add_signed(
                                -((i >> 0 as c_int + 7 as c_int + 8 as c_int
                                    & !(!(0 as c_int as u32)
                                        << 8 as c_int + 8 as c_int + 1 as c_int)
                                        << 0 as c_int) as c_int
                                    as isize),
                            );
                        }
                    } else if floatforloop(ra_71) != 0 {
                        pc = pc.wrapping_add_signed(
                            -((i >> 0 as c_int + 7 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int + 8 as c_int + 1 as c_int)
                                    << 0 as c_int) as c_int as isize),
                        );
                    }
                    next!();
                }
                OP_FORPREP => {
                    let ra_72 = base.offset(
                        (i >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );

                    (*th).top.set(th.stack.get().add((*ci).top.get()));
                    (*ci).pc = pc;

                    if forprep(th, ra_72)? {
                        pc = pc.wrapping_add_signed(
                            ((i >> 0 as c_int + 7 as c_int + 8 as c_int
                                & !(!(0 as c_int as u32) << 8 as c_int + 8 as c_int + 1 as c_int)
                                    << 0 as c_int) as c_int
                                + 1 as c_int) as isize,
                        );
                    }
                    next!();
                }
                OP_TFORPREP => {
                    let ra_73 = base.offset(
                        (i >> 0 as c_int + 7 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int) << 0 as c_int)
                            as c_int as isize,
                    );

                    (*th).top.set(th.stack.get().add((*ci).top.get()));
                    (*ci).pc = pc;

                    luaF_newtbcupval(th, ra_73.offset(3 as c_int as isize))?;
                    pc = pc.wrapping_add_signed(
                        (i >> 0 as c_int + 7 as c_int + 8 as c_int
                            & !(!(0 as c_int as u32) << 8 as c_int + 8 as c_int + 1 as c_int)
                                << 0 as c_int) as c_int as isize,
                    );

                    i = code[pc];
                    pc += 1;

                    current_block = 13973394567113199817;
                }
                OP_TFORCALL => {
                    current_block = 13973394567113199817;
                }
                OP_TFORLOOP => {
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
                        (*th).top.set(th.stack.get().add((*ci).top.get()));
                    }
                    last = last.wrapping_add(n_4 as c_uint);
                    if (i & (1 as c_uint) << 0 as c_int + 7 as c_int + 8 as c_int) as c_int != 0 {
                        let i = code[pc];

                        pc += 1;

                        last = last.wrapping_add(
                            ((i >> 0 as c_int + 7 as c_int
                                & !(!(0 as c_int as u32)
                                    << 8 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
                                    << 0 as c_int) as c_int
                                * (((1 as c_int) << 8 as c_int) - 1 as c_int + 1 as c_int))
                                as c_uint,
                        );
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

                    (*th).top.set(th.stack.get().add((*ci).top.get()));

                    pushclosure(th, p, &cl.upvals, base, ra_77);

                    (*th).top.set(ra_77.offset(1 as c_int as isize));
                    (*th).hdr.global().gc.step();

                    base = th.stack.get().add((*ci).func + 1);

                    next!();
                }
                OP_VARARG => {
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

                    (*th).top.set(th.stack.get().add((*ci).top.get()));
                    (*ci).pc = pc;

                    luaT_getvarargs(th, ci, ra_78, n_5)?;
                    base = th.stack.get().add((*ci).func + 1);
                    next!();
                }
                OP_VARARGPREP => {
                    (*ci).pc = pc;

                    luaT_adjustvarargs(
                        th,
                        (i >> 0 as c_int + 7 as c_int & !(!(0 as c_int as u32) << 8 as c_int) << 0)
                            as c_int,
                        ci,
                        (*cl).p.get(),
                    )?;

                    base = th.stack.get().add((*ci).func + 1);
                    next!();
                }
                OP_EXTRAARG => {
                    next!();
                }
                _ => unreachable_unchecked(), // TODO: Remove this once we converted to enum.
            }
            match current_block {
                0 => {
                    (*th).top.set(th.stack.get().add((*ci).top.get()));
                    (*ci).pc = pc;

                    let val = luaV_finishget(th, tab, &raw const key, true)?;

                    base = th.stack.get().add((*ci).func + 1);

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

                    ra_74.copy_to_nonoverlapping(ra_74.add(4), 3);

                    (*th).top.set(
                        ra_74
                            .offset(4 as c_int as isize)
                            .offset(3 as c_int as isize),
                    );
                    (*ci).pc = pc;

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

                    base = th.stack.get().add((*ci).func + 1);
                    i = code[pc];
                    pc += 1;
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

                pc = pc.wrapping_add_signed(
                    -((i >> 0 as c_int + 7 as c_int + 8 as c_int
                        & !(!(0 as c_int as u32) << 8 as c_int + 8 as c_int + 1 as c_int)
                            << 0 as c_int) as c_int as isize),
                );
            }
            next!();
        }

        if (*ci).callstatus as c_int & (1 as c_int) << 2 as c_int != 0 {
            return Ok(());
        }

        ci = (*ci).previous;
    }
}
