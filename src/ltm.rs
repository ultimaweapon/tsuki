#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments
)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::ldebug::{luaG_concaterror, luaG_opinterror, luaG_ordererror, luaG_tointerror};
use crate::ldo::{luaD_call, luaD_growstack};
use crate::lobject::Proto;
use crate::lstate::CallInfo;
use crate::table::luaH_getshortstr;
use crate::value::UnsafeValue;
use crate::{CallError, Lua, NON_YIELDABLE_WAKER, StackValue, Str, Table, Thread, UserData};
use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::string::String;
use core::convert::identity;
use core::ffi::c_char;
use core::pin::pin;
use core::ptr::null;
use core::task::{Context, Poll, Waker};

pub type TMS = u32;

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

type c_int = i32;
type c_uint = u32;
type c_long = i64;

pub unsafe fn luaT_gettm<D>(events: *const Table<D>, event: TMS) -> *const UnsafeValue<D> {
    let ename = (*events)
        .hdr
        .global()
        .events()
        .get_raw_int_key(event.into());
    let tm = luaH_getshortstr(events, ename.value_.gc.cast());

    if (*tm).tt_ as c_int & 0xf as c_int == 0 as c_int {
        (*events)
            .flags
            .set(((*events).flags.get() as c_int | (1 << event) as u8 as c_int) as u8);
        return null();
    } else {
        return tm;
    }
}

pub unsafe fn luaT_gettmbyobj<D>(
    L: *const Thread<D>,
    o: *const UnsafeValue<D>,
    event: TMS,
) -> *const UnsafeValue<D> {
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

pub unsafe fn luaT_objtypename<D>(g: *const Lua<D>, o: *const UnsafeValue<D>) -> Cow<'static, str> {
    let mut mt;

    if (*o).tt_ as c_int == 5 as c_int | (0 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int
        && {
            mt = (*((*o).value_.gc as *mut Table<D>)).metatable.get();
            !mt.is_null()
        }
        || (*o).tt_ as c_int == 7 as c_int | (0 as c_int) << 4 as c_int | (1 as c_int) << 6 as c_int
            && {
                mt = (*(*o).value_.gc.cast::<UserData<D, ()>>()).mt;
                !mt.is_null()
            }
    {
        let name = luaH_getshortstr(mt, Str::from_str(g, "__name").unwrap_or_else(identity));

        if (*name).tt_ as c_int & 0xf as c_int == 4 as c_int {
            return String::from_utf8_lossy((*(*name).value_.gc.cast::<Str<D>>()).as_bytes())
                .into_owned()
                .into();
        }
    }

    luaT_typenames_[(((*o).tt_ as c_int & 0xf) + 1 as c_int) as usize].into()
}

pub unsafe fn luaT_callTM<D>(
    L: *const Thread<D>,
    f: *const UnsafeValue<D>,
    p1: *const UnsafeValue<D>,
    p2: *const UnsafeValue<D>,
    p3: *const UnsafeValue<D>,
) -> Result<(), Box<CallError>> {
    let func = (*L).top.get();
    let io1 = func;
    let io2 = f;
    (*io1).value_ = (*io2).value_;
    (*io1).tt_ = (*io2).tt_;
    let io1_0 = func.offset(1 as c_int as isize);
    let io2_0 = p1;
    (*io1_0).value_ = (*io2_0).value_;
    (*io1_0).tt_ = (*io2_0).tt_;
    let io1_1 = func.offset(2 as c_int as isize);
    let io2_1 = p2;
    (*io1_1).value_ = (*io2_1).value_;
    (*io1_1).tt_ = (*io2_1).tt_;
    let io1_2 = func.offset(3 as c_int as isize);
    let io2_2 = p3;
    (*io1_2).value_ = (*io2_2).value_;
    (*io1_2).tt_ = (*io2_2).tt_;
    (*L).top.set(func.offset(4 as c_int as isize));

    // Invoke.
    let f = pin!(luaD_call(L, func, 0));
    let w = Waker::new(null(), &NON_YIELDABLE_WAKER);

    match f.poll(&mut Context::from_waker(&w)) {
        Poll::Ready(v) => v,
        Poll::Pending => unreachable!(),
    }
}

/// The result will be on the top of stack.
pub unsafe fn luaT_callTMres<D>(
    L: *const Thread<D>,
    f: *const UnsafeValue<D>,
    p1: *const UnsafeValue<D>,
    p2: *const UnsafeValue<D>,
) -> Result<(), Box<CallError>> {
    let func = (*L).top.get();
    let io1 = func;
    let io2 = f;
    (*io1).value_ = (*io2).value_;
    (*io1).tt_ = (*io2).tt_;
    let io1_0 = func.offset(1 as c_int as isize);
    let io2_0 = p1;
    (*io1_0).value_ = (*io2_0).value_;
    (*io1_0).tt_ = (*io2_0).tt_;
    let io1_1 = func.offset(2 as c_int as isize);
    let io2_1 = p2;
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
unsafe fn callbinTM<D>(
    L: *const Thread<D>,
    p1: *const UnsafeValue<D>,
    p2: *const UnsafeValue<D>,
    event: TMS,
) -> Result<c_int, Box<dyn core::error::Error>> {
    let mut tm = luaT_gettmbyobj(L, p1, event);

    if (*tm).tt_ as c_int & 0xf as c_int == 0 as c_int {
        tm = luaT_gettmbyobj(L, p2, event);
    }
    if (*tm).tt_ as c_int & 0xf as c_int == 0 as c_int {
        return Ok(0 as c_int);
    }

    match luaT_callTMres(L, tm, p1, p2) {
        Ok(_) => Ok(1),
        Err(e) => Err(e), // Requires unsized coercion.
    }
}

pub unsafe fn luaT_trybinTM<D>(
    L: *const Thread<D>,
    p1: *const UnsafeValue<D>,
    p2: *const UnsafeValue<D>,
    event: TMS,
) -> Result<UnsafeValue<D>, Box<dyn core::error::Error>> {
    if callbinTM(L, p1, p2, event)? == 0 {
        match event as c_uint {
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

pub unsafe fn luaT_tryconcatTM<D>(L: *const Thread<D>) -> Result<(), Box<dyn core::error::Error>> {
    let top = (*L).top.get();

    if callbinTM(
        L,
        top.offset(-(2 as c_int as isize)).cast(),
        top.offset(-(1 as c_int as isize)).cast(),
        TM_CONCAT,
    )? == 0
    {
        return Err(luaG_concaterror(
            L,
            top.offset(-(2 as c_int as isize)).cast(),
            top.offset(-(1 as c_int as isize)).cast(),
        ));
    }

    // Move result.
    (*L).top.sub(1);
    (*L).top.copy(0, -2);

    Ok(())
}

pub unsafe fn luaT_trybinassocTM<D>(
    L: *const Thread<D>,
    p1: *const UnsafeValue<D>,
    p2: *const UnsafeValue<D>,
    flip: c_int,
    event: TMS,
) -> Result<UnsafeValue<D>, Box<dyn core::error::Error>> {
    if flip != 0 {
        luaT_trybinTM(L, p2, p1, event)
    } else {
        luaT_trybinTM(L, p1, p2, event)
    }
}

pub unsafe fn luaT_trybiniTM<D>(
    L: *const Thread<D>,
    p1: *const UnsafeValue<D>,
    i2: i64,
    flip: c_int,
    event: TMS,
) -> Result<UnsafeValue<D>, Box<dyn core::error::Error>> {
    let mut aux = UnsafeValue::default();
    let io = &raw mut aux;

    (*io).value_.i = i2;
    (*io).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
    luaT_trybinassocTM(L, p1, &mut aux, flip, event)
}

pub unsafe fn luaT_callorderTM<D>(
    L: *const Thread<D>,
    p1: *const UnsafeValue<D>,
    p2: *const UnsafeValue<D>,
    event: TMS,
) -> Result<c_int, Box<dyn core::error::Error>> {
    if callbinTM(L, p1, p2, event)? != 0 {
        (*L).top.sub(1);

        return Ok(
            !((*(*L).top.get()).tt_ as c_int == 1 as c_int | (0 as c_int) << 4 as c_int
                || (*(*L).top.get()).tt_ as c_int & 0xf as c_int == 0 as c_int)
                as c_int,
        );
    }

    Err(luaG_ordererror(L, p1, p2))
}

pub unsafe fn luaT_callorderiTM<D>(
    L: *const Thread<D>,
    mut p1: *const UnsafeValue<D>,
    v2: c_int,
    flip: c_int,
    isfloat: c_int,
    event: TMS,
) -> Result<c_int, Box<dyn core::error::Error>> {
    let mut aux = UnsafeValue::default();
    let mut p2 = null();

    if isfloat != 0 {
        let io = &raw mut aux;

        (*io).value_.n = v2 as f64;
        (*io).tt_ = (3 as c_int | (1 as c_int) << 4 as c_int) as u8;
    } else {
        let io_0 = &raw mut aux;

        (*io_0).value_.i = v2 as i64;
        (*io_0).tt_ = (3 as c_int | (0 as c_int) << 4 as c_int) as u8;
    }
    if flip != 0 {
        p2 = p1;
        p1 = &mut aux;
    } else {
        p2 = &mut aux;
    }
    return luaT_callorderTM(L, p1, p2, event);
}

pub unsafe fn luaT_adjustvarargs<D>(
    L: *const Thread<D>,
    nfixparams: c_int,
    ci: *mut CallInfo<D>,
    p: *const Proto<D>,
) -> Result<(), Box<dyn core::error::Error>> {
    let mut i: c_int = 0;
    let actual: c_int = ((*L).top.get()).offset_from((*ci).func) as c_long as c_int - 1 as c_int;
    let nextra: c_int = actual - nfixparams;
    (*ci).u.nextraargs = nextra;

    if ((((*L).stack_last.get()).offset_from((*L).top.get()) as c_long
        <= ((*p).maxstacksize as c_int + 1 as c_int) as c_long) as c_int
        != 0) as c_int as c_long
        != 0
    {
        luaD_growstack(L, usize::from((*p).maxstacksize) + 1)?;
    }

    let fresh0 = (*L).top.get();
    (*L).top.add(1);
    let io1 = fresh0;
    let io2 = (*ci).func;

    (*io1).value_ = (*io2).value_;
    (*io1).tt_ = (*io2).tt_;
    i = 1 as c_int;
    while i <= nfixparams {
        let fresh1 = (*L).top.get();
        (*L).top.add(1);
        let io1_0 = fresh1;
        let io2_0 = ((*ci).func).offset(i as isize);

        (*io1_0).value_ = (*io2_0).value_;
        (*io1_0).tt_ = (*io2_0).tt_;
        (*((*ci).func).offset(i as isize)).tt_ = (0 as c_int | (0 as c_int) << 4 as c_int) as u8;
        i += 1;
    }
    (*ci).func = ((*ci).func).offset((actual + 1 as c_int) as isize);
    (*ci).top = ((*ci).top).offset((actual + 1 as c_int) as isize);
    Ok(())
}

pub unsafe fn luaT_getvarargs<D>(
    L: *const Thread<D>,
    ci: *mut CallInfo<D>,
    mut where_0: *mut StackValue<D>,
    mut wanted: c_int,
) -> Result<(), Box<dyn core::error::Error>> {
    let mut i: c_int = 0;
    let nextra: c_int = (*ci).u.nextraargs;

    if wanted < 0 as c_int {
        wanted = nextra;

        if ((*L).stack_last.get()).offset_from((*L).top.get()) as c_long <= nextra as c_long {
            let t__: isize = (where_0 as *mut c_char).offset_from((*L).stack.get() as *mut c_char);

            luaD_growstack(L, nextra.try_into().unwrap())?;
            where_0 = ((*L).stack.get() as *mut c_char).offset(t__ as isize) as *mut StackValue<D>;
        }
        (*L).top.set(where_0.offset(nextra as isize));
    }
    i = 0 as c_int;
    while i < wanted && i < nextra {
        let io1 = where_0.offset(i as isize);
        let io2 = ((*ci).func).offset(-(nextra as isize)).offset(i as isize);

        (*io1).value_ = (*io2).value_;
        (*io1).tt_ = (*io2).tt_;
        i += 1;
    }

    while i < wanted {
        (*where_0.offset(i as isize)).tt_ = (0 as c_int | (0 as c_int) << 4 as c_int) as u8;
        i += 1;
    }

    Ok(())
}
