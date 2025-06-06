#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments
)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::gc::{Object, luaC_fix};
use crate::ldebug::{luaG_concaterror, luaG_opinterror, luaG_ordererror, luaG_tointerror};
use crate::ldo::{luaD_call, luaD_growstack};
use crate::lobject::{Proto, StkId, TString, TValue, Udata, UntaggedValue};
use crate::lstate::CallInfo;
use crate::lstring::luaS_new;
use crate::ltable::luaH_getshortstr;
use crate::{Lua, Table, Thread};
use alloc::borrow::Cow;
use alloc::boxed::Box;
use core::ffi::CStr;

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
#[allow(dead_code)]
pub const TM_IDIV: TMS = 12;
#[allow(dead_code)]
pub const TM_DIV: TMS = 11;
#[allow(dead_code)]
pub const TM_POW: TMS = 10;
#[allow(dead_code)]
pub const TM_MOD: TMS = 9;
#[allow(dead_code)]
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
    "no value", "nil", "boolean", "userdata", "number", "string", "table", "function", "userdata",
    "thread", "upvalue", "proto",
];

pub unsafe fn luaT_init(g: *const Lua) {
    static mut luaT_eventname: [*const libc::c_char; 25] = [
        b"__index\0" as *const u8 as *const libc::c_char,
        b"__newindex\0" as *const u8 as *const libc::c_char,
        b"__gc\0" as *const u8 as *const libc::c_char,
        b"__mode\0" as *const u8 as *const libc::c_char,
        b"__len\0" as *const u8 as *const libc::c_char,
        b"__eq\0" as *const u8 as *const libc::c_char,
        b"__add\0" as *const u8 as *const libc::c_char,
        b"__sub\0" as *const u8 as *const libc::c_char,
        b"__mul\0" as *const u8 as *const libc::c_char,
        b"__mod\0" as *const u8 as *const libc::c_char,
        b"__pow\0" as *const u8 as *const libc::c_char,
        b"__div\0" as *const u8 as *const libc::c_char,
        b"__idiv\0" as *const u8 as *const libc::c_char,
        b"__band\0" as *const u8 as *const libc::c_char,
        b"__bor\0" as *const u8 as *const libc::c_char,
        b"__bxor\0" as *const u8 as *const libc::c_char,
        b"__shl\0" as *const u8 as *const libc::c_char,
        b"__shr\0" as *const u8 as *const libc::c_char,
        b"__unm\0" as *const u8 as *const libc::c_char,
        b"__bnot\0" as *const u8 as *const libc::c_char,
        b"__lt\0" as *const u8 as *const libc::c_char,
        b"__le\0" as *const u8 as *const libc::c_char,
        b"__concat\0" as *const u8 as *const libc::c_char,
        b"__call\0" as *const u8 as *const libc::c_char,
        b"__close\0" as *const u8 as *const libc::c_char,
    ];
    let mut i: libc::c_int = 0;
    i = 0 as libc::c_int;

    while i < TM_N as libc::c_int {
        (*g).tmname[i as usize].set(luaS_new(g, luaT_eventname[i as usize]));
        luaC_fix(&*g, (*g).tmname[i as usize].get() as *mut Object);
        i += 1;
    }
}

pub unsafe fn luaT_gettm(events: *const Table, event: TMS, ename: *mut TString) -> *const TValue {
    let tm: *const TValue = luaH_getshortstr(events, ename);
    if (*tm).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int {
        (*events)
            .flags
            .set(((*events).flags.get() as libc::c_int | (1 << event) as u8 as libc::c_int) as u8);
        return 0 as *const TValue;
    } else {
        return tm;
    };
}

pub unsafe fn luaT_gettmbyobj(L: *const Thread, o: *const TValue, event: TMS) -> *const TValue {
    let mt = match (*o).tt_ as libc::c_int & 0xf as libc::c_int {
        5 => (*((*o).value_.gc as *mut Table)).metatable.get(),
        7 => (*((*o).value_.gc as *mut Udata)).metatable,
        _ => (*(*L).global).mt[((*o).tt_ & 0xf) as usize].get(),
    };

    return if !mt.is_null() {
        luaH_getshortstr(mt, (*(*L).global).tmname[event as usize].get())
    } else {
        (*(*L).global).nilvalue.get()
    };
}

pub unsafe fn luaT_objtypename(g: *const Lua, o: *const TValue) -> Cow<'static, str> {
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
        let name: *const TValue = luaH_getshortstr(
            mt,
            luaS_new(g, b"__name\0" as *const u8 as *const libc::c_char),
        );
        if (*name).tt_ as libc::c_int & 0xf as libc::c_int == 4 as libc::c_int {
            return CStr::from_ptr(((*((*name).value_.gc as *mut TString)).contents).as_mut_ptr())
                .to_string_lossy()
                .into_owned()
                .into();
        }
    }

    luaT_typenames_[(((*o).tt_ as libc::c_int & 0xf) + 1 as libc::c_int) as usize].into()
}

pub unsafe fn luaT_callTM(
    L: *const Thread,
    f: *const TValue,
    p1: *const TValue,
    p2: *const TValue,
    p3: *const TValue,
) -> Result<(), Box<dyn core::error::Error>> {
    let func: StkId = (*L).top.get();
    let io1: *mut TValue = &mut (*func).val;
    let io2: *const TValue = f;
    (*io1).value_ = (*io2).value_;
    (*io1).tt_ = (*io2).tt_;
    let io1_0: *mut TValue = &mut (*func.offset(1 as libc::c_int as isize)).val;
    let io2_0: *const TValue = p1;
    (*io1_0).value_ = (*io2_0).value_;
    (*io1_0).tt_ = (*io2_0).tt_;
    let io1_1: *mut TValue = &mut (*func.offset(2 as libc::c_int as isize)).val;
    let io2_1: *const TValue = p2;
    (*io1_1).value_ = (*io2_1).value_;
    (*io1_1).tt_ = (*io2_1).tt_;
    let io1_2: *mut TValue = &mut (*func.offset(3 as libc::c_int as isize)).val;
    let io2_2: *const TValue = p3;
    (*io1_2).value_ = (*io2_2).value_;
    (*io1_2).tt_ = (*io2_2).tt_;
    (*L).top.set(func.offset(4 as libc::c_int as isize));

    luaD_call(L, func, 0 as libc::c_int)
}

pub unsafe fn luaT_callTMres(
    L: *const Thread,
    f: *const TValue,
    p1: *const TValue,
    p2: *const TValue,
    mut res: StkId,
) -> Result<(), Box<dyn core::error::Error>> {
    let result: isize =
        (res as *mut libc::c_char).offset_from((*L).stack.get() as *mut libc::c_char);
    let func: StkId = (*L).top.get();
    let io1: *mut TValue = &raw mut (*func).val;
    let io2: *const TValue = f;
    (*io1).value_ = (*io2).value_;
    (*io1).tt_ = (*io2).tt_;
    let io1_0: *mut TValue = &raw mut (*func.offset(1 as libc::c_int as isize)).val;
    let io2_0: *const TValue = p1;
    (*io1_0).value_ = (*io2_0).value_;
    (*io1_0).tt_ = (*io2_0).tt_;
    let io1_1: *mut TValue = &mut (*func.offset(2 as libc::c_int as isize)).val;
    let io2_1: *const TValue = p2;
    (*io1_1).value_ = (*io2_1).value_;
    (*io1_1).tt_ = (*io2_1).tt_;
    (*L).top.add(3);

    luaD_call(L, func, 1 as libc::c_int)?;

    res = ((*L).stack.get() as *mut libc::c_char).offset(result as isize) as StkId;
    let io1_2: *mut TValue = &raw mut (*res).val;
    (*L).top.sub(1);
    let io2_2: *const TValue = &raw mut (*(*L).top.get()).val;
    (*io1_2).value_ = (*io2_2).value_;
    (*io1_2).tt_ = (*io2_2).tt_;

    Ok(())
}

unsafe fn callbinTM(
    L: *const Thread,
    p1: *const TValue,
    p2: *const TValue,
    res: StkId,
    event: TMS,
) -> Result<libc::c_int, Box<dyn core::error::Error>> {
    let mut tm: *const TValue = luaT_gettmbyobj(L, p1, event);
    if (*tm).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int {
        tm = luaT_gettmbyobj(L, p2, event);
    }
    if (*tm).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int {
        return Ok(0 as libc::c_int);
    }
    luaT_callTMres(L, tm, p1, p2, res)?;
    return Ok(1 as libc::c_int);
}

pub unsafe fn luaT_trybinTM(
    L: *const Thread,
    p1: *const TValue,
    p2: *const TValue,
    res: StkId,
    event: TMS,
) -> Result<(), Box<dyn core::error::Error>> {
    if callbinTM(L, p1, p2, res, event)? == 0 {
        match event as libc::c_uint {
            TM_BAND | TM_BOR | TM_BXOR | 16 | 17 | 19 => {
                if (*p1).tt_ as libc::c_int & 0xf as libc::c_int == 3 as libc::c_int
                    && (*p2).tt_ as libc::c_int & 0xf as libc::c_int == 3 as libc::c_int
                {
                    luaG_tointerror(L, p1, p2)?;
                } else {
                    luaG_opinterror(L, p1, p2, "perform bitwise operation on")?;
                }
            }
            _ => luaG_opinterror(L, p1, p2, "perform arithmetic on")?,
        }
    }

    Ok(())
}

pub unsafe fn luaT_tryconcatTM(L: *const Thread) -> Result<(), Box<dyn core::error::Error>> {
    let top: StkId = (*L).top.get();
    if ((callbinTM(
        L,
        &mut (*top.offset(-(2 as libc::c_int as isize))).val,
        &mut (*top.offset(-(1 as libc::c_int as isize))).val,
        top.offset(-(2 as libc::c_int as isize)),
        TM_CONCAT,
    )? == 0) as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        luaG_concaterror(
            L,
            &mut (*top.offset(-(2 as libc::c_int as isize))).val,
            &mut (*top.offset(-(1 as libc::c_int as isize))).val,
        )?;
    }

    Ok(())
}

pub unsafe fn luaT_trybinassocTM(
    L: *const Thread,
    p1: *const TValue,
    p2: *const TValue,
    flip: libc::c_int,
    res: StkId,
    event: TMS,
) -> Result<(), Box<dyn core::error::Error>> {
    if flip != 0 {
        luaT_trybinTM(L, p2, p1, res, event)
    } else {
        luaT_trybinTM(L, p1, p2, res, event)
    }
}

pub unsafe fn luaT_trybiniTM(
    L: *const Thread,
    p1: *const TValue,
    i2: i64,
    flip: libc::c_int,
    res: StkId,
    event: TMS,
) -> Result<(), Box<dyn core::error::Error>> {
    let mut aux: TValue = TValue {
        value_: UntaggedValue {
            gc: 0 as *mut Object,
        },
        tt_: 0,
    };
    let io: *mut TValue = &mut aux;
    (*io).value_.i = i2;
    (*io).tt_ = (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    luaT_trybinassocTM(L, p1, &mut aux, flip, res, event)
}

pub unsafe fn luaT_callorderTM(
    L: *const Thread,
    p1: *const TValue,
    p2: *const TValue,
    event: TMS,
) -> Result<libc::c_int, Box<dyn core::error::Error>> {
    if callbinTM(L, p1, p2, (*L).top.get(), event)? != 0 {
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
    mut p1: *const TValue,
    v2: libc::c_int,
    flip: libc::c_int,
    isfloat: libc::c_int,
    event: TMS,
) -> Result<libc::c_int, Box<dyn core::error::Error>> {
    let mut aux: TValue = TValue {
        value_: UntaggedValue {
            gc: 0 as *mut Object,
        },
        tt_: 0,
    };
    let mut p2: *const TValue = 0 as *const TValue;
    if isfloat != 0 {
        let io: *mut TValue = &mut aux;
        (*io).value_.n = v2 as f64;
        (*io).tt_ = (3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
    } else {
        let io_0: *mut TValue = &mut aux;
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
    let io1: *mut TValue = &raw mut (*fresh0).val;
    let io2: *const TValue = &raw mut (*(*ci).func).val;
    (*io1).value_ = (*io2).value_;
    (*io1).tt_ = (*io2).tt_;
    i = 1 as libc::c_int;
    while i <= nfixparams {
        let fresh1 = (*L).top.get();
        (*L).top.add(1);
        let io1_0: *mut TValue = &raw mut (*fresh1).val;
        let io2_0: *const TValue = &raw mut (*((*ci).func).offset(i as isize)).val;
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

            if (*(*L).global).gc.debt() > 0 {
                crate::gc::step((*L).global);
            }

            luaD_growstack(L, nextra.try_into().unwrap())?;
            where_0 = ((*L).stack.get() as *mut libc::c_char).offset(t__ as isize) as StkId;
        }
        (*L).top.set(where_0.offset(nextra as isize));
    }
    i = 0 as libc::c_int;
    while i < wanted && i < nextra {
        let io1: *mut TValue = &mut (*where_0.offset(i as isize)).val;
        let io2: *const TValue =
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
