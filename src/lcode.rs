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
#![allow(path_statements)]

use crate::gc::luaC_barrier_;
use crate::llex::{LexState, luaX_syntaxerror};
use crate::lmem::luaM_growaux_;
use crate::lobject::{
    AbsLineInfo, Proto, TString, TValue, Table, Value, luaO_ceillog2, luaO_rawarith,
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
use crate::ltable::{luaH_finishset, luaH_get};
use crate::ltm::{TM_ADD, TM_SHL, TM_SHR, TM_SUB, TMS};
use crate::lvm::{F2Ieq, luaV_equalobj, luaV_flttointeger, luaV_tointegerns};
use crate::{Object, Thread};
use libc::abs;
use libm::ldexp;
use std::ffi::c_int;
use std::fmt::Display;

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

pub unsafe fn luaK_semerror(
    mut ls: *mut LexState,
    msg: impl Display,
) -> Result<(), Box<dyn std::error::Error>> {
    (*ls).t.token = 0 as libc::c_int;

    luaX_syntaxerror(ls, msg)
}

unsafe extern "C" fn tonumeral(mut e: *const expdesc, mut v: *mut TValue) -> libc::c_int {
    if (*e).t != (*e).f {
        return 0 as libc::c_int;
    }
    match (*e).k as libc::c_uint {
        6 => {
            if !v.is_null() {
                let mut io: *mut TValue = v;
                (*io).value_.i = (*e).u.ival;
                (*io).tt_ = (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
            }
            return 1 as libc::c_int;
        }
        5 => {
            if !v.is_null() {
                let mut io_0: *mut TValue = v;
                (*io_0).value_.n = (*e).u.nval;
                (*io_0).tt_ = (3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
            }
            return 1 as libc::c_int;
        }
        _ => return 0 as libc::c_int,
    };
}

unsafe extern "C" fn const2val(mut fs: *mut FuncState, mut e: *const expdesc) -> *mut TValue {
    return &mut (*((*(*(*fs).ls).dyd).actvar.arr).offset((*e).u.info as isize)).k;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaK_exp2const(
    mut fs: *mut FuncState,
    mut e: *const expdesc,
    mut v: *mut TValue,
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
            let mut io: *mut TValue = v;
            let mut x_: *mut TString = (*e).u.strval;
            (*io).value_.gc = x_ as *mut Object;
            (*io).tt_ =
                ((*x_).hdr.tt as libc::c_int | (1 as libc::c_int) << 6 as libc::c_int) as u8;
            return 1 as libc::c_int;
        }
        11 => {
            let mut io1: *mut TValue = v;
            let mut io2: *const TValue = const2val(fs, e);
            (*io1).value_ = (*io2).value_;
            (*io1).tt_ = (*io2).tt_;
            return 1 as libc::c_int;
        }
        _ => return tonumeral(e, v),
    };
}

unsafe extern "C" fn previousinstruction(mut fs: *mut FuncState) -> *mut u32 {
    static mut invalidinstruction: u32 = !(0 as libc::c_int as u32);
    if (*fs).pc > (*fs).lasttarget {
        return &mut *((*(*fs).f).code).offset(((*fs).pc - 1 as libc::c_int) as isize) as *mut u32;
    } else {
        return &raw mut invalidinstruction as *const u32 as *mut u32;
    };
}

pub unsafe fn luaK_nil(
    mut fs: *mut FuncState,
    mut from: libc::c_int,
    mut n: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut l: libc::c_int = from + n - 1 as libc::c_int;
    let mut previous: *mut u32 = previousinstruction(fs);
    if (*previous >> 0 as libc::c_int
        & !(!(0 as libc::c_int as u32) << 7 as libc::c_int) << 0 as libc::c_int) as OpCode
        as libc::c_uint
        == OP_LOADNIL as libc::c_int as libc::c_uint
    {
        let mut pfrom: libc::c_int = (*previous >> 0 as libc::c_int + 7 as libc::c_int
            & !(!(0 as libc::c_int as u32) << 8 as libc::c_int) << 0 as libc::c_int)
            as libc::c_int;
        let mut pl: libc::c_int = pfrom
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

unsafe extern "C" fn getjump(mut fs: *mut FuncState, mut pc: libc::c_int) -> libc::c_int {
    let mut offset: libc::c_int = (*((*(*fs).f).code).offset(pc as isize)
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
    mut fs: *mut FuncState,
    mut pc: libc::c_int,
    mut dest: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut jmp: *mut u32 = &mut *((*(*fs).f).code).offset(pc as isize) as *mut u32;
    let mut offset: libc::c_int = dest - (pc + 1 as libc::c_int);
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
        luaX_syntaxerror((*fs).ls, "control structure too long")?;
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
    mut fs: *mut FuncState,
    mut l1: *mut libc::c_int,
    mut l2: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
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

pub unsafe fn luaK_jump(mut fs: *mut FuncState) -> Result<c_int, Box<dyn std::error::Error>> {
    return codesJ(fs, OP_JMP, -(1 as libc::c_int), 0 as libc::c_int);
}

pub unsafe fn luaK_ret(
    mut fs: *mut FuncState,
    mut first: libc::c_int,
    mut nret: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
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
    mut fs: *mut FuncState,
    mut op: OpCode,
    mut A: libc::c_int,
    mut B: libc::c_int,
    mut C: libc::c_int,
    mut k: libc::c_int,
) -> Result<c_int, Box<dyn std::error::Error>> {
    luaK_codeABCk(fs, op, A, B, C, k)?;
    return luaK_jump(fs);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaK_getlabel(mut fs: *mut FuncState) -> libc::c_int {
    (*fs).lasttarget = (*fs).pc;
    return (*fs).pc;
}

unsafe extern "C" fn getjumpcontrol(mut fs: *mut FuncState, mut pc: libc::c_int) -> *mut u32 {
    let mut pi: *mut u32 = &mut *((*(*fs).f).code).offset(pc as isize) as *mut u32;
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

unsafe extern "C" fn patchtestreg(
    mut fs: *mut FuncState,
    mut node: libc::c_int,
    mut reg: libc::c_int,
) -> libc::c_int {
    let mut i: *mut u32 = getjumpcontrol(fs, node);
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

unsafe extern "C" fn removevalues(mut fs: *mut FuncState, mut list: libc::c_int) {
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
    mut fs: *mut FuncState,
    mut list: libc::c_int,
    mut vtarget: libc::c_int,
    mut reg: libc::c_int,
    mut dtarget: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    while list != -(1 as libc::c_int) {
        let mut next: libc::c_int = getjump(fs, list);
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
    mut fs: *mut FuncState,
    mut list: libc::c_int,
    mut target: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    patchlistaux(
        fs,
        list,
        target,
        ((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int,
        target,
    )
}

pub unsafe fn luaK_patchtohere(
    mut fs: *mut FuncState,
    mut list: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut hr: libc::c_int = luaK_getlabel(fs);
    luaK_patchlist(fs, list, hr)
}

unsafe fn savelineinfo(
    mut fs: *mut FuncState,
    mut f: *mut Proto,
    mut line: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut linedif: libc::c_int = line - (*fs).previousline;
    let mut pc: libc::c_int = (*fs).pc - 1 as libc::c_int;
    if abs(linedif) >= 0x80 as libc::c_int || {
        let fresh0 = (*fs).iwthabs;
        (*fs).iwthabs = ((*fs).iwthabs).wrapping_add(1);
        fresh0 as libc::c_int >= 128 as libc::c_int
    } {
        (*f).abslineinfo = luaM_growaux_(
            (*(*fs).ls).L,
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
            b"lines\0" as *const u8 as *const libc::c_char,
        )? as *mut AbsLineInfo;
        (*((*f).abslineinfo).offset((*fs).nabslineinfo as isize)).pc = pc;
        let fresh1 = (*fs).nabslineinfo;
        (*fs).nabslineinfo = (*fs).nabslineinfo + 1;
        (*((*f).abslineinfo).offset(fresh1 as isize)).line = line;
        linedif = -(0x80 as libc::c_int);
        (*fs).iwthabs = 1 as libc::c_int as u8;
    }
    (*f).lineinfo = luaM_growaux_(
        (*(*fs).ls).L,
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
        b"opcodes\0" as *const u8 as *const libc::c_char,
    )? as *mut i8;
    *((*f).lineinfo).offset(pc as isize) = linedif as i8;
    (*fs).previousline = line;
    Ok(())
}

unsafe extern "C" fn removelastlineinfo(mut fs: *mut FuncState) {
    let mut f: *mut Proto = (*fs).f;
    let mut pc: libc::c_int = (*fs).pc - 1 as libc::c_int;
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

unsafe extern "C" fn removelastinstruction(mut fs: *mut FuncState) {
    removelastlineinfo(fs);
    (*fs).pc -= 1;
    (*fs).pc;
}

pub unsafe fn luaK_code(
    mut fs: *mut FuncState,
    mut i: u32,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut f: *mut Proto = (*fs).f;
    (*f).code = luaM_growaux_(
        (*(*fs).ls).L,
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
        b"opcodes\0" as *const u8 as *const libc::c_char,
    )? as *mut u32;
    let fresh2 = (*fs).pc;
    (*fs).pc = (*fs).pc + 1;
    *((*f).code).offset(fresh2 as isize) = i;
    savelineinfo(fs, f, (*(*fs).ls).lastline)?;
    return Ok((*fs).pc - 1 as libc::c_int);
}

pub unsafe fn luaK_codeABCk(
    mut fs: *mut FuncState,
    mut o: OpCode,
    mut a: libc::c_int,
    mut b: libc::c_int,
    mut c: libc::c_int,
    mut k: libc::c_int,
) -> Result<c_int, Box<dyn std::error::Error>> {
    return luaK_code(
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
    );
}

pub unsafe fn luaK_codeABx(
    mut fs: *mut FuncState,
    mut o: OpCode,
    mut a: libc::c_int,
    mut bc: libc::c_uint,
) -> Result<c_int, Box<dyn std::error::Error>> {
    return luaK_code(
        fs,
        (o as u32) << 0 as libc::c_int
            | (a as u32) << 0 as libc::c_int + 7 as libc::c_int
            | bc << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int,
    );
}

unsafe fn codeAsBx(
    mut fs: *mut FuncState,
    mut o: OpCode,
    mut a: libc::c_int,
    mut bc: libc::c_int,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut b: libc::c_uint = (bc
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
    mut fs: *mut FuncState,
    mut o: OpCode,
    mut sj: libc::c_int,
    mut k: libc::c_int,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut j: libc::c_uint = (sj
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

unsafe fn codeextraarg(
    mut fs: *mut FuncState,
    mut a: libc::c_int,
) -> Result<c_int, Box<dyn std::error::Error>> {
    return luaK_code(
        fs,
        (OP_EXTRAARG as libc::c_int as u32) << 0 as libc::c_int
            | (a as u32) << 0 as libc::c_int + 7 as libc::c_int,
    );
}

unsafe fn luaK_codek(
    mut fs: *mut FuncState,
    mut reg: libc::c_int,
    mut k: libc::c_int,
) -> Result<c_int, Box<dyn std::error::Error>> {
    if k <= ((1 as libc::c_int) << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int)
        - 1 as libc::c_int
    {
        return luaK_codeABx(fs, OP_LOADK, reg, k as libc::c_uint);
    } else {
        let mut p: libc::c_int =
            luaK_codeABx(fs, OP_LOADKX, reg, 0 as libc::c_int as libc::c_uint)?;
        codeextraarg(fs, k)?;
        return Ok(p);
    };
}

pub unsafe fn luaK_checkstack(
    mut fs: *mut FuncState,
    mut n: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut newstack: libc::c_int = (*fs).freereg as libc::c_int + n;
    if newstack > (*(*fs).f).maxstacksize as libc::c_int {
        if newstack >= 255 as libc::c_int {
            luaX_syntaxerror((*fs).ls, "function or expression needs too many registers")?;
        }
        (*(*fs).f).maxstacksize = newstack as u8;
    }
    Ok(())
}

pub unsafe fn luaK_reserveregs(
    mut fs: *mut FuncState,
    mut n: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    luaK_checkstack(fs, n)?;
    (*fs).freereg = ((*fs).freereg as libc::c_int + n) as u8;
    Ok(())
}

unsafe fn freereg(mut fs: *mut FuncState, mut reg: libc::c_int) {
    if reg >= luaY_nvarstack(fs) {
        (*fs).freereg = ((*fs).freereg).wrapping_sub(1);
        (*fs).freereg;
    }
}

unsafe fn freeregs(mut fs: *mut FuncState, mut r1: libc::c_int, mut r2: libc::c_int) {
    if r1 > r2 {
        freereg(fs, r1);
        freereg(fs, r2);
    } else {
        freereg(fs, r2);
        freereg(fs, r1);
    };
}

unsafe fn freeexp(mut fs: *mut FuncState, mut e: *mut expdesc) {
    if (*e).k as libc::c_uint == VNONRELOC as libc::c_int as libc::c_uint {
        freereg(fs, (*e).u.info);
    }
}

unsafe fn freeexps(mut fs: *mut FuncState, mut e1: *mut expdesc, mut e2: *mut expdesc) {
    let mut r1: libc::c_int = if (*e1).k as libc::c_uint == VNONRELOC as libc::c_int as libc::c_uint
    {
        (*e1).u.info
    } else {
        -(1 as libc::c_int)
    };
    let mut r2: libc::c_int = if (*e2).k as libc::c_uint == VNONRELOC as libc::c_int as libc::c_uint
    {
        (*e2).u.info
    } else {
        -(1 as libc::c_int)
    };
    freeregs(fs, r1, r2);
}

unsafe fn addk(
    mut fs: *mut FuncState,
    mut key: *mut TValue,
    mut v: *mut TValue,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut val: TValue = TValue {
        value_: Value {
            gc: 0 as *mut Object,
        },
        tt_: 0,
    };
    let mut L = (*(*fs).ls).L;
    let mut f: *mut Proto = (*fs).f;
    let mut idx: *const TValue = luaH_get((*(*fs).ls).h, key);
    let mut k: libc::c_int = 0;
    let mut oldsize: libc::c_int = 0;
    if (*idx).tt_ as libc::c_int == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int {
        k = (*idx).value_.i as libc::c_int;
        if k < (*fs).nk
            && (*((*f).k).offset(k as isize)).tt_ as libc::c_int & 0x3f as libc::c_int
                == (*v).tt_ as libc::c_int & 0x3f as libc::c_int
            && luaV_equalobj(0 as *mut Thread, &mut *((*f).k).offset(k as isize), v)? != 0
        {
            return Ok(k);
        }
    }
    oldsize = (*f).sizek;
    k = (*fs).nk;
    let mut io: *mut TValue = &mut val;
    (*io).value_.i = k as i64;
    (*io).tt_ = (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    luaH_finishset(L, (*(*fs).ls).h, key, idx, &mut val)?;
    (*f).k = luaM_growaux_(
        L,
        (*f).k as *mut libc::c_void,
        k,
        &mut (*f).sizek,
        ::core::mem::size_of::<TValue>() as libc::c_ulong as libc::c_int,
        (if (((1 as libc::c_int)
            << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int + 8 as libc::c_int)
            - 1 as libc::c_int) as usize
            <= (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<TValue>())
        {
            (((1 as libc::c_int)
                << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int + 8 as libc::c_int)
                - 1 as libc::c_int) as libc::c_uint
        } else {
            (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<TValue>())
                as libc::c_uint
        }) as libc::c_int,
        b"constants\0" as *const u8 as *const libc::c_char,
    )? as *mut TValue;
    while oldsize < (*f).sizek {
        let fresh3 = oldsize;
        oldsize = oldsize + 1;
        (*((*f).k).offset(fresh3 as isize)).tt_ =
            (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    }
    let mut io1: *mut TValue = &mut *((*f).k).offset(k as isize) as *mut TValue;
    let mut io2: *const TValue = v;
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
            luaC_barrier_((*L).global, f as *mut Object, (*v).value_.gc as *mut Object);
        } else {
        };
    } else {
    };
    return Ok(k);
}

unsafe fn stringK(
    mut fs: *mut FuncState,
    mut s: *mut TString,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut o: TValue = TValue {
        value_: Value {
            gc: 0 as *mut Object,
        },
        tt_: 0,
    };
    let mut io: *mut TValue = &mut o;
    let mut x_: *mut TString = s;
    (*io).value_.gc = x_ as *mut Object;
    (*io).tt_ = ((*x_).hdr.tt as libc::c_int | (1 as libc::c_int) << 6 as libc::c_int) as u8;
    return addk(fs, &mut o, &mut o);
}

unsafe fn luaK_intK(
    mut fs: *mut FuncState,
    mut n: i64,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut o: TValue = TValue {
        value_: Value {
            gc: 0 as *mut Object,
        },
        tt_: 0,
    };
    let mut io: *mut TValue = &mut o;
    (*io).value_.i = n;
    (*io).tt_ = (3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    return addk(fs, &mut o, &mut o);
}

unsafe fn luaK_numberK(
    mut fs: *mut FuncState,
    mut r: f64,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut o: TValue = TValue {
        value_: Value {
            gc: 0 as *mut Object,
        },
        tt_: 0,
    };
    let mut ik: i64 = 0;
    let mut io: *mut TValue = &mut o;
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
        let mut kv: TValue = TValue {
            value_: Value {
                gc: 0 as *mut Object,
            },
            tt_: 0,
        };
        let mut io_0: *mut TValue = &mut kv;
        (*io_0).value_.n = k;
        (*io_0).tt_ = (3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
        return addk(fs, &mut kv, &mut o);
    };
}

unsafe fn boolF(mut fs: *mut FuncState) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut o: TValue = TValue {
        value_: Value {
            gc: 0 as *mut Object,
        },
        tt_: 0,
    };
    o.tt_ = (1 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    return addk(fs, &mut o, &mut o);
}

unsafe fn boolT(mut fs: *mut FuncState) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut o: TValue = TValue {
        value_: Value {
            gc: 0 as *mut Object,
        },
        tt_: 0,
    };
    o.tt_ = (1 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int) as u8;
    return addk(fs, &mut o, &mut o);
}

unsafe fn nilK(mut fs: *mut FuncState) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut k: TValue = TValue {
        value_: Value {
            gc: 0 as *mut Object,
        },
        tt_: 0,
    };
    let mut v: TValue = TValue {
        value_: Value {
            gc: 0 as *mut Object,
        },
        tt_: 0,
    };
    v.tt_ = (0 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int) as u8;
    let mut io: *mut TValue = &mut k;
    let mut x_: *mut Table = (*(*fs).ls).h;
    (*io).value_.gc = x_ as *mut Object;
    (*io).tt_ = (5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 6 as libc::c_int) as u8;
    return addk(fs, &mut k, &mut v);
}

unsafe extern "C" fn fitsC(mut i: i64) -> libc::c_int {
    return ((i as u64).wrapping_add(
        (((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int >> 1 as libc::c_int) as u64,
    ) <= (((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int) as libc::c_uint
        as u64) as libc::c_int;
}

unsafe extern "C" fn fitsBx(mut i: i64) -> libc::c_int {
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
    mut fs: *mut FuncState,
    mut reg: libc::c_int,
    mut i: i64,
) -> Result<(), Box<dyn std::error::Error>> {
    if fitsBx(i) != 0 {
        codeAsBx(fs, OP_LOADI, reg, i as libc::c_int)?;
    } else {
        luaK_codek(fs, reg, luaK_intK(fs, i)?)?;
    };

    Ok(())
}

unsafe fn luaK_float(
    mut fs: *mut FuncState,
    mut reg: libc::c_int,
    mut f: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut fi: i64 = 0;
    if luaV_flttointeger(f, &mut fi, F2Ieq) != 0 && fitsBx(fi) != 0 {
        codeAsBx(fs, OP_LOADF, reg, fi as libc::c_int)?;
    } else {
        luaK_codek(fs, reg, luaK_numberK(fs, f)?)?;
    };

    Ok(())
}

unsafe extern "C" fn const2exp(mut v: *mut TValue, mut e: *mut expdesc) {
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
    mut fs: *mut FuncState,
    mut e: *mut expdesc,
    mut nresults: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut pc: *mut u32 = &mut *((*(*fs).f).code).offset((*e).u.info as isize) as *mut u32;
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

unsafe fn str2K(
    mut fs: *mut FuncState,
    mut e: *mut expdesc,
) -> Result<(), Box<dyn std::error::Error>> {
    (*e).u.info = stringK(fs, (*e).u.strval)?;
    (*e).k = VK;
    Ok(())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaK_setoneret(mut fs: *mut FuncState, mut e: *mut expdesc) {
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
    mut fs: *mut FuncState,
    mut e: *mut expdesc,
) -> Result<(), Box<dyn std::error::Error>> {
    match (*e).k as libc::c_uint {
        11 => {
            const2exp(const2val(fs, e), e);
        }
        9 => {
            let mut temp: libc::c_int = (*e).u.var.ridx as libc::c_int;
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
    mut fs: *mut FuncState,
    mut e: *mut expdesc,
    mut reg: c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    luaK_dischargevars(fs, e)?;
    let mut current_block_14: u64;
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
            let mut pc: *mut u32 = &mut *((*(*fs).f).code).offset((*e).u.info as isize) as *mut u32;
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

unsafe fn discharge2anyreg(
    mut fs: *mut FuncState,
    mut e: *mut expdesc,
) -> Result<(), Box<dyn std::error::Error>> {
    if (*e).k as libc::c_uint != VNONRELOC as libc::c_int as libc::c_uint {
        luaK_reserveregs(fs, 1 as libc::c_int)?;
        discharge2reg(fs, e, (*fs).freereg as libc::c_int - 1 as libc::c_int)?;
    }
    Ok(())
}

unsafe fn code_loadbool(
    mut fs: *mut FuncState,
    mut A: libc::c_int,
    mut op: OpCode,
) -> Result<c_int, Box<dyn std::error::Error>> {
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

unsafe extern "C" fn need_value(mut fs: *mut FuncState, mut list: libc::c_int) -> libc::c_int {
    while list != -(1 as libc::c_int) {
        let mut i: u32 = *getjumpcontrol(fs, list);
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
    mut fs: *mut FuncState,
    mut e: *mut expdesc,
    mut reg: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    discharge2reg(fs, e, reg)?;
    if (*e).k as libc::c_uint == VJMP as libc::c_int as libc::c_uint {
        luaK_concat(fs, &mut (*e).t, (*e).u.info)?;
    }
    if (*e).t != (*e).f {
        let mut final_0: libc::c_int = 0;
        let mut p_f: libc::c_int = -(1 as libc::c_int);
        let mut p_t: libc::c_int = -(1 as libc::c_int);
        if need_value(fs, (*e).t) != 0 || need_value(fs, (*e).f) != 0 {
            let mut fj: libc::c_int =
                if (*e).k as libc::c_uint == VJMP as libc::c_int as libc::c_uint {
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

pub unsafe fn luaK_exp2nextreg(
    mut fs: *mut FuncState,
    mut e: *mut expdesc,
) -> Result<(), Box<dyn std::error::Error>> {
    luaK_dischargevars(fs, e)?;
    freeexp(fs, e);
    luaK_reserveregs(fs, 1 as libc::c_int)?;
    exp2reg(fs, e, (*fs).freereg as libc::c_int - 1 as libc::c_int)
}

pub unsafe fn luaK_exp2anyreg(
    mut fs: *mut FuncState,
    mut e: *mut expdesc,
) -> Result<c_int, Box<dyn std::error::Error>> {
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

pub unsafe fn luaK_exp2anyregup(
    mut fs: *mut FuncState,
    mut e: *mut expdesc,
) -> Result<(), Box<dyn std::error::Error>> {
    if (*e).k as libc::c_uint != VUPVAL as libc::c_int as libc::c_uint || (*e).t != (*e).f {
        luaK_exp2anyreg(fs, e)?;
    }
    Ok(())
}

pub unsafe fn luaK_exp2val(
    mut fs: *mut FuncState,
    mut e: *mut expdesc,
) -> Result<(), Box<dyn std::error::Error>> {
    if (*e).t != (*e).f {
        luaK_exp2anyreg(fs, e)?;
    } else {
        luaK_dischargevars(fs, e)?;
    };
    Ok(())
}

unsafe fn luaK_exp2K(
    mut fs: *mut FuncState,
    mut e: *mut expdesc,
) -> Result<c_int, Box<dyn std::error::Error>> {
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

unsafe fn exp2RK(
    mut fs: *mut FuncState,
    mut e: *mut expdesc,
) -> Result<c_int, Box<dyn std::error::Error>> {
    if luaK_exp2K(fs, e)? != 0 {
        return Ok(1 as libc::c_int);
    } else {
        luaK_exp2anyreg(fs, e)?;
        return Ok(0 as libc::c_int);
    };
}

unsafe fn codeABRK(
    mut fs: *mut FuncState,
    mut o: OpCode,
    mut a: libc::c_int,
    mut b: libc::c_int,
    mut ec: *mut expdesc,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut k: libc::c_int = exp2RK(fs, ec)?;
    luaK_codeABCk(fs, o, a, b, (*ec).u.info, k)?;
    Ok(())
}

pub unsafe fn luaK_storevar(
    mut fs: *mut FuncState,
    mut var: *mut expdesc,
    mut ex: *mut expdesc,
) -> Result<(), Box<dyn std::error::Error>> {
    match (*var).k as libc::c_uint {
        9 => {
            freeexp(fs, ex);
            return exp2reg(fs, ex, (*var).u.var.ridx as libc::c_int);
        }
        10 => {
            let mut e: libc::c_int = luaK_exp2anyreg(fs, ex)?;
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
    mut fs: *mut FuncState,
    mut e: *mut expdesc,
    mut key: *mut expdesc,
) -> Result<(), Box<dyn std::error::Error>> {
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

unsafe extern "C" fn negatecondition(mut fs: *mut FuncState, mut e: *mut expdesc) {
    let mut pc: *mut u32 = getjumpcontrol(fs, (*e).u.info);
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
    mut fs: *mut FuncState,
    mut e: *mut expdesc,
    mut cond: libc::c_int,
) -> Result<c_int, Box<dyn std::error::Error>> {
    if (*e).k as libc::c_uint == VRELOC as libc::c_int as libc::c_uint {
        let mut ie: u32 = *((*(*fs).f).code).offset((*e).u.info as isize);
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

pub unsafe fn luaK_goiftrue(
    mut fs: *mut FuncState,
    mut e: *mut expdesc,
) -> Result<(), Box<dyn std::error::Error>> {
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

pub unsafe fn luaK_goiffalse(
    mut fs: *mut FuncState,
    mut e: *mut expdesc,
) -> Result<(), Box<dyn std::error::Error>> {
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

unsafe fn codenot(
    mut fs: *mut FuncState,
    mut e: *mut expdesc,
) -> Result<(), Box<dyn std::error::Error>> {
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
    let mut temp: libc::c_int = (*e).f;
    (*e).f = (*e).t;
    (*e).t = temp;
    removevalues(fs, (*e).f);
    removevalues(fs, (*e).t);
    Ok(())
}

unsafe extern "C" fn isKstr(mut fs: *mut FuncState, mut e: *mut expdesc) -> libc::c_int {
    return ((*e).k as libc::c_uint == VK as libc::c_int as libc::c_uint
        && !((*e).t != (*e).f)
        && (*e).u.info <= ((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int
        && (*((*(*fs).f).k).offset((*e).u.info as isize)).tt_ as libc::c_int
            == 4 as libc::c_int
                | (0 as libc::c_int) << 4 as libc::c_int
                | (1 as libc::c_int) << 6 as libc::c_int) as libc::c_int;
}

unsafe extern "C" fn isKint(mut e: *mut expdesc) -> libc::c_int {
    return ((*e).k as libc::c_uint == VKINT as libc::c_int as libc::c_uint && !((*e).t != (*e).f))
        as libc::c_int;
}

unsafe extern "C" fn isCint(mut e: *mut expdesc) -> libc::c_int {
    return (isKint(e) != 0
        && (*e).u.ival as u64
            <= (((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int) as u64)
        as libc::c_int;
}

unsafe extern "C" fn isSCint(mut e: *mut expdesc) -> libc::c_int {
    return (isKint(e) != 0 && fitsC((*e).u.ival) != 0) as libc::c_int;
}

unsafe extern "C" fn isSCnumber(
    mut e: *mut expdesc,
    mut pi: *mut libc::c_int,
    mut isfloat: *mut libc::c_int,
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
    mut fs: *mut FuncState,
    mut t: *mut expdesc,
    mut k: *mut expdesc,
) -> Result<(), Box<dyn std::error::Error>> {
    if (*k).k as libc::c_uint == VKSTR as libc::c_int as libc::c_uint {
        str2K(fs, k)?;
    }
    if (*t).k as libc::c_uint == VUPVAL as libc::c_int as libc::c_uint && isKstr(fs, k) == 0 {
        luaK_exp2anyreg(fs, t)?;
    }
    if (*t).k as libc::c_uint == VUPVAL as libc::c_int as libc::c_uint {
        let mut temp: libc::c_int = (*t).u.info;
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

unsafe extern "C" fn validop(
    mut op: libc::c_int,
    mut v1: *mut TValue,
    mut v2: *mut TValue,
) -> libc::c_int {
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
    mut fs: *mut FuncState,
    mut op: libc::c_int,
    mut e1: *mut expdesc,
    mut e2: *const expdesc,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut v1: TValue = TValue {
        value_: Value {
            gc: 0 as *mut Object,
        },
        tt_: 0,
    };
    let mut v2: TValue = TValue {
        value_: Value {
            gc: 0 as *mut Object,
        },
        tt_: 0,
    };
    let mut res: TValue = TValue {
        value_: Value {
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
    luaO_rawarith((*(*fs).ls).L, op, &mut v1, &mut v2, &mut res)?;
    if res.tt_ as libc::c_int == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int {
        (*e1).k = VKINT;
        (*e1).u.ival = res.value_.i;
    } else {
        let mut n: f64 = res.value_.n;
        if !(n == n) || n == 0 as libc::c_int as f64 {
            return Ok(0 as libc::c_int);
        }
        (*e1).k = VKFLT;
        (*e1).u.nval = n;
    }
    return Ok(1 as libc::c_int);
}

#[inline]
unsafe extern "C" fn binopr2op(mut opr: BinOpr, mut baser: BinOpr, mut base: OpCode) -> OpCode {
    return (opr as libc::c_int - baser as libc::c_int + base as libc::c_int) as OpCode;
}

#[inline]
unsafe extern "C" fn unopr2op(mut opr: UnOpr) -> OpCode {
    return (opr as libc::c_int - OPR_MINUS as libc::c_int + OP_UNM as libc::c_int) as OpCode;
}

#[inline]
unsafe extern "C" fn binopr2TM(mut opr: BinOpr) -> TMS {
    return (opr as libc::c_int - OPR_ADD as libc::c_int + TM_ADD as libc::c_int) as TMS;
}

unsafe fn codeunexpval(
    mut fs: *mut FuncState,
    mut op: OpCode,
    mut e: *mut expdesc,
    mut line: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut r: libc::c_int = luaK_exp2anyreg(fs, e)?;
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
    mut fs: *mut FuncState,
    mut e1: *mut expdesc,
    mut e2: *mut expdesc,
    mut op: OpCode,
    mut v2: libc::c_int,
    mut flip: libc::c_int,
    mut line: libc::c_int,
    mut mmop: OpCode,
    mut event: TMS,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut v1: libc::c_int = luaK_exp2anyreg(fs, e1)?;
    let mut pc: libc::c_int = luaK_codeABCk(fs, op, 0 as libc::c_int, v1, v2, 0 as libc::c_int)?;
    freeexps(fs, e1, e2);
    (*e1).u.info = pc;
    (*e1).k = VRELOC;
    luaK_fixline(fs, line)?;
    luaK_codeABCk(fs, mmop, v1, v2, event as libc::c_int, flip)?;
    luaK_fixline(fs, line)
}

unsafe fn codebinexpval(
    mut fs: *mut FuncState,
    mut opr: BinOpr,
    mut e1: *mut expdesc,
    mut e2: *mut expdesc,
    mut line: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut op: OpCode = binopr2op(opr, OPR_ADD, OP_ADD);
    let mut v2: libc::c_int = luaK_exp2anyreg(fs, e2)?;
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
    mut fs: *mut FuncState,
    mut op: OpCode,
    mut e1: *mut expdesc,
    mut e2: *mut expdesc,
    mut flip: libc::c_int,
    mut line: libc::c_int,
    mut event: TMS,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut v2: libc::c_int = (*e2).u.ival as libc::c_int
        + (((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int >> 1 as libc::c_int);
    finishbinexpval(fs, e1, e2, op, v2, flip, line, OP_MMBINI, event)
}

unsafe fn codebinK(
    mut fs: *mut FuncState,
    mut opr: BinOpr,
    mut e1: *mut expdesc,
    mut e2: *mut expdesc,
    mut flip: libc::c_int,
    mut line: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut event: TMS = binopr2TM(opr);
    let mut v2: libc::c_int = (*e2).u.info;
    let mut op: OpCode = binopr2op(opr, OPR_ADD, OP_ADDK);
    finishbinexpval(fs, e1, e2, op, v2, flip, line, OP_MMBINK, event)
}

unsafe fn finishbinexpneg(
    mut fs: *mut FuncState,
    mut e1: *mut expdesc,
    mut e2: *mut expdesc,
    mut op: OpCode,
    mut line: libc::c_int,
    mut event: TMS,
) -> Result<c_int, Box<dyn std::error::Error>> {
    if isKint(e2) == 0 {
        return Ok(0 as libc::c_int);
    } else {
        let mut i2: i64 = (*e2).u.ival;
        if !(fitsC(i2) != 0 && fitsC(-i2) != 0) {
            return Ok(0 as libc::c_int);
        } else {
            let mut v2: libc::c_int = i2 as libc::c_int;
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

unsafe extern "C" fn swapexps(mut e1: *mut expdesc, mut e2: *mut expdesc) {
    let mut temp: expdesc = *e1;
    *e1 = *e2;
    *e2 = temp;
}

unsafe fn codebinNoK(
    mut fs: *mut FuncState,
    mut opr: BinOpr,
    mut e1: *mut expdesc,
    mut e2: *mut expdesc,
    mut flip: libc::c_int,
    mut line: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    if flip != 0 {
        swapexps(e1, e2);
    }
    codebinexpval(fs, opr, e1, e2, line)
}

unsafe fn codearith(
    mut fs: *mut FuncState,
    mut opr: BinOpr,
    mut e1: *mut expdesc,
    mut e2: *mut expdesc,
    mut flip: libc::c_int,
    mut line: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    if tonumeral(e2, 0 as *mut TValue) != 0 && luaK_exp2K(fs, e2)? != 0 {
        codebinK(fs, opr, e1, e2, flip, line)
    } else {
        codebinNoK(fs, opr, e1, e2, flip, line)
    }
}

unsafe fn codecommutative(
    mut fs: *mut FuncState,
    mut op: BinOpr,
    mut e1: *mut expdesc,
    mut e2: *mut expdesc,
    mut line: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut flip: libc::c_int = 0 as libc::c_int;
    if tonumeral(e1, 0 as *mut TValue) != 0 {
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
    mut fs: *mut FuncState,
    mut opr: BinOpr,
    mut e1: *mut expdesc,
    mut e2: *mut expdesc,
    mut line: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
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
    mut fs: *mut FuncState,
    mut opr: BinOpr,
    mut e1: *mut expdesc,
    mut e2: *mut expdesc,
) -> Result<(), Box<dyn std::error::Error>> {
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
    mut fs: *mut FuncState,
    mut opr: BinOpr,
    mut e1: *mut expdesc,
    mut e2: *mut expdesc,
) -> Result<(), Box<dyn std::error::Error>> {
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
    mut fs: *mut FuncState,
    mut opr: UnOpr,
    mut e: *mut expdesc,
    mut line: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    static mut ef: expdesc = {
        let mut init = expdesc {
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
    let mut current_block_3: u64;
    match opr as libc::c_uint {
        0 | 1 => {
            if constfolding(
                fs,
                (opr as libc::c_uint).wrapping_add(12 as libc::c_int as libc::c_uint)
                    as libc::c_int,
                e,
                &raw mut ef,
            )? != 0
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
    mut fs: *mut FuncState,
    mut op: BinOpr,
    mut v: *mut expdesc,
) -> Result<(), Box<dyn std::error::Error>> {
    luaK_dischargevars(fs, v)?;
    match op as libc::c_uint {
        19 => luaK_goiftrue(fs, v)?,
        20 => luaK_goiffalse(fs, v)?,
        12 => luaK_exp2nextreg(fs, v)?,
        0 | 1 | 2 | 5 | 6 | 3 | 4 | 7 | 8 | 9 | 10 | 11 => {
            if tonumeral(v, 0 as *mut TValue) == 0 {
                luaK_exp2anyreg(fs, v)?;
            }
        }
        13 | 16 => {
            if tonumeral(v, 0 as *mut TValue) == 0 {
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
    mut fs: *mut FuncState,
    mut e1: *mut expdesc,
    mut e2: *mut expdesc,
    mut line: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut ie2: *mut u32 = previousinstruction(fs);
    if (*ie2 >> 0 as libc::c_int
        & !(!(0 as libc::c_int as u32) << 7 as libc::c_int) << 0 as libc::c_int) as OpCode
        as libc::c_uint
        == OP_CONCAT as libc::c_int as libc::c_uint
    {
        let mut n: libc::c_int = (*ie2
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
    mut fs: *mut FuncState,
    mut opr: BinOpr,
    mut e1: *mut expdesc,
    mut e2: *mut expdesc,
    mut line: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    luaK_dischargevars(fs, e2)?;
    if opr as libc::c_uint <= OPR_SHR as libc::c_int as libc::c_uint
        && constfolding(
            fs,
            (opr as libc::c_uint).wrapping_add(0 as libc::c_int as libc::c_uint) as libc::c_int,
            e1,
            e2,
        )? != 0
    {
        return Ok(());
    }
    let mut current_block_30: u64;
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

pub unsafe fn luaK_fixline(
    mut fs: *mut FuncState,
    mut line: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    removelastlineinfo(fs);
    savelineinfo(fs, (*fs).f, line)
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaK_settablesize(
    mut fs: *mut FuncState,
    mut pc: libc::c_int,
    mut ra: libc::c_int,
    mut asize: libc::c_int,
    mut hsize: libc::c_int,
) {
    let mut inst: *mut u32 = &mut *((*(*fs).f).code).offset(pc as isize) as *mut u32;
    let mut rb: libc::c_int = if hsize != 0 as libc::c_int {
        luaO_ceillog2(hsize as libc::c_uint) + 1 as libc::c_int
    } else {
        0 as libc::c_int
    };
    let mut extra: libc::c_int =
        asize / (((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int + 1 as libc::c_int);
    let mut rc: libc::c_int =
        asize % (((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int + 1 as libc::c_int);
    let mut k: libc::c_int = (extra > 0 as libc::c_int) as libc::c_int;
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
    mut fs: *mut FuncState,
    mut base: libc::c_int,
    mut nelems: libc::c_int,
    mut tostore: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    if tostore == -(1 as libc::c_int) {
        tostore = 0 as libc::c_int;
    }
    if nelems <= ((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int {
        luaK_codeABCk(fs, OP_SETLIST, base, tostore, nelems, 0 as libc::c_int)?;
    } else {
        let mut extra: libc::c_int = nelems
            / (((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int + 1 as libc::c_int);
        nelems %= ((1 as libc::c_int) << 8 as libc::c_int) - 1 as libc::c_int + 1 as libc::c_int;
        luaK_codeABCk(fs, OP_SETLIST, base, tostore, nelems, 1 as libc::c_int)?;
        codeextraarg(fs, extra)?;
    }
    (*fs).freereg = (base + 1 as libc::c_int) as u8;
    Ok(())
}

unsafe extern "C" fn finaltarget(mut code: *mut u32, mut i: libc::c_int) -> libc::c_int {
    let mut count: libc::c_int = 0;
    count = 0 as libc::c_int;
    while count < 100 as libc::c_int {
        let mut pc: u32 = *code.offset(i as isize);
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
        count;
    }
    return i;
}

pub unsafe fn luaK_finish(mut fs: *mut FuncState) -> Result<(), Box<dyn std::error::Error>> {
    let mut i: libc::c_int = 0;
    let mut p: *mut Proto = (*fs).f;
    i = 0 as libc::c_int;
    while i < (*fs).pc {
        let mut pc: *mut u32 = &mut *((*p).code).offset(i as isize) as *mut u32;
        let mut current_block_7: u64;
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
                let mut target: libc::c_int = finaltarget((*p).code, i);
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
        i;
    }
    Ok(())
}
