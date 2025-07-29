#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments
)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::llex::{LexState, luaX_syntaxerror};
use crate::lmem::luaM_growaux_;
use crate::lobject::{AbsLineInfo, Proto, luaO_ceillog2, luaO_rawarith};
use crate::lparser::{
    C2RustUnnamed_11, FuncState, VCALL, VFALSE, VINDEXED, VINDEXI, VINDEXSTR, VINDEXUP, VJMP, VK,
    VKFLT, VKINT, VKSTR, VLOCAL, VNIL, VNONRELOC, VRELOC, VTRUE, VUPVAL, VVARARG, expdesc,
    luaY_nvarstack,
};
use crate::ltm::{TM_ADD, TM_SHL, TM_SHR, TM_SUB, TMS};
use crate::table::{luaH_finishset, luaH_get};
use crate::value::{UnsafeValue, UntaggedValue};
use crate::vm::{
    F2Ieq, OP_ADD, OP_ADDI, OP_ADDK, OP_CONCAT, OP_EQ, OP_EQI, OP_EQK, OP_EXTRAARG, OP_GETFIELD,
    OP_GETI, OP_GETTABLE, OP_GETTABUP, OP_GETUPVAL, OP_GTI, OP_JMP, OP_LFALSESKIP, OP_LOADF,
    OP_LOADFALSE, OP_LOADI, OP_LOADK, OP_LOADKX, OP_LOADNIL, OP_LOADTRUE, OP_LT, OP_LTI, OP_MMBIN,
    OP_MMBINI, OP_MMBINK, OP_MOVE, OP_NEWTABLE, OP_NOT, OP_RETURN, OP_RETURN0, OP_RETURN1, OP_SELF,
    OP_SETFIELD, OP_SETI, OP_SETLIST, OP_SETTABLE, OP_SETTABUP, OP_SETUPVAL, OP_SHLI, OP_SHRI,
    OP_TEST, OP_TESTSET, OP_UNM, OpCode, luaP_opmodes, luaV_equalobj, luaV_flttointeger,
    luaV_tointegerns,
};
use crate::{ArithError, Object, Ops, ParseError, Str, Thread};
use core::fmt::Display;
use core::ops::Deref;
use libc::abs;
use libm::ldexp;

pub type BinOpr = libc::c_uint;
pub type UnOpr = libc::c_uint;

pub const OPR_NOBINOPR: BinOpr = 21;
pub const OPR_OR: BinOpr = 20;
pub const OPR_AND: BinOpr = 19;
pub const OPR_GE: BinOpr = 18;
pub const OPR_GT: BinOpr = 17;
pub const OPR_NE: BinOpr = 16;
pub const OPR_LE: BinOpr = 15;
pub const OPR_LT: BinOpr = 14;
pub const OPR_EQ: BinOpr = 13;
pub const OPR_CONCAT: BinOpr = 12;
pub const OPR_SHR: BinOpr = 11;
pub const OPR_SHL: BinOpr = 10;
pub const OPR_BXOR: BinOpr = 9;
pub const OPR_BOR: BinOpr = 8;
pub const OPR_BAND: BinOpr = 7;
pub const OPR_IDIV: BinOpr = 6;
pub const OPR_DIV: BinOpr = 5;
pub const OPR_POW: BinOpr = 4;
pub const OPR_MOD: BinOpr = 3;
pub const OPR_MUL: BinOpr = 2;
pub const OPR_SUB: BinOpr = 1;
pub const OPR_ADD: BinOpr = 0;

pub const OPR_NOUNOPR: UnOpr = 4;
pub const OPR_LEN: UnOpr = 3;
pub const OPR_NOT: UnOpr = 2;
pub const OPR_BNOT: UnOpr = 1;
pub const OPR_MINUS: UnOpr = 0;

pub unsafe fn luaK_semerror(ls: *mut LexState, msg: impl Display) -> ParseError {
    (*ls).t.token = 0 as libc::c_int;

    luaX_syntaxerror(ls, msg)
}

unsafe fn tonumeral(e: *const expdesc, v: *mut UnsafeValue) -> libc::c_int {
    if (*e).t != (*e).f {
        return 0 as libc::c_int;
    }
    match (*e).k as libc::c_uint {
        6 => {
            if !v.is_null() {
                let io: *mut UnsafeValue = v;
                (*io).value_.i = (*e).u.ival;
                (*io).tt_ = (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
            }
            return 1 as libc::c_int;
        }
        5 => {
            if !v.is_null() {
                let io_0: *mut UnsafeValue = v;
                (*io_0).value_.n = (*e).u.nval;
                (*io_0).tt_ = (3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
            }
            return 1 as libc::c_int;
        }
        _ => return 0 as libc::c_int,
    };
}

unsafe fn const2val(ls: *mut LexState, e: *const expdesc) -> *mut UnsafeValue {
    return &raw mut (*((*(*ls).dyd).actvar.arr).offset((*e).u.info as isize)).k;
}

pub unsafe fn luaK_exp2const(
    ls: *mut LexState,
    e: *const expdesc,
    v: *mut UnsafeValue,
) -> libc::c_int {
    if (*e).t != (*e).f {
        return 0 as libc::c_int;
    }
    match (*e).k as libc::c_uint {
        3 => {
            (*v).tt_ = (1 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
            return 1 as libc::c_int;
        }
        2 => {
            (*v).tt_ = (1 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
            return 1 as libc::c_int;
        }
        1 => {
            (*v).tt_ = (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
            return 1 as libc::c_int;
        }
        7 => {
            let io: *mut UnsafeValue = v;
            let x_ = (*e).u.strval;

            (*io).value_.gc = x_.cast();
            (*io).tt_ = ((*x_).hdr.tt as libc::c_int | (1 as libc::c_int) << 6) as u8;

            return 1 as libc::c_int;
        }
        11 => {
            let io1: *mut UnsafeValue = v;
            let io2: *const UnsafeValue = const2val(ls, e);
            (*io1).value_ = (*io2).value_;
            (*io1).tt_ = (*io2).tt_;
            return 1 as libc::c_int;
        }
        _ => return tonumeral(e, v),
    };
}

unsafe fn previousinstruction(fs: *mut FuncState) -> *mut u32 {
    static mut invalidinstruction: u32 = !(0 as libc::c_int as u32);
    if (*fs).pc > (*fs).lasttarget {
        return &mut *((*(*fs).f).code).offset(((*fs).pc - 1 as libc::c_int) as isize) as *mut u32;
    } else {
        return &raw mut invalidinstruction as *const u32 as *mut u32;
    };
}

pub unsafe fn luaK_nil(
    ls: *mut LexState,
    fs: *mut FuncState,
    mut from: libc::c_int,
    n: libc::c_int,
) -> Result<(), ParseError> {
    let mut l: libc::c_int = from + n - 1 as libc::c_int;
    let previous: *mut u32 = previousinstruction(fs);
    if (*previous >> 0 as libc::c_int
        & !(!(0 as libc::c_int as u32) << 7 as libc::c_int) << 0 as libc::c_int) as OpCode
        as libc::c_uint
        == OP_LOADNIL as libc::c_int as libc::c_uint
    {
        let pfrom: libc::c_int = (*previous >> 0 as libc::c_int + 7 as libc::c_int
            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
            as libc::c_int;
        let pl: libc::c_int = pfrom
            + (*previous
                >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int
                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                as libc::c_int;
        if pfrom <= from && from <= pl + 1 as libc::c_int
            || from <= pfrom && pfrom <= l + 1 as libc::c_int
        {
            if pfrom < from {
                from = pfrom;
            }
            if pl > l {
                l = pl;
            }
            *previous = *previous
                & !(!(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                    << 0 as libc::c_int + 7 as libc::c_int)
                | (from as u32) << 0 as libc::c_int + 7 as libc::c_int
                    & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                        << 0 as libc::c_int + 7 as libc::c_int;
            *previous = *previous
                & !(!(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                    << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int)
                | ((l - from) as u32)
                    << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int
                    & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                        << 0 as libc::c_int
                            + 7 as libc::c_int
                            + 8 as libc::c_int
                            + 1 as libc::c_int;
            return Ok(());
        }
    }
    luaK_codeABCk(
        ls,
        fs,
        OP_LOADNIL,
        from,
        n - 1 as libc::c_int,
        0 as libc::c_int,
        0 as libc::c_int,
    )?;
    Ok(())
}

unsafe fn getjump(fs: *mut FuncState, pc: libc::c_int) -> libc::c_int {
    let offset: libc::c_int = (*((*(*fs).f).code).offset(pc as isize)
        >> 0 as libc::c_int + 7 as libc::c_int
        & !(!(0 as libc::c_int as u32)
            << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int + 8 as libc::c_int)
            << 0 as libc::c_int) as libc::c_int
        - (((1 as libc::c_int)
            << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int + 8 as libc::c_int)
            - 1 as libc::c_int
            >> 1 as libc::c_int);
    if offset == -(1 as libc::c_int) {
        return -(1 as libc::c_int);
    } else {
        return pc + 1 as libc::c_int + offset;
    };
}

unsafe fn fixjump(
    ls: *mut LexState,
    fs: *mut FuncState,
    pc: libc::c_int,
    dest: libc::c_int,
) -> Result<(), ParseError> {
    let jmp: *mut u32 = &mut *((*(*fs).f).code).offset(pc as isize) as *mut u32;
    let offset: libc::c_int = dest - (pc + 1 as libc::c_int);
    if !(-(((1 as libc::c_int)
        << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int + 8 as libc::c_int)
        - 1 as libc::c_int
        >> 1 as libc::c_int)
        <= offset
        && offset
            <= ((1 as libc::c_int)
                << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int + 8 as libc::c_int)
                - 1 as libc::c_int
                - (((1 as libc::c_int)
                    << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int + 8 as libc::c_int)
                    - 1 as libc::c_int
                    >> 1 as libc::c_int))
    {
        return Err(luaX_syntaxerror(ls, "control structure too long"));
    }
    *jmp = *jmp
        & !(!(!(0 as libc::c_int as u32)
            << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int + 8 as libc::c_int)
            << 0 as libc::c_int + 7 as libc::c_int)
        | ((offset
            + (((1 as libc::c_int)
                << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int + 8 as libc::c_int)
                - 1 as libc::c_int
                >> 1 as libc::c_int)) as libc::c_uint)
            << 0 as libc::c_int + 7 as libc::c_int
            & !(!(0 as libc::c_int as u32)
                << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int + 8 as libc::c_int)
                << 0 as libc::c_int + 7 as libc::c_int;
    Ok(())
}

pub unsafe fn luaK_concat(
    ls: *mut LexState,
    fs: *mut FuncState,
    l1: *mut libc::c_int,
    l2: libc::c_int,
) -> Result<(), ParseError> {
    if l2 == -(1 as libc::c_int) {
        return Ok(());
    } else if *l1 == -(1 as libc::c_int) {
        *l1 = l2;
    } else {
        let mut list: libc::c_int = *l1;
        let mut next: libc::c_int = 0;
        loop {
            next = getjump(fs, list);
            if !(next != -(1 as libc::c_int)) {
                break;
            }
            list = next;
        }
        fixjump(ls, fs, list, l2)?;
    };
    Ok(())
}

pub unsafe fn luaK_jump(ls: *mut LexState, fs: *mut FuncState) -> Result<libc::c_int, ParseError> {
    return codesJ(ls, fs, OP_JMP, -(1 as libc::c_int), 0 as libc::c_int);
}

pub unsafe fn luaK_ret(
    ls: *mut LexState,
    fs: *mut FuncState,
    first: libc::c_int,
    nret: libc::c_int,
) -> Result<(), ParseError> {
    let mut op: OpCode = OP_MOVE;
    match nret {
        0 => {
            op = OP_RETURN0;
        }
        1 => {
            op = OP_RETURN1;
        }
        _ => {
            op = OP_RETURN;
        }
    }
    luaK_codeABCk(
        ls,
        fs,
        op,
        first,
        nret + 1 as libc::c_int,
        0 as libc::c_int,
        0 as libc::c_int,
    )?;
    Ok(())
}

unsafe fn condjump(
    ls: *mut LexState,
    fs: *mut FuncState,
    op: OpCode,
    A: libc::c_int,
    B: libc::c_int,
    C: libc::c_int,
    k: libc::c_int,
) -> Result<libc::c_int, ParseError> {
    luaK_codeABCk(ls, fs, op, A, B, C, k)?;
    return luaK_jump(ls, fs);
}

pub unsafe fn luaK_getlabel(fs: *mut FuncState) -> libc::c_int {
    (*fs).lasttarget = (*fs).pc;
    return (*fs).pc;
}

unsafe fn getjumpcontrol(fs: *mut FuncState, pc: libc::c_int) -> *mut u32 {
    let pi: *mut u32 = &mut *((*(*fs).f).code).offset(pc as isize) as *mut u32;
    if pc >= 1 as libc::c_int
        && luaP_opmodes[(*pi.offset(-(1 as libc::c_int as isize)) >> 0 as libc::c_int
            & !(!(0 as libc::c_int as u32) << 7 as libc::c_int) << 0 as libc::c_int)
            as OpCode as usize] as libc::c_int
            & (1 as libc::c_int) << 4 as libc::c_int
            != 0
    {
        return pi.offset(-(1 as libc::c_int as isize));
    } else {
        return pi;
    };
}

unsafe fn patchtestreg(fs: *mut FuncState, node: libc::c_int, reg: libc::c_int) -> libc::c_int {
    let i: *mut u32 = getjumpcontrol(fs, node);
    if (*i >> 0 as libc::c_int
        & !(!(0 as libc::c_int as u32) << 7 as libc::c_int) << 0 as libc::c_int) as OpCode
        as libc::c_uint
        != OP_TESTSET as libc::c_int as libc::c_uint
    {
        return 0 as libc::c_int;
    }
    if reg != ((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int
        && reg
            != (*i >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int
                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                as libc::c_int
    {
        *i = *i
            & !(!(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                << 0 as libc::c_int + 7 as libc::c_int)
            | (reg as u32) << 0 as libc::c_int + 7 as libc::c_int
                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                    << 0 as libc::c_int + 7 as libc::c_int;
    } else {
        *i = (OP_TEST as libc::c_int as u32) << 0 as libc::c_int
            | ((*i >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int
                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                as libc::c_int as u32)
                << 0 as libc::c_int + 7 as libc::c_int
            | (0 as libc::c_int as u32)
                << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int
            | (0 as libc::c_int as u32)
                << 0 as libc::c_int
                    + 7 as libc::c_int
                    + 8 as libc::c_int
                    + 1 as libc::c_int
                    + 8 as libc::c_int
            | ((*i >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int
                & !(!(0 as libc::c_int as u32) << 1 as libc::c_int) << 0 as libc::c_int)
                as libc::c_int as u32)
                << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int;
    }
    return 1 as libc::c_int;
}

unsafe fn removevalues(fs: *mut FuncState, mut list: libc::c_int) {
    while list != -(1 as libc::c_int) {
        patchtestreg(
            fs,
            list,
            ((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int,
        );
        list = getjump(fs, list);
    }
}

unsafe fn patchlistaux(
    ls: *mut LexState,
    fs: *mut FuncState,
    mut list: libc::c_int,
    vtarget: libc::c_int,
    reg: libc::c_int,
    dtarget: libc::c_int,
) -> Result<(), ParseError> {
    while list != -(1 as libc::c_int) {
        let next: libc::c_int = getjump(fs, list);
        if patchtestreg(fs, list, reg) != 0 {
            fixjump(ls, fs, list, vtarget)?;
        } else {
            fixjump(ls, fs, list, dtarget)?;
        }
        list = next;
    }
    Ok(())
}

pub unsafe fn luaK_patchlist(
    ls: *mut LexState,
    fs: *mut FuncState,
    list: libc::c_int,
    target: libc::c_int,
) -> Result<(), ParseError> {
    patchlistaux(
        ls,
        fs,
        list,
        target,
        ((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int,
        target,
    )
}

pub unsafe fn luaK_patchtohere(
    ls: *mut LexState,
    fs: *mut FuncState,
    list: libc::c_int,
) -> Result<(), ParseError> {
    let hr: libc::c_int = luaK_getlabel(fs);
    luaK_patchlist(ls, fs, list, hr)
}

unsafe fn savelineinfo(
    ls: *mut LexState,
    fs: *mut FuncState,
    f: *mut Proto,
    line: libc::c_int,
) -> Result<(), ParseError> {
    let mut linedif: libc::c_int = line - (*fs).previousline;
    let pc: libc::c_int = (*fs).pc - 1 as libc::c_int;

    if abs(linedif) >= 0x80 as libc::c_int || {
        let fresh0 = (*fs).iwthabs;
        (*fs).iwthabs = ((*fs).iwthabs).wrapping_add(1);
        fresh0 as libc::c_int >= 128 as libc::c_int
    } {
        (*f).abslineinfo = luaM_growaux_(
            &(*ls).g,
            (*f).abslineinfo as *mut libc::c_void,
            (*fs).nabslineinfo,
            &mut (*f).sizeabslineinfo,
            ::core::mem::size_of::<AbsLineInfo>() as libc::c_ulong as libc::c_int,
            (if 2147483647 as libc::c_int as usize
                <= (!(0 as libc::c_int as usize))
                    .wrapping_div(::core::mem::size_of::<AbsLineInfo>())
            {
                2147483647 as libc::c_int as libc::c_uint
            } else {
                (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<AbsLineInfo>())
                    as libc::c_uint
            }) as libc::c_int,
            "lines",
            (*ls).linenumber,
        )? as *mut AbsLineInfo;
        (*((*f).abslineinfo).offset((*fs).nabslineinfo as isize)).pc = pc;
        let fresh1 = (*fs).nabslineinfo;
        (*fs).nabslineinfo = (*fs).nabslineinfo + 1;
        (*((*f).abslineinfo).offset(fresh1 as isize)).line = line;
        linedif = -(0x80 as libc::c_int);
        (*fs).iwthabs = 1 as libc::c_int as u8;
    }

    (*f).lineinfo = luaM_growaux_(
        &(*ls).g,
        (*f).lineinfo as *mut libc::c_void,
        pc,
        &mut (*f).sizelineinfo,
        ::core::mem::size_of::<i8>() as libc::c_ulong as libc::c_int,
        (if 2147483647 as libc::c_int as usize
            <= (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<i8>())
        {
            2147483647 as libc::c_int as libc::c_uint
        } else {
            (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<i8>())
                as libc::c_uint
        }) as libc::c_int,
        "opcodes",
        (*ls).linenumber,
    )? as *mut i8;

    *((*f).lineinfo).offset(pc as isize) = linedif as i8;
    (*fs).previousline = line;

    Ok(())
}

unsafe fn removelastlineinfo(fs: *mut FuncState) {
    let f: *mut Proto = (*fs).f;
    let pc: libc::c_int = (*fs).pc - 1 as libc::c_int;
    if *((*f).lineinfo).offset(pc as isize) as libc::c_int != -(0x80 as libc::c_int) {
        (*fs).previousline -= *((*f).lineinfo).offset(pc as isize) as libc::c_int;
        (*fs).iwthabs = ((*fs).iwthabs).wrapping_sub(1);
        (*fs).iwthabs;
    } else {
        (*fs).nabslineinfo -= 1;
        (*fs).nabslineinfo;
        (*fs).iwthabs = (128 as libc::c_int + 1 as libc::c_int) as u8;
    };
}

unsafe fn removelastinstruction(fs: *mut FuncState) {
    removelastlineinfo(fs);
    (*fs).pc -= 1;
    (*fs).pc;
}

pub unsafe fn luaK_code(
    ls: *mut LexState,
    fs: *mut FuncState,
    i: u32,
) -> Result<libc::c_int, ParseError> {
    let f: *mut Proto = (*fs).f;

    (*f).code = luaM_growaux_(
        &(*ls).g,
        (*f).code as *mut libc::c_void,
        (*fs).pc,
        &mut (*f).sizecode,
        ::core::mem::size_of::<u32>() as libc::c_ulong as libc::c_int,
        (if 2147483647 as libc::c_int as usize
            <= (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<u32>())
        {
            2147483647 as libc::c_int as libc::c_uint
        } else {
            (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<u32>())
                as libc::c_uint
        }) as libc::c_int,
        "opcodes",
        (*ls).linenumber,
    )? as *mut u32;

    let fresh2 = (*fs).pc;
    (*fs).pc = (*fs).pc + 1;
    *((*f).code).offset(fresh2 as isize) = i;
    savelineinfo(ls, fs, f, (*ls).lastline)?;
    return Ok((*fs).pc - 1 as libc::c_int);
}

pub unsafe fn luaK_codeABCk(
    ls: *mut LexState,
    fs: *mut FuncState,
    o: OpCode,
    a: libc::c_int,
    b: libc::c_int,
    c: libc::c_int,
    k: libc::c_int,
) -> Result<libc::c_int, ParseError> {
    luaK_code(
        ls,
        fs,
        (o as u32) << 0 as libc::c_int
            | (a as u32) << 0 as libc::c_int + 7 as libc::c_int
            | (b as u32)
                << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int
            | (c as u32)
                << 0 as libc::c_int
                    + 7 as libc::c_int
                    + 8 as libc::c_int
                    + 1 as libc::c_int
                    + 8 as libc::c_int
            | (k as u32) << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int,
    )
}

pub unsafe fn luaK_codeABx(
    ls: *mut LexState,
    fs: *mut FuncState,
    o: OpCode,
    a: libc::c_int,
    bc: libc::c_uint,
) -> Result<libc::c_int, ParseError> {
    return luaK_code(
        ls,
        fs,
        (o as u32) << 0 as libc::c_int
            | (a as u32) << 0 as libc::c_int + 7 as libc::c_int
            | bc << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int,
    );
}

unsafe fn codeAsBx(
    ls: *mut LexState,
    fs: *mut FuncState,
    o: OpCode,
    a: libc::c_int,
    bc: libc::c_int,
) -> Result<libc::c_int, ParseError> {
    let b: libc::c_uint = (bc
        + (((1 as libc::c_int) << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int)
            - 1 as libc::c_int
            >> 1 as libc::c_int)) as libc::c_uint;
    return luaK_code(
        ls,
        fs,
        (o as u32) << 0 as libc::c_int
            | (a as u32) << 0 as libc::c_int + 7 as libc::c_int
            | b << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int,
    );
}

unsafe fn codesJ(
    ls: *mut LexState,
    fs: *mut FuncState,
    o: OpCode,
    sj: libc::c_int,
    k: libc::c_int,
) -> Result<libc::c_int, ParseError> {
    let j: libc::c_uint = (sj
        + (((1 as libc::c_int)
            << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int + 8 as libc::c_int)
            - 1 as libc::c_int
            >> 1 as libc::c_int)) as libc::c_uint;
    return luaK_code(
        ls,
        fs,
        (o as u32) << 0 as libc::c_int
            | j << 0 as libc::c_int + 7 as libc::c_int
            | (k as u32) << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int,
    );
}

unsafe fn codeextraarg(
    ls: *mut LexState,
    fs: *mut FuncState,
    a: libc::c_int,
) -> Result<libc::c_int, ParseError> {
    return luaK_code(
        ls,
        fs,
        (OP_EXTRAARG as libc::c_int as u32) << 0 as libc::c_int
            | (a as u32) << 0 as libc::c_int + 7 as libc::c_int,
    );
}

unsafe fn luaK_codek(
    ls: *mut LexState,
    fs: *mut FuncState,
    reg: libc::c_int,
    k: libc::c_int,
) -> Result<libc::c_int, ParseError> {
    if k <= ((1 as libc::c_int) << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int)
        - 1 as libc::c_int
    {
        return luaK_codeABx(ls, fs, OP_LOADK, reg, k as libc::c_uint);
    } else {
        let p: libc::c_int =
            luaK_codeABx(ls, fs, OP_LOADKX, reg, 0 as libc::c_int as libc::c_uint)?;
        codeextraarg(ls, fs, k)?;
        return Ok(p);
    };
}

pub unsafe fn luaK_checkstack(
    ls: *mut LexState,
    fs: *mut FuncState,
    n: libc::c_int,
) -> Result<(), ParseError> {
    let newstack: libc::c_int = (*fs).freereg as libc::c_int + n;
    if newstack > (*(*fs).f).maxstacksize as libc::c_int {
        if newstack >= 255 as libc::c_int {
            return Err(luaX_syntaxerror(
                ls,
                "function or expression needs too many registers",
            ));
        }
        (*(*fs).f).maxstacksize = newstack as u8;
    }
    Ok(())
}

pub unsafe fn luaK_reserveregs(
    ls: *mut LexState,
    fs: *mut FuncState,
    n: libc::c_int,
) -> Result<(), ParseError> {
    luaK_checkstack(ls, fs, n)?;
    (*fs).freereg = ((*fs).freereg as libc::c_int + n) as u8;
    Ok(())
}

unsafe fn freereg(ls: *mut LexState, fs: *mut FuncState, reg: libc::c_int) {
    if reg >= luaY_nvarstack(ls, fs) {
        (*fs).freereg = ((*fs).freereg).wrapping_sub(1);
        (*fs).freereg;
    }
}

unsafe fn freeregs(ls: *mut LexState, fs: *mut FuncState, r1: libc::c_int, r2: libc::c_int) {
    if r1 > r2 {
        freereg(ls, fs, r1);
        freereg(ls, fs, r2);
    } else {
        freereg(ls, fs, r2);
        freereg(ls, fs, r1);
    };
}

unsafe fn freeexp(ls: *mut LexState, fs: *mut FuncState, e: *mut expdesc) {
    if (*e).k as libc::c_uint == VNONRELOC as libc::c_int as libc::c_uint {
        freereg(ls, fs, (*e).u.info);
    }
}

unsafe fn freeexps(ls: *mut LexState, fs: *mut FuncState, e1: *mut expdesc, e2: *mut expdesc) {
    let r1: libc::c_int = if (*e1).k as libc::c_uint == VNONRELOC as libc::c_int as libc::c_uint {
        (*e1).u.info
    } else {
        -(1 as libc::c_int)
    };
    let r2: libc::c_int = if (*e2).k as libc::c_uint == VNONRELOC as libc::c_int as libc::c_uint {
        (*e2).u.info
    } else {
        -(1 as libc::c_int)
    };
    freeregs(ls, fs, r1, r2);
}

unsafe fn addk(
    ls: *mut LexState,
    fs: *mut FuncState,
    key: *mut UnsafeValue,
    v: *mut UnsafeValue,
) -> Result<libc::c_int, ParseError> {
    let mut val: UnsafeValue = UnsafeValue {
        value_: UntaggedValue {
            gc: 0 as *mut Object,
        },
        tt_: 0,
    };

    let f: *mut Proto = (*fs).f;
    let idx: *const UnsafeValue = luaH_get((*ls).h.deref(), key);
    let mut k: libc::c_int = 0;
    let mut oldsize: libc::c_int = 0;
    if (*idx).tt_ as libc::c_int == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int {
        k = (*idx).value_.i as libc::c_int;
        if k < (*fs).nk
            && (*((*f).k).offset(k as isize)).tt_ as libc::c_int & 0x3f as libc::c_int
                == (*v).tt_ as libc::c_int & 0x3f as libc::c_int
            && luaV_equalobj(0 as *mut Thread, &mut *((*f).k).offset(k as isize), v).unwrap() != 0
        {
            return Ok(k);
        }
    }
    oldsize = (*f).sizek;
    k = (*fs).nk;
    let io: *mut UnsafeValue = &mut val;
    (*io).value_.i = k as i64;
    (*io).tt_ = (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    luaH_finishset((*ls).h.deref(), key, idx, &raw const val).unwrap(); // This should never fails.
    (*f).k = luaM_growaux_(
        &(*ls).g,
        (*f).k as *mut libc::c_void,
        k,
        &mut (*f).sizek,
        ::core::mem::size_of::<UnsafeValue>() as libc::c_ulong as libc::c_int,
        (if (((1 as libc::c_int)
            << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int + 8 as libc::c_int)
            - 1 as libc::c_int) as usize
            <= (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<UnsafeValue>())
        {
            (((1 as libc::c_int)
                << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int + 8 as libc::c_int)
                - 1 as libc::c_int) as libc::c_uint
        } else {
            (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<UnsafeValue>())
                as libc::c_uint
        }) as libc::c_int,
        "constants",
        (*ls).linenumber,
    )? as *mut UnsafeValue;
    while oldsize < (*f).sizek {
        let fresh3 = oldsize;
        oldsize = oldsize + 1;
        (*((*f).k).offset(fresh3 as isize)).tt_ =
            (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    }
    let io1: *mut UnsafeValue = &mut *((*f).k).offset(k as isize) as *mut UnsafeValue;
    let io2: *const UnsafeValue = v;
    (*io1).value_ = (*io2).value_;
    (*io1).tt_ = (*io2).tt_;
    (*fs).nk += 1;
    (*fs).nk;

    if (*v).tt_ as libc::c_int & (1 as libc::c_int) << 6 as libc::c_int != 0 {
        if (*f).hdr.marked.get() as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
            && (*(*v).value_.gc).marked.get() as libc::c_int
                & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
                != 0
        {
            (*ls)
                .g
                .gc
                .barrier(f as *mut Object, (*v).value_.gc as *mut Object);
        }
    }

    return Ok(k);
}

unsafe fn stringK(
    ls: *mut LexState,
    fs: *mut FuncState,
    s: *const Str,
) -> Result<libc::c_int, ParseError> {
    let mut o: UnsafeValue = UnsafeValue {
        value_: UntaggedValue {
            gc: 0 as *mut Object,
        },
        tt_: 0,
    };
    let io: *mut UnsafeValue = &raw mut o;

    (*io).value_.gc = s.cast();
    (*io).tt_ = ((*s).hdr.tt as libc::c_int | (1 as libc::c_int) << 6 as libc::c_int) as u8;

    return addk(ls, fs, &raw mut o, &raw mut o);
}

unsafe fn luaK_intK(
    ls: *mut LexState,
    fs: *mut FuncState,
    n: i64,
) -> Result<libc::c_int, ParseError> {
    let mut o: UnsafeValue = UnsafeValue {
        value_: UntaggedValue {
            gc: 0 as *mut Object,
        },
        tt_: 0,
    };
    let io: *mut UnsafeValue = &mut o;
    (*io).value_.i = n;
    (*io).tt_ = (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    return addk(ls, fs, &mut o, &mut o);
}

unsafe fn luaK_numberK(
    ls: *mut LexState,
    fs: *mut FuncState,
    r: f64,
) -> Result<libc::c_int, ParseError> {
    let mut o: UnsafeValue = UnsafeValue {
        value_: UntaggedValue {
            gc: 0 as *mut Object,
        },
        tt_: 0,
    };
    let mut ik: i64 = 0;
    let io: *mut UnsafeValue = &mut o;
    (*io).value_.n = r;
    (*io).tt_ = (3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
    if luaV_flttointeger(r, &mut ik, F2Ieq) == 0 {
        return addk(ls, fs, &mut o, &mut o);
    } else {
        let nbm: libc::c_int = 53 as libc::c_int;
        let q: f64 = ldexp(1.0f64, -nbm + 1 as libc::c_int);
        let k: f64 = if ik == 0 as libc::c_int as i64 {
            q
        } else {
            r + r * q
        };
        let mut kv: UnsafeValue = UnsafeValue {
            value_: UntaggedValue {
                gc: 0 as *mut Object,
            },
            tt_: 0,
        };
        let io_0: *mut UnsafeValue = &mut kv;
        (*io_0).value_.n = k;
        (*io_0).tt_ = (3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
        return addk(ls, fs, &mut kv, &mut o);
    };
}

unsafe fn boolF(ls: *mut LexState, fs: *mut FuncState) -> Result<libc::c_int, ParseError> {
    let mut o: UnsafeValue = UnsafeValue {
        value_: UntaggedValue {
            gc: 0 as *mut Object,
        },
        tt_: 0,
    };
    o.tt_ = (1 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    return addk(ls, fs, &mut o, &mut o);
}

unsafe fn boolT(ls: *mut LexState, fs: *mut FuncState) -> Result<libc::c_int, ParseError> {
    let mut o: UnsafeValue = UnsafeValue {
        value_: UntaggedValue {
            gc: 0 as *mut Object,
        },
        tt_: 0,
    };
    o.tt_ = (1 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
    return addk(ls, fs, &mut o, &mut o);
}

unsafe fn nilK(ls: *mut LexState, fs: *mut FuncState) -> Result<libc::c_int, ParseError> {
    let mut k: UnsafeValue = UnsafeValue {
        value_: UntaggedValue {
            gc: 0 as *mut Object,
        },
        tt_: 0,
    };
    let mut v: UnsafeValue = UnsafeValue {
        value_: UntaggedValue {
            gc: 0 as *mut Object,
        },
        tt_: 0,
    };
    v.tt_ = (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    let io: *mut UnsafeValue = &mut k;

    (*io).value_.gc = &(&(*ls).h).hdr;
    (*io).tt_ = (5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 6 as libc::c_int) as u8;
    return addk(ls, fs, &mut k, &mut v);
}

unsafe fn fitsC(i: i64) -> libc::c_int {
    return ((i as u64).wrapping_add(
        (((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int >> 1 as libc::c_int) as u64,
    ) <= (((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int) as libc::c_uint
        as u64) as libc::c_int;
}

unsafe fn fitsBx(i: i64) -> libc::c_int {
    return (-(((1 as libc::c_int) << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int)
        - 1 as libc::c_int
        >> 1 as libc::c_int) as i64
        <= i
        && i <= (((1 as libc::c_int) << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int)
            - 1 as libc::c_int
            - (((1 as libc::c_int) << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int)
                - 1 as libc::c_int
                >> 1 as libc::c_int)) as i64) as libc::c_int;
}

pub unsafe fn luaK_int(
    ls: *mut LexState,
    fs: *mut FuncState,
    reg: libc::c_int,
    i: i64,
) -> Result<(), ParseError> {
    if fitsBx(i) != 0 {
        codeAsBx(ls, fs, OP_LOADI, reg, i as libc::c_int)?;
    } else {
        luaK_codek(ls, fs, reg, luaK_intK(ls, fs, i)?)?;
    };

    Ok(())
}

unsafe fn luaK_float(
    ls: *mut LexState,
    fs: *mut FuncState,
    reg: libc::c_int,
    f: f64,
) -> Result<(), ParseError> {
    let mut fi: i64 = 0;
    if luaV_flttointeger(f, &mut fi, F2Ieq) != 0 && fitsBx(fi) != 0 {
        codeAsBx(ls, fs, OP_LOADF, reg, fi as libc::c_int)?;
    } else {
        luaK_codek(ls, fs, reg, luaK_numberK(ls, fs, f)?)?;
    };

    Ok(())
}

unsafe fn const2exp(v: *mut UnsafeValue, e: *mut expdesc) {
    match (*v).tt_ as libc::c_int & 0x3f as libc::c_int {
        3 => {
            (*e).k = VKINT;
            (*e).u.ival = (*v).value_.i;
        }
        19 => {
            (*e).k = VKFLT;
            (*e).u.nval = (*v).value_.n;
        }
        1 => {
            (*e).k = VFALSE;
        }
        17 => {
            (*e).k = VTRUE;
        }
        0 => {
            (*e).k = VNIL;
        }
        4 | 20 => {
            (*e).k = VKSTR;
            (*e).u.strval = (*v).value_.gc as *mut Str;
        }
        _ => {}
    };
}

pub unsafe fn luaK_setreturns(
    ls: *mut LexState,
    fs: *mut FuncState,
    e: *mut expdesc,
    nresults: libc::c_int,
) -> Result<(), ParseError> {
    let pc: *mut u32 = &mut *((*(*fs).f).code).offset((*e).u.info as isize) as *mut u32;
    if (*e).k as libc::c_uint == VCALL as libc::c_int as libc::c_uint {
        *pc = *pc
            & !(!(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                << 0 as libc::c_int
                    + 7 as libc::c_int
                    + 8 as libc::c_int
                    + 1 as libc::c_int
                    + 8 as libc::c_int)
            | ((nresults + 1 as libc::c_int) as u32)
                << 0 as libc::c_int
                    + 7 as libc::c_int
                    + 8 as libc::c_int
                    + 1 as libc::c_int
                    + 8 as libc::c_int
                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                    << 0 as libc::c_int
                        + 7 as libc::c_int
                        + 8 as libc::c_int
                        + 1 as libc::c_int
                        + 8 as libc::c_int;
    } else {
        *pc = *pc
            & !(!(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                << 0 as libc::c_int
                    + 7 as libc::c_int
                    + 8 as libc::c_int
                    + 1 as libc::c_int
                    + 8 as libc::c_int)
            | ((nresults + 1 as libc::c_int) as u32)
                << 0 as libc::c_int
                    + 7 as libc::c_int
                    + 8 as libc::c_int
                    + 1 as libc::c_int
                    + 8 as libc::c_int
                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                    << 0 as libc::c_int
                        + 7 as libc::c_int
                        + 8 as libc::c_int
                        + 1 as libc::c_int
                        + 8 as libc::c_int;
        *pc = *pc
            & !(!(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                << 0 as libc::c_int + 7 as libc::c_int)
            | ((*fs).freereg as u32) << 0 as libc::c_int + 7 as libc::c_int
                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                    << 0 as libc::c_int + 7 as libc::c_int;
        luaK_reserveregs(ls, fs, 1 as libc::c_int)?;
    };
    Ok(())
}

unsafe fn str2K(ls: *mut LexState, fs: *mut FuncState, e: *mut expdesc) -> Result<(), ParseError> {
    (*e).u.info = stringK(ls, fs, (*e).u.strval)?;
    (*e).k = VK;
    Ok(())
}

pub unsafe fn luaK_setoneret(fs: *mut FuncState, e: *mut expdesc) {
    if (*e).k as libc::c_uint == VCALL as libc::c_int as libc::c_uint {
        (*e).k = VNONRELOC;
        (*e).u.info = (*((*(*fs).f).code).offset((*e).u.info as isize)
            >> 0 as libc::c_int + 7 as libc::c_int
            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
            as libc::c_int;
    } else if (*e).k as libc::c_uint == VVARARG as libc::c_int as libc::c_uint {
        *((*(*fs).f).code).offset((*e).u.info as isize) = *((*(*fs).f).code)
            .offset((*e).u.info as isize)
            & !(!(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                << 0 as libc::c_int
                    + 7 as libc::c_int
                    + 8 as libc::c_int
                    + 1 as libc::c_int
                    + 8 as libc::c_int)
            | (2 as libc::c_int as u32)
                << 0 as libc::c_int
                    + 7 as libc::c_int
                    + 8 as libc::c_int
                    + 1 as libc::c_int
                    + 8 as libc::c_int
                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                    << 0 as libc::c_int
                        + 7 as libc::c_int
                        + 8 as libc::c_int
                        + 1 as libc::c_int
                        + 8 as libc::c_int;
        (*e).k = VRELOC;
    }
}

pub unsafe fn luaK_dischargevars(
    ls: *mut LexState,
    fs: *mut FuncState,
    e: *mut expdesc,
) -> Result<(), ParseError> {
    match (*e).k as libc::c_uint {
        11 => const2exp(const2val(ls, e), e),
        9 => {
            let temp: libc::c_int = (*e).u.var.ridx as libc::c_int;
            (*e).u.info = temp;
            (*e).k = VNONRELOC;
        }
        10 => {
            (*e).u.info = luaK_codeABCk(
                ls,
                fs,
                OP_GETUPVAL,
                0 as libc::c_int,
                (*e).u.info,
                0 as libc::c_int,
                0 as libc::c_int,
            )?;
            (*e).k = VRELOC;
        }
        13 => {
            (*e).u.info = luaK_codeABCk(
                ls,
                fs,
                OP_GETTABUP,
                0 as libc::c_int,
                (*e).u.ind.t as libc::c_int,
                (*e).u.ind.idx as libc::c_int,
                0 as libc::c_int,
            )?;
            (*e).k = VRELOC;
        }
        14 => {
            freereg(ls, fs, (*e).u.ind.t as libc::c_int);
            (*e).u.info = luaK_codeABCk(
                ls,
                fs,
                OP_GETI,
                0 as libc::c_int,
                (*e).u.ind.t as libc::c_int,
                (*e).u.ind.idx as libc::c_int,
                0 as libc::c_int,
            )?;
            (*e).k = VRELOC;
        }
        15 => {
            freereg(ls, fs, (*e).u.ind.t as libc::c_int);
            (*e).u.info = luaK_codeABCk(
                ls,
                fs,
                OP_GETFIELD,
                0 as libc::c_int,
                (*e).u.ind.t as libc::c_int,
                (*e).u.ind.idx as libc::c_int,
                0 as libc::c_int,
            )?;
            (*e).k = VRELOC;
        }
        12 => {
            freeregs(
                ls,
                fs,
                (*e).u.ind.t as libc::c_int,
                (*e).u.ind.idx as libc::c_int,
            );
            (*e).u.info = luaK_codeABCk(
                ls,
                fs,
                OP_GETTABLE,
                0 as libc::c_int,
                (*e).u.ind.t as libc::c_int,
                (*e).u.ind.idx as libc::c_int,
                0 as libc::c_int,
            )?;
            (*e).k = VRELOC;
        }
        19 | 18 => {
            luaK_setoneret(fs, e);
        }
        _ => {}
    };
    Ok(())
}

unsafe fn discharge2reg(
    ls: *mut LexState,
    fs: *mut FuncState,
    e: *mut expdesc,
    reg: libc::c_int,
) -> Result<(), ParseError> {
    luaK_dischargevars(ls, fs, e)?;
    let current_block_14: u64;
    match (*e).k as libc::c_uint {
        1 => {
            luaK_nil(ls, fs, reg, 1 as libc::c_int)?;
            current_block_14 = 13242334135786603907;
        }
        3 => {
            luaK_codeABCk(
                ls,
                fs,
                OP_LOADFALSE,
                reg,
                0 as libc::c_int,
                0 as libc::c_int,
                0 as libc::c_int,
            )?;
            current_block_14 = 13242334135786603907;
        }
        2 => {
            luaK_codeABCk(
                ls,
                fs,
                OP_LOADTRUE,
                reg,
                0 as libc::c_int,
                0 as libc::c_int,
                0 as libc::c_int,
            )?;
            current_block_14 = 13242334135786603907;
        }
        7 => {
            str2K(ls, fs, e)?;
            current_block_14 = 6937071982253665452;
        }
        4 => {
            current_block_14 = 6937071982253665452;
        }
        5 => {
            luaK_float(ls, fs, reg, (*e).u.nval)?;
            current_block_14 = 13242334135786603907;
        }
        6 => {
            luaK_int(ls, fs, reg, (*e).u.ival)?;
            current_block_14 = 13242334135786603907;
        }
        17 => {
            let pc: *mut u32 = &mut *((*(*fs).f).code).offset((*e).u.info as isize) as *mut u32;
            *pc = *pc
                & !(!(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                    << 0 as libc::c_int + 7 as libc::c_int)
                | (reg as u32) << 0 as libc::c_int + 7 as libc::c_int
                    & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                        << 0 as libc::c_int + 7 as libc::c_int;
            current_block_14 = 13242334135786603907;
        }
        8 => {
            if reg != (*e).u.info {
                luaK_codeABCk(
                    ls,
                    fs,
                    OP_MOVE,
                    reg,
                    (*e).u.info,
                    0 as libc::c_int,
                    0 as libc::c_int,
                )?;
            }
            current_block_14 = 13242334135786603907;
        }
        _ => return Ok(()),
    }
    match current_block_14 {
        6937071982253665452 => {
            luaK_codek(ls, fs, reg, (*e).u.info)?;
        }
        _ => {}
    }
    (*e).u.info = reg;
    (*e).k = VNONRELOC;
    Ok(())
}

unsafe fn discharge2anyreg(
    ls: *mut LexState,
    fs: *mut FuncState,
    e: *mut expdesc,
) -> Result<(), ParseError> {
    if (*e).k as libc::c_uint != VNONRELOC as libc::c_int as libc::c_uint {
        luaK_reserveregs(ls, fs, 1 as libc::c_int)?;
        discharge2reg(ls, fs, e, (*fs).freereg as libc::c_int - 1 as libc::c_int)?;
    }
    Ok(())
}

unsafe fn code_loadbool(
    ls: *mut LexState,
    fs: *mut FuncState,
    A: libc::c_int,
    op: OpCode,
) -> Result<libc::c_int, ParseError> {
    luaK_getlabel(fs);
    return luaK_codeABCk(
        ls,
        fs,
        op,
        A,
        0 as libc::c_int,
        0 as libc::c_int,
        0 as libc::c_int,
    );
}

unsafe fn need_value(fs: *mut FuncState, mut list: libc::c_int) -> libc::c_int {
    while list != -(1 as libc::c_int) {
        let i: u32 = *getjumpcontrol(fs, list);
        if (i >> 0 as libc::c_int
            & !(!(0 as libc::c_int as u32) << 7 as libc::c_int) << 0 as libc::c_int)
            as OpCode as libc::c_uint
            != OP_TESTSET as libc::c_int as libc::c_uint
        {
            return 1 as libc::c_int;
        }
        list = getjump(fs, list);
    }
    return 0 as libc::c_int;
}

unsafe fn exp2reg(
    ls: *mut LexState,
    fs: *mut FuncState,
    e: *mut expdesc,
    reg: libc::c_int,
) -> Result<(), ParseError> {
    discharge2reg(ls, fs, e, reg)?;
    if (*e).k as libc::c_uint == VJMP as libc::c_int as libc::c_uint {
        luaK_concat(ls, fs, &mut (*e).t, (*e).u.info)?;
    }
    if (*e).t != (*e).f {
        let mut final_0: libc::c_int = 0;
        let mut p_f: libc::c_int = -(1 as libc::c_int);
        let mut p_t: libc::c_int = -(1 as libc::c_int);
        if need_value(fs, (*e).t) != 0 || need_value(fs, (*e).f) != 0 {
            let fj: libc::c_int = if (*e).k as libc::c_uint == VJMP as libc::c_int as libc::c_uint {
                -(1 as libc::c_int)
            } else {
                luaK_jump(ls, fs)?
            };
            p_f = code_loadbool(ls, fs, reg, OP_LFALSESKIP)?;
            p_t = code_loadbool(ls, fs, reg, OP_LOADTRUE)?;
            luaK_patchtohere(ls, fs, fj)?;
        }
        final_0 = luaK_getlabel(fs);
        patchlistaux(ls, fs, (*e).f, final_0, reg, p_f)?;
        patchlistaux(ls, fs, (*e).t, final_0, reg, p_t)?;
    }
    (*e).t = -(1 as libc::c_int);
    (*e).f = (*e).t;
    (*e).u.info = reg;
    (*e).k = VNONRELOC;
    Ok(())
}

pub unsafe fn luaK_exp2nextreg(
    ls: *mut LexState,
    fs: *mut FuncState,
    e: *mut expdesc,
) -> Result<(), ParseError> {
    luaK_dischargevars(ls, fs, e)?;
    freeexp(ls, fs, e);
    luaK_reserveregs(ls, fs, 1 as libc::c_int)?;
    exp2reg(ls, fs, e, (*fs).freereg as libc::c_int - 1 as libc::c_int)
}

pub unsafe fn luaK_exp2anyreg(
    ls: *mut LexState,
    fs: *mut FuncState,
    e: *mut expdesc,
) -> Result<libc::c_int, ParseError> {
    luaK_dischargevars(ls, fs, e)?;
    if (*e).k as libc::c_uint == VNONRELOC as libc::c_int as libc::c_uint {
        if !((*e).t != (*e).f) {
            return Ok((*e).u.info);
        }
        if (*e).u.info >= luaY_nvarstack(ls, fs) {
            exp2reg(ls, fs, e, (*e).u.info)?;
            return Ok((*e).u.info);
        }
    }
    luaK_exp2nextreg(ls, fs, e)?;
    return Ok((*e).u.info);
}

pub unsafe fn luaK_exp2anyregup(
    ls: *mut LexState,
    fs: *mut FuncState,
    e: *mut expdesc,
) -> Result<(), ParseError> {
    if (*e).k as libc::c_uint != VUPVAL as libc::c_int as libc::c_uint || (*e).t != (*e).f {
        luaK_exp2anyreg(ls, fs, e)?;
    }
    Ok(())
}

pub unsafe fn luaK_exp2val(
    ls: *mut LexState,
    fs: *mut FuncState,
    e: *mut expdesc,
) -> Result<(), ParseError> {
    if (*e).k == VJMP || (*e).t != (*e).f {
        luaK_exp2anyreg(ls, fs, e)?;
    } else {
        luaK_dischargevars(ls, fs, e)?;
    };
    Ok(())
}

unsafe fn luaK_exp2K(
    ls: *mut LexState,
    fs: *mut FuncState,
    e: *mut expdesc,
) -> Result<libc::c_int, ParseError> {
    if !((*e).t != (*e).f) {
        let mut info: libc::c_int = 0;
        match (*e).k as libc::c_uint {
            2 => info = boolT(ls, fs)?,
            3 => info = boolF(ls, fs)?,
            1 => info = nilK(ls, fs)?,
            6 => info = luaK_intK(ls, fs, (*e).u.ival)?,
            5 => info = luaK_numberK(ls, fs, (*e).u.nval)?,
            7 => info = stringK(ls, fs, (*e).u.strval)?,
            4 => info = (*e).u.info,
            _ => return Ok(0 as libc::c_int),
        }
        if info <= ((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int {
            (*e).k = VK;
            (*e).u.info = info;
            return Ok(1 as libc::c_int);
        }
    }
    return Ok(0 as libc::c_int);
}

unsafe fn exp2RK(
    ls: *mut LexState,
    fs: *mut FuncState,
    e: *mut expdesc,
) -> Result<libc::c_int, ParseError> {
    if luaK_exp2K(ls, fs, e)? != 0 {
        return Ok(1 as libc::c_int);
    } else {
        luaK_exp2anyreg(ls, fs, e)?;
        return Ok(0 as libc::c_int);
    };
}

unsafe fn codeABRK(
    ls: *mut LexState,
    fs: *mut FuncState,
    o: OpCode,
    a: libc::c_int,
    b: libc::c_int,
    ec: *mut expdesc,
) -> Result<(), ParseError> {
    let k: libc::c_int = exp2RK(ls, fs, ec)?;
    luaK_codeABCk(ls, fs, o, a, b, (*ec).u.info, k)?;
    Ok(())
}

pub unsafe fn luaK_storevar(
    ls: *mut LexState,
    fs: *mut FuncState,
    var: *mut expdesc,
    ex: *mut expdesc,
) -> Result<(), ParseError> {
    match (*var).k as libc::c_uint {
        9 => {
            freeexp(ls, fs, ex);
            return exp2reg(ls, fs, ex, (*var).u.var.ridx as libc::c_int);
        }
        10 => {
            let e: libc::c_int = luaK_exp2anyreg(ls, fs, ex)?;
            luaK_codeABCk(
                ls,
                fs,
                OP_SETUPVAL,
                e,
                (*var).u.info,
                0 as libc::c_int,
                0 as libc::c_int,
            )?;
        }
        13 => codeABRK(
            ls,
            fs,
            OP_SETTABUP,
            (*var).u.ind.t as libc::c_int,
            (*var).u.ind.idx as libc::c_int,
            ex,
        )?,
        14 => codeABRK(
            ls,
            fs,
            OP_SETI,
            (*var).u.ind.t as libc::c_int,
            (*var).u.ind.idx as libc::c_int,
            ex,
        )?,
        15 => codeABRK(
            ls,
            fs,
            OP_SETFIELD,
            (*var).u.ind.t as libc::c_int,
            (*var).u.ind.idx as libc::c_int,
            ex,
        )?,
        12 => codeABRK(
            ls,
            fs,
            OP_SETTABLE,
            (*var).u.ind.t as libc::c_int,
            (*var).u.ind.idx as libc::c_int,
            ex,
        )?,
        _ => {}
    }
    freeexp(ls, fs, ex);
    Ok(())
}

pub unsafe fn luaK_self(
    ls: *mut LexState,
    fs: *mut FuncState,
    e: *mut expdesc,
    key: *mut expdesc,
) -> Result<(), ParseError> {
    let mut ereg: libc::c_int = 0;
    luaK_exp2anyreg(ls, fs, e)?;
    ereg = (*e).u.info;
    freeexp(ls, fs, e);
    (*e).u.info = (*fs).freereg as libc::c_int;
    (*e).k = VNONRELOC;
    luaK_reserveregs(ls, fs, 2 as libc::c_int)?;
    codeABRK(ls, fs, OP_SELF, (*e).u.info, ereg, key)?;
    freeexp(ls, fs, key);
    Ok(())
}

unsafe fn negatecondition(fs: *mut FuncState, e: *mut expdesc) {
    let pc: *mut u32 = getjumpcontrol(fs, (*e).u.info);
    *pc = *pc
        & !(!(!(0 as libc::c_int as u32) << 1 as libc::c_int)
            << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int)
        | (((*pc >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int
            & !(!(0 as libc::c_int as u32) << 1 as libc::c_int) << 0 as libc::c_int)
            as libc::c_int
            ^ 1 as libc::c_int) as u32)
            << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int
            & !(!(0 as libc::c_int as u32) << 1 as libc::c_int)
                << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int;
}

unsafe fn jumponcond(
    ls: *mut LexState,
    fs: *mut FuncState,
    e: *mut expdesc,
    cond: libc::c_int,
) -> Result<libc::c_int, ParseError> {
    if (*e).k as libc::c_uint == VRELOC as libc::c_int as libc::c_uint {
        let ie: u32 = *((*(*fs).f).code).offset((*e).u.info as isize);
        if (ie >> 0 as libc::c_int
            & !(!(0 as libc::c_int as u32) << 7 as libc::c_int) << 0 as libc::c_int)
            as OpCode as libc::c_uint
            == OP_NOT as libc::c_int as libc::c_uint
        {
            removelastinstruction(fs);
            return condjump(
                ls,
                fs,
                OP_TEST,
                (ie >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int
                    & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
                    as libc::c_int,
                0 as libc::c_int,
                0 as libc::c_int,
                (cond == 0) as libc::c_int,
            );
        }
    }
    discharge2anyreg(ls, fs, e)?;
    freeexp(ls, fs, e);
    return condjump(
        ls,
        fs,
        OP_TESTSET,
        ((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int,
        (*e).u.info,
        0 as libc::c_int,
        cond,
    );
}

pub unsafe fn luaK_goiftrue(
    ls: *mut LexState,
    fs: *mut FuncState,
    e: *mut expdesc,
) -> Result<(), ParseError> {
    let mut pc: libc::c_int = 0;
    luaK_dischargevars(ls, fs, e)?;
    match (*e).k as libc::c_uint {
        16 => {
            negatecondition(fs, e);
            pc = (*e).u.info;
        }
        4 | 5 | 6 | 7 | 2 => {
            pc = -(1 as libc::c_int);
        }
        _ => pc = jumponcond(ls, fs, e, 0 as libc::c_int)?,
    }
    luaK_concat(ls, fs, &mut (*e).f, pc)?;
    luaK_patchtohere(ls, fs, (*e).t)?;
    (*e).t = -(1 as libc::c_int);
    Ok(())
}

pub unsafe fn luaK_goiffalse(
    ls: *mut LexState,
    fs: *mut FuncState,
    e: *mut expdesc,
) -> Result<(), ParseError> {
    let mut pc: libc::c_int = 0;
    luaK_dischargevars(ls, fs, e)?;
    match (*e).k as libc::c_uint {
        16 => {
            pc = (*e).u.info;
        }
        1 | 3 => {
            pc = -(1 as libc::c_int);
        }
        _ => pc = jumponcond(ls, fs, e, 1 as libc::c_int)?,
    }
    luaK_concat(ls, fs, &mut (*e).t, pc)?;
    luaK_patchtohere(ls, fs, (*e).f)?;
    (*e).f = -(1 as libc::c_int);
    Ok(())
}

unsafe fn codenot(
    ls: *mut LexState,
    fs: *mut FuncState,
    e: *mut expdesc,
) -> Result<(), ParseError> {
    match (*e).k as libc::c_uint {
        1 | 3 => {
            (*e).k = VTRUE;
        }
        4 | 5 | 6 | 7 | 2 => {
            (*e).k = VFALSE;
        }
        16 => {
            negatecondition(fs, e);
        }
        17 | 8 => {
            discharge2anyreg(ls, fs, e)?;
            freeexp(ls, fs, e);
            (*e).u.info = luaK_codeABCk(
                ls,
                fs,
                OP_NOT,
                0 as libc::c_int,
                (*e).u.info,
                0 as libc::c_int,
                0 as libc::c_int,
            )?;
            (*e).k = VRELOC;
        }
        _ => {}
    }
    let temp: libc::c_int = (*e).f;
    (*e).f = (*e).t;
    (*e).t = temp;
    removevalues(fs, (*e).f);
    removevalues(fs, (*e).t);
    Ok(())
}

unsafe fn isKstr(fs: *mut FuncState, e: *mut expdesc) -> libc::c_int {
    return ((*e).k as libc::c_uint == VK as libc::c_int as libc::c_uint
        && !((*e).t != (*e).f)
        && (*e).u.info <= ((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int
        && (*((*(*fs).f).k).offset((*e).u.info as isize)).tt_ as libc::c_int
            == 4 as libc::c_int
                | (0 as libc::c_int) << 4 as libc::c_int
                | (1 as libc::c_int) << 6 as libc::c_int) as libc::c_int;
}

unsafe fn isKint(e: *mut expdesc) -> libc::c_int {
    return ((*e).k as libc::c_uint == VKINT as libc::c_int as libc::c_uint && !((*e).t != (*e).f))
        as libc::c_int;
}

unsafe fn isCint(e: *mut expdesc) -> libc::c_int {
    return (isKint(e) != 0
        && (*e).u.ival as u64
            <= (((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int) as u64)
        as libc::c_int;
}

unsafe fn isSCint(e: *mut expdesc) -> libc::c_int {
    return (isKint(e) != 0 && fitsC((*e).u.ival) != 0) as libc::c_int;
}

unsafe fn isSCnumber(
    e: *mut expdesc,
    pi: *mut libc::c_int,
    isfloat: *mut libc::c_int,
) -> libc::c_int {
    let mut i: i64 = 0;
    if (*e).k as libc::c_uint == VKINT as libc::c_int as libc::c_uint {
        i = (*e).u.ival;
    } else if (*e).k as libc::c_uint == VKFLT as libc::c_int as libc::c_uint
        && luaV_flttointeger((*e).u.nval, &mut i, F2Ieq) != 0
    {
        *isfloat = 1 as libc::c_int;
    } else {
        return 0 as libc::c_int;
    }
    if !((*e).t != (*e).f) && fitsC(i) != 0 {
        *pi = i as libc::c_int
            + (((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int >> 1 as libc::c_int);
        return 1 as libc::c_int;
    } else {
        return 0 as libc::c_int;
    };
}

pub unsafe fn luaK_indexed(
    ls: *mut LexState,
    fs: *mut FuncState,
    t: *mut expdesc,
    k: *mut expdesc,
) -> Result<(), ParseError> {
    if (*k).k as libc::c_uint == VKSTR as libc::c_int as libc::c_uint {
        str2K(ls, fs, k)?;
    }
    if (*t).k as libc::c_uint == VUPVAL as libc::c_int as libc::c_uint && isKstr(fs, k) == 0 {
        luaK_exp2anyreg(ls, fs, t)?;
    }
    if (*t).k as libc::c_uint == VUPVAL as libc::c_int as libc::c_uint {
        let temp: libc::c_int = (*t).u.info;
        (*t).u.ind.t = temp as u8;
        (*t).u.ind.idx = (*k).u.info as libc::c_short;
        (*t).k = VINDEXUP;
    } else {
        (*t).u.ind.t = (if (*t).k as libc::c_uint == VLOCAL as libc::c_int as libc::c_uint {
            (*t).u.var.ridx as libc::c_int
        } else {
            (*t).u.info
        }) as u8;
        if isKstr(fs, k) != 0 {
            (*t).u.ind.idx = (*k).u.info as libc::c_short;
            (*t).k = VINDEXSTR;
        } else if isCint(k) != 0 {
            (*t).u.ind.idx = (*k).u.ival as libc::c_int as libc::c_short;
            (*t).k = VINDEXI;
        } else {
            (*t).u.ind.idx = luaK_exp2anyreg(ls, fs, k)? as libc::c_short;
            (*t).k = VINDEXED;
        }
    };
    Ok(())
}

unsafe fn validop(op: Ops, v1: *mut UnsafeValue, v2: *mut UnsafeValue) -> libc::c_int {
    match op {
        Ops::And | Ops::Or | Ops::Xor | Ops::Shl | Ops::Shr | Ops::Not => {
            let mut i: i64 = 0;

            (luaV_tointegerns(v1, &mut i, F2Ieq) != 0 && luaV_tointegerns(v2, &mut i, F2Ieq) != 0)
                as libc::c_int
        }
        Ops::NumDiv | Ops::IntDiv | Ops::Mod => {
            return ((if (*v2).tt_ as libc::c_int
                == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
            {
                (*v2).value_.i as f64
            } else {
                (*v2).value_.n
            }) != 0 as libc::c_int as f64) as libc::c_int;
        }
        _ => 1,
    }
}

unsafe fn constfolding(
    op: Ops,
    e1: *mut expdesc,
    e2: *const expdesc,
) -> Result<libc::c_int, ArithError> {
    let mut v1: UnsafeValue = UnsafeValue {
        value_: UntaggedValue {
            gc: 0 as *mut Object,
        },
        tt_: 0,
    };
    let mut v2: UnsafeValue = UnsafeValue {
        value_: UntaggedValue {
            gc: 0 as *mut Object,
        },
        tt_: 0,
    };

    if tonumeral(e1, &mut v1) == 0
        || tonumeral(e2, &mut v2) == 0
        || validop(op, &mut v1, &mut v2) == 0
    {
        return Ok(0 as libc::c_int);
    }

    let res = match luaO_rawarith(op, &mut v1, &mut v2)? {
        Some(v) => v,
        None => return Ok(0),
    };

    if res.tt_ as libc::c_int == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int {
        (*e1).k = VKINT;
        (*e1).u.ival = res.value_.i;
    } else {
        (*e1).k = VKFLT;
        (*e1).u.nval = res.value_.n;
    }

    return Ok(1 as libc::c_int);
}

unsafe fn binopr2op(opr: BinOpr, baser: BinOpr, base: OpCode) -> OpCode {
    return (opr as libc::c_int - baser as libc::c_int + base as libc::c_int) as OpCode;
}

unsafe fn unopr2op(opr: UnOpr) -> OpCode {
    return (opr as libc::c_int - OPR_MINUS as libc::c_int + OP_UNM as libc::c_int) as OpCode;
}

unsafe fn binopr2TM(opr: BinOpr) -> TMS {
    return (opr as libc::c_int - OPR_ADD as libc::c_int + TM_ADD as libc::c_int) as TMS;
}

unsafe fn codeunexpval(
    ls: *mut LexState,
    fs: *mut FuncState,
    op: OpCode,
    e: *mut expdesc,
    line: libc::c_int,
) -> Result<(), ParseError> {
    let r: libc::c_int = luaK_exp2anyreg(ls, fs, e)?;
    freeexp(ls, fs, e);
    (*e).u.info = luaK_codeABCk(
        ls,
        fs,
        op,
        0 as libc::c_int,
        r,
        0 as libc::c_int,
        0 as libc::c_int,
    )?;
    (*e).k = VRELOC;

    luaK_fixline(ls, fs, line)
}

unsafe fn finishbinexpval(
    ls: *mut LexState,
    fs: *mut FuncState,
    e1: *mut expdesc,
    e2: *mut expdesc,
    op: OpCode,
    v2: libc::c_int,
    flip: libc::c_int,
    line: libc::c_int,
    mmop: OpCode,
    event: TMS,
) -> Result<(), ParseError> {
    let v1: libc::c_int = luaK_exp2anyreg(ls, fs, e1)?;
    let pc: libc::c_int = luaK_codeABCk(ls, fs, op, 0 as libc::c_int, v1, v2, 0 as libc::c_int)?;
    freeexps(ls, fs, e1, e2);
    (*e1).u.info = pc;
    (*e1).k = VRELOC;
    luaK_fixline(ls, fs, line)?;
    luaK_codeABCk(ls, fs, mmop, v1, v2, event as libc::c_int, flip)?;
    luaK_fixline(ls, fs, line)
}

unsafe fn codebinexpval(
    ls: *mut LexState,
    fs: *mut FuncState,
    opr: BinOpr,
    e1: *mut expdesc,
    e2: *mut expdesc,
    line: libc::c_int,
) -> Result<(), ParseError> {
    let op: OpCode = binopr2op(opr, OPR_ADD, OP_ADD);
    let v2: libc::c_int = luaK_exp2anyreg(ls, fs, e2)?;
    finishbinexpval(
        ls,
        fs,
        e1,
        e2,
        op,
        v2,
        0 as libc::c_int,
        line,
        OP_MMBIN,
        binopr2TM(opr),
    )
}

unsafe fn codebini(
    ls: *mut LexState,
    fs: *mut FuncState,
    op: OpCode,
    e1: *mut expdesc,
    e2: *mut expdesc,
    flip: libc::c_int,
    line: libc::c_int,
    event: TMS,
) -> Result<(), ParseError> {
    let v2: libc::c_int = (*e2).u.ival as libc::c_int
        + (((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int >> 1 as libc::c_int);
    finishbinexpval(ls, fs, e1, e2, op, v2, flip, line, OP_MMBINI, event)
}

unsafe fn codebinK(
    ls: *mut LexState,
    fs: *mut FuncState,
    opr: BinOpr,
    e1: *mut expdesc,
    e2: *mut expdesc,
    flip: libc::c_int,
    line: libc::c_int,
) -> Result<(), ParseError> {
    let event: TMS = binopr2TM(opr);
    let v2: libc::c_int = (*e2).u.info;
    let op: OpCode = binopr2op(opr, OPR_ADD, OP_ADDK);
    finishbinexpval(ls, fs, e1, e2, op, v2, flip, line, OP_MMBINK, event)
}

unsafe fn finishbinexpneg(
    ls: *mut LexState,
    fs: *mut FuncState,
    e1: *mut expdesc,
    e2: *mut expdesc,
    op: OpCode,
    line: libc::c_int,
    event: TMS,
) -> Result<libc::c_int, ParseError> {
    if isKint(e2) == 0 {
        return Ok(0 as libc::c_int);
    } else {
        let i2: i64 = (*e2).u.ival;
        if !(fitsC(i2) != 0 && fitsC(-i2) != 0) {
            return Ok(0 as libc::c_int);
        } else {
            let v2: libc::c_int = i2 as libc::c_int;
            finishbinexpval(
                ls,
                fs,
                e1,
                e2,
                op,
                -v2 + (((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int
                    >> 1 as libc::c_int),
                0 as libc::c_int,
                line,
                OP_MMBINI,
                event,
            )?;
            *((*(*fs).f).code).offset(((*fs).pc - 1 as libc::c_int) as isize) = *((*(*fs).f).code)
                .offset(((*fs).pc - 1 as libc::c_int) as isize)
                & !(!(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                    << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int)
                | ((v2
                    + (((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int
                        >> 1 as libc::c_int)) as u32)
                    << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int
                    & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                        << 0 as libc::c_int
                            + 7 as libc::c_int
                            + 8 as libc::c_int
                            + 1 as libc::c_int;
            return Ok(1 as libc::c_int);
        }
    };
}

unsafe fn swapexps(e1: *mut expdesc, e2: *mut expdesc) {
    let temp: expdesc = *e1;
    *e1 = *e2;
    *e2 = temp;
}

unsafe fn codebinNoK(
    ls: *mut LexState,
    fs: *mut FuncState,
    opr: BinOpr,
    e1: *mut expdesc,
    e2: *mut expdesc,
    flip: libc::c_int,
    line: libc::c_int,
) -> Result<(), ParseError> {
    if flip != 0 {
        swapexps(e1, e2);
    }
    codebinexpval(ls, fs, opr, e1, e2, line)
}

unsafe fn codearith(
    ls: *mut LexState,
    fs: *mut FuncState,
    opr: BinOpr,
    e1: *mut expdesc,
    e2: *mut expdesc,
    flip: libc::c_int,
    line: libc::c_int,
) -> Result<(), ParseError> {
    if tonumeral(e2, 0 as *mut UnsafeValue) != 0 && luaK_exp2K(ls, fs, e2)? != 0 {
        codebinK(ls, fs, opr, e1, e2, flip, line)
    } else {
        codebinNoK(ls, fs, opr, e1, e2, flip, line)
    }
}

unsafe fn codecommutative(
    ls: *mut LexState,
    fs: *mut FuncState,
    op: BinOpr,
    e1: *mut expdesc,
    e2: *mut expdesc,
    line: libc::c_int,
) -> Result<(), ParseError> {
    let mut flip: libc::c_int = 0 as libc::c_int;
    if tonumeral(e1, 0 as *mut UnsafeValue) != 0 {
        swapexps(e1, e2);
        flip = 1 as libc::c_int;
    }

    if op as libc::c_uint == OPR_ADD as libc::c_int as libc::c_uint && isSCint(e2) != 0 {
        codebini(ls, fs, OP_ADDI, e1, e2, flip, line, TM_ADD)
    } else {
        codearith(ls, fs, op, e1, e2, flip, line)
    }
}

unsafe fn codebitwise(
    ls: *mut LexState,
    fs: *mut FuncState,
    opr: BinOpr,
    e1: *mut expdesc,
    e2: *mut expdesc,
    line: libc::c_int,
) -> Result<(), ParseError> {
    let mut flip: libc::c_int = 0 as libc::c_int;
    if (*e1).k as libc::c_uint == VKINT as libc::c_int as libc::c_uint {
        swapexps(e1, e2);
        flip = 1 as libc::c_int;
    }

    if (*e2).k == VKINT && luaK_exp2K(ls, fs, e2)? != 0 {
        codebinK(ls, fs, opr, e1, e2, flip, line)
    } else {
        codebinNoK(ls, fs, opr, e1, e2, flip, line)
    }
}

unsafe fn codeorder(
    ls: *mut LexState,
    fs: *mut FuncState,
    opr: BinOpr,
    e1: *mut expdesc,
    e2: *mut expdesc,
) -> Result<(), ParseError> {
    let mut r1: libc::c_int = 0;
    let mut r2: libc::c_int = 0;
    let mut im: libc::c_int = 0;
    let mut isfloat: libc::c_int = 0 as libc::c_int;
    let mut op: OpCode = OP_MOVE;
    if isSCnumber(e2, &mut im, &mut isfloat) != 0 {
        r1 = luaK_exp2anyreg(ls, fs, e1)?;
        r2 = im;
        op = binopr2op(opr, OPR_LT, OP_LTI);
    } else if isSCnumber(e1, &mut im, &mut isfloat) != 0 {
        r1 = luaK_exp2anyreg(ls, fs, e2)?;
        r2 = im;
        op = binopr2op(opr, OPR_LT, OP_GTI);
    } else {
        r1 = luaK_exp2anyreg(ls, fs, e1)?;
        r2 = luaK_exp2anyreg(ls, fs, e2)?;
        op = binopr2op(opr, OPR_LT, OP_LT);
    }
    freeexps(ls, fs, e1, e2);
    (*e1).u.info = condjump(ls, fs, op, r1, r2, isfloat, 1 as libc::c_int)?;
    (*e1).k = VJMP;
    Ok(())
}

unsafe fn codeeq(
    ls: *mut LexState,
    fs: *mut FuncState,
    opr: BinOpr,
    e1: *mut expdesc,
    e2: *mut expdesc,
) -> Result<(), ParseError> {
    let mut r1: libc::c_int = 0;
    let mut r2: libc::c_int = 0;
    let mut im: libc::c_int = 0;
    let mut isfloat: libc::c_int = 0 as libc::c_int;
    let mut op: OpCode = OP_MOVE;
    if (*e1).k as libc::c_uint != VNONRELOC as libc::c_int as libc::c_uint {
        swapexps(e1, e2);
    }
    r1 = luaK_exp2anyreg(ls, fs, e1)?;
    if isSCnumber(e2, &mut im, &mut isfloat) != 0 {
        op = OP_EQI;
        r2 = im;
    } else if exp2RK(ls, fs, e2)? != 0 {
        op = OP_EQK;
        r2 = (*e2).u.info;
    } else {
        op = OP_EQ;
        r2 = luaK_exp2anyreg(ls, fs, e2)?;
    }
    freeexps(ls, fs, e1, e2);
    (*e1).u.info = condjump(
        ls,
        fs,
        op,
        r1,
        r2,
        isfloat,
        (opr as libc::c_uint == OPR_EQ as libc::c_int as libc::c_uint) as libc::c_int,
    )?;
    (*e1).k = VJMP;
    Ok(())
}

pub unsafe fn luaK_prefix(
    ls: *mut LexState,
    fs: *mut FuncState,
    opr: UnOpr,
    e: *mut expdesc,
    line: libc::c_int,
) -> Result<(), ParseError> {
    static mut ef: expdesc = {
        let init = expdesc {
            k: VKINT,
            u: C2RustUnnamed_11 {
                ival: 0 as libc::c_int as i64,
            },
            t: -(1 as libc::c_int),
            f: -(1 as libc::c_int),
        };
        init
    };
    luaK_dischargevars(ls, fs, e)?;
    let current_block_3: u64;
    match opr as libc::c_uint {
        0 | 1 => {
            let opr = (opr + 12).try_into().ok().and_then(Ops::from_u8).unwrap();

            if constfolding(opr, e, &raw mut ef).map_err(|e| luaK_semerror(ls, e))? != 0 {
                current_block_3 = 7815301370352969686;
            } else {
                current_block_3 = 4299225766812711900;
            }
        }
        3 => {
            current_block_3 = 4299225766812711900;
        }
        2 => {
            codenot(ls, fs, e)?;
            current_block_3 = 7815301370352969686;
        }
        _ => {
            current_block_3 = 7815301370352969686;
        }
    }
    match current_block_3 {
        4299225766812711900 => {
            codeunexpval(ls, fs, unopr2op(opr), e, line)?;
        }
        _ => {}
    };
    Ok(())
}

pub unsafe fn luaK_infix(
    ls: *mut LexState,
    fs: *mut FuncState,
    op: BinOpr,
    v: *mut expdesc,
) -> Result<(), ParseError> {
    luaK_dischargevars(ls, fs, v)?;
    match op as libc::c_uint {
        19 => luaK_goiftrue(ls, fs, v)?,
        20 => luaK_goiffalse(ls, fs, v)?,
        12 => luaK_exp2nextreg(ls, fs, v)?,
        0 | 1 | 2 | 5 | 6 | 3 | 4 | 7 | 8 | 9 | 10 | 11 => {
            if tonumeral(v, 0 as *mut UnsafeValue) == 0 {
                luaK_exp2anyreg(ls, fs, v)?;
            }
        }
        13 | 16 => {
            if tonumeral(v, 0 as *mut UnsafeValue) == 0 {
                exp2RK(ls, fs, v)?;
            }
        }
        14 | 15 | 17 | 18 => {
            let mut dummy: libc::c_int = 0;
            let mut dummy2: libc::c_int = 0;
            if isSCnumber(v, &mut dummy, &mut dummy2) == 0 {
                luaK_exp2anyreg(ls, fs, v)?;
            }
        }
        _ => {}
    };
    Ok(())
}

unsafe fn codeconcat(
    ls: *mut LexState,
    fs: *mut FuncState,
    e1: *mut expdesc,
    e2: *mut expdesc,
    line: libc::c_int,
) -> Result<(), ParseError> {
    let ie2: *mut u32 = previousinstruction(fs);
    if (*ie2 >> 0 as libc::c_int
        & !(!(0 as libc::c_int as u32) << 7 as libc::c_int) << 0 as libc::c_int) as OpCode
        as libc::c_uint
        == OP_CONCAT as libc::c_int as libc::c_uint
    {
        let n: libc::c_int = (*ie2
            >> 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int
            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
            as libc::c_int;
        freeexp(ls, fs, e2);
        *ie2 = *ie2
            & !(!(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                << 0 as libc::c_int + 7 as libc::c_int)
            | ((*e1).u.info as u32) << 0 as libc::c_int + 7 as libc::c_int
                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                    << 0 as libc::c_int + 7 as libc::c_int;
        *ie2 = *ie2
            & !(!(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int)
            | ((n + 1 as libc::c_int) as u32)
                << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int
                & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                    << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int;
    } else {
        luaK_codeABCk(
            ls,
            fs,
            OP_CONCAT,
            (*e1).u.info,
            2 as libc::c_int,
            0 as libc::c_int,
            0 as libc::c_int,
        )?;
        freeexp(ls, fs, e2);
        luaK_fixline(ls, fs, line)?;
    };
    Ok(())
}

pub unsafe fn luaK_posfix(
    ls: *mut LexState,
    fs: *mut FuncState,
    mut opr: BinOpr,
    e1: *mut expdesc,
    e2: *mut expdesc,
    line: libc::c_int,
) -> Result<(), ParseError> {
    luaK_dischargevars(ls, fs, e2)?;

    if opr <= OPR_SHR
        && constfolding(opr.try_into().ok().and_then(Ops::from_u8).unwrap(), e1, e2)
            .map_err(|e| luaK_semerror(ls, e))?
            != 0
    {
        return Ok(());
    }

    let current_block_30: u64;
    match opr as libc::c_uint {
        19 => {
            luaK_concat(ls, fs, &mut (*e2).f, (*e1).f)?;
            *e1 = *e2;
            current_block_30 = 8180496224585318153;
        }
        20 => {
            luaK_concat(ls, fs, &mut (*e2).t, (*e1).t)?;
            *e1 = *e2;
            current_block_30 = 8180496224585318153;
        }
        12 => {
            luaK_exp2nextreg(ls, fs, e2)?;
            codeconcat(ls, fs, e1, e2, line)?;
            current_block_30 = 8180496224585318153;
        }
        0 | 2 => {
            codecommutative(ls, fs, opr, e1, e2, line)?;
            current_block_30 = 8180496224585318153;
        }
        1 => {
            if finishbinexpneg(ls, fs, e1, e2, OP_ADDI, line, TM_SUB)? != 0 {
                current_block_30 = 8180496224585318153;
            } else {
                current_block_30 = 12599329904712511516;
            }
        }
        5 | 6 | 3 | 4 => {
            current_block_30 = 12599329904712511516;
        }
        7 | 8 | 9 => {
            codebitwise(ls, fs, opr, e1, e2, line)?;
            current_block_30 = 8180496224585318153;
        }
        10 => {
            if isSCint(e1) != 0 {
                swapexps(e1, e2);
                codebini(ls, fs, OP_SHLI, e1, e2, 1 as libc::c_int, line, TM_SHL)?;
            } else if !(finishbinexpneg(ls, fs, e1, e2, OP_SHRI, line, TM_SHL)? != 0) {
                codebinexpval(ls, fs, opr, e1, e2, line)?;
            }
            current_block_30 = 8180496224585318153;
        }
        11 => {
            if isSCint(e2) != 0 {
                codebini(ls, fs, OP_SHRI, e1, e2, 0 as libc::c_int, line, TM_SHR)?;
            } else {
                codebinexpval(ls, fs, opr, e1, e2, line)?;
            }
            current_block_30 = 8180496224585318153;
        }
        13 | 16 => {
            codeeq(ls, fs, opr, e1, e2)?;
            current_block_30 = 8180496224585318153;
        }
        17 | 18 => {
            swapexps(e1, e2);
            opr = (opr as libc::c_uint)
                .wrapping_sub(OPR_GT as libc::c_int as libc::c_uint)
                .wrapping_add(OPR_LT as libc::c_int as libc::c_uint) as BinOpr;
            current_block_30 = 1118134448028020070;
        }
        14 | 15 => {
            current_block_30 = 1118134448028020070;
        }
        _ => {
            current_block_30 = 8180496224585318153;
        }
    }
    match current_block_30 {
        12599329904712511516 => {
            codearith(ls, fs, opr, e1, e2, 0 as libc::c_int, line)?;
        }
        1118134448028020070 => {
            codeorder(ls, fs, opr, e1, e2)?;
        }
        _ => {}
    };
    Ok(())
}

pub unsafe fn luaK_fixline(
    ls: *mut LexState,
    fs: *mut FuncState,
    line: libc::c_int,
) -> Result<(), ParseError> {
    removelastlineinfo(fs);
    savelineinfo(ls, fs, (*fs).f, line)
}

pub unsafe fn luaK_settablesize(
    fs: *mut FuncState,
    pc: libc::c_int,
    ra: libc::c_int,
    asize: libc::c_int,
    hsize: libc::c_int,
) {
    let inst: *mut u32 = &mut *((*(*fs).f).code).offset(pc as isize) as *mut u32;
    let rb: libc::c_int = if hsize != 0 as libc::c_int {
        luaO_ceillog2(hsize as libc::c_uint) + 1 as libc::c_int
    } else {
        0 as libc::c_int
    };
    let extra: libc::c_int =
        asize / (((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int + 1 as libc::c_int);
    let rc: libc::c_int =
        asize % (((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int + 1 as libc::c_int);
    let k: libc::c_int = (extra > 0 as libc::c_int) as libc::c_int;
    *inst = (OP_NEWTABLE as libc::c_int as u32) << 0 as libc::c_int
        | (ra as u32) << 0 as libc::c_int + 7 as libc::c_int
        | (rb as u32) << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int
        | (rc as u32)
            << 0 as libc::c_int
                + 7 as libc::c_int
                + 8 as libc::c_int
                + 1 as libc::c_int
                + 8 as libc::c_int
        | (k as u32) << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int;
    *inst.offset(1 as libc::c_int as isize) = (OP_EXTRAARG as libc::c_int as u32)
        << 0 as libc::c_int
        | (extra as u32) << 0 as libc::c_int + 7 as libc::c_int;
}

pub unsafe fn luaK_setlist(
    ls: *mut LexState,
    fs: *mut FuncState,
    base: libc::c_int,
    mut nelems: libc::c_int,
    mut tostore: libc::c_int,
) -> Result<(), ParseError> {
    if tostore == -(1 as libc::c_int) {
        tostore = 0 as libc::c_int;
    }
    if nelems <= ((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int {
        luaK_codeABCk(ls, fs, OP_SETLIST, base, tostore, nelems, 0 as libc::c_int)?;
    } else {
        let extra: libc::c_int = nelems
            / (((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int + 1 as libc::c_int);
        nelems %= ((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int + 1 as libc::c_int;
        luaK_codeABCk(ls, fs, OP_SETLIST, base, tostore, nelems, 1 as libc::c_int)?;
        codeextraarg(ls, fs, extra)?;
    }
    (*fs).freereg = (base + 1 as libc::c_int) as u8;
    Ok(())
}

unsafe fn finaltarget(code: *mut u32, mut i: libc::c_int) -> libc::c_int {
    let mut count: libc::c_int = 0;
    count = 0 as libc::c_int;
    while count < 100 as libc::c_int {
        let pc: u32 = *code.offset(i as isize);
        if (pc >> 0 as libc::c_int
            & !(!(0 as libc::c_int as u32) << 7 as libc::c_int) << 0 as libc::c_int)
            as OpCode as libc::c_uint
            != OP_JMP as libc::c_int as libc::c_uint
        {
            break;
        }
        i += (pc >> 0 as libc::c_int + 7 as libc::c_int
            & !(!(0 as libc::c_int as u32)
                << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int + 8 as libc::c_int)
                << 0 as libc::c_int) as libc::c_int
            - (((1 as libc::c_int)
                << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int + 8 as libc::c_int)
                - 1 as libc::c_int
                >> 1 as libc::c_int)
            + 1 as libc::c_int;
        count += 1;
    }
    return i;
}

pub unsafe fn luaK_finish(ls: *mut LexState, fs: *mut FuncState) -> Result<(), ParseError> {
    let mut i: libc::c_int = 0;
    let p: *mut Proto = (*fs).f;
    i = 0 as libc::c_int;
    while i < (*fs).pc {
        let pc: *mut u32 = &mut *((*p).code).offset(i as isize) as *mut u32;
        let current_block_7: u64;
        match (*pc >> 0 as libc::c_int
            & !(!(0 as libc::c_int as u32) << 7 as libc::c_int) << 0 as libc::c_int)
            as OpCode as libc::c_uint
        {
            71 | 72 => {
                if !((*fs).needclose as libc::c_int != 0 || (*p).is_vararg as libc::c_int != 0) {
                    current_block_7 = 12599329904712511516;
                } else {
                    *pc = *pc
                        & !(!(!(0 as libc::c_int as u32) << 7 as libc::c_int) << 0 as libc::c_int)
                        | (OP_RETURN as libc::c_int as u32) << 0 as libc::c_int
                            & !(!(0 as libc::c_int as u32) << 7 as libc::c_int) << 0 as libc::c_int;
                    current_block_7 = 11006700562992250127;
                }
            }
            70 | 69 => {
                current_block_7 = 11006700562992250127;
            }
            56 => {
                let target: libc::c_int = finaltarget((*p).code, i);
                fixjump(ls, fs, i, target)?;
                current_block_7 = 12599329904712511516;
            }
            _ => {
                current_block_7 = 12599329904712511516;
            }
        }
        match current_block_7 {
            11006700562992250127 => {
                if (*fs).needclose != 0 {
                    *pc = *pc
                        & !(!(!(0 as libc::c_int as u32) << 1 as libc::c_int)
                            << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int)
                        | (1 as libc::c_int as u32)
                            << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int
                            & !(!(0 as libc::c_int as u32) << 1 as libc::c_int)
                                << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int;
                }
                if (*p).is_vararg != 0 {
                    *pc = *pc
                        & !(!(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                            << 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int)
                        | (((*p).numparams as libc::c_int + 1 as libc::c_int) as u32)
                            << 0 as libc::c_int
                                + 7 as libc::c_int
                                + 8 as libc::c_int
                                + 1 as libc::c_int
                                + 8 as libc::c_int
                            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                                << 0 as libc::c_int
                                    + 7 as libc::c_int
                                    + 8 as libc::c_int
                                    + 1 as libc::c_int
                                    + 8 as libc::c_int;
                }
            }
            _ => {}
        }
        i += 1;
    }
    Ok(())
}
