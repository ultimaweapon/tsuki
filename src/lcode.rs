#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments
)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::gc::luaC_barrier_;
use crate::llex::{LexState, luaX_syntaxerror};
use crate::lmem::luaM_growaux_;
use crate::lobject::{
    AbsLineInfo, Proto, TString, UnsafeValue, UntaggedValue, luaO_ceillog2, luaO_rawarith,
};
use crate::lopcodes::{
    OP_ADD, OP_ADDI, OP_ADDK, OP_CONCAT, OP_EQ, OP_EQI, OP_EQK, OP_EXTRAARG, OP_GETFIELD, OP_GETI,
    OP_GETTABLE, OP_GETTABUP, OP_GETUPVAL, OP_GTI, OP_JMP, OP_LFALSESKIP, OP_LOADF, OP_LOADFALSE,
    OP_LOADI, OP_LOADK, OP_LOADKX, OP_LOADNIL, OP_LOADTRUE, OP_LT, OP_LTI, OP_MMBIN, OP_MMBINI,
    OP_MMBINK, OP_MOVE, OP_NEWTABLE, OP_NOT, OP_RETURN, OP_RETURN0, OP_RETURN1, OP_SELF,
    OP_SETFIELD, OP_SETI, OP_SETLIST, OP_SETTABLE, OP_SETTABUP, OP_SETUPVAL, OP_SHLI, OP_SHRI,
    OP_TEST, OP_TESTSET, OP_UNM, OpCode, luaP_opmodes,
};
use crate::lparser::{
    C2RustUnnamed_11, FuncState, VCALL, VFALSE, VINDEXED, VINDEXI, VINDEXSTR, VINDEXUP, VJMP, VK,
    VKFLT, VKINT, VKSTR, VLOCAL, VNIL, VNONRELOC, VRELOC, VTRUE, VUPVAL, VVARARG, expdesc,
    luaY_nvarstack,
};
use crate::ltm::{TM_ADD, TM_SHL, TM_SHR, TM_SUB, TMS};
use crate::lvm::{F2Ieq, luaV_equalobj, luaV_flttointeger, luaV_tointegerns};
use crate::table::{luaH_finishset, luaH_get};
use crate::{ArithError, Object, ParseError, Thread};
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

unsafe fn const2val(fs: *mut FuncState, e: *const expdesc) -> *mut UnsafeValue {
    return &mut (*((*(*(*fs).ls).dyd).actvar.arr).offset((*e).u.info as isize)).k;
}

pub unsafe fn luaK_exp2const(
    fs: *mut FuncState,
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
            let x_: *mut TString = (*e).u.strval;
            (*io).value_.gc = x_ as *mut Object;
            (*io).tt_ =
                ((*x_).hdr.tt as libc::c_int | (1 as libc::c_int) << 6 as libc::c_int) as u8;
            return 1 as libc::c_int;
        }
        11 => {
            let io1: *mut UnsafeValue = v;
            let io2: *const UnsafeValue = const2val(fs, e);
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
        return Err(luaX_syntaxerror((*fs).ls, "control structure too long"));
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
        fixjump(fs, list, l2)?;
    };
    Ok(())
}

pub unsafe fn luaK_jump(fs: *mut FuncState) -> Result<libc::c_int, ParseError> {
    return codesJ(fs, OP_JMP, -(1 as libc::c_int), 0 as libc::c_int);
}

pub unsafe fn luaK_ret(
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
    fs: *mut FuncState,
    op: OpCode,
    A: libc::c_int,
    B: libc::c_int,
    C: libc::c_int,
    k: libc::c_int,
) -> Result<libc::c_int, ParseError> {
    luaK_codeABCk(fs, op, A, B, C, k)?;
    return luaK_jump(fs);
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
    fs: *mut FuncState,
    mut list: libc::c_int,
    vtarget: libc::c_int,
    reg: libc::c_int,
    dtarget: libc::c_int,
) -> Result<(), ParseError> {
    while list != -(1 as libc::c_int) {
        let next: libc::c_int = getjump(fs, list);
        if patchtestreg(fs, list, reg) != 0 {
            fixjump(fs, list, vtarget)?;
        } else {
            fixjump(fs, list, dtarget)?;
        }
        list = next;
    }
    Ok(())
}

pub unsafe fn luaK_patchlist(
    fs: *mut FuncState,
    list: libc::c_int,
    target: libc::c_int,
) -> Result<(), ParseError> {
    patchlistaux(
        fs,
        list,
        target,
        ((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int,
        target,
    )
}

pub unsafe fn luaK_patchtohere(fs: *mut FuncState, list: libc::c_int) -> Result<(), ParseError> {
    let hr: libc::c_int = luaK_getlabel(fs);
    luaK_patchlist(fs, list, hr)
}

unsafe fn savelineinfo(
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
            &(*(*fs).ls).g,
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
        )? as *mut AbsLineInfo;
        (*((*f).abslineinfo).offset((*fs).nabslineinfo as isize)).pc = pc;
        let fresh1 = (*fs).nabslineinfo;
        (*fs).nabslineinfo = (*fs).nabslineinfo + 1;
        (*((*f).abslineinfo).offset(fresh1 as isize)).line = line;
        linedif = -(0x80 as libc::c_int);
        (*fs).iwthabs = 1 as libc::c_int as u8;
    }
    (*f).lineinfo = luaM_growaux_(
        &(*(*fs).ls).g,
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

pub unsafe fn luaK_code(fs: *mut FuncState, i: u32) -> Result<libc::c_int, ParseError> {
    let f: *mut Proto = (*fs).f;
    (*f).code = luaM_growaux_(
        &(*(*fs).ls).g,
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
    )? as *mut u32;
    let fresh2 = (*fs).pc;
    (*fs).pc = (*fs).pc + 1;
    *((*f).code).offset(fresh2 as isize) = i;
    savelineinfo(fs, f, (*(*fs).ls).lastline)?;
    return Ok((*fs).pc - 1 as libc::c_int);
}

pub unsafe fn luaK_codeABCk(
    fs: *mut FuncState,
    o: OpCode,
    a: libc::c_int,
    b: libc::c_int,
    c: libc::c_int,
    k: libc::c_int,
) -> Result<libc::c_int, ParseError> {
    luaK_code(
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
    fs: *mut FuncState,
    o: OpCode,
    a: libc::c_int,
    bc: libc::c_uint,
) -> Result<libc::c_int, ParseError> {
    return luaK_code(
        fs,
        (o as u32) << 0 as libc::c_int
            | (a as u32) << 0 as libc::c_int + 7 as libc::c_int
            | bc << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int,
    );
}

unsafe fn codeAsBx(
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
        fs,
        (o as u32) << 0 as libc::c_int
            | (a as u32) << 0 as libc::c_int + 7 as libc::c_int
            | b << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int,
    );
}

unsafe fn codesJ(
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
        fs,
        (o as u32) << 0 as libc::c_int
            | j << 0 as libc::c_int + 7 as libc::c_int
            | (k as u32) << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int,
    );
}

unsafe fn codeextraarg(fs: *mut FuncState, a: libc::c_int) -> Result<libc::c_int, ParseError> {
    return luaK_code(
        fs,
        (OP_EXTRAARG as libc::c_int as u32) << 0 as libc::c_int
            | (a as u32) << 0 as libc::c_int + 7 as libc::c_int,
    );
}

unsafe fn luaK_codek(
    fs: *mut FuncState,
    reg: libc::c_int,
    k: libc::c_int,
) -> Result<libc::c_int, ParseError> {
    if k <= ((1 as libc::c_int) << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int)
        - 1 as libc::c_int
    {
        return luaK_codeABx(fs, OP_LOADK, reg, k as libc::c_uint);
    } else {
        let p: libc::c_int = luaK_codeABx(fs, OP_LOADKX, reg, 0 as libc::c_int as libc::c_uint)?;
        codeextraarg(fs, k)?;
        return Ok(p);
    };
}

pub unsafe fn luaK_checkstack(fs: *mut FuncState, n: libc::c_int) -> Result<(), ParseError> {
    let newstack: libc::c_int = (*fs).freereg as libc::c_int + n;
    if newstack > (*(*fs).f).maxstacksize as libc::c_int {
        if newstack >= 255 as libc::c_int {
            return Err(luaX_syntaxerror(
                (*fs).ls,
                "function or expression needs too many registers",
            ));
        }
        (*(*fs).f).maxstacksize = newstack as u8;
    }
    Ok(())
}

pub unsafe fn luaK_reserveregs(fs: *mut FuncState, n: libc::c_int) -> Result<(), ParseError> {
    luaK_checkstack(fs, n)?;
    (*fs).freereg = ((*fs).freereg as libc::c_int + n) as u8;
    Ok(())
}

unsafe fn freereg(fs: *mut FuncState, reg: libc::c_int) {
    if reg >= luaY_nvarstack(fs) {
        (*fs).freereg = ((*fs).freereg).wrapping_sub(1);
        (*fs).freereg;
    }
}

unsafe fn freeregs(fs: *mut FuncState, r1: libc::c_int, r2: libc::c_int) {
    if r1 > r2 {
        freereg(fs, r1);
        freereg(fs, r2);
    } else {
        freereg(fs, r2);
        freereg(fs, r1);
    };
}

unsafe fn freeexp(fs: *mut FuncState, e: *mut expdesc) {
    if (*e).k as libc::c_uint == VNONRELOC as libc::c_int as libc::c_uint {
        freereg(fs, (*e).u.info);
    }
}

unsafe fn freeexps(fs: *mut FuncState, e1: *mut expdesc, e2: *mut expdesc) {
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
    freeregs(fs, r1, r2);
}

unsafe fn addk(
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
    let idx: *const UnsafeValue = luaH_get((*(*fs).ls).h.deref(), key);
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
    luaH_finishset((*(*fs).ls).h.deref(), key, idx, &raw const val).unwrap(); // This should never fails.
    (*f).k = luaM_growaux_(
        &(*(*fs).ls).g,
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
            luaC_barrier_(
                (*(*fs).ls).g.deref(),
                f as *mut Object,
                (*v).value_.gc as *mut Object,
            );
        } else {
        };
    } else {
    };
    return Ok(k);
}

unsafe fn stringK(fs: *mut FuncState, s: *mut TString) -> Result<libc::c_int, ParseError> {
    let mut o: UnsafeValue = UnsafeValue {
        value_: UntaggedValue {
            gc: 0 as *mut Object,
        },
        tt_: 0,
    };
    let io: *mut UnsafeValue = &mut o;
    let x_: *mut TString = s;
    (*io).value_.gc = x_ as *mut Object;
    (*io).tt_ = ((*x_).hdr.tt as libc::c_int | (1 as libc::c_int) << 6 as libc::c_int) as u8;
    return addk(fs, &mut o, &mut o);
}

unsafe fn luaK_intK(fs: *mut FuncState, n: i64) -> Result<libc::c_int, ParseError> {
    let mut o: UnsafeValue = UnsafeValue {
        value_: UntaggedValue {
            gc: 0 as *mut Object,
        },
        tt_: 0,
    };
    let io: *mut UnsafeValue = &mut o;
    (*io).value_.i = n;
    (*io).tt_ = (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    return addk(fs, &mut o, &mut o);
}

unsafe fn luaK_numberK(fs: *mut FuncState, r: f64) -> Result<libc::c_int, ParseError> {
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
        return addk(fs, &mut o, &mut o);
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
        return addk(fs, &mut kv, &mut o);
    };
}

unsafe fn boolF(fs: *mut FuncState) -> Result<libc::c_int, ParseError> {
    let mut o: UnsafeValue = UnsafeValue {
        value_: UntaggedValue {
            gc: 0 as *mut Object,
        },
        tt_: 0,
    };
    o.tt_ = (1 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    return addk(fs, &mut o, &mut o);
}

unsafe fn boolT(fs: *mut FuncState) -> Result<libc::c_int, ParseError> {
    let mut o: UnsafeValue = UnsafeValue {
        value_: UntaggedValue {
            gc: 0 as *mut Object,
        },
        tt_: 0,
    };
    o.tt_ = (1 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
    return addk(fs, &mut o, &mut o);
}

unsafe fn nilK(fs: *mut FuncState) -> Result<libc::c_int, ParseError> {
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

    (*io).value_.gc = &(*(*fs).ls).h.hdr;
    (*io).tt_ = (5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 6 as libc::c_int) as u8;
    return addk(fs, &mut k, &mut v);
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

pub unsafe fn luaK_int(fs: *mut FuncState, reg: libc::c_int, i: i64) -> Result<(), ParseError> {
    if fitsBx(i) != 0 {
        codeAsBx(fs, OP_LOADI, reg, i as libc::c_int)?;
    } else {
        luaK_codek(fs, reg, luaK_intK(fs, i)?)?;
    };

    Ok(())
}

unsafe fn luaK_float(fs: *mut FuncState, reg: libc::c_int, f: f64) -> Result<(), ParseError> {
    let mut fi: i64 = 0;
    if luaV_flttointeger(f, &mut fi, F2Ieq) != 0 && fitsBx(fi) != 0 {
        codeAsBx(fs, OP_LOADF, reg, fi as libc::c_int)?;
    } else {
        luaK_codek(fs, reg, luaK_numberK(fs, f)?)?;
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
            (*e).u.strval = (*v).value_.gc as *mut TString;
        }
        _ => {}
    };
}

pub unsafe fn luaK_setreturns(
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
        luaK_reserveregs(fs, 1 as libc::c_int)?;
    };
    Ok(())
}

unsafe fn str2K(fs: *mut FuncState, e: *mut expdesc) -> Result<(), ParseError> {
    (*e).u.info = stringK(fs, (*e).u.strval)?;
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

pub unsafe fn luaK_dischargevars(fs: *mut FuncState, e: *mut expdesc) -> Result<(), ParseError> {
    match (*e).k as libc::c_uint {
        11 => {
            const2exp(const2val(fs, e), e);
        }
        9 => {
            let temp: libc::c_int = (*e).u.var.ridx as libc::c_int;
            (*e).u.info = temp;
            (*e).k = VNONRELOC;
        }
        10 => {
            (*e).u.info = luaK_codeABCk(
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
            freereg(fs, (*e).u.ind.t as libc::c_int);
            (*e).u.info = luaK_codeABCk(
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
            freereg(fs, (*e).u.ind.t as libc::c_int);
            (*e).u.info = luaK_codeABCk(
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
                fs,
                (*e).u.ind.t as libc::c_int,
                (*e).u.ind.idx as libc::c_int,
            );
            (*e).u.info = luaK_codeABCk(
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
    fs: *mut FuncState,
    e: *mut expdesc,
    reg: libc::c_int,
) -> Result<(), ParseError> {
    luaK_dischargevars(fs, e)?;
    let current_block_14: u64;
    match (*e).k as libc::c_uint {
        1 => {
            luaK_nil(fs, reg, 1 as libc::c_int)?;
            current_block_14 = 13242334135786603907;
        }
        3 => {
            luaK_codeABCk(
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
            str2K(fs, e)?;
            current_block_14 = 6937071982253665452;
        }
        4 => {
            current_block_14 = 6937071982253665452;
        }
        5 => {
            luaK_float(fs, reg, (*e).u.nval)?;
            current_block_14 = 13242334135786603907;
        }
        6 => {
            luaK_int(fs, reg, (*e).u.ival)?;
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
            luaK_codek(fs, reg, (*e).u.info)?;
        }
        _ => {}
    }
    (*e).u.info = reg;
    (*e).k = VNONRELOC;
    Ok(())
}

unsafe fn discharge2anyreg(fs: *mut FuncState, e: *mut expdesc) -> Result<(), ParseError> {
    if (*e).k as libc::c_uint != VNONRELOC as libc::c_int as libc::c_uint {
        luaK_reserveregs(fs, 1 as libc::c_int)?;
        discharge2reg(fs, e, (*fs).freereg as libc::c_int - 1 as libc::c_int)?;
    }
    Ok(())
}

unsafe fn code_loadbool(
    fs: *mut FuncState,
    A: libc::c_int,
    op: OpCode,
) -> Result<libc::c_int, ParseError> {
    luaK_getlabel(fs);
    return luaK_codeABCk(
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

unsafe fn exp2reg(fs: *mut FuncState, e: *mut expdesc, reg: libc::c_int) -> Result<(), ParseError> {
    discharge2reg(fs, e, reg)?;
    if (*e).k as libc::c_uint == VJMP as libc::c_int as libc::c_uint {
        luaK_concat(fs, &mut (*e).t, (*e).u.info)?;
    }
    if (*e).t != (*e).f {
        let mut final_0: libc::c_int = 0;
        let mut p_f: libc::c_int = -(1 as libc::c_int);
        let mut p_t: libc::c_int = -(1 as libc::c_int);
        if need_value(fs, (*e).t) != 0 || need_value(fs, (*e).f) != 0 {
            let fj: libc::c_int = if (*e).k as libc::c_uint == VJMP as libc::c_int as libc::c_uint {
                -(1 as libc::c_int)
            } else {
                luaK_jump(fs)?
            };
            p_f = code_loadbool(fs, reg, OP_LFALSESKIP)?;
            p_t = code_loadbool(fs, reg, OP_LOADTRUE)?;
            luaK_patchtohere(fs, fj)?;
        }
        final_0 = luaK_getlabel(fs);
        patchlistaux(fs, (*e).f, final_0, reg, p_f)?;
        patchlistaux(fs, (*e).t, final_0, reg, p_t)?;
    }
    (*e).t = -(1 as libc::c_int);
    (*e).f = (*e).t;
    (*e).u.info = reg;
    (*e).k = VNONRELOC;
    Ok(())
}

pub unsafe fn luaK_exp2nextreg(fs: *mut FuncState, e: *mut expdesc) -> Result<(), ParseError> {
    luaK_dischargevars(fs, e)?;
    freeexp(fs, e);
    luaK_reserveregs(fs, 1 as libc::c_int)?;
    exp2reg(fs, e, (*fs).freereg as libc::c_int - 1 as libc::c_int)
}

pub unsafe fn luaK_exp2anyreg(
    fs: *mut FuncState,
    e: *mut expdesc,
) -> Result<libc::c_int, ParseError> {
    luaK_dischargevars(fs, e)?;
    if (*e).k as libc::c_uint == VNONRELOC as libc::c_int as libc::c_uint {
        if !((*e).t != (*e).f) {
            return Ok((*e).u.info);
        }
        if (*e).u.info >= luaY_nvarstack(fs) {
            exp2reg(fs, e, (*e).u.info)?;
            return Ok((*e).u.info);
        }
    }
    luaK_exp2nextreg(fs, e)?;
    return Ok((*e).u.info);
}

pub unsafe fn luaK_exp2anyregup(fs: *mut FuncState, e: *mut expdesc) -> Result<(), ParseError> {
    if (*e).k as libc::c_uint != VUPVAL as libc::c_int as libc::c_uint || (*e).t != (*e).f {
        luaK_exp2anyreg(fs, e)?;
    }
    Ok(())
}

pub unsafe fn luaK_exp2val(fs: *mut FuncState, e: *mut expdesc) -> Result<(), ParseError> {
    if (*e).k == VJMP || (*e).t != (*e).f {
        luaK_exp2anyreg(fs, e)?;
    } else {
        luaK_dischargevars(fs, e)?;
    };
    Ok(())
}

unsafe fn luaK_exp2K(fs: *mut FuncState, e: *mut expdesc) -> Result<libc::c_int, ParseError> {
    if !((*e).t != (*e).f) {
        let mut info: libc::c_int = 0;
        match (*e).k as libc::c_uint {
            2 => {
                info = boolT(fs)?;
            }
            3 => {
                info = boolF(fs)?;
            }
            1 => {
                info = nilK(fs)?;
            }
            6 => {
                info = luaK_intK(fs, (*e).u.ival)?;
            }
            5 => {
                info = luaK_numberK(fs, (*e).u.nval)?;
            }
            7 => {
                info = stringK(fs, (*e).u.strval)?;
            }
            4 => {
                info = (*e).u.info;
            }
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

unsafe fn exp2RK(fs: *mut FuncState, e: *mut expdesc) -> Result<libc::c_int, ParseError> {
    if luaK_exp2K(fs, e)? != 0 {
        return Ok(1 as libc::c_int);
    } else {
        luaK_exp2anyreg(fs, e)?;
        return Ok(0 as libc::c_int);
    };
}

unsafe fn codeABRK(
    fs: *mut FuncState,
    o: OpCode,
    a: libc::c_int,
    b: libc::c_int,
    ec: *mut expdesc,
) -> Result<(), ParseError> {
    let k: libc::c_int = exp2RK(fs, ec)?;
    luaK_codeABCk(fs, o, a, b, (*ec).u.info, k)?;
    Ok(())
}

pub unsafe fn luaK_storevar(
    fs: *mut FuncState,
    var: *mut expdesc,
    ex: *mut expdesc,
) -> Result<(), ParseError> {
    match (*var).k as libc::c_uint {
        9 => {
            freeexp(fs, ex);
            return exp2reg(fs, ex, (*var).u.var.ridx as libc::c_int);
        }
        10 => {
            let e: libc::c_int = luaK_exp2anyreg(fs, ex)?;
            luaK_codeABCk(
                fs,
                OP_SETUPVAL,
                e,
                (*var).u.info,
                0 as libc::c_int,
                0 as libc::c_int,
            )?;
        }
        13 => codeABRK(
            fs,
            OP_SETTABUP,
            (*var).u.ind.t as libc::c_int,
            (*var).u.ind.idx as libc::c_int,
            ex,
        )?,
        14 => codeABRK(
            fs,
            OP_SETI,
            (*var).u.ind.t as libc::c_int,
            (*var).u.ind.idx as libc::c_int,
            ex,
        )?,
        15 => codeABRK(
            fs,
            OP_SETFIELD,
            (*var).u.ind.t as libc::c_int,
            (*var).u.ind.idx as libc::c_int,
            ex,
        )?,
        12 => codeABRK(
            fs,
            OP_SETTABLE,
            (*var).u.ind.t as libc::c_int,
            (*var).u.ind.idx as libc::c_int,
            ex,
        )?,
        _ => {}
    }
    freeexp(fs, ex);
    Ok(())
}

pub unsafe fn luaK_self(
    fs: *mut FuncState,
    e: *mut expdesc,
    key: *mut expdesc,
) -> Result<(), ParseError> {
    let mut ereg: libc::c_int = 0;
    luaK_exp2anyreg(fs, e)?;
    ereg = (*e).u.info;
    freeexp(fs, e);
    (*e).u.info = (*fs).freereg as libc::c_int;
    (*e).k = VNONRELOC;
    luaK_reserveregs(fs, 2 as libc::c_int)?;
    codeABRK(fs, OP_SELF, (*e).u.info, ereg, key)?;
    freeexp(fs, key);
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
    discharge2anyreg(fs, e)?;
    freeexp(fs, e);
    return condjump(
        fs,
        OP_TESTSET,
        ((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int,
        (*e).u.info,
        0 as libc::c_int,
        cond,
    );
}

pub unsafe fn luaK_goiftrue(fs: *mut FuncState, e: *mut expdesc) -> Result<(), ParseError> {
    let mut pc: libc::c_int = 0;
    luaK_dischargevars(fs, e)?;
    match (*e).k as libc::c_uint {
        16 => {
            negatecondition(fs, e);
            pc = (*e).u.info;
        }
        4 | 5 | 6 | 7 | 2 => {
            pc = -(1 as libc::c_int);
        }
        _ => {
            pc = jumponcond(fs, e, 0 as libc::c_int)?;
        }
    }
    luaK_concat(fs, &mut (*e).f, pc)?;
    luaK_patchtohere(fs, (*e).t)?;
    (*e).t = -(1 as libc::c_int);
    Ok(())
}

pub unsafe fn luaK_goiffalse(fs: *mut FuncState, e: *mut expdesc) -> Result<(), ParseError> {
    let mut pc: libc::c_int = 0;
    luaK_dischargevars(fs, e)?;
    match (*e).k as libc::c_uint {
        16 => {
            pc = (*e).u.info;
        }
        1 | 3 => {
            pc = -(1 as libc::c_int);
        }
        _ => {
            pc = jumponcond(fs, e, 1 as libc::c_int)?;
        }
    }
    luaK_concat(fs, &mut (*e).t, pc)?;
    luaK_patchtohere(fs, (*e).f)?;
    (*e).f = -(1 as libc::c_int);
    Ok(())
}

unsafe fn codenot(fs: *mut FuncState, e: *mut expdesc) -> Result<(), ParseError> {
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
            discharge2anyreg(fs, e)?;
            freeexp(fs, e);
            (*e).u.info = luaK_codeABCk(
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
    fs: *mut FuncState,
    t: *mut expdesc,
    k: *mut expdesc,
) -> Result<(), ParseError> {
    if (*k).k as libc::c_uint == VKSTR as libc::c_int as libc::c_uint {
        str2K(fs, k)?;
    }
    if (*t).k as libc::c_uint == VUPVAL as libc::c_int as libc::c_uint && isKstr(fs, k) == 0 {
        luaK_exp2anyreg(fs, t)?;
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
            (*t).u.ind.idx = luaK_exp2anyreg(fs, k)? as libc::c_short;
            (*t).k = VINDEXED;
        }
    };
    Ok(())
}

unsafe fn validop(op: libc::c_int, v1: *mut UnsafeValue, v2: *mut UnsafeValue) -> libc::c_int {
    match op {
        7 | 8 | 9 | 10 | 11 | 13 => {
            let mut i: i64 = 0;
            return (luaV_tointegerns(v1, &mut i, F2Ieq) != 0
                && luaV_tointegerns(v2, &mut i, F2Ieq) != 0) as libc::c_int;
        }
        5 | 6 | 3 => {
            return ((if (*v2).tt_ as libc::c_int
                == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
            {
                (*v2).value_.i as f64
            } else {
                (*v2).value_.n
            }) != 0 as libc::c_int as f64) as libc::c_int;
        }
        _ => return 1 as libc::c_int,
    };
}

unsafe fn constfolding(
    op: libc::c_int,
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
    let mut res: UnsafeValue = UnsafeValue {
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
    luaO_rawarith(op, &mut v1, &mut v2, &mut res)?;
    if res.tt_ as libc::c_int == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int {
        (*e1).k = VKINT;
        (*e1).u.ival = res.value_.i;
    } else {
        let n: f64 = res.value_.n;
        if !(n == n) || n == 0 as libc::c_int as f64 {
            return Ok(0 as libc::c_int);
        }
        (*e1).k = VKFLT;
        (*e1).u.nval = n;
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
    fs: *mut FuncState,
    op: OpCode,
    e: *mut expdesc,
    line: libc::c_int,
) -> Result<(), ParseError> {
    let r: libc::c_int = luaK_exp2anyreg(fs, e)?;
    freeexp(fs, e);
    (*e).u.info = luaK_codeABCk(
        fs,
        op,
        0 as libc::c_int,
        r,
        0 as libc::c_int,
        0 as libc::c_int,
    )?;
    (*e).k = VRELOC;

    luaK_fixline(fs, line)
}

unsafe fn finishbinexpval(
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
    let v1: libc::c_int = luaK_exp2anyreg(fs, e1)?;
    let pc: libc::c_int = luaK_codeABCk(fs, op, 0 as libc::c_int, v1, v2, 0 as libc::c_int)?;
    freeexps(fs, e1, e2);
    (*e1).u.info = pc;
    (*e1).k = VRELOC;
    luaK_fixline(fs, line)?;
    luaK_codeABCk(fs, mmop, v1, v2, event as libc::c_int, flip)?;
    luaK_fixline(fs, line)
}

unsafe fn codebinexpval(
    fs: *mut FuncState,
    opr: BinOpr,
    e1: *mut expdesc,
    e2: *mut expdesc,
    line: libc::c_int,
) -> Result<(), ParseError> {
    let op: OpCode = binopr2op(opr, OPR_ADD, OP_ADD);
    let v2: libc::c_int = luaK_exp2anyreg(fs, e2)?;
    finishbinexpval(
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
    finishbinexpval(fs, e1, e2, op, v2, flip, line, OP_MMBINI, event)
}

unsafe fn codebinK(
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
    finishbinexpval(fs, e1, e2, op, v2, flip, line, OP_MMBINK, event)
}

unsafe fn finishbinexpneg(
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
    codebinexpval(fs, opr, e1, e2, line)
}

unsafe fn codearith(
    fs: *mut FuncState,
    opr: BinOpr,
    e1: *mut expdesc,
    e2: *mut expdesc,
    flip: libc::c_int,
    line: libc::c_int,
) -> Result<(), ParseError> {
    if tonumeral(e2, 0 as *mut UnsafeValue) != 0 && luaK_exp2K(fs, e2)? != 0 {
        codebinK(fs, opr, e1, e2, flip, line)
    } else {
        codebinNoK(fs, opr, e1, e2, flip, line)
    }
}

unsafe fn codecommutative(
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
        codebini(fs, OP_ADDI, e1, e2, flip, line, TM_ADD)
    } else {
        codearith(fs, op, e1, e2, flip, line)
    }
}

unsafe fn codebitwise(
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

    if (*e2).k as libc::c_uint == VKINT as libc::c_int as libc::c_uint && luaK_exp2K(fs, e2)? != 0 {
        codebinK(fs, opr, e1, e2, flip, line)
    } else {
        codebinNoK(fs, opr, e1, e2, flip, line)
    }
}

unsafe fn codeorder(
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
        r1 = luaK_exp2anyreg(fs, e1)?;
        r2 = im;
        op = binopr2op(opr, OPR_LT, OP_LTI);
    } else if isSCnumber(e1, &mut im, &mut isfloat) != 0 {
        r1 = luaK_exp2anyreg(fs, e2)?;
        r2 = im;
        op = binopr2op(opr, OPR_LT, OP_GTI);
    } else {
        r1 = luaK_exp2anyreg(fs, e1)?;
        r2 = luaK_exp2anyreg(fs, e2)?;
        op = binopr2op(opr, OPR_LT, OP_LT);
    }
    freeexps(fs, e1, e2);
    (*e1).u.info = condjump(fs, op, r1, r2, isfloat, 1 as libc::c_int)?;
    (*e1).k = VJMP;
    Ok(())
}

unsafe fn codeeq(
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
    r1 = luaK_exp2anyreg(fs, e1)?;
    if isSCnumber(e2, &mut im, &mut isfloat) != 0 {
        op = OP_EQI;
        r2 = im;
    } else if exp2RK(fs, e2)? != 0 {
        op = OP_EQK;
        r2 = (*e2).u.info;
    } else {
        op = OP_EQ;
        r2 = luaK_exp2anyreg(fs, e2)?;
    }
    freeexps(fs, e1, e2);
    (*e1).u.info = condjump(
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
    luaK_dischargevars(fs, e)?;
    let current_block_3: u64;
    match opr as libc::c_uint {
        0 | 1 => {
            if constfolding(
                (opr as libc::c_uint).wrapping_add(12 as libc::c_int as libc::c_uint)
                    as libc::c_int,
                e,
                &raw mut ef,
            )
            .map_err(|e| luaK_semerror((*fs).ls, e))?
                != 0
            {
                current_block_3 = 7815301370352969686;
            } else {
                current_block_3 = 4299225766812711900;
            }
        }
        3 => {
            current_block_3 = 4299225766812711900;
        }
        2 => {
            codenot(fs, e)?;
            current_block_3 = 7815301370352969686;
        }
        _ => {
            current_block_3 = 7815301370352969686;
        }
    }
    match current_block_3 {
        4299225766812711900 => {
            codeunexpval(fs, unopr2op(opr), e, line)?;
        }
        _ => {}
    };
    Ok(())
}

pub unsafe fn luaK_infix(
    fs: *mut FuncState,
    op: BinOpr,
    v: *mut expdesc,
) -> Result<(), ParseError> {
    luaK_dischargevars(fs, v)?;
    match op as libc::c_uint {
        19 => luaK_goiftrue(fs, v)?,
        20 => luaK_goiffalse(fs, v)?,
        12 => luaK_exp2nextreg(fs, v)?,
        0 | 1 | 2 | 5 | 6 | 3 | 4 | 7 | 8 | 9 | 10 | 11 => {
            if tonumeral(v, 0 as *mut UnsafeValue) == 0 {
                luaK_exp2anyreg(fs, v)?;
            }
        }
        13 | 16 => {
            if tonumeral(v, 0 as *mut UnsafeValue) == 0 {
                exp2RK(fs, v)?;
            }
        }
        14 | 15 | 17 | 18 => {
            let mut dummy: libc::c_int = 0;
            let mut dummy2: libc::c_int = 0;
            if isSCnumber(v, &mut dummy, &mut dummy2) == 0 {
                luaK_exp2anyreg(fs, v)?;
            }
        }
        _ => {}
    };
    Ok(())
}

unsafe fn codeconcat(
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
        freeexp(fs, e2);
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
            fs,
            OP_CONCAT,
            (*e1).u.info,
            2 as libc::c_int,
            0 as libc::c_int,
            0 as libc::c_int,
        )?;
        freeexp(fs, e2);
        luaK_fixline(fs, line)?;
    };
    Ok(())
}

pub unsafe fn luaK_posfix(
    fs: *mut FuncState,
    mut opr: BinOpr,
    e1: *mut expdesc,
    e2: *mut expdesc,
    line: libc::c_int,
) -> Result<(), ParseError> {
    luaK_dischargevars(fs, e2)?;
    if opr as libc::c_uint <= OPR_SHR as libc::c_int as libc::c_uint
        && constfolding(
            (opr as libc::c_uint).wrapping_add(0 as libc::c_int as libc::c_uint) as libc::c_int,
            e1,
            e2,
        )
        .map_err(|e| luaK_semerror((*fs).ls, e))?
            != 0
    {
        return Ok(());
    }
    let current_block_30: u64;
    match opr as libc::c_uint {
        19 => {
            luaK_concat(fs, &mut (*e2).f, (*e1).f)?;
            *e1 = *e2;
            current_block_30 = 8180496224585318153;
        }
        20 => {
            luaK_concat(fs, &mut (*e2).t, (*e1).t)?;
            *e1 = *e2;
            current_block_30 = 8180496224585318153;
        }
        12 => {
            luaK_exp2nextreg(fs, e2)?;
            codeconcat(fs, e1, e2, line)?;
            current_block_30 = 8180496224585318153;
        }
        0 | 2 => {
            codecommutative(fs, opr, e1, e2, line)?;
            current_block_30 = 8180496224585318153;
        }
        1 => {
            if finishbinexpneg(fs, e1, e2, OP_ADDI, line, TM_SUB)? != 0 {
                current_block_30 = 8180496224585318153;
            } else {
                current_block_30 = 12599329904712511516;
            }
        }
        5 | 6 | 3 | 4 => {
            current_block_30 = 12599329904712511516;
        }
        7 | 8 | 9 => {
            codebitwise(fs, opr, e1, e2, line)?;
            current_block_30 = 8180496224585318153;
        }
        10 => {
            if isSCint(e1) != 0 {
                swapexps(e1, e2);
                codebini(fs, OP_SHLI, e1, e2, 1 as libc::c_int, line, TM_SHL)?;
            } else if !(finishbinexpneg(fs, e1, e2, OP_SHRI, line, TM_SHL)? != 0) {
                codebinexpval(fs, opr, e1, e2, line)?;
            }
            current_block_30 = 8180496224585318153;
        }
        11 => {
            if isSCint(e2) != 0 {
                codebini(fs, OP_SHRI, e1, e2, 0 as libc::c_int, line, TM_SHR)?;
            } else {
                codebinexpval(fs, opr, e1, e2, line)?;
            }
            current_block_30 = 8180496224585318153;
        }
        13 | 16 => {
            codeeq(fs, opr, e1, e2)?;
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
            codearith(fs, opr, e1, e2, 0 as libc::c_int, line)?;
        }
        1118134448028020070 => {
            codeorder(fs, opr, e1, e2)?;
        }
        _ => {}
    };
    Ok(())
}

pub unsafe fn luaK_fixline(fs: *mut FuncState, line: libc::c_int) -> Result<(), ParseError> {
    removelastlineinfo(fs);
    savelineinfo(fs, (*fs).f, line)
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
    fs: *mut FuncState,
    base: libc::c_int,
    mut nelems: libc::c_int,
    mut tostore: libc::c_int,
) -> Result<(), ParseError> {
    if tostore == -(1 as libc::c_int) {
        tostore = 0 as libc::c_int;
    }
    if nelems <= ((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int {
        luaK_codeABCk(fs, OP_SETLIST, base, tostore, nelems, 0 as libc::c_int)?;
    } else {
        let extra: libc::c_int = nelems
            / (((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int + 1 as libc::c_int);
        nelems %= ((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int + 1 as libc::c_int;
        luaK_codeABCk(fs, OP_SETLIST, base, tostore, nelems, 1 as libc::c_int)?;
        codeextraarg(fs, extra)?;
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

pub unsafe fn luaK_finish(fs: *mut FuncState) -> Result<(), ParseError> {
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
                fixjump(fs, i, target)?;
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
