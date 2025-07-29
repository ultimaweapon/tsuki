#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments
)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::gc::Object;
use crate::ldebug::{luaG_concaterror, luaG_opinterror, luaG_ordererror, luaG_tointerror};
use crate::ldo::{luaD_call, luaD_growstack};
use crate::lobject::{Proto, StackValue, StkId, Udata};
use crate::lstate::CallInfo;
use crate::table::luaH_getshortstr;
use crate::value::{UnsafeValue, UntaggedValue};
use crate::{CallError, Lua, NON_YIELDABLE_WAKER, Str, Table, Thread};
use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::string::String;
use core::pin::pin;
use core::ptr::null;
use core::task::{Context, Poll, Waker};

pub type TMS = libc::c_uint;

pub const TM_N: TMS = 25;
pub const TM_CLOSE: TMS = 24;
pub const TM_CALL: TMS = 23;
pub const TM_CONCAT: TMS = 22;
pub const TM_LE: TMS = 21;
pub const TM_LT: TMS = 20;
pub const TM_BNOT: TMS = 19;
pub const TM_UNM: TMS = 18;
pub const TM_SHR: TMS = 17;
pub const TM_SHL: TMS = 16;
pub const TM_BXOR: TMS = 15;
pub const TM_BOR: TMS = 14;
pub const TM_BAND: TMS = 13;
pub const TM_IDIV: TMS = 12;
pub const TM_DIV: TMS = 11;
pub const TM_POW: TMS = 10;
pub const TM_MOD: TMS = 9;
pub const TM_MUL: TMS = 8;
pub const TM_SUB: TMS = 7;
pub const TM_ADD: TMS = 6;
pub const TM_EQ: TMS = 5;
pub const TM_LEN: TMS = 4;
pub const TM_MODE: TMS = 3;
pub const TM_GC: TMS = 2;
pub const TM_NEWINDEX: TMS = 1;
pub const TM_INDEX: TMS = 0;
pub const luaT_typenames_: [&str; 12] = [
    "no value", "nil", "boolean", "function", "number", "string", "table", "function", "userdata",
    "thread", "upvalue", "proto",
];

pub unsafe fn luaT_gettm(events: *const Table, event: TMS) -> *const UnsafeValue {
    let ename = (*events)
        .hdr
        .global()
        .events()
        .get_raw_int_key(event.into());
    let tm: *const UnsafeValue = luaH_getshortstr(events, ename.value_.gc.cast());

    if (*tm).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int {
        (*events)
            .flags
            .set(((*events).flags.get() as libc::c_int | (1 << event) as u8 as libc::c_int) as u8);
        return 0 as *const UnsafeValue;
    } else {
        return tm;
    }
}

pub unsafe fn luaT_gettmbyobj(
    L: *const Thread,
    o: *const UnsafeValue,
    event: TMS,
) -> *const UnsafeValue {
    let mt = (*L).hdr.global().metatable(o);

    if !mt.is_null() {
        luaH_getshortstr(
            mt,
            (*L).hdr
                .global()
                .events()
                .get_raw_int_key(event.into())
                .value_
                .gc
                .cast(),
        )
    } else {
        (*(*L).hdr.global).nilvalue.get()
    }
}

pub unsafe fn luaT_objtypename(g: *const Lua, o: *const UnsafeValue) -> Cow<'static, str> {
    let mut mt: *const Table;

    if (*o).tt_ as libc::c_int
        == 5 as libc::c_int
            | (0 as libc::c_int) << 4 as libc::c_int
            | (1 as libc::c_int) << 6 as libc::c_int
        && {
            mt = (*((*o).value_.gc as *mut Table)).metatable.get();
            !mt.is_null()
        }
        || (*o).tt_ as libc::c_int
            == 7 as libc::c_int
                | (0 as libc::c_int) << 4 as libc::c_int
                | (1 as libc::c_int) << 6 as libc::c_int
            && {
                mt = (*((*o).value_.gc as *mut Udata)).metatable;
                !mt.is_null()
            }
    {
        let name: *const UnsafeValue = luaH_getshortstr(mt, Str::from_str(g, "__name"));

        if (*name).tt_ as libc::c_int & 0xf as libc::c_int == 4 as libc::c_int {
            return String::from_utf8_lossy((*((*name).value_.gc.cast::<Str>())).as_bytes());
        }
    }

    luaT_typenames_[(((*o).tt_ as libc::c_int & 0xf) + 1 as libc::c_int) as usize].into()
}

pub unsafe fn luaT_callTM(
    L: *const Thread,
    f: *const UnsafeValue,
    p1: *const UnsafeValue,
    p2: *const UnsafeValue,
    p3: *const UnsafeValue,
) -> Result<(), Box<CallError>> {
    let func: StkId = (*L).top.get();
    let io1: *mut UnsafeValue = &mut (*func).val;
    let io2: *const UnsafeValue = f;
    (*io1).value_ = (*io2).value_;
    (*io1).tt_ = (*io2).tt_;
    let io1_0: *mut UnsafeValue = &mut (*func.offset(1 as libc::c_int as isize)).val;
    let io2_0: *const UnsafeValue = p1;
    (*io1_0).value_ = (*io2_0).value_;
    (*io1_0).tt_ = (*io2_0).tt_;
    let io1_1: *mut UnsafeValue = &mut (*func.offset(2 as libc::c_int as isize)).val;
    let io2_1: *const UnsafeValue = p2;
    (*io1_1).value_ = (*io2_1).value_;
    (*io1_1).tt_ = (*io2_1).tt_;
    let io1_2: *mut UnsafeValue = &mut (*func.offset(3 as libc::c_int as isize)).val;
    let io2_2: *const UnsafeValue = p3;
    (*io1_2).value_ = (*io2_2).value_;
    (*io1_2).tt_ = (*io2_2).tt_;
    (*L).top.set(func.offset(4 as libc::c_int as isize));

    // Invoke.
    let f = pin!(luaD_call(L, func, 0));
    let w = Waker::new(null(), &NON_YIELDABLE_WAKER);

    match f.poll(&mut Context::from_waker(&w)) {
        Poll::Ready(v) => v,
        Poll::Pending => unreachable!(),
    }
}

/// The result will be on the top of stack.
pub unsafe fn luaT_callTMres(
    L: *const Thread,
    f: *const UnsafeValue,
    p1: *const UnsafeValue,
    p2: *const UnsafeValue,
) -> Result<(), Box<CallError>> {
    let func: StkId = (*L).top.get();
    let io1: *mut UnsafeValue = &raw mut (*func).val;
    let io2: *const UnsafeValue = f;
    (*io1).value_ = (*io2).value_;
    (*io1).tt_ = (*io2).tt_;
    let io1_0: *mut UnsafeValue = &raw mut (*func.offset(1 as libc::c_int as isize)).val;
    let io2_0: *const UnsafeValue = p1;
    (*io1_0).value_ = (*io2_0).value_;
    (*io1_0).tt_ = (*io2_0).tt_;
    let io1_1: *mut UnsafeValue = &mut (*func.offset(2 as libc::c_int as isize)).val;
    let io2_1: *const UnsafeValue = p2;
    (*io1_1).value_ = (*io2_1).value_;
    (*io1_1).tt_ = (*io2_1).tt_;
    (*L).top.add(3);

    // Invoke.
    let f = pin!(luaD_call(L, func, 1));
    let w = Waker::new(null(), &NON_YIELDABLE_WAKER);

    match f.poll(&mut Context::from_waker(&w)) {
        Poll::Ready(v) => v,
        Poll::Pending => unreachable!(),
    }
}

/// The result will be on the top of stack.
unsafe fn callbinTM(
    L: *const Thread,
    p1: *const UnsafeValue,
    p2: *const UnsafeValue,
    event: TMS,
) -> Result<libc::c_int, Box<dyn core::error::Error>> {
    let mut tm: *const UnsafeValue = luaT_gettmbyobj(L, p1, event);
    if (*tm).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int {
        tm = luaT_gettmbyobj(L, p2, event);
    }
    if (*tm).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int {
        return Ok(0 as libc::c_int);
    }

    match luaT_callTMres(L, tm, p1, p2) {
        Ok(_) => Ok(1),
        Err(e) => Err(e), // Requires unsized coercion.
    }
}

pub unsafe fn luaT_trybinTM(
    L: *const Thread,
    p1: *const UnsafeValue,
    p2: *const UnsafeValue,
    event: TMS,
) -> Result<UnsafeValue, Box<dyn core::error::Error>> {
    if callbinTM(L, p1, p2, event)? == 0 {
        match event as libc::c_uint {
            TM_BAND | TM_BOR | TM_BXOR | 16 | 17 | 19 => {
                if (*p1).tt_ & 0xf == 3 && (*p2).tt_ & 0xf == 3 {
                    luaG_tointerror(L, p1, p2)?;
                } else {
                    return Err(luaG_opinterror(L, p1, p2, "perform bitwise operation on"));
                }
            }
            _ => return Err(luaG_opinterror(L, p1, p2, "perform arithmetic on")),
        }
    }

    (*L).top.sub(1);

    Ok((*L).top.read(0))
}

pub unsafe fn luaT_tryconcatTM(L: *const Thread) -> Result<(), Box<dyn core::error::Error>> {
    let top: StkId = (*L).top.get();

    if callbinTM(
        L,
        &raw const (*top.offset(-(2 as libc::c_int as isize))).val,
        &raw const (*top.offset(-(1 as libc::c_int as isize))).val,
        TM_CONCAT,
    )? == 0
    {
        return Err(luaG_concaterror(
            L,
            &raw const (*top.offset(-(2 as libc::c_int as isize))).val,
            &raw const (*top.offset(-(1 as libc::c_int as isize))).val,
        ));
    }

    (*L).top.sub(1);

    // Move result.
    let val = (*L).top.read(0);

    (*L).top.get().sub(2).write(StackValue { val });

    Ok(())
}

pub unsafe fn luaT_trybinassocTM(
    L: *const Thread,
    p1: *const UnsafeValue,
    p2: *const UnsafeValue,
    flip: libc::c_int,
    event: TMS,
) -> Result<UnsafeValue, Box<dyn core::error::Error>> {
    if flip != 0 {
        luaT_trybinTM(L, p2, p1, event)
    } else {
        luaT_trybinTM(L, p1, p2, event)
    }
}

pub unsafe fn luaT_trybiniTM(
    L: *const Thread,
    p1: *const UnsafeValue,
    i2: i64,
    flip: libc::c_int,
    event: TMS,
) -> Result<UnsafeValue, Box<dyn core::error::Error>> {
    let mut aux: UnsafeValue = UnsafeValue {
        value_: UntaggedValue {
            gc: 0 as *mut Object,
        },
        tt_: 0,
    };
    let io: *mut UnsafeValue = &mut aux;
    (*io).value_.i = i2;
    (*io).tt_ = (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    luaT_trybinassocTM(L, p1, &mut aux, flip, event)
}

pub unsafe fn luaT_callorderTM(
    L: *const Thread,
    p1: *const UnsafeValue,
    p2: *const UnsafeValue,
    event: TMS,
) -> Result<libc::c_int, Box<dyn core::error::Error>> {
    if callbinTM(L, p1, p2, event)? != 0 {
        (*L).top.sub(1);

        return Ok(!((*(*L).top.get()).val.tt_ as libc::c_int
            == 1 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
            || (*(*L).top.get()).val.tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int)
            as libc::c_int);
    }
    luaG_ordererror(L, p1, p2)?;
    unreachable!("luaG_ordererror always return Err");
}

pub unsafe fn luaT_callorderiTM(
    L: *const Thread,
    mut p1: *const UnsafeValue,
    v2: libc::c_int,
    flip: libc::c_int,
    isfloat: libc::c_int,
    event: TMS,
) -> Result<libc::c_int, Box<dyn core::error::Error>> {
    let mut aux: UnsafeValue = UnsafeValue {
        value_: UntaggedValue {
            gc: 0 as *mut Object,
        },
        tt_: 0,
    };
    let mut p2: *const UnsafeValue = 0 as *const UnsafeValue;
    if isfloat != 0 {
        let io: *mut UnsafeValue = &mut aux;
        (*io).value_.n = v2 as f64;
        (*io).tt_ = (3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
    } else {
        let io_0: *mut UnsafeValue = &mut aux;
        (*io_0).value_.i = v2 as i64;
        (*io_0).tt_ = (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    }
    if flip != 0 {
        p2 = p1;
        p1 = &mut aux;
    } else {
        p2 = &mut aux;
    }
    return luaT_callorderTM(L, p1, p2, event);
}

pub unsafe fn luaT_adjustvarargs(
    L: *const Thread,
    nfixparams: libc::c_int,
    ci: *mut CallInfo,
    p: *const Proto,
) -> Result<(), Box<dyn core::error::Error>> {
    let mut i: libc::c_int = 0;
    let actual: libc::c_int =
        ((*L).top.get()).offset_from((*ci).func) as libc::c_long as libc::c_int - 1 as libc::c_int;
    let nextra: libc::c_int = actual - nfixparams;
    (*ci).u.nextraargs = nextra;

    if ((((*L).stack_last.get()).offset_from((*L).top.get()) as libc::c_long
        <= ((*p).maxstacksize as libc::c_int + 1 as libc::c_int) as libc::c_long)
        as libc::c_int
        != 0) as libc::c_int as libc::c_long
        != 0
    {
        luaD_growstack(L, usize::from((*p).maxstacksize) + 1)?;
    }

    let fresh0 = (*L).top.get();
    (*L).top.add(1);
    let io1: *mut UnsafeValue = &raw mut (*fresh0).val;
    let io2: *const UnsafeValue = &raw mut (*(*ci).func).val;
    (*io1).value_ = (*io2).value_;
    (*io1).tt_ = (*io2).tt_;
    i = 1 as libc::c_int;
    while i <= nfixparams {
        let fresh1 = (*L).top.get();
        (*L).top.add(1);
        let io1_0: *mut UnsafeValue = &raw mut (*fresh1).val;
        let io2_0: *const UnsafeValue = &raw mut (*((*ci).func).offset(i as isize)).val;
        (*io1_0).value_ = (*io2_0).value_;
        (*io1_0).tt_ = (*io2_0).tt_;
        (*((*ci).func).offset(i as isize)).val.tt_ =
            (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
        i += 1;
    }
    (*ci).func = ((*ci).func).offset((actual + 1 as libc::c_int) as isize);
    (*ci).top = ((*ci).top).offset((actual + 1 as libc::c_int) as isize);
    Ok(())
}

pub unsafe fn luaT_getvarargs(
    L: *const Thread,
    ci: *mut CallInfo,
    mut where_0: StkId,
    mut wanted: libc::c_int,
) -> Result<(), Box<dyn core::error::Error>> {
    let mut i: libc::c_int = 0;
    let nextra: libc::c_int = (*ci).u.nextraargs;

    if wanted < 0 as libc::c_int {
        wanted = nextra;

        if ((*L).stack_last.get()).offset_from((*L).top.get()) as libc::c_long
            <= nextra as libc::c_long
        {
            let t__: isize =
                (where_0 as *mut libc::c_char).offset_from((*L).stack.get() as *mut libc::c_char);

            (*L).hdr.global().gc.step();
            luaD_growstack(L, nextra.try_into().unwrap())?;
            where_0 = ((*L).stack.get() as *mut libc::c_char).offset(t__ as isize) as StkId;
        }
        (*L).top.set(where_0.offset(nextra as isize));
    }
    i = 0 as libc::c_int;
    while i < wanted && i < nextra {
        let io1: *mut UnsafeValue = &mut (*where_0.offset(i as isize)).val;
        let io2: *const UnsafeValue =
            &raw mut (*((*ci).func).offset(-(nextra as isize)).offset(i as isize)).val;
        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        i += 1;
    }

    while i < wanted {
        (*where_0.offset(i as isize)).val.tt_ =
            (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
        i += 1;
    }

    Ok(())
}
