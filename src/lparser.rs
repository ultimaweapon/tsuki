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
#![allow(path_statements)]

use crate::lcode::{
    BinOpr, OPR_ADD, OPR_AND, OPR_BAND, OPR_BNOT, OPR_BOR, OPR_BXOR, OPR_CONCAT, OPR_DIV, OPR_EQ,
    OPR_GE, OPR_GT, OPR_IDIV, OPR_LE, OPR_LEN, OPR_LT, OPR_MINUS, OPR_MOD, OPR_MUL, OPR_NE,
    OPR_NOBINOPR, OPR_NOT, OPR_NOUNOPR, OPR_OR, OPR_POW, OPR_SHL, OPR_SHR, OPR_SUB, UnOpr,
    luaK_checkstack, luaK_code, luaK_codeABCk, luaK_codeABx, luaK_concat, luaK_dischargevars,
    luaK_exp2anyreg, luaK_exp2anyregup, luaK_exp2const, luaK_exp2nextreg, luaK_exp2val,
    luaK_finish, luaK_fixline, luaK_getlabel, luaK_goiffalse, luaK_goiftrue, luaK_indexed,
    luaK_infix, luaK_int, luaK_jump, luaK_nil, luaK_patchlist, luaK_patchtohere, luaK_posfix,
    luaK_prefix, luaK_reserveregs, luaK_ret, luaK_self, luaK_semerror, luaK_setlist,
    luaK_setoneret, luaK_setreturns, luaK_settablesize, luaK_storevar,
};
use crate::ldo::luaD_inctop;
use crate::lfunc::{luaF_newLclosure, luaF_newproto};
use crate::lgc::{luaC_barrier_, luaC_step};
use crate::llex::{
    LexState, SemInfo, TK_BREAK, TK_DBCOLON, TK_DO, TK_ELSE, TK_ELSEIF, TK_END, TK_EOS, TK_FOR,
    TK_FUNCTION, TK_IF, TK_IN, TK_NAME, TK_REPEAT, TK_RETURN, TK_THEN, TK_UNTIL, TK_WHILE, Token,
    luaX_lookahead, luaX_newstring, luaX_next, luaX_setinput, luaX_syntaxerror, luaX_token2str,
};
use crate::lmem::{luaM_growaux_, luaM_shrinkvector_};
use crate::lobject::{
    AbsLineInfo, LClosure, LocVar, Proto, TString, TValue, Table, Upvaldesc, Value,
};
use crate::lopcodes::{
    OP_CALL, OP_CLOSE, OP_CLOSURE, OP_FORLOOP, OP_FORPREP, OP_GETUPVAL, OP_MOVE, OP_NEWTABLE,
    OP_TAILCALL, OP_TBC, OP_TFORCALL, OP_TFORLOOP, OP_TFORPREP, OP_VARARG, OP_VARARGPREP, OpCode,
};
use crate::lstate::{GCUnion, lua_State, luaE_incCstack};
use crate::lstring::{luaS_new, luaS_newlstr};
use crate::ltable::luaH_new;
use crate::lzio::{Mbuffer, ZIO};
use libc::strcmp;
use std::borrow::Cow;
use std::ffi::CStr;
use std::fmt::Display;

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Dyndata {
    pub actvar: C2RustUnnamed_9,
    pub gt: Labellist,
    pub label: Labellist,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Labellist {
    pub arr: *mut Labeldesc,
    pub n: libc::c_int,
    pub size: libc::c_int,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Labeldesc {
    pub name: *mut TString,
    pub pc: libc::c_int,
    pub line: libc::c_int,
    pub nactvar: u8,
    pub close: u8,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct C2RustUnnamed_9 {
    pub arr: *mut Vardesc,
    pub n: libc::c_int,
    pub size: libc::c_int,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub union Vardesc {
    pub vd: C2RustUnnamed_10,
    pub k: TValue,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct C2RustUnnamed_10 {
    pub value_: Value,
    pub tt_: u8,
    pub kind: u8,
    pub ridx: u8,
    pub pidx: libc::c_short,
    pub name: *mut TString,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct FuncState {
    pub f: *mut Proto,
    pub prev: *mut FuncState,
    pub ls: *mut LexState,
    pub bl: *mut BlockCnt,
    pub pc: libc::c_int,
    pub lasttarget: libc::c_int,
    pub previousline: libc::c_int,
    pub nk: libc::c_int,
    pub np: libc::c_int,
    pub nabslineinfo: libc::c_int,
    pub firstlocal: libc::c_int,
    pub firstlabel: libc::c_int,
    pub ndebugvars: libc::c_short,
    pub nactvar: u8,
    pub nups: u8,
    pub freereg: u8,
    pub iwthabs: u8,
    pub needclose: u8,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct BlockCnt {
    pub previous: *mut BlockCnt,
    pub firstlabel: libc::c_int,
    pub firstgoto: libc::c_int,
    pub nactvar: u8,
    pub upval: u8,
    pub isloop: u8,
    pub insidetbc: u8,
}

pub type expkind = libc::c_uint;
pub const VVARARG: expkind = 19;
pub const VCALL: expkind = 18;
pub const VRELOC: expkind = 17;
pub const VJMP: expkind = 16;
pub const VINDEXSTR: expkind = 15;
pub const VINDEXI: expkind = 14;
pub const VINDEXUP: expkind = 13;
pub const VINDEXED: expkind = 12;
pub const VCONST: expkind = 11;
pub const VUPVAL: expkind = 10;
pub const VLOCAL: expkind = 9;
pub const VNONRELOC: expkind = 8;
pub const VKSTR: expkind = 7;
pub const VKINT: expkind = 6;
pub const VKFLT: expkind = 5;
pub const VK: expkind = 4;
pub const VFALSE: expkind = 3;
pub const VTRUE: expkind = 2;
pub const VNIL: expkind = 1;
pub const VVOID: expkind = 0;

#[derive(Copy, Clone)]
#[repr(C)]
pub struct expdesc {
    pub k: expkind,
    pub u: C2RustUnnamed_11,
    pub t: libc::c_int,
    pub f: libc::c_int,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub union C2RustUnnamed_11 {
    pub ival: i64,
    pub nval: f64,
    pub strval: *mut TString,
    pub info: libc::c_int,
    pub ind: C2RustUnnamed_13,
    pub var: C2RustUnnamed_12,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct C2RustUnnamed_12 {
    pub ridx: u8,
    pub vidx: libc::c_ushort,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct C2RustUnnamed_13 {
    pub idx: libc::c_short,
    pub t: u8,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct LHS_assign {
    pub prev: *mut LHS_assign,
    pub v: expdesc,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct C2RustUnnamed_14 {
    pub left: u8,
    pub right: u8,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct ConsControl {
    pub v: expdesc,
    pub t: *mut expdesc,
    pub nh: libc::c_int,
    pub na: libc::c_int,
    pub tostore: libc::c_int,
}

unsafe extern "C" fn error_expected(mut ls: *mut LexState, mut token: libc::c_int) -> ! {
    luaX_syntaxerror(ls, format_args!("{} expected", luaX_token2str(ls, token)));
}

unsafe extern "C" fn errorlimit(
    mut fs: *mut FuncState,
    limit: libc::c_int,
    what: impl Display,
) -> ! {
    let mut L: *mut lua_State = (*(*fs).ls).L;
    let mut line: libc::c_int = (*(*fs).f).linedefined;
    let where_0: Cow<'static, str> = if line == 0 as libc::c_int {
        "main function".into()
    } else {
        format!("function at line {line}").into()
    };

    luaX_syntaxerror(
        (*fs).ls,
        format_args!("too many {what} (limit is {limit}) in {where_0}"),
    );
}

unsafe extern "C" fn checklimit(
    mut fs: *mut FuncState,
    mut v: libc::c_int,
    mut l: libc::c_int,
    what: impl Display,
) {
    if v > l {
        errorlimit(fs, l, what);
    }
}

unsafe extern "C" fn testnext(mut ls: *mut LexState, mut c: libc::c_int) -> libc::c_int {
    if (*ls).t.token == c {
        luaX_next(ls);
        return 1 as libc::c_int;
    } else {
        return 0 as libc::c_int;
    };
}

unsafe extern "C" fn check(mut ls: *mut LexState, mut c: libc::c_int) {
    if (*ls).t.token != c {
        error_expected(ls, c);
    }
}

unsafe extern "C" fn checknext(mut ls: *mut LexState, mut c: libc::c_int) {
    check(ls, c);
    luaX_next(ls);
}

unsafe extern "C" fn check_match(
    mut ls: *mut LexState,
    mut what: libc::c_int,
    mut who: libc::c_int,
    mut where_0: libc::c_int,
) {
    if ((testnext(ls, what) == 0) as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        if where_0 == (*ls).linenumber {
            error_expected(ls, what);
        } else {
            luaX_syntaxerror(
                ls,
                format_args!(
                    "{} expected (to close {} at line {})",
                    luaX_token2str(ls, what),
                    luaX_token2str(ls, who),
                    where_0,
                ),
            );
        }
    }
}

unsafe extern "C" fn str_checkname(mut ls: *mut LexState) -> *mut TString {
    let mut ts: *mut TString = 0 as *mut TString;
    check(ls, TK_NAME as libc::c_int);
    ts = (*ls).t.seminfo.ts;
    luaX_next(ls);
    return ts;
}

unsafe extern "C" fn init_exp(mut e: *mut expdesc, mut k: expkind, mut i: libc::c_int) {
    (*e).t = -(1 as libc::c_int);
    (*e).f = (*e).t;
    (*e).k = k;
    (*e).u.info = i;
}

unsafe extern "C" fn codestring(mut e: *mut expdesc, mut s: *mut TString) {
    (*e).t = -(1 as libc::c_int);
    (*e).f = (*e).t;
    (*e).k = VKSTR;
    (*e).u.strval = s;
}

unsafe extern "C" fn codename(mut ls: *mut LexState, mut e: *mut expdesc) {
    codestring(e, str_checkname(ls));
}

unsafe extern "C" fn registerlocalvar(
    mut ls: *mut LexState,
    mut fs: *mut FuncState,
    mut varname: *mut TString,
) -> libc::c_int {
    let mut f: *mut Proto = (*fs).f;
    let mut oldsize: libc::c_int = (*f).sizelocvars;
    (*f).locvars = luaM_growaux_(
        (*ls).L,
        (*f).locvars as *mut libc::c_void,
        (*fs).ndebugvars as libc::c_int,
        &mut (*f).sizelocvars,
        ::core::mem::size_of::<LocVar>() as libc::c_ulong as libc::c_int,
        (if 32767 as libc::c_int as usize
            <= (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<LocVar>())
        {
            32767 as libc::c_int as libc::c_uint
        } else {
            (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<LocVar>())
                as libc::c_uint
        }) as libc::c_int,
        b"local variables\0" as *const u8 as *const libc::c_char,
    ) as *mut LocVar;
    while oldsize < (*f).sizelocvars {
        let fresh0 = oldsize;
        oldsize = oldsize + 1;
        let ref mut fresh1 = (*((*f).locvars).offset(fresh0 as isize)).varname;
        *fresh1 = 0 as *mut TString;
    }
    let ref mut fresh2 = (*((*f).locvars).offset((*fs).ndebugvars as isize)).varname;
    *fresh2 = varname;
    (*((*f).locvars).offset((*fs).ndebugvars as isize)).startpc = (*fs).pc;
    if (*f).marked as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
        && (*varname).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            != 0
    {
        luaC_barrier_(
            (*ls).L,
            &mut (*(f as *mut GCUnion)).gc,
            &mut (*(varname as *mut GCUnion)).gc,
        );
    } else {
    };
    let fresh3 = (*fs).ndebugvars;
    (*fs).ndebugvars = (*fs).ndebugvars + 1;
    return fresh3 as libc::c_int;
}

unsafe extern "C" fn new_localvar(mut ls: *mut LexState, mut name: *mut TString) -> libc::c_int {
    let mut L: *mut lua_State = (*ls).L;
    let mut fs: *mut FuncState = (*ls).fs;
    let mut dyd: *mut Dyndata = (*ls).dyd;
    let mut var: *mut Vardesc = 0 as *mut Vardesc;
    checklimit(
        fs,
        (*dyd).actvar.n + 1 as libc::c_int - (*fs).firstlocal,
        200 as libc::c_int,
        "local variables",
    );
    (*dyd).actvar.arr = luaM_growaux_(
        L,
        (*dyd).actvar.arr as *mut libc::c_void,
        (*dyd).actvar.n + 1 as libc::c_int,
        &mut (*dyd).actvar.size,
        ::core::mem::size_of::<Vardesc>() as libc::c_ulong as libc::c_int,
        (if 65535 as libc::c_int as usize
            <= (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<Vardesc>())
        {
            65535 as libc::c_int as libc::c_uint
        } else {
            (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<Vardesc>())
                as libc::c_uint
        }) as libc::c_int,
        b"local variables\0" as *const u8 as *const libc::c_char,
    ) as *mut Vardesc;
    let fresh4 = (*dyd).actvar.n;
    (*dyd).actvar.n = (*dyd).actvar.n + 1;
    var = &mut *((*dyd).actvar.arr).offset(fresh4 as isize) as *mut Vardesc;
    (*var).vd.kind = 0 as libc::c_int as u8;
    (*var).vd.name = name;
    return (*dyd).actvar.n - 1 as libc::c_int - (*fs).firstlocal;
}

unsafe extern "C" fn getlocalvardesc(
    mut fs: *mut FuncState,
    mut vidx: libc::c_int,
) -> *mut Vardesc {
    return &mut *((*(*(*fs).ls).dyd).actvar.arr).offset(((*fs).firstlocal + vidx) as isize)
        as *mut Vardesc;
}

unsafe extern "C" fn reglevel(mut fs: *mut FuncState, mut nvar: libc::c_int) -> libc::c_int {
    loop {
        let fresh5 = nvar;
        nvar = nvar - 1;
        if !(fresh5 > 0 as libc::c_int) {
            break;
        }
        let mut vd: *mut Vardesc = getlocalvardesc(fs, nvar);
        if (*vd).vd.kind as libc::c_int != 3 as libc::c_int {
            return (*vd).vd.ridx as libc::c_int + 1 as libc::c_int;
        }
    }
    return 0 as libc::c_int;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaY_nvarstack(mut fs: *mut FuncState) -> libc::c_int {
    return reglevel(fs, (*fs).nactvar as libc::c_int);
}

unsafe extern "C" fn localdebuginfo(mut fs: *mut FuncState, mut vidx: libc::c_int) -> *mut LocVar {
    let mut vd: *mut Vardesc = getlocalvardesc(fs, vidx);
    if (*vd).vd.kind as libc::c_int == 3 as libc::c_int {
        return 0 as *mut LocVar;
    } else {
        let mut idx: libc::c_int = (*vd).vd.pidx as libc::c_int;
        return &mut *((*(*fs).f).locvars).offset(idx as isize) as *mut LocVar;
    };
}

unsafe extern "C" fn init_var(mut fs: *mut FuncState, mut e: *mut expdesc, mut vidx: libc::c_int) {
    (*e).t = -(1 as libc::c_int);
    (*e).f = (*e).t;
    (*e).k = VLOCAL;
    (*e).u.var.vidx = vidx as libc::c_ushort;
    (*e).u.var.ridx = (*getlocalvardesc(fs, vidx)).vd.ridx;
}

unsafe extern "C" fn check_readonly(mut ls: *mut LexState, mut e: *mut expdesc) {
    let mut fs: *mut FuncState = (*ls).fs;
    let mut varname: *mut TString = 0 as *mut TString;
    match (*e).k as libc::c_uint {
        11 => {
            varname = (*((*(*ls).dyd).actvar.arr).offset((*e).u.info as isize))
                .vd
                .name;
        }
        9 => {
            let mut vardesc: *mut Vardesc = getlocalvardesc(fs, (*e).u.var.vidx as libc::c_int);
            if (*vardesc).vd.kind as libc::c_int != 0 as libc::c_int {
                varname = (*vardesc).vd.name;
            }
        }
        10 => {
            let mut up: *mut Upvaldesc =
                &mut *((*(*fs).f).upvalues).offset((*e).u.info as isize) as *mut Upvaldesc;
            if (*up).kind as libc::c_int != 0 as libc::c_int {
                varname = (*up).name;
            }
        }
        _ => return,
    }

    if !varname.is_null() {
        luaK_semerror(
            ls,
            format_args!(
                "attempt to assign to const variable '{}'",
                CStr::from_ptr(((*varname).contents).as_mut_ptr()).to_string_lossy(),
            ),
        );
    }
}

unsafe extern "C" fn adjustlocalvars(mut ls: *mut LexState, mut nvars: libc::c_int) {
    let mut fs: *mut FuncState = (*ls).fs;
    let mut reglevel_0: libc::c_int = luaY_nvarstack(fs);
    let mut i: libc::c_int = 0;
    i = 0 as libc::c_int;
    while i < nvars {
        let fresh6 = (*fs).nactvar;
        (*fs).nactvar = ((*fs).nactvar).wrapping_add(1);
        let mut vidx: libc::c_int = fresh6 as libc::c_int;
        let mut var: *mut Vardesc = getlocalvardesc(fs, vidx);
        let fresh7 = reglevel_0;
        reglevel_0 = reglevel_0 + 1;
        (*var).vd.ridx = fresh7 as u8;
        (*var).vd.pidx = registerlocalvar(ls, fs, (*var).vd.name) as libc::c_short;
        i += 1;
        i;
    }
}

unsafe extern "C" fn removevars(mut fs: *mut FuncState, mut tolevel: libc::c_int) {
    (*(*(*fs).ls).dyd).actvar.n -= (*fs).nactvar as libc::c_int - tolevel;
    while (*fs).nactvar as libc::c_int > tolevel {
        (*fs).nactvar = ((*fs).nactvar).wrapping_sub(1);
        let mut var: *mut LocVar = localdebuginfo(fs, (*fs).nactvar as libc::c_int);
        if !var.is_null() {
            (*var).endpc = (*fs).pc;
        }
    }
}

unsafe extern "C" fn searchupvalue(mut fs: *mut FuncState, mut name: *mut TString) -> libc::c_int {
    let mut i: libc::c_int = 0;
    let mut up: *mut Upvaldesc = (*(*fs).f).upvalues;
    i = 0 as libc::c_int;
    while i < (*fs).nups as libc::c_int {
        if (*up.offset(i as isize)).name == name {
            return i;
        }
        i += 1;
        i;
    }
    return -(1 as libc::c_int);
}

unsafe extern "C" fn allocupvalue(mut fs: *mut FuncState) -> *mut Upvaldesc {
    let mut f: *mut Proto = (*fs).f;
    let mut oldsize: libc::c_int = (*f).sizeupvalues;
    checklimit(
        fs,
        (*fs).nups as libc::c_int + 1 as libc::c_int,
        255 as libc::c_int,
        "upvalues",
    );
    (*f).upvalues = luaM_growaux_(
        (*(*fs).ls).L,
        (*f).upvalues as *mut libc::c_void,
        (*fs).nups as libc::c_int,
        &mut (*f).sizeupvalues,
        ::core::mem::size_of::<Upvaldesc>() as libc::c_ulong as libc::c_int,
        (if 255 as libc::c_int as usize
            <= (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<Upvaldesc>())
        {
            255 as libc::c_int as libc::c_uint
        } else {
            (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<Upvaldesc>())
                as libc::c_uint
        }) as libc::c_int,
        b"upvalues\0" as *const u8 as *const libc::c_char,
    ) as *mut Upvaldesc;
    while oldsize < (*f).sizeupvalues {
        let fresh8 = oldsize;
        oldsize = oldsize + 1;
        let ref mut fresh9 = (*((*f).upvalues).offset(fresh8 as isize)).name;
        *fresh9 = 0 as *mut TString;
    }
    let fresh10 = (*fs).nups;
    (*fs).nups = ((*fs).nups).wrapping_add(1);
    return &mut *((*f).upvalues).offset(fresh10 as isize) as *mut Upvaldesc;
}

unsafe extern "C" fn newupvalue(
    mut fs: *mut FuncState,
    mut name: *mut TString,
    mut v: *mut expdesc,
) -> libc::c_int {
    let mut up: *mut Upvaldesc = allocupvalue(fs);
    let mut prev: *mut FuncState = (*fs).prev;
    if (*v).k as libc::c_uint == VLOCAL as libc::c_int as libc::c_uint {
        (*up).instack = 1 as libc::c_int as u8;
        (*up).idx = (*v).u.var.ridx;
        (*up).kind = (*getlocalvardesc(prev, (*v).u.var.vidx as libc::c_int))
            .vd
            .kind;
    } else {
        (*up).instack = 0 as libc::c_int as u8;
        (*up).idx = (*v).u.info as u8;
        (*up).kind = (*((*(*prev).f).upvalues).offset((*v).u.info as isize)).kind;
    }
    (*up).name = name;
    if (*(*fs).f).marked as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
        && (*name).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            != 0
    {
        luaC_barrier_(
            (*(*fs).ls).L,
            &mut (*((*fs).f as *mut GCUnion)).gc,
            &mut (*(name as *mut GCUnion)).gc,
        );
    } else {
    };
    return (*fs).nups as libc::c_int - 1 as libc::c_int;
}

unsafe extern "C" fn searchvar(
    mut fs: *mut FuncState,
    mut n: *mut TString,
    mut var: *mut expdesc,
) -> libc::c_int {
    let mut i: libc::c_int = 0;
    i = (*fs).nactvar as libc::c_int - 1 as libc::c_int;
    while i >= 0 as libc::c_int {
        let mut vd: *mut Vardesc = getlocalvardesc(fs, i);
        if n == (*vd).vd.name {
            if (*vd).vd.kind as libc::c_int == 3 as libc::c_int {
                init_exp(var, VCONST, (*fs).firstlocal + i);
            } else {
                init_var(fs, var, i);
            }
            return (*var).k as libc::c_int;
        }
        i -= 1;
        i;
    }
    return -(1 as libc::c_int);
}

unsafe extern "C" fn markupval(mut fs: *mut FuncState, mut level: libc::c_int) {
    let mut bl: *mut BlockCnt = (*fs).bl;
    while (*bl).nactvar as libc::c_int > level {
        bl = (*bl).previous;
    }
    (*bl).upval = 1 as libc::c_int as u8;
    (*fs).needclose = 1 as libc::c_int as u8;
}

unsafe extern "C" fn marktobeclosed(mut fs: *mut FuncState) {
    let mut bl: *mut BlockCnt = (*fs).bl;
    (*bl).upval = 1 as libc::c_int as u8;
    (*bl).insidetbc = 1 as libc::c_int as u8;
    (*fs).needclose = 1 as libc::c_int as u8;
}

unsafe extern "C" fn singlevaraux(
    mut fs: *mut FuncState,
    mut n: *mut TString,
    mut var: *mut expdesc,
    mut base: libc::c_int,
) {
    if fs.is_null() {
        init_exp(var, VVOID, 0 as libc::c_int);
    } else {
        let mut v: libc::c_int = searchvar(fs, n, var);
        if v >= 0 as libc::c_int {
            if v == VLOCAL as libc::c_int && base == 0 {
                markupval(fs, (*var).u.var.vidx as libc::c_int);
            }
        } else {
            let mut idx: libc::c_int = searchupvalue(fs, n);
            if idx < 0 as libc::c_int {
                singlevaraux((*fs).prev, n, var, 0 as libc::c_int);
                if (*var).k as libc::c_uint == VLOCAL as libc::c_int as libc::c_uint
                    || (*var).k as libc::c_uint == VUPVAL as libc::c_int as libc::c_uint
                {
                    idx = newupvalue(fs, n, var);
                } else {
                    return;
                }
            }
            init_exp(var, VUPVAL, idx);
        }
    };
}

unsafe extern "C" fn singlevar(mut ls: *mut LexState, mut var: *mut expdesc) {
    let mut varname: *mut TString = str_checkname(ls);
    let mut fs: *mut FuncState = (*ls).fs;
    singlevaraux(fs, varname, var, 1 as libc::c_int);
    if (*var).k as libc::c_uint == VVOID as libc::c_int as libc::c_uint {
        let mut key: expdesc = expdesc {
            k: VVOID,
            u: C2RustUnnamed_11 { ival: 0 },
            t: 0,
            f: 0,
        };
        singlevaraux(fs, (*ls).envn, var, 1 as libc::c_int);
        luaK_exp2anyregup(fs, var);
        codestring(&mut key, varname);
        luaK_indexed(fs, var, &mut key);
    }
}

unsafe extern "C" fn adjust_assign(
    mut ls: *mut LexState,
    mut nvars: libc::c_int,
    mut nexps: libc::c_int,
    mut e: *mut expdesc,
) {
    let mut fs: *mut FuncState = (*ls).fs;
    let mut needed: libc::c_int = nvars - nexps;
    if (*e).k as libc::c_uint == VCALL as libc::c_int as libc::c_uint
        || (*e).k as libc::c_uint == VVARARG as libc::c_int as libc::c_uint
    {
        let mut extra: libc::c_int = needed + 1 as libc::c_int;
        if extra < 0 as libc::c_int {
            extra = 0 as libc::c_int;
        }
        luaK_setreturns(fs, e, extra);
    } else {
        if (*e).k as libc::c_uint != VVOID as libc::c_int as libc::c_uint {
            luaK_exp2nextreg(fs, e);
        }
        if needed > 0 as libc::c_int {
            luaK_nil(fs, (*fs).freereg as libc::c_int, needed);
        }
    }
    if needed > 0 as libc::c_int {
        luaK_reserveregs(fs, needed);
    } else {
        (*fs).freereg = ((*fs).freereg as libc::c_int + needed) as u8;
    };
}

unsafe extern "C" fn jumpscopeerror(mut ls: *mut LexState, mut gt: *mut Labeldesc) -> ! {
    let mut varname: *const libc::c_char =
        ((*(*getlocalvardesc((*ls).fs, (*gt).nactvar as libc::c_int))
            .vd
            .name)
            .contents)
            .as_mut_ptr();

    luaK_semerror(
        ls,
        format_args!(
            "<goto {}> at line {} jumps into the scope of local '{}'",
            CStr::from_ptr(((*(*gt).name).contents).as_mut_ptr()).to_string_lossy(),
            (*gt).line,
            CStr::from_ptr(varname).to_string_lossy(),
        ),
    );
}

unsafe extern "C" fn solvegoto(
    mut ls: *mut LexState,
    mut g: libc::c_int,
    mut label: *mut Labeldesc,
) {
    let mut i: libc::c_int = 0;
    let mut gl: *mut Labellist = &mut (*(*ls).dyd).gt;
    let mut gt: *mut Labeldesc = &mut *((*gl).arr).offset(g as isize) as *mut Labeldesc;
    if ((((*gt).nactvar as libc::c_int) < (*label).nactvar as libc::c_int) as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        jumpscopeerror(ls, gt);
    }
    luaK_patchlist((*ls).fs, (*gt).pc, (*label).pc);
    i = g;
    while i < (*gl).n - 1 as libc::c_int {
        *((*gl).arr).offset(i as isize) = *((*gl).arr).offset((i + 1 as libc::c_int) as isize);
        i += 1;
        i;
    }
    (*gl).n -= 1;
    (*gl).n;
}

unsafe extern "C" fn findlabel(mut ls: *mut LexState, mut name: *mut TString) -> *mut Labeldesc {
    let mut i: libc::c_int = 0;
    let mut dyd: *mut Dyndata = (*ls).dyd;
    i = (*(*ls).fs).firstlabel;
    while i < (*dyd).label.n {
        let mut lb: *mut Labeldesc = &mut *((*dyd).label.arr).offset(i as isize) as *mut Labeldesc;
        if (*lb).name == name {
            return lb;
        }
        i += 1;
        i;
    }
    return 0 as *mut Labeldesc;
}

unsafe extern "C" fn newlabelentry(
    mut ls: *mut LexState,
    mut l: *mut Labellist,
    mut name: *mut TString,
    mut line: libc::c_int,
    mut pc: libc::c_int,
) -> libc::c_int {
    let mut n: libc::c_int = (*l).n;
    (*l).arr = luaM_growaux_(
        (*ls).L,
        (*l).arr as *mut libc::c_void,
        n,
        &mut (*l).size,
        ::core::mem::size_of::<Labeldesc>() as libc::c_ulong as libc::c_int,
        (if 32767 as libc::c_int as usize
            <= (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<Labeldesc>())
        {
            32767 as libc::c_int as libc::c_uint
        } else {
            (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<Labeldesc>())
                as libc::c_uint
        }) as libc::c_int,
        b"labels/gotos\0" as *const u8 as *const libc::c_char,
    ) as *mut Labeldesc;
    let ref mut fresh11 = (*((*l).arr).offset(n as isize)).name;
    *fresh11 = name;
    (*((*l).arr).offset(n as isize)).line = line;
    (*((*l).arr).offset(n as isize)).nactvar = (*(*ls).fs).nactvar;
    (*((*l).arr).offset(n as isize)).close = 0 as libc::c_int as u8;
    (*((*l).arr).offset(n as isize)).pc = pc;
    (*l).n = n + 1 as libc::c_int;
    return n;
}

unsafe extern "C" fn newgotoentry(
    mut ls: *mut LexState,
    mut name: *mut TString,
    mut line: libc::c_int,
    mut pc: libc::c_int,
) -> libc::c_int {
    return newlabelentry(ls, &mut (*(*ls).dyd).gt, name, line, pc);
}

unsafe extern "C" fn solvegotos(mut ls: *mut LexState, mut lb: *mut Labeldesc) -> libc::c_int {
    let mut gl: *mut Labellist = &mut (*(*ls).dyd).gt;
    let mut i: libc::c_int = (*(*(*ls).fs).bl).firstgoto;
    let mut needsclose: libc::c_int = 0 as libc::c_int;
    while i < (*gl).n {
        if (*((*gl).arr).offset(i as isize)).name == (*lb).name {
            needsclose |= (*((*gl).arr).offset(i as isize)).close as libc::c_int;
            solvegoto(ls, i, lb);
        } else {
            i += 1;
            i;
        }
    }
    return needsclose;
}

unsafe extern "C" fn createlabel(
    mut ls: *mut LexState,
    mut name: *mut TString,
    mut line: libc::c_int,
    mut last: libc::c_int,
) -> libc::c_int {
    let mut fs: *mut FuncState = (*ls).fs;
    let mut ll: *mut Labellist = &mut (*(*ls).dyd).label;
    let mut l: libc::c_int = newlabelentry(ls, ll, name, line, luaK_getlabel(fs));
    if last != 0 {
        (*((*ll).arr).offset(l as isize)).nactvar = (*(*fs).bl).nactvar;
    }
    if solvegotos(ls, &mut *((*ll).arr).offset(l as isize)) != 0 {
        luaK_codeABCk(
            fs,
            OP_CLOSE,
            luaY_nvarstack(fs),
            0 as libc::c_int,
            0 as libc::c_int,
            0 as libc::c_int,
        );
        return 1 as libc::c_int;
    }
    return 0 as libc::c_int;
}

unsafe extern "C" fn movegotosout(mut fs: *mut FuncState, mut bl: *mut BlockCnt) {
    let mut i: libc::c_int = 0;
    let mut gl: *mut Labellist = &mut (*(*(*fs).ls).dyd).gt;
    i = (*bl).firstgoto;
    while i < (*gl).n {
        let mut gt: *mut Labeldesc = &mut *((*gl).arr).offset(i as isize) as *mut Labeldesc;
        if reglevel(fs, (*gt).nactvar as libc::c_int) > reglevel(fs, (*bl).nactvar as libc::c_int) {
            (*gt).close = ((*gt).close as libc::c_int | (*bl).upval as libc::c_int) as u8;
        }
        (*gt).nactvar = (*bl).nactvar;
        i += 1;
        i;
    }
}

unsafe extern "C" fn enterblock(mut fs: *mut FuncState, mut bl: *mut BlockCnt, mut isloop: u8) {
    (*bl).isloop = isloop;
    (*bl).nactvar = (*fs).nactvar;
    (*bl).firstlabel = (*(*(*fs).ls).dyd).label.n;
    (*bl).firstgoto = (*(*(*fs).ls).dyd).gt.n;
    (*bl).upval = 0 as libc::c_int as u8;
    (*bl).insidetbc =
        (!((*fs).bl).is_null() && (*(*fs).bl).insidetbc as libc::c_int != 0) as libc::c_int as u8;
    (*bl).previous = (*fs).bl;
    (*fs).bl = bl;
}

unsafe extern "C" fn undefgoto(mut ls: *mut LexState, mut gt: *mut Labeldesc) -> ! {
    if (*gt).name
        == luaS_newlstr(
            (*ls).L,
            b"break\0" as *const u8 as *const libc::c_char,
            ::core::mem::size_of::<[libc::c_char; 6]>()
                .wrapping_div(::core::mem::size_of::<libc::c_char>())
                .wrapping_sub(1),
        )
    {
        luaK_semerror(
            ls,
            format_args!("break outside loop at line {}", (*gt).line),
        );
    } else {
        luaK_semerror(
            ls,
            format_args!(
                "no visible label '{}' for <goto> at line {}",
                CStr::from_ptr(((*(*gt).name).contents).as_mut_ptr()).to_string_lossy(),
                (*gt).line,
            ),
        );
    }
}

unsafe extern "C" fn leaveblock(mut fs: *mut FuncState) {
    let mut bl: *mut BlockCnt = (*fs).bl;
    let mut ls: *mut LexState = (*fs).ls;
    let mut hasclose: libc::c_int = 0 as libc::c_int;
    let mut stklevel: libc::c_int = reglevel(fs, (*bl).nactvar as libc::c_int);
    removevars(fs, (*bl).nactvar as libc::c_int);
    if (*bl).isloop != 0 {
        hasclose = createlabel(
            ls,
            luaS_newlstr(
                (*ls).L,
                b"break\0" as *const u8 as *const libc::c_char,
                ::core::mem::size_of::<[libc::c_char; 6]>()
                    .wrapping_div(::core::mem::size_of::<libc::c_char>())
                    .wrapping_sub(1),
            ),
            0 as libc::c_int,
            0 as libc::c_int,
        );
    }
    if hasclose == 0 && !((*bl).previous).is_null() && (*bl).upval as libc::c_int != 0 {
        luaK_codeABCk(
            fs,
            OP_CLOSE,
            stklevel,
            0 as libc::c_int,
            0 as libc::c_int,
            0 as libc::c_int,
        );
    }
    (*fs).freereg = stklevel as u8;
    (*(*ls).dyd).label.n = (*bl).firstlabel;
    (*fs).bl = (*bl).previous;
    if !((*bl).previous).is_null() {
        movegotosout(fs, bl);
    } else if (*bl).firstgoto < (*(*ls).dyd).gt.n {
        undefgoto(
            ls,
            &mut *((*(*ls).dyd).gt.arr).offset((*bl).firstgoto as isize),
        );
    }
}

unsafe extern "C" fn addprototype(mut ls: *mut LexState) -> *mut Proto {
    let mut clp: *mut Proto = 0 as *mut Proto;
    let mut L: *mut lua_State = (*ls).L;
    let mut fs: *mut FuncState = (*ls).fs;
    let mut f: *mut Proto = (*fs).f;
    if (*fs).np >= (*f).sizep {
        let mut oldsize: libc::c_int = (*f).sizep;
        (*f).p = luaM_growaux_(
            L,
            (*f).p as *mut libc::c_void,
            (*fs).np,
            &mut (*f).sizep,
            ::core::mem::size_of::<*mut Proto>() as libc::c_ulong as libc::c_int,
            (if (((1 as libc::c_int) << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int)
                - 1 as libc::c_int) as usize
                <= (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<*mut Proto>())
            {
                (((1 as libc::c_int) << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int)
                    - 1 as libc::c_int) as libc::c_uint
            } else {
                (!(0 as libc::c_int as usize)).wrapping_div(::core::mem::size_of::<*mut Proto>())
                    as libc::c_uint
            }) as libc::c_int,
            b"functions\0" as *const u8 as *const libc::c_char,
        ) as *mut *mut Proto;
        while oldsize < (*f).sizep {
            let fresh12 = oldsize;
            oldsize = oldsize + 1;
            let ref mut fresh13 = *((*f).p).offset(fresh12 as isize);
            *fresh13 = 0 as *mut Proto;
        }
    }
    clp = luaF_newproto(L);
    let fresh14 = (*fs).np;
    (*fs).np = (*fs).np + 1;
    let ref mut fresh15 = *((*f).p).offset(fresh14 as isize);
    *fresh15 = clp;
    if (*f).marked as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
        && (*clp).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            != 0
    {
        luaC_barrier_(
            L,
            &mut (*(f as *mut GCUnion)).gc,
            &mut (*(clp as *mut GCUnion)).gc,
        );
    } else {
    };
    return clp;
}

unsafe extern "C" fn codeclosure(mut ls: *mut LexState, mut v: *mut expdesc) {
    let mut fs: *mut FuncState = (*(*ls).fs).prev;
    init_exp(
        v,
        VRELOC,
        luaK_codeABx(
            fs,
            OP_CLOSURE,
            0 as libc::c_int,
            ((*fs).np - 1 as libc::c_int) as libc::c_uint,
        ),
    );
    luaK_exp2nextreg(fs, v);
}

unsafe extern "C" fn open_func(
    mut ls: *mut LexState,
    mut fs: *mut FuncState,
    mut bl: *mut BlockCnt,
) {
    let mut f: *mut Proto = (*fs).f;
    (*fs).prev = (*ls).fs;
    (*fs).ls = ls;
    (*ls).fs = fs;
    (*fs).pc = 0 as libc::c_int;
    (*fs).previousline = (*f).linedefined;
    (*fs).iwthabs = 0 as libc::c_int as u8;
    (*fs).lasttarget = 0 as libc::c_int;
    (*fs).freereg = 0 as libc::c_int as u8;
    (*fs).nk = 0 as libc::c_int;
    (*fs).nabslineinfo = 0 as libc::c_int;
    (*fs).np = 0 as libc::c_int;
    (*fs).nups = 0 as libc::c_int as u8;
    (*fs).ndebugvars = 0 as libc::c_int as libc::c_short;
    (*fs).nactvar = 0 as libc::c_int as u8;
    (*fs).needclose = 0 as libc::c_int as u8;
    (*fs).firstlocal = (*(*ls).dyd).actvar.n;
    (*fs).firstlabel = (*(*ls).dyd).label.n;
    (*fs).bl = 0 as *mut BlockCnt;
    (*f).source = (*ls).source;
    if (*f).marked as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
        && (*(*f).source).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            != 0
    {
        luaC_barrier_(
            (*ls).L,
            &mut (*(f as *mut GCUnion)).gc,
            &mut (*((*f).source as *mut GCUnion)).gc,
        );
    } else {
    };
    (*f).maxstacksize = 2 as libc::c_int as u8;
    enterblock(fs, bl, 0 as libc::c_int as u8);
}

unsafe extern "C" fn close_func(mut ls: *mut LexState) {
    let mut L: *mut lua_State = (*ls).L;
    let mut fs: *mut FuncState = (*ls).fs;
    let mut f: *mut Proto = (*fs).f;
    luaK_ret(fs, luaY_nvarstack(fs), 0 as libc::c_int);
    leaveblock(fs);
    luaK_finish(fs);
    (*f).code = luaM_shrinkvector_(
        L,
        (*f).code as *mut libc::c_void,
        &mut (*f).sizecode,
        (*fs).pc,
        ::core::mem::size_of::<u32>() as libc::c_ulong as libc::c_int,
    ) as *mut u32;
    (*f).lineinfo = luaM_shrinkvector_(
        L,
        (*f).lineinfo as *mut libc::c_void,
        &mut (*f).sizelineinfo,
        (*fs).pc,
        ::core::mem::size_of::<i8>() as libc::c_ulong as libc::c_int,
    ) as *mut i8;
    (*f).abslineinfo = luaM_shrinkvector_(
        L,
        (*f).abslineinfo as *mut libc::c_void,
        &mut (*f).sizeabslineinfo,
        (*fs).nabslineinfo,
        ::core::mem::size_of::<AbsLineInfo>() as libc::c_ulong as libc::c_int,
    ) as *mut AbsLineInfo;
    (*f).k = luaM_shrinkvector_(
        L,
        (*f).k as *mut libc::c_void,
        &mut (*f).sizek,
        (*fs).nk,
        ::core::mem::size_of::<TValue>() as libc::c_ulong as libc::c_int,
    ) as *mut TValue;
    (*f).p = luaM_shrinkvector_(
        L,
        (*f).p as *mut libc::c_void,
        &mut (*f).sizep,
        (*fs).np,
        ::core::mem::size_of::<*mut Proto>() as libc::c_ulong as libc::c_int,
    ) as *mut *mut Proto;
    (*f).locvars = luaM_shrinkvector_(
        L,
        (*f).locvars as *mut libc::c_void,
        &mut (*f).sizelocvars,
        (*fs).ndebugvars as libc::c_int,
        ::core::mem::size_of::<LocVar>() as libc::c_ulong as libc::c_int,
    ) as *mut LocVar;
    (*f).upvalues = luaM_shrinkvector_(
        L,
        (*f).upvalues as *mut libc::c_void,
        &mut (*f).sizeupvalues,
        (*fs).nups as libc::c_int,
        ::core::mem::size_of::<Upvaldesc>() as libc::c_ulong as libc::c_int,
    ) as *mut Upvaldesc;
    (*ls).fs = (*fs).prev;
    if (*(*L).l_G).GCdebt > 0 as libc::c_int as isize {
        luaC_step(L);
    }
}

unsafe extern "C" fn block_follow(
    mut ls: *mut LexState,
    mut withuntil: libc::c_int,
) -> libc::c_int {
    match (*ls).t.token {
        259 | 260 | 261 | 288 => return 1 as libc::c_int,
        276 => return withuntil,
        _ => return 0 as libc::c_int,
    };
}

unsafe extern "C" fn statlist(mut ls: *mut LexState) {
    while block_follow(ls, 1 as libc::c_int) == 0 {
        if (*ls).t.token == TK_RETURN as libc::c_int {
            statement(ls);
            return;
        }
        statement(ls);
    }
}

unsafe extern "C" fn fieldsel(mut ls: *mut LexState, mut v: *mut expdesc) {
    let mut fs: *mut FuncState = (*ls).fs;
    let mut key: expdesc = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    luaK_exp2anyregup(fs, v);
    luaX_next(ls);
    codename(ls, &mut key);
    luaK_indexed(fs, v, &mut key);
}

unsafe extern "C" fn yindex(mut ls: *mut LexState, mut v: *mut expdesc) {
    luaX_next(ls);
    expr(ls, v);
    luaK_exp2val((*ls).fs, v);
    checknext(ls, ']' as i32);
}

unsafe extern "C" fn recfield(mut ls: *mut LexState, mut cc: *mut ConsControl) {
    let mut fs: *mut FuncState = (*ls).fs;
    let mut reg: libc::c_int = (*(*ls).fs).freereg as libc::c_int;
    let mut tab: expdesc = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    let mut key: expdesc = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    let mut val: expdesc = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    if (*ls).t.token == TK_NAME as libc::c_int {
        checklimit(
            fs,
            (*cc).nh,
            2147483647 as libc::c_int,
            "items in a constructor",
        );
        codename(ls, &mut key);
    } else {
        yindex(ls, &mut key);
    }
    (*cc).nh += 1;
    (*cc).nh;
    checknext(ls, '=' as i32);
    tab = *(*cc).t;
    luaK_indexed(fs, &mut tab, &mut key);
    expr(ls, &mut val);
    luaK_storevar(fs, &mut tab, &mut val);
    (*fs).freereg = reg as u8;
}

unsafe extern "C" fn closelistfield(mut fs: *mut FuncState, mut cc: *mut ConsControl) {
    if (*cc).v.k as libc::c_uint == VVOID as libc::c_int as libc::c_uint {
        return;
    }
    luaK_exp2nextreg(fs, &mut (*cc).v);
    (*cc).v.k = VVOID;
    if (*cc).tostore == 50 as libc::c_int {
        luaK_setlist(fs, (*(*cc).t).u.info, (*cc).na, (*cc).tostore);
        (*cc).na += (*cc).tostore;
        (*cc).tostore = 0 as libc::c_int;
    }
}

unsafe extern "C" fn lastlistfield(mut fs: *mut FuncState, mut cc: *mut ConsControl) {
    if (*cc).tostore == 0 as libc::c_int {
        return;
    }
    if (*cc).v.k as libc::c_uint == VCALL as libc::c_int as libc::c_uint
        || (*cc).v.k as libc::c_uint == VVARARG as libc::c_int as libc::c_uint
    {
        luaK_setreturns(fs, &mut (*cc).v, -(1 as libc::c_int));
        luaK_setlist(fs, (*(*cc).t).u.info, (*cc).na, -(1 as libc::c_int));
        (*cc).na -= 1;
        (*cc).na;
    } else {
        if (*cc).v.k as libc::c_uint != VVOID as libc::c_int as libc::c_uint {
            luaK_exp2nextreg(fs, &mut (*cc).v);
        }
        luaK_setlist(fs, (*(*cc).t).u.info, (*cc).na, (*cc).tostore);
    }
    (*cc).na += (*cc).tostore;
}

unsafe extern "C" fn listfield(mut ls: *mut LexState, mut cc: *mut ConsControl) {
    expr(ls, &mut (*cc).v);
    (*cc).tostore += 1;
    (*cc).tostore;
}

unsafe extern "C" fn field(mut ls: *mut LexState, mut cc: *mut ConsControl) {
    match (*ls).t.token {
        291 => {
            if luaX_lookahead(ls) != '=' as i32 {
                listfield(ls, cc);
            } else {
                recfield(ls, cc);
            }
        }
        91 => {
            recfield(ls, cc);
        }
        _ => {
            listfield(ls, cc);
        }
    };
}

unsafe extern "C" fn constructor(mut ls: *mut LexState, mut t: *mut expdesc) {
    let mut fs: *mut FuncState = (*ls).fs;
    let mut line: libc::c_int = (*ls).linenumber;
    let mut pc: libc::c_int = luaK_codeABCk(
        fs,
        OP_NEWTABLE,
        0 as libc::c_int,
        0 as libc::c_int,
        0 as libc::c_int,
        0 as libc::c_int,
    );
    let mut cc: ConsControl = ConsControl {
        v: expdesc {
            k: VVOID,
            u: C2RustUnnamed_11 { ival: 0 },
            t: 0,
            f: 0,
        },
        t: 0 as *mut expdesc,
        nh: 0,
        na: 0,
        tostore: 0,
    };
    luaK_code(fs, 0 as libc::c_int as u32);
    cc.tostore = 0 as libc::c_int;
    cc.nh = cc.tostore;
    cc.na = cc.nh;
    cc.t = t;
    init_exp(t, VNONRELOC, (*fs).freereg as libc::c_int);
    luaK_reserveregs(fs, 1 as libc::c_int);
    init_exp(&mut cc.v, VVOID, 0 as libc::c_int);
    checknext(ls, '{' as i32);
    while !((*ls).t.token == '}' as i32) {
        closelistfield(fs, &mut cc);
        field(ls, &mut cc);
        if !(testnext(ls, ',' as i32) != 0 || testnext(ls, ';' as i32) != 0) {
            break;
        }
    }
    check_match(ls, '}' as i32, '{' as i32, line);
    lastlistfield(fs, &mut cc);
    luaK_settablesize(fs, pc, (*t).u.info, cc.na, cc.nh);
}

unsafe extern "C" fn setvararg(mut fs: *mut FuncState, mut nparams: libc::c_int) {
    (*(*fs).f).is_vararg = 1 as libc::c_int as u8;
    luaK_codeABCk(
        fs,
        OP_VARARGPREP,
        nparams,
        0 as libc::c_int,
        0 as libc::c_int,
        0 as libc::c_int,
    );
}

unsafe extern "C" fn parlist(mut ls: *mut LexState) {
    let mut fs: *mut FuncState = (*ls).fs;
    let mut f: *mut Proto = (*fs).f;
    let mut nparams: libc::c_int = 0 as libc::c_int;
    let mut isvararg: libc::c_int = 0 as libc::c_int;
    if (*ls).t.token != ')' as i32 {
        loop {
            match (*ls).t.token {
                291 => {
                    new_localvar(ls, str_checkname(ls));
                    nparams += 1;
                    nparams;
                }
                280 => {
                    luaX_next(ls);
                    isvararg = 1 as libc::c_int;
                }
                _ => {
                    luaX_syntaxerror(ls, "<name> or '...' expected");
                }
            }
            if !(isvararg == 0 && testnext(ls, ',' as i32) != 0) {
                break;
            }
        }
    }
    adjustlocalvars(ls, nparams);
    (*f).numparams = (*fs).nactvar;
    if isvararg != 0 {
        setvararg(fs, (*f).numparams as libc::c_int);
    }
    luaK_reserveregs(fs, (*fs).nactvar as libc::c_int);
}

unsafe extern "C" fn body(
    mut ls: *mut LexState,
    mut e: *mut expdesc,
    mut ismethod: libc::c_int,
    mut line: libc::c_int,
) {
    let mut new_fs: FuncState = FuncState {
        f: 0 as *mut Proto,
        prev: 0 as *mut FuncState,
        ls: 0 as *mut LexState,
        bl: 0 as *mut BlockCnt,
        pc: 0,
        lasttarget: 0,
        previousline: 0,
        nk: 0,
        np: 0,
        nabslineinfo: 0,
        firstlocal: 0,
        firstlabel: 0,
        ndebugvars: 0,
        nactvar: 0,
        nups: 0,
        freereg: 0,
        iwthabs: 0,
        needclose: 0,
    };
    let mut bl: BlockCnt = BlockCnt {
        previous: 0 as *mut BlockCnt,
        firstlabel: 0,
        firstgoto: 0,
        nactvar: 0,
        upval: 0,
        isloop: 0,
        insidetbc: 0,
    };
    new_fs.f = addprototype(ls);
    (*new_fs.f).linedefined = line;
    open_func(ls, &mut new_fs, &mut bl);
    checknext(ls, '(' as i32);
    if ismethod != 0 {
        new_localvar(
            ls,
            luaX_newstring(
                ls,
                b"self\0" as *const u8 as *const libc::c_char,
                ::core::mem::size_of::<[libc::c_char; 5]>()
                    .wrapping_div(::core::mem::size_of::<libc::c_char>())
                    .wrapping_sub(1),
            ),
        );
        adjustlocalvars(ls, 1 as libc::c_int);
    }
    parlist(ls);
    checknext(ls, ')' as i32);
    statlist(ls);
    (*new_fs.f).lastlinedefined = (*ls).linenumber;
    check_match(ls, TK_END as libc::c_int, TK_FUNCTION as libc::c_int, line);
    codeclosure(ls, e);
    close_func(ls);
}

unsafe extern "C" fn explist(mut ls: *mut LexState, mut v: *mut expdesc) -> libc::c_int {
    let mut n: libc::c_int = 1 as libc::c_int;
    expr(ls, v);
    while testnext(ls, ',' as i32) != 0 {
        luaK_exp2nextreg((*ls).fs, v);
        expr(ls, v);
        n += 1;
        n;
    }
    return n;
}

unsafe extern "C" fn funcargs(mut ls: *mut LexState, mut f: *mut expdesc) {
    let mut fs: *mut FuncState = (*ls).fs;
    let mut args: expdesc = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    let mut base: libc::c_int = 0;
    let mut nparams: libc::c_int = 0;
    let mut line: libc::c_int = (*ls).linenumber;
    match (*ls).t.token {
        40 => {
            luaX_next(ls);
            if (*ls).t.token == ')' as i32 {
                args.k = VVOID;
            } else {
                explist(ls, &mut args);
                if args.k as libc::c_uint == VCALL as libc::c_int as libc::c_uint
                    || args.k as libc::c_uint == VVARARG as libc::c_int as libc::c_uint
                {
                    luaK_setreturns(fs, &mut args, -(1 as libc::c_int));
                }
            }
            check_match(ls, ')' as i32, '(' as i32, line);
        }
        123 => {
            constructor(ls, &mut args);
        }
        292 => {
            codestring(&mut args, (*ls).t.seminfo.ts);
            luaX_next(ls);
        }
        _ => {
            luaX_syntaxerror(ls, "function arguments expected");
        }
    }
    base = (*f).u.info;
    if args.k as libc::c_uint == VCALL as libc::c_int as libc::c_uint
        || args.k as libc::c_uint == VVARARG as libc::c_int as libc::c_uint
    {
        nparams = -(1 as libc::c_int);
    } else {
        if args.k as libc::c_uint != VVOID as libc::c_int as libc::c_uint {
            luaK_exp2nextreg(fs, &mut args);
        }
        nparams = (*fs).freereg as libc::c_int - (base + 1 as libc::c_int);
    }
    init_exp(
        f,
        VCALL,
        luaK_codeABCk(
            fs,
            OP_CALL,
            base,
            nparams + 1 as libc::c_int,
            2 as libc::c_int,
            0 as libc::c_int,
        ),
    );
    luaK_fixline(fs, line);
    (*fs).freereg = (base + 1 as libc::c_int) as u8;
}

unsafe extern "C" fn primaryexp(mut ls: *mut LexState, mut v: *mut expdesc) {
    match (*ls).t.token {
        40 => {
            let mut line: libc::c_int = (*ls).linenumber;
            luaX_next(ls);
            expr(ls, v);
            check_match(ls, ')' as i32, '(' as i32, line);
            luaK_dischargevars((*ls).fs, v);
            return;
        }
        291 => {
            singlevar(ls, v);
            return;
        }
        _ => {
            luaX_syntaxerror(ls, "unexpected symbol");
        }
    };
}

unsafe extern "C" fn suffixedexp(mut ls: *mut LexState, mut v: *mut expdesc) {
    let mut fs: *mut FuncState = (*ls).fs;
    primaryexp(ls, v);
    loop {
        match (*ls).t.token {
            46 => {
                fieldsel(ls, v);
            }
            91 => {
                let mut key: expdesc = expdesc {
                    k: VVOID,
                    u: C2RustUnnamed_11 { ival: 0 },
                    t: 0,
                    f: 0,
                };
                luaK_exp2anyregup(fs, v);
                yindex(ls, &mut key);
                luaK_indexed(fs, v, &mut key);
            }
            58 => {
                let mut key_0: expdesc = expdesc {
                    k: VVOID,
                    u: C2RustUnnamed_11 { ival: 0 },
                    t: 0,
                    f: 0,
                };
                luaX_next(ls);
                codename(ls, &mut key_0);
                luaK_self(fs, v, &mut key_0);
                funcargs(ls, v);
            }
            40 | 292 | 123 => {
                luaK_exp2nextreg(fs, v);
                funcargs(ls, v);
            }
            _ => return,
        }
    }
}

unsafe extern "C" fn simpleexp(mut ls: *mut LexState, mut v: *mut expdesc) {
    match (*ls).t.token {
        289 => {
            init_exp(v, VKFLT, 0 as libc::c_int);
            (*v).u.nval = (*ls).t.seminfo.r;
        }
        290 => {
            init_exp(v, VKINT, 0 as libc::c_int);
            (*v).u.ival = (*ls).t.seminfo.i;
        }
        292 => {
            codestring(v, (*ls).t.seminfo.ts);
        }
        269 => {
            init_exp(v, VNIL, 0 as libc::c_int);
        }
        275 => {
            init_exp(v, VTRUE, 0 as libc::c_int);
        }
        262 => {
            init_exp(v, VFALSE, 0 as libc::c_int);
        }
        280 => {
            let mut fs: *mut FuncState = (*ls).fs;
            if (*(*fs).f).is_vararg == 0 {
                luaX_syntaxerror(ls, "cannot use '...' outside a vararg function");
            }
            init_exp(
                v,
                VVARARG,
                luaK_codeABCk(
                    fs,
                    OP_VARARG,
                    0 as libc::c_int,
                    0 as libc::c_int,
                    1 as libc::c_int,
                    0 as libc::c_int,
                ),
            );
        }
        123 => {
            constructor(ls, v);
            return;
        }
        264 => {
            luaX_next(ls);
            body(ls, v, 0 as libc::c_int, (*ls).linenumber);
            return;
        }
        _ => {
            suffixedexp(ls, v);
            return;
        }
    }
    luaX_next(ls);
}

unsafe extern "C" fn getunopr(mut op: libc::c_int) -> UnOpr {
    match op {
        270 => return OPR_NOT,
        45 => return OPR_MINUS,
        126 => return OPR_BNOT,
        35 => return OPR_LEN,
        _ => return OPR_NOUNOPR,
    };
}

unsafe extern "C" fn getbinopr(mut op: libc::c_int) -> BinOpr {
    match op {
        43 => return OPR_ADD,
        45 => return OPR_SUB,
        42 => return OPR_MUL,
        37 => return OPR_MOD,
        94 => return OPR_POW,
        47 => return OPR_DIV,
        278 => return OPR_IDIV,
        38 => return OPR_BAND,
        124 => return OPR_BOR,
        126 => return OPR_BXOR,
        285 => return OPR_SHL,
        286 => return OPR_SHR,
        279 => return OPR_CONCAT,
        284 => return OPR_NE,
        281 => return OPR_EQ,
        60 => return OPR_LT,
        283 => return OPR_LE,
        62 => return OPR_GT,
        282 => return OPR_GE,
        256 => return OPR_AND,
        271 => return OPR_OR,
        _ => return OPR_NOBINOPR,
    };
}

static mut priority: [C2RustUnnamed_14; 21] = [
    {
        let mut init = C2RustUnnamed_14 {
            left: 10 as libc::c_int as u8,
            right: 10 as libc::c_int as u8,
        };
        init
    },
    {
        let mut init = C2RustUnnamed_14 {
            left: 10 as libc::c_int as u8,
            right: 10 as libc::c_int as u8,
        };
        init
    },
    {
        let mut init = C2RustUnnamed_14 {
            left: 11 as libc::c_int as u8,
            right: 11 as libc::c_int as u8,
        };
        init
    },
    {
        let mut init = C2RustUnnamed_14 {
            left: 11 as libc::c_int as u8,
            right: 11 as libc::c_int as u8,
        };
        init
    },
    {
        let mut init = C2RustUnnamed_14 {
            left: 14 as libc::c_int as u8,
            right: 13 as libc::c_int as u8,
        };
        init
    },
    {
        let mut init = C2RustUnnamed_14 {
            left: 11 as libc::c_int as u8,
            right: 11 as libc::c_int as u8,
        };
        init
    },
    {
        let mut init = C2RustUnnamed_14 {
            left: 11 as libc::c_int as u8,
            right: 11 as libc::c_int as u8,
        };
        init
    },
    {
        let mut init = C2RustUnnamed_14 {
            left: 6 as libc::c_int as u8,
            right: 6 as libc::c_int as u8,
        };
        init
    },
    {
        let mut init = C2RustUnnamed_14 {
            left: 4 as libc::c_int as u8,
            right: 4 as libc::c_int as u8,
        };
        init
    },
    {
        let mut init = C2RustUnnamed_14 {
            left: 5 as libc::c_int as u8,
            right: 5 as libc::c_int as u8,
        };
        init
    },
    {
        let mut init = C2RustUnnamed_14 {
            left: 7 as libc::c_int as u8,
            right: 7 as libc::c_int as u8,
        };
        init
    },
    {
        let mut init = C2RustUnnamed_14 {
            left: 7 as libc::c_int as u8,
            right: 7 as libc::c_int as u8,
        };
        init
    },
    {
        let mut init = C2RustUnnamed_14 {
            left: 9 as libc::c_int as u8,
            right: 8 as libc::c_int as u8,
        };
        init
    },
    {
        let mut init = C2RustUnnamed_14 {
            left: 3 as libc::c_int as u8,
            right: 3 as libc::c_int as u8,
        };
        init
    },
    {
        let mut init = C2RustUnnamed_14 {
            left: 3 as libc::c_int as u8,
            right: 3 as libc::c_int as u8,
        };
        init
    },
    {
        let mut init = C2RustUnnamed_14 {
            left: 3 as libc::c_int as u8,
            right: 3 as libc::c_int as u8,
        };
        init
    },
    {
        let mut init = C2RustUnnamed_14 {
            left: 3 as libc::c_int as u8,
            right: 3 as libc::c_int as u8,
        };
        init
    },
    {
        let mut init = C2RustUnnamed_14 {
            left: 3 as libc::c_int as u8,
            right: 3 as libc::c_int as u8,
        };
        init
    },
    {
        let mut init = C2RustUnnamed_14 {
            left: 3 as libc::c_int as u8,
            right: 3 as libc::c_int as u8,
        };
        init
    },
    {
        let mut init = C2RustUnnamed_14 {
            left: 2 as libc::c_int as u8,
            right: 2 as libc::c_int as u8,
        };
        init
    },
    {
        let mut init = C2RustUnnamed_14 {
            left: 1 as libc::c_int as u8,
            right: 1 as libc::c_int as u8,
        };
        init
    },
];

unsafe extern "C" fn subexpr(
    mut ls: *mut LexState,
    mut v: *mut expdesc,
    mut limit: libc::c_int,
) -> BinOpr {
    let mut op: BinOpr = OPR_ADD;
    let mut uop: UnOpr = OPR_MINUS;
    luaE_incCstack((*ls).L);
    uop = getunopr((*ls).t.token);
    if uop as libc::c_uint != OPR_NOUNOPR as libc::c_int as libc::c_uint {
        let mut line: libc::c_int = (*ls).linenumber;
        luaX_next(ls);
        subexpr(ls, v, 12 as libc::c_int);
        luaK_prefix((*ls).fs, uop, v, line);
    } else {
        simpleexp(ls, v);
    }
    op = getbinopr((*ls).t.token);
    while op as libc::c_uint != OPR_NOBINOPR as libc::c_int as libc::c_uint
        && priority[op as usize].left as libc::c_int > limit
    {
        let mut v2: expdesc = expdesc {
            k: VVOID,
            u: C2RustUnnamed_11 { ival: 0 },
            t: 0,
            f: 0,
        };
        let mut nextop: BinOpr = OPR_ADD;
        let mut line_0: libc::c_int = (*ls).linenumber;
        luaX_next(ls);
        luaK_infix((*ls).fs, op, v);
        nextop = subexpr(ls, &mut v2, priority[op as usize].right as libc::c_int);
        luaK_posfix((*ls).fs, op, v, &mut v2, line_0);
        op = nextop;
    }
    (*(*ls).L).nCcalls = ((*(*ls).L).nCcalls).wrapping_sub(1);
    (*(*ls).L).nCcalls;
    return op;
}

unsafe extern "C" fn expr(mut ls: *mut LexState, mut v: *mut expdesc) {
    subexpr(ls, v, 0 as libc::c_int);
}

unsafe extern "C" fn block(mut ls: *mut LexState) {
    let mut fs: *mut FuncState = (*ls).fs;
    let mut bl: BlockCnt = BlockCnt {
        previous: 0 as *mut BlockCnt,
        firstlabel: 0,
        firstgoto: 0,
        nactvar: 0,
        upval: 0,
        isloop: 0,
        insidetbc: 0,
    };
    enterblock(fs, &mut bl, 0 as libc::c_int as u8);
    statlist(ls);
    leaveblock(fs);
}

unsafe extern "C" fn check_conflict(
    mut ls: *mut LexState,
    mut lh: *mut LHS_assign,
    mut v: *mut expdesc,
) {
    let mut fs: *mut FuncState = (*ls).fs;
    let mut extra: libc::c_int = (*fs).freereg as libc::c_int;
    let mut conflict: libc::c_int = 0 as libc::c_int;
    while !lh.is_null() {
        if VINDEXED as libc::c_int as libc::c_uint <= (*lh).v.k as libc::c_uint
            && (*lh).v.k as libc::c_uint <= VINDEXSTR as libc::c_int as libc::c_uint
        {
            if (*lh).v.k as libc::c_uint == VINDEXUP as libc::c_int as libc::c_uint {
                if (*v).k as libc::c_uint == VUPVAL as libc::c_int as libc::c_uint
                    && (*lh).v.u.ind.t as libc::c_int == (*v).u.info
                {
                    conflict = 1 as libc::c_int;
                    (*lh).v.k = VINDEXSTR;
                    (*lh).v.u.ind.t = extra as u8;
                }
            } else {
                if (*v).k as libc::c_uint == VLOCAL as libc::c_int as libc::c_uint
                    && (*lh).v.u.ind.t as libc::c_int == (*v).u.var.ridx as libc::c_int
                {
                    conflict = 1 as libc::c_int;
                    (*lh).v.u.ind.t = extra as u8;
                }
                if (*lh).v.k as libc::c_uint == VINDEXED as libc::c_int as libc::c_uint
                    && (*v).k as libc::c_uint == VLOCAL as libc::c_int as libc::c_uint
                    && (*lh).v.u.ind.idx as libc::c_int == (*v).u.var.ridx as libc::c_int
                {
                    conflict = 1 as libc::c_int;
                    (*lh).v.u.ind.idx = extra as libc::c_short;
                }
            }
        }
        lh = (*lh).prev;
    }
    if conflict != 0 {
        if (*v).k as libc::c_uint == VLOCAL as libc::c_int as libc::c_uint {
            luaK_codeABCk(
                fs,
                OP_MOVE,
                extra,
                (*v).u.var.ridx as libc::c_int,
                0 as libc::c_int,
                0 as libc::c_int,
            );
        } else {
            luaK_codeABCk(
                fs,
                OP_GETUPVAL,
                extra,
                (*v).u.info,
                0 as libc::c_int,
                0 as libc::c_int,
            );
        }
        luaK_reserveregs(fs, 1 as libc::c_int);
    }
}

unsafe extern "C" fn restassign(
    mut ls: *mut LexState,
    mut lh: *mut LHS_assign,
    mut nvars: libc::c_int,
) {
    let mut e: expdesc = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    if !(VLOCAL as libc::c_int as libc::c_uint <= (*lh).v.k as libc::c_uint
        && (*lh).v.k as libc::c_uint <= VINDEXSTR as libc::c_int as libc::c_uint)
    {
        luaX_syntaxerror(ls, "syntax error");
    }
    check_readonly(ls, &mut (*lh).v);
    if testnext(ls, ',' as i32) != 0 {
        let mut nv: LHS_assign = LHS_assign {
            prev: 0 as *mut LHS_assign,
            v: expdesc {
                k: VVOID,
                u: C2RustUnnamed_11 { ival: 0 },
                t: 0,
                f: 0,
            },
        };
        nv.prev = lh;
        suffixedexp(ls, &mut nv.v);
        if !(VINDEXED as libc::c_int as libc::c_uint <= nv.v.k as libc::c_uint
            && nv.v.k as libc::c_uint <= VINDEXSTR as libc::c_int as libc::c_uint)
        {
            check_conflict(ls, lh, &mut nv.v);
        }
        luaE_incCstack((*ls).L);
        restassign(ls, &mut nv, nvars + 1 as libc::c_int);
        (*(*ls).L).nCcalls = ((*(*ls).L).nCcalls).wrapping_sub(1);
        (*(*ls).L).nCcalls;
    } else {
        let mut nexps: libc::c_int = 0;
        checknext(ls, '=' as i32);
        nexps = explist(ls, &mut e);
        if nexps != nvars {
            adjust_assign(ls, nvars, nexps, &mut e);
        } else {
            luaK_setoneret((*ls).fs, &mut e);
            luaK_storevar((*ls).fs, &mut (*lh).v, &mut e);
            return;
        }
    }
    init_exp(
        &mut e,
        VNONRELOC,
        (*(*ls).fs).freereg as libc::c_int - 1 as libc::c_int,
    );
    luaK_storevar((*ls).fs, &mut (*lh).v, &mut e);
}

unsafe extern "C" fn cond(mut ls: *mut LexState) -> libc::c_int {
    let mut v: expdesc = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    expr(ls, &mut v);
    if v.k as libc::c_uint == VNIL as libc::c_int as libc::c_uint {
        v.k = VFALSE;
    }
    luaK_goiftrue((*ls).fs, &mut v);
    return v.f;
}

unsafe extern "C" fn gotostat(mut ls: *mut LexState) {
    let mut fs: *mut FuncState = (*ls).fs;
    let mut line: libc::c_int = (*ls).linenumber;
    let mut name: *mut TString = str_checkname(ls);
    let mut lb: *mut Labeldesc = findlabel(ls, name);
    if lb.is_null() {
        newgotoentry(ls, name, line, luaK_jump(fs));
    } else {
        let mut lblevel: libc::c_int = reglevel(fs, (*lb).nactvar as libc::c_int);
        if luaY_nvarstack(fs) > lblevel {
            luaK_codeABCk(
                fs,
                OP_CLOSE,
                lblevel,
                0 as libc::c_int,
                0 as libc::c_int,
                0 as libc::c_int,
            );
        }
        luaK_patchlist(fs, luaK_jump(fs), (*lb).pc);
    };
}

unsafe extern "C" fn breakstat(mut ls: *mut LexState) {
    let mut line: libc::c_int = (*ls).linenumber;
    luaX_next(ls);
    newgotoentry(
        ls,
        luaS_newlstr(
            (*ls).L,
            b"break\0" as *const u8 as *const libc::c_char,
            ::core::mem::size_of::<[libc::c_char; 6]>()
                .wrapping_div(::core::mem::size_of::<libc::c_char>())
                .wrapping_sub(1),
        ),
        line,
        luaK_jump((*ls).fs),
    );
}

unsafe extern "C" fn checkrepeated(mut ls: *mut LexState, mut name: *mut TString) {
    let mut lb: *mut Labeldesc = findlabel(ls, name);
    if ((lb != 0 as *mut libc::c_void as *mut Labeldesc) as libc::c_int != 0 as libc::c_int)
        as libc::c_int as libc::c_long
        != 0
    {
        luaK_semerror(
            ls,
            format_args!(
                "label '{}' already defined on line {}",
                CStr::from_ptr(((*name).contents).as_mut_ptr()).to_string_lossy(),
                (*lb).line
            ),
        );
    }
}

unsafe extern "C" fn labelstat(
    mut ls: *mut LexState,
    mut name: *mut TString,
    mut line: libc::c_int,
) {
    checknext(ls, TK_DBCOLON as libc::c_int);
    while (*ls).t.token == ';' as i32 || (*ls).t.token == TK_DBCOLON as libc::c_int {
        statement(ls);
    }
    checkrepeated(ls, name);
    createlabel(ls, name, line, block_follow(ls, 0 as libc::c_int));
}

unsafe extern "C" fn whilestat(mut ls: *mut LexState, mut line: libc::c_int) {
    let mut fs: *mut FuncState = (*ls).fs;
    let mut whileinit: libc::c_int = 0;
    let mut condexit: libc::c_int = 0;
    let mut bl: BlockCnt = BlockCnt {
        previous: 0 as *mut BlockCnt,
        firstlabel: 0,
        firstgoto: 0,
        nactvar: 0,
        upval: 0,
        isloop: 0,
        insidetbc: 0,
    };
    luaX_next(ls);
    whileinit = luaK_getlabel(fs);
    condexit = cond(ls);
    enterblock(fs, &mut bl, 1 as libc::c_int as u8);
    checknext(ls, TK_DO as libc::c_int);
    block(ls);
    luaK_patchlist(fs, luaK_jump(fs), whileinit);
    check_match(ls, TK_END as libc::c_int, TK_WHILE as libc::c_int, line);
    leaveblock(fs);
    luaK_patchtohere(fs, condexit);
}

unsafe extern "C" fn repeatstat(mut ls: *mut LexState, mut line: libc::c_int) {
    let mut condexit: libc::c_int = 0;
    let mut fs: *mut FuncState = (*ls).fs;
    let mut repeat_init: libc::c_int = luaK_getlabel(fs);
    let mut bl1: BlockCnt = BlockCnt {
        previous: 0 as *mut BlockCnt,
        firstlabel: 0,
        firstgoto: 0,
        nactvar: 0,
        upval: 0,
        isloop: 0,
        insidetbc: 0,
    };
    let mut bl2: BlockCnt = BlockCnt {
        previous: 0 as *mut BlockCnt,
        firstlabel: 0,
        firstgoto: 0,
        nactvar: 0,
        upval: 0,
        isloop: 0,
        insidetbc: 0,
    };
    enterblock(fs, &mut bl1, 1 as libc::c_int as u8);
    enterblock(fs, &mut bl2, 0 as libc::c_int as u8);
    luaX_next(ls);
    statlist(ls);
    check_match(ls, TK_UNTIL as libc::c_int, TK_REPEAT as libc::c_int, line);
    condexit = cond(ls);
    leaveblock(fs);
    if bl2.upval != 0 {
        let mut exit: libc::c_int = luaK_jump(fs);
        luaK_patchtohere(fs, condexit);
        luaK_codeABCk(
            fs,
            OP_CLOSE,
            reglevel(fs, bl2.nactvar as libc::c_int),
            0 as libc::c_int,
            0 as libc::c_int,
            0 as libc::c_int,
        );
        condexit = luaK_jump(fs);
        luaK_patchtohere(fs, exit);
    }
    luaK_patchlist(fs, condexit, repeat_init);
    leaveblock(fs);
}

unsafe extern "C" fn exp1(mut ls: *mut LexState) {
    let mut e: expdesc = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    expr(ls, &mut e);
    luaK_exp2nextreg((*ls).fs, &mut e);
}

unsafe extern "C" fn fixforjump(
    mut fs: *mut FuncState,
    mut pc: libc::c_int,
    mut dest: libc::c_int,
    mut back: libc::c_int,
) {
    let mut jmp: *mut u32 = &mut *((*(*fs).f).code).offset(pc as isize) as *mut u32;
    let mut offset: libc::c_int = dest - (pc + 1 as libc::c_int);
    if back != 0 {
        offset = -offset;
    }
    if ((offset
        > ((1 as libc::c_int) << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int)
            - 1 as libc::c_int) as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        luaX_syntaxerror((*fs).ls, "control structure too long");
    }
    *jmp = *jmp
        & !(!(!(0 as libc::c_int as u32)
            << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int)
            << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int)
        | (offset as u32) << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int
            & !(!(0 as libc::c_int as u32)
                << 8 as libc::c_int + 8 as libc::c_int + 1 as libc::c_int)
                << 0 as libc::c_int + 7 as libc::c_int + 8 as libc::c_int;
}

unsafe extern "C" fn forbody(
    mut ls: *mut LexState,
    mut base: libc::c_int,
    mut line: libc::c_int,
    mut nvars: libc::c_int,
    mut isgen: libc::c_int,
) {
    static mut forprep: [OpCode; 2] = [OP_FORPREP, OP_TFORPREP];
    static mut forloop: [OpCode; 2] = [OP_FORLOOP, OP_TFORLOOP];
    let mut bl: BlockCnt = BlockCnt {
        previous: 0 as *mut BlockCnt,
        firstlabel: 0,
        firstgoto: 0,
        nactvar: 0,
        upval: 0,
        isloop: 0,
        insidetbc: 0,
    };
    let mut fs: *mut FuncState = (*ls).fs;
    let mut prep: libc::c_int = 0;
    let mut endfor: libc::c_int = 0;
    checknext(ls, TK_DO as libc::c_int);
    prep = luaK_codeABx(
        fs,
        forprep[isgen as usize],
        base,
        0 as libc::c_int as libc::c_uint,
    );
    enterblock(fs, &mut bl, 0 as libc::c_int as u8);
    adjustlocalvars(ls, nvars);
    luaK_reserveregs(fs, nvars);
    block(ls);
    leaveblock(fs);
    fixforjump(fs, prep, luaK_getlabel(fs), 0 as libc::c_int);
    if isgen != 0 {
        luaK_codeABCk(
            fs,
            OP_TFORCALL,
            base,
            0 as libc::c_int,
            nvars,
            0 as libc::c_int,
        );
        luaK_fixline(fs, line);
    }
    endfor = luaK_codeABx(
        fs,
        forloop[isgen as usize],
        base,
        0 as libc::c_int as libc::c_uint,
    );
    fixforjump(fs, endfor, prep + 1 as libc::c_int, 1 as libc::c_int);
    luaK_fixline(fs, line);
}

unsafe extern "C" fn fornum(
    mut ls: *mut LexState,
    mut varname: *mut TString,
    mut line: libc::c_int,
) {
    let mut fs: *mut FuncState = (*ls).fs;
    let mut base: libc::c_int = (*fs).freereg as libc::c_int;
    new_localvar(
        ls,
        luaX_newstring(
            ls,
            b"(for state)\0" as *const u8 as *const libc::c_char,
            ::core::mem::size_of::<[libc::c_char; 12]>()
                .wrapping_div(::core::mem::size_of::<libc::c_char>())
                .wrapping_sub(1),
        ),
    );
    new_localvar(
        ls,
        luaX_newstring(
            ls,
            b"(for state)\0" as *const u8 as *const libc::c_char,
            ::core::mem::size_of::<[libc::c_char; 12]>()
                .wrapping_div(::core::mem::size_of::<libc::c_char>())
                .wrapping_sub(1),
        ),
    );
    new_localvar(
        ls,
        luaX_newstring(
            ls,
            b"(for state)\0" as *const u8 as *const libc::c_char,
            ::core::mem::size_of::<[libc::c_char; 12]>()
                .wrapping_div(::core::mem::size_of::<libc::c_char>())
                .wrapping_sub(1),
        ),
    );
    new_localvar(ls, varname);
    checknext(ls, '=' as i32);
    exp1(ls);
    checknext(ls, ',' as i32);
    exp1(ls);
    if testnext(ls, ',' as i32) != 0 {
        exp1(ls);
    } else {
        luaK_int(fs, (*fs).freereg as libc::c_int, 1 as libc::c_int as i64);
        luaK_reserveregs(fs, 1 as libc::c_int);
    }
    adjustlocalvars(ls, 3 as libc::c_int);
    forbody(ls, base, line, 1 as libc::c_int, 0 as libc::c_int);
}

unsafe extern "C" fn forlist(mut ls: *mut LexState, mut indexname: *mut TString) {
    let mut fs: *mut FuncState = (*ls).fs;
    let mut e: expdesc = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    let mut nvars: libc::c_int = 5 as libc::c_int;
    let mut line: libc::c_int = 0;
    let mut base: libc::c_int = (*fs).freereg as libc::c_int;
    new_localvar(
        ls,
        luaX_newstring(
            ls,
            b"(for state)\0" as *const u8 as *const libc::c_char,
            ::core::mem::size_of::<[libc::c_char; 12]>()
                .wrapping_div(::core::mem::size_of::<libc::c_char>())
                .wrapping_sub(1),
        ),
    );
    new_localvar(
        ls,
        luaX_newstring(
            ls,
            b"(for state)\0" as *const u8 as *const libc::c_char,
            ::core::mem::size_of::<[libc::c_char; 12]>()
                .wrapping_div(::core::mem::size_of::<libc::c_char>())
                .wrapping_sub(1),
        ),
    );
    new_localvar(
        ls,
        luaX_newstring(
            ls,
            b"(for state)\0" as *const u8 as *const libc::c_char,
            ::core::mem::size_of::<[libc::c_char; 12]>()
                .wrapping_div(::core::mem::size_of::<libc::c_char>())
                .wrapping_sub(1),
        ),
    );
    new_localvar(
        ls,
        luaX_newstring(
            ls,
            b"(for state)\0" as *const u8 as *const libc::c_char,
            ::core::mem::size_of::<[libc::c_char; 12]>()
                .wrapping_div(::core::mem::size_of::<libc::c_char>())
                .wrapping_sub(1),
        ),
    );
    new_localvar(ls, indexname);
    while testnext(ls, ',' as i32) != 0 {
        new_localvar(ls, str_checkname(ls));
        nvars += 1;
        nvars;
    }
    checknext(ls, TK_IN as libc::c_int);
    line = (*ls).linenumber;
    adjust_assign(ls, 4 as libc::c_int, explist(ls, &mut e), &mut e);
    adjustlocalvars(ls, 4 as libc::c_int);
    marktobeclosed(fs);
    luaK_checkstack(fs, 3 as libc::c_int);
    forbody(ls, base, line, nvars - 4 as libc::c_int, 1 as libc::c_int);
}

unsafe extern "C" fn forstat(mut ls: *mut LexState, mut line: libc::c_int) {
    let mut fs: *mut FuncState = (*ls).fs;
    let mut varname: *mut TString = 0 as *mut TString;
    let mut bl: BlockCnt = BlockCnt {
        previous: 0 as *mut BlockCnt,
        firstlabel: 0,
        firstgoto: 0,
        nactvar: 0,
        upval: 0,
        isloop: 0,
        insidetbc: 0,
    };
    enterblock(fs, &mut bl, 1 as libc::c_int as u8);
    luaX_next(ls);
    varname = str_checkname(ls);
    match (*ls).t.token {
        61 => {
            fornum(ls, varname, line);
        }
        44 | 267 => {
            forlist(ls, varname);
        }
        _ => {
            luaX_syntaxerror(ls, "'=' or 'in' expected");
        }
    }
    check_match(ls, TK_END as libc::c_int, TK_FOR as libc::c_int, line);
    leaveblock(fs);
}

unsafe extern "C" fn test_then_block(mut ls: *mut LexState, mut escapelist: *mut libc::c_int) {
    let mut bl: BlockCnt = BlockCnt {
        previous: 0 as *mut BlockCnt,
        firstlabel: 0,
        firstgoto: 0,
        nactvar: 0,
        upval: 0,
        isloop: 0,
        insidetbc: 0,
    };
    let mut fs: *mut FuncState = (*ls).fs;
    let mut v: expdesc = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    let mut jf: libc::c_int = 0;
    luaX_next(ls);
    expr(ls, &mut v);
    checknext(ls, TK_THEN as libc::c_int);
    if (*ls).t.token == TK_BREAK as libc::c_int {
        let mut line: libc::c_int = (*ls).linenumber;
        luaK_goiffalse((*ls).fs, &mut v);
        luaX_next(ls);
        enterblock(fs, &mut bl, 0 as libc::c_int as u8);
        newgotoentry(
            ls,
            luaS_newlstr(
                (*ls).L,
                b"break\0" as *const u8 as *const libc::c_char,
                ::core::mem::size_of::<[libc::c_char; 6]>()
                    .wrapping_div(::core::mem::size_of::<libc::c_char>())
                    .wrapping_sub(1),
            ),
            line,
            v.t,
        );
        while testnext(ls, ';' as i32) != 0 {}
        if block_follow(ls, 0 as libc::c_int) != 0 {
            leaveblock(fs);
            return;
        } else {
            jf = luaK_jump(fs);
        }
    } else {
        luaK_goiftrue((*ls).fs, &mut v);
        enterblock(fs, &mut bl, 0 as libc::c_int as u8);
        jf = v.f;
    }
    statlist(ls);
    leaveblock(fs);
    if (*ls).t.token == TK_ELSE as libc::c_int || (*ls).t.token == TK_ELSEIF as libc::c_int {
        luaK_concat(fs, escapelist, luaK_jump(fs));
    }
    luaK_patchtohere(fs, jf);
}

unsafe extern "C" fn ifstat(mut ls: *mut LexState, mut line: libc::c_int) {
    let mut fs: *mut FuncState = (*ls).fs;
    let mut escapelist: libc::c_int = -(1 as libc::c_int);
    test_then_block(ls, &mut escapelist);
    while (*ls).t.token == TK_ELSEIF as libc::c_int {
        test_then_block(ls, &mut escapelist);
    }
    if testnext(ls, TK_ELSE as libc::c_int) != 0 {
        block(ls);
    }
    check_match(ls, TK_END as libc::c_int, TK_IF as libc::c_int, line);
    luaK_patchtohere(fs, escapelist);
}

unsafe extern "C" fn localfunc(mut ls: *mut LexState) {
    let mut b: expdesc = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    let mut fs: *mut FuncState = (*ls).fs;
    let mut fvar: libc::c_int = (*fs).nactvar as libc::c_int;
    new_localvar(ls, str_checkname(ls));
    adjustlocalvars(ls, 1 as libc::c_int);
    body(ls, &mut b, 0 as libc::c_int, (*ls).linenumber);
    (*localdebuginfo(fs, fvar)).startpc = (*fs).pc;
}

unsafe extern "C" fn getlocalattribute(mut ls: *mut LexState) -> libc::c_int {
    if testnext(ls, '<' as i32) != 0 {
        let mut attr: *const libc::c_char = ((*str_checkname(ls)).contents).as_mut_ptr();
        checknext(ls, '>' as i32);
        if strcmp(attr, b"const\0" as *const u8 as *const libc::c_char) == 0 as libc::c_int {
            return 1 as libc::c_int;
        } else if strcmp(attr, b"close\0" as *const u8 as *const libc::c_char) == 0 as libc::c_int {
            return 2 as libc::c_int;
        } else {
            luaK_semerror(
                ls,
                format_args!(
                    "unknown attribute '{}'",
                    CStr::from_ptr(attr).to_string_lossy()
                ),
            );
        }
    }
    return 0 as libc::c_int;
}

unsafe extern "C" fn checktoclose(mut fs: *mut FuncState, mut level: libc::c_int) {
    if level != -(1 as libc::c_int) {
        marktobeclosed(fs);
        luaK_codeABCk(
            fs,
            OP_TBC,
            reglevel(fs, level),
            0 as libc::c_int,
            0 as libc::c_int,
            0 as libc::c_int,
        );
    }
}

unsafe extern "C" fn localstat(mut ls: *mut LexState) {
    let mut fs: *mut FuncState = (*ls).fs;
    let mut toclose: libc::c_int = -(1 as libc::c_int);
    let mut var: *mut Vardesc = 0 as *mut Vardesc;
    let mut vidx: libc::c_int = 0;
    let mut kind: libc::c_int = 0;
    let mut nvars: libc::c_int = 0 as libc::c_int;
    let mut nexps: libc::c_int = 0;
    let mut e: expdesc = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    loop {
        vidx = new_localvar(ls, str_checkname(ls));
        kind = getlocalattribute(ls);
        (*getlocalvardesc(fs, vidx)).vd.kind = kind as u8;
        if kind == 2 as libc::c_int {
            if toclose != -(1 as libc::c_int) {
                luaK_semerror(ls, "multiple to-be-closed variables in local list\0");
            }
            toclose = (*fs).nactvar as libc::c_int + nvars;
        }
        nvars += 1;
        nvars;
        if !(testnext(ls, ',' as i32) != 0) {
            break;
        }
    }
    if testnext(ls, '=' as i32) != 0 {
        nexps = explist(ls, &mut e);
    } else {
        e.k = VVOID;
        nexps = 0 as libc::c_int;
    }
    var = getlocalvardesc(fs, vidx);
    if nvars == nexps
        && (*var).vd.kind as libc::c_int == 1 as libc::c_int
        && luaK_exp2const(fs, &mut e, &mut (*var).k) != 0
    {
        (*var).vd.kind = 3 as libc::c_int as u8;
        adjustlocalvars(ls, nvars - 1 as libc::c_int);
        (*fs).nactvar = ((*fs).nactvar).wrapping_add(1);
        (*fs).nactvar;
    } else {
        adjust_assign(ls, nvars, nexps, &mut e);
        adjustlocalvars(ls, nvars);
    }
    checktoclose(fs, toclose);
}

unsafe extern "C" fn funcname(mut ls: *mut LexState, mut v: *mut expdesc) -> libc::c_int {
    let mut ismethod: libc::c_int = 0 as libc::c_int;
    singlevar(ls, v);
    while (*ls).t.token == '.' as i32 {
        fieldsel(ls, v);
    }
    if (*ls).t.token == ':' as i32 {
        ismethod = 1 as libc::c_int;
        fieldsel(ls, v);
    }
    return ismethod;
}

unsafe extern "C" fn funcstat(mut ls: *mut LexState, mut line: libc::c_int) {
    let mut ismethod: libc::c_int = 0;
    let mut v: expdesc = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    let mut b: expdesc = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    luaX_next(ls);
    ismethod = funcname(ls, &mut v);
    body(ls, &mut b, ismethod, line);
    check_readonly(ls, &mut v);
    luaK_storevar((*ls).fs, &mut v, &mut b);
    luaK_fixline((*ls).fs, line);
}

unsafe extern "C" fn exprstat(mut ls: *mut LexState) {
    let mut fs: *mut FuncState = (*ls).fs;
    let mut v: LHS_assign = LHS_assign {
        prev: 0 as *mut LHS_assign,
        v: expdesc {
            k: VVOID,
            u: C2RustUnnamed_11 { ival: 0 },
            t: 0,
            f: 0,
        },
    };
    suffixedexp(ls, &mut v.v);
    if (*ls).t.token == '=' as i32 || (*ls).t.token == ',' as i32 {
        v.prev = 0 as *mut LHS_assign;
        restassign(ls, &mut v, 1 as libc::c_int);
    } else {
        let mut inst: *mut u32 = 0 as *mut u32;
        if !(v.v.k as libc::c_uint == VCALL as libc::c_int as libc::c_uint) {
            luaX_syntaxerror(ls, "syntax error");
        }
        inst = &mut *((*(*fs).f).code).offset(v.v.u.info as isize) as *mut u32;
        *inst = *inst
            & !(!(!(0 as libc::c_int as u32) << 8 as libc::c_int)
                << 0 as libc::c_int
                    + 7 as libc::c_int
                    + 8 as libc::c_int
                    + 1 as libc::c_int
                    + 8 as libc::c_int)
            | (1 as libc::c_int as u32)
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
    };
}

unsafe extern "C" fn retstat(mut ls: *mut LexState) {
    let mut fs: *mut FuncState = (*ls).fs;
    let mut e: expdesc = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    let mut nret: libc::c_int = 0;
    let mut first: libc::c_int = luaY_nvarstack(fs);
    if block_follow(ls, 1 as libc::c_int) != 0 || (*ls).t.token == ';' as i32 {
        nret = 0 as libc::c_int;
    } else {
        nret = explist(ls, &mut e);
        if e.k as libc::c_uint == VCALL as libc::c_int as libc::c_uint
            || e.k as libc::c_uint == VVARARG as libc::c_int as libc::c_uint
        {
            luaK_setreturns(fs, &mut e, -(1 as libc::c_int));
            if e.k as libc::c_uint == VCALL as libc::c_int as libc::c_uint
                && nret == 1 as libc::c_int
                && (*(*fs).bl).insidetbc == 0
            {
                *((*(*fs).f).code).offset(e.u.info as isize) = *((*(*fs).f).code)
                    .offset(e.u.info as isize)
                    & !(!(!(0 as libc::c_int as u32) << 7 as libc::c_int) << 0 as libc::c_int)
                    | (OP_TAILCALL as libc::c_int as u32) << 0 as libc::c_int
                        & !(!(0 as libc::c_int as u32) << 7 as libc::c_int) << 0 as libc::c_int;
            }
            nret = -(1 as libc::c_int);
        } else if nret == 1 as libc::c_int {
            first = luaK_exp2anyreg(fs, &mut e);
        } else {
            luaK_exp2nextreg(fs, &mut e);
        }
    }
    luaK_ret(fs, first, nret);
    testnext(ls, ';' as i32);
}

unsafe extern "C" fn statement(mut ls: *mut LexState) {
    let mut line: libc::c_int = (*ls).linenumber;
    luaE_incCstack((*ls).L);
    match (*ls).t.token {
        59 => {
            luaX_next(ls);
        }
        266 => {
            ifstat(ls, line);
        }
        277 => {
            whilestat(ls, line);
        }
        258 => {
            luaX_next(ls);
            block(ls);
            check_match(ls, TK_END as libc::c_int, TK_DO as libc::c_int, line);
        }
        263 => {
            forstat(ls, line);
        }
        272 => {
            repeatstat(ls, line);
        }
        264 => {
            funcstat(ls, line);
        }
        268 => {
            luaX_next(ls);
            if testnext(ls, TK_FUNCTION as libc::c_int) != 0 {
                localfunc(ls);
            } else {
                localstat(ls);
            }
        }
        287 => {
            luaX_next(ls);
            labelstat(ls, str_checkname(ls), line);
        }
        273 => {
            luaX_next(ls);
            retstat(ls);
        }
        257 => {
            breakstat(ls);
        }
        265 => {
            luaX_next(ls);
            gotostat(ls);
        }
        _ => {
            exprstat(ls);
        }
    }
    (*(*ls).fs).freereg = luaY_nvarstack((*ls).fs) as u8;
    (*(*ls).L).nCcalls = ((*(*ls).L).nCcalls).wrapping_sub(1);
    (*(*ls).L).nCcalls;
}

unsafe extern "C" fn mainfunc(mut ls: *mut LexState, mut fs: *mut FuncState) {
    let mut bl: BlockCnt = BlockCnt {
        previous: 0 as *mut BlockCnt,
        firstlabel: 0,
        firstgoto: 0,
        nactvar: 0,
        upval: 0,
        isloop: 0,
        insidetbc: 0,
    };
    let mut env: *mut Upvaldesc = 0 as *mut Upvaldesc;
    open_func(ls, fs, &mut bl);
    setvararg(fs, 0 as libc::c_int);
    env = allocupvalue(fs);
    (*env).instack = 1 as libc::c_int as u8;
    (*env).idx = 0 as libc::c_int as u8;
    (*env).kind = 0 as libc::c_int as u8;
    (*env).name = (*ls).envn;
    if (*(*fs).f).marked as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
        && (*(*env).name).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            != 0
    {
        luaC_barrier_(
            (*ls).L,
            &mut (*((*fs).f as *mut GCUnion)).gc,
            &mut (*((*env).name as *mut GCUnion)).gc,
        );
    } else {
    };
    luaX_next(ls);
    statlist(ls);
    check(ls, TK_EOS as libc::c_int);
    close_func(ls);
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaY_parser(
    mut L: *mut lua_State,
    mut z: *mut ZIO,
    mut buff: *mut Mbuffer,
    mut dyd: *mut Dyndata,
    mut name: *const libc::c_char,
    mut firstchar: libc::c_int,
) -> *mut LClosure {
    let mut lexstate: LexState = LexState {
        current: 0,
        linenumber: 0,
        lastline: 0,
        t: Token {
            token: 0,
            seminfo: SemInfo { r: 0. },
        },
        lookahead: Token {
            token: 0,
            seminfo: SemInfo { r: 0. },
        },
        fs: 0 as *mut FuncState,
        L: 0 as *mut lua_State,
        z: 0 as *mut ZIO,
        buff: 0 as *mut Mbuffer,
        h: 0 as *mut Table,
        dyd: 0 as *mut Dyndata,
        source: 0 as *mut TString,
        envn: 0 as *mut TString,
    };
    let mut funcstate: FuncState = FuncState {
        f: 0 as *mut Proto,
        prev: 0 as *mut FuncState,
        ls: 0 as *mut LexState,
        bl: 0 as *mut BlockCnt,
        pc: 0,
        lasttarget: 0,
        previousline: 0,
        nk: 0,
        np: 0,
        nabslineinfo: 0,
        firstlocal: 0,
        firstlabel: 0,
        ndebugvars: 0,
        nactvar: 0,
        nups: 0,
        freereg: 0,
        iwthabs: 0,
        needclose: 0,
    };
    let mut cl: *mut LClosure = luaF_newLclosure(L, 1 as libc::c_int);
    let mut io: *mut TValue = &mut (*(*L).top.p).val;
    let mut x_: *mut LClosure = cl;
    (*io).value_.gc = &mut (*(x_ as *mut GCUnion)).gc;
    (*io).tt_ = (6 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 6 as libc::c_int) as u8;
    luaD_inctop(L);
    lexstate.h = luaH_new(L);
    let mut io_0: *mut TValue = &mut (*(*L).top.p).val;
    let mut x__0: *mut Table = lexstate.h;
    (*io_0).value_.gc = &mut (*(x__0 as *mut GCUnion)).gc;
    (*io_0).tt_ = (5 as libc::c_int
        | (0 as libc::c_int) << 4 as libc::c_int
        | (1 as libc::c_int) << 6 as libc::c_int) as u8;
    luaD_inctop(L);
    (*cl).p = luaF_newproto(L);
    funcstate.f = (*cl).p;
    if (*cl).marked as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
        && (*(*cl).p).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            != 0
    {
        luaC_barrier_(
            L,
            &mut (*(cl as *mut GCUnion)).gc,
            &mut (*((*cl).p as *mut GCUnion)).gc,
        );
    } else {
    };
    (*funcstate.f).source = luaS_new(L, name);
    if (*funcstate.f).marked as libc::c_int & (1 as libc::c_int) << 5 as libc::c_int != 0
        && (*(*funcstate.f).source).marked as libc::c_int
            & ((1 as libc::c_int) << 3 as libc::c_int | (1 as libc::c_int) << 4 as libc::c_int)
            != 0
    {
        luaC_barrier_(
            L,
            &mut (*(funcstate.f as *mut GCUnion)).gc,
            &mut (*((*funcstate.f).source as *mut GCUnion)).gc,
        );
    } else {
    };
    lexstate.buff = buff;
    lexstate.dyd = dyd;
    (*dyd).label.n = 0 as libc::c_int;
    (*dyd).gt.n = (*dyd).label.n;
    (*dyd).actvar.n = (*dyd).gt.n;
    luaX_setinput(L, &mut lexstate, z, (*funcstate.f).source, firstchar);
    mainfunc(&mut lexstate, &mut funcstate);
    (*L).top.p = ((*L).top.p).offset(-1);
    (*L).top.p;
    return cl;
}
