#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments
)]
#![allow(unsafe_op_in_unsafe_fn)]

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
use crate::lfunc::{luaF_newLclosure, luaF_newproto};
use crate::llex::{
    LexState, SemInfo, TK_AND, TK_BREAK, TK_DBCOLON, TK_DO, TK_ELSE, TK_ELSEIF, TK_END, TK_EOS,
    TK_FALSE, TK_FOR, TK_FUNCTION, TK_GOTO, TK_IF, TK_IN, TK_LOCAL, TK_NAME, TK_NIL, TK_NOT, TK_OR,
    TK_REPEAT, TK_RETURN, TK_THEN, TK_TRUE, TK_UNTIL, TK_WHILE, Token, luaX_lookahead,
    luaX_newstring, luaX_next, luaX_setinput, luaX_syntaxerror, luaX_token2str,
};
use crate::lmem::{luaM_growaux_, luaM_shrinkvector_};
use crate::lobject::{AbsLineInfo, LocVar, Proto, Upvaldesc};
use crate::lzio::{Mbuffer, ZIO};
use crate::value::{UnsafeValue, UntaggedValue};
use crate::vm::{
    OP_CALL, OP_CLOSE, OP_CLOSURE, OP_FORLOOP, OP_FORPREP, OP_GETUPVAL, OP_MOVE, OP_NEWTABLE,
    OP_TAILCALL, OP_TBC, OP_TFORCALL, OP_TFORLOOP, OP_TFORPREP, OP_VARARG, OP_VARARGPREP, OpCode,
};
use crate::{ChunkInfo, Lua, LuaFn, Object, ParseError, Ref, Str, Table};
use alloc::borrow::Cow;
use alloc::format;
use alloc::string::String;
use core::fmt::Display;
use core::ops::Deref;
use core::ptr::null;

type c_int = i32;

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
    pub n: c_int,
    pub size: c_int,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Labeldesc {
    pub name: *const Str,
    pub pc: c_int,
    pub line: c_int,
    pub nactvar: u8,
    pub close: u8,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct C2RustUnnamed_9 {
    pub arr: *mut Vardesc,
    pub n: c_int,
    pub size: c_int,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub union Vardesc {
    pub vd: C2RustUnnamed_10,
    pub k: UnsafeValue,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct C2RustUnnamed_10 {
    pub value_: UntaggedValue,
    pub tt_: u8,
    pub kind: u8,
    pub ridx: u8,
    pub pidx: libc::c_short,
    pub name: *const Str,
}

#[repr(C)]
pub struct FuncState {
    pub f: *mut Proto,
    pub prev: *mut FuncState,
    pub bl: *mut BlockCnt,
    pub pc: c_int,
    pub lasttarget: c_int,
    pub previousline: c_int,
    pub nk: c_int,
    pub np: c_int,
    pub nabslineinfo: c_int,
    pub firstlocal: c_int,
    pub firstlabel: c_int,
    pub ndebugvars: libc::c_short,
    pub nactvar: u8,
    pub nups: u8,
    pub freereg: u8,
    pub iwthabs: u8,
    pub needclose: u8,
}

impl Default for FuncState {
    fn default() -> Self {
        Self {
            f: 0 as *mut Proto,
            prev: 0 as *mut FuncState,
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
        }
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct BlockCnt {
    pub previous: *mut BlockCnt,
    pub firstlabel: c_int,
    pub firstgoto: c_int,
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
    pub t: c_int,
    pub f: c_int,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub union C2RustUnnamed_11 {
    pub ival: i64,
    pub nval: f64,
    pub strval: *const Str,
    pub info: c_int,
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
    pub nh: c_int,
    pub na: c_int,
    pub tostore: c_int,
}

unsafe fn error_expected(ls: *mut LexState, token: c_int) -> ParseError {
    luaX_syntaxerror(ls, format_args!("{} expected", luaX_token2str(token)))
}

unsafe fn errorlimit(
    ls: *mut LexState,
    fs: *mut FuncState,
    limit: c_int,
    what: impl Display,
) -> ParseError {
    let line: c_int = (*(*fs).f).linedefined;
    let where_0: Cow<'static, str> = if line == 0 as c_int {
        "main function".into()
    } else {
        format!("function at line {line}").into()
    };

    luaX_syntaxerror(
        ls,
        format_args!("too many {what} (limit is {limit}) in {where_0}"),
    )
}

unsafe fn checklimit(
    ls: *mut LexState,
    fs: *mut FuncState,
    v: c_int,
    l: c_int,
    what: impl Display,
) -> Result<(), ParseError> {
    if v > l {
        return Err(errorlimit(ls, fs, l, what));
    }
    Ok(())
}

unsafe fn testnext(ls: *mut LexState, c: c_int) -> Result<c_int, ParseError> {
    if (*ls).t.token == c {
        luaX_next(ls)?;
        return Ok(1 as c_int);
    } else {
        return Ok(0 as c_int);
    };
}

unsafe fn check(ls: *mut LexState, c: c_int) -> Result<(), ParseError> {
    if (*ls).t.token != c {
        return Err(error_expected(ls, c));
    }
    Ok(())
}

unsafe fn checknext(ls: *mut LexState, c: c_int) -> Result<(), ParseError> {
    check(ls, c)?;
    luaX_next(ls)
}

unsafe fn check_match(
    ls: *mut LexState,
    what: c_int,
    who: c_int,
    where_0: c_int,
) -> Result<(), ParseError> {
    if ((testnext(ls, what)? == 0) as c_int != 0 as c_int) as c_int as libc::c_long != 0 {
        if where_0 == (*ls).linenumber {
            return Err(error_expected(ls, what));
        } else {
            return Err(luaX_syntaxerror(
                ls,
                format_args!(
                    "{} expected (to close {} at line {})",
                    luaX_token2str(what),
                    luaX_token2str(who),
                    where_0,
                ),
            ));
        }
    }
    Ok(())
}

unsafe fn str_checkname(ls: *mut LexState) -> Result<*const Str, ParseError> {
    check(ls, TK_NAME as c_int)?;
    let ts = (*ls).t.seminfo.ts;
    luaX_next(ls)?;
    return Ok(ts);
}

unsafe fn init_exp(e: *mut expdesc, k: expkind, i: c_int) {
    (*e).t = -(1 as c_int);
    (*e).f = (*e).t;
    (*e).k = k;
    (*e).u.info = i;
}

unsafe fn codestring(e: *mut expdesc, s: *const Str) {
    (*e).t = -(1 as c_int);
    (*e).f = (*e).t;
    (*e).k = VKSTR;
    (*e).u.strval = s;
}

unsafe fn codename(ls: *mut LexState, e: *mut expdesc) -> Result<(), ParseError> {
    codestring(e, str_checkname(ls)?);
    Ok(())
}

unsafe fn registerlocalvar(
    ls: *mut LexState,
    fs: *mut FuncState,
    varname: *const Str,
) -> Result<c_int, ParseError> {
    let f: *mut Proto = (*fs).f;
    let mut oldsize: c_int = (*f).sizelocvars;
    (*f).locvars = luaM_growaux_(
        &(*ls).g,
        (*f).locvars as *mut libc::c_void,
        (*fs).ndebugvars as c_int,
        &mut (*f).sizelocvars,
        ::core::mem::size_of::<LocVar>() as libc::c_ulong as c_int,
        (if 32767 as c_int as usize
            <= (!(0 as c_int as usize)).wrapping_div(::core::mem::size_of::<LocVar>())
        {
            32767 as c_int as libc::c_uint
        } else {
            (!(0 as c_int as usize)).wrapping_div(::core::mem::size_of::<LocVar>()) as libc::c_uint
        }) as c_int,
        "local variables",
        (*ls).linenumber,
    )? as *mut LocVar;
    while oldsize < (*f).sizelocvars {
        let fresh0 = oldsize;
        oldsize = oldsize + 1;
        let ref mut fresh1 = (*((*f).locvars).offset(fresh0 as isize)).varname;
        *fresh1 = 0 as *mut Str;
    }
    let ref mut fresh2 = (*((*f).locvars).offset((*fs).ndebugvars as isize)).varname;
    *fresh2 = varname;
    (*((*f).locvars).offset((*fs).ndebugvars as isize)).startpc = (*fs).pc;

    if (*f).hdr.marked.get() as c_int & (1 as c_int) << 5 as c_int != 0
        && (*varname).hdr.marked.get() as c_int
            & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
            != 0
    {
        (*ls).g.gc.barrier(f as *mut Object, varname as *mut Object);
    }

    let fresh3 = (*fs).ndebugvars;
    (*fs).ndebugvars = (*fs).ndebugvars + 1;
    return Ok(fresh3 as c_int);
}

unsafe fn new_localvar(ls: *mut LexState, name: *const Str) -> Result<c_int, ParseError> {
    let fs: *mut FuncState = (*ls).fs;
    let dyd: *mut Dyndata = (*ls).dyd;
    let mut var: *mut Vardesc = 0 as *mut Vardesc;

    checklimit(
        ls,
        fs,
        (*dyd).actvar.n + 1 as c_int - (*fs).firstlocal,
        200 as c_int,
        "local variables",
    )?;

    (*dyd).actvar.arr = luaM_growaux_(
        &(*ls).g,
        (*dyd).actvar.arr as *mut libc::c_void,
        (*dyd).actvar.n + 1 as c_int,
        &raw mut (*dyd).actvar.size,
        size_of::<Vardesc>() as libc::c_ulong as c_int,
        i16::MAX.into(),
        "local variables",
        (*ls).linenumber,
    )? as *mut Vardesc;
    let fresh4 = (*dyd).actvar.n;
    (*dyd).actvar.n = (*dyd).actvar.n + 1;
    var = &mut *((*dyd).actvar.arr).offset(fresh4 as isize) as *mut Vardesc;
    (*var).vd.kind = 0 as c_int as u8;
    (*var).vd.name = name;
    return Ok((*dyd).actvar.n - 1 as c_int - (*fs).firstlocal);
}

unsafe fn getlocalvardesc(ls: *mut LexState, fs: *mut FuncState, vidx: c_int) -> *mut Vardesc {
    return ((*(*ls).dyd).actvar.arr).offset(((*fs).firstlocal + vidx) as isize) as *mut Vardesc;
}

unsafe fn reglevel(ls: *mut LexState, fs: *mut FuncState, mut nvar: c_int) -> c_int {
    loop {
        let fresh5 = nvar;
        nvar = nvar - 1;
        if !(fresh5 > 0 as c_int) {
            break;
        }
        let vd: *mut Vardesc = getlocalvardesc(ls, fs, nvar);
        if (*vd).vd.kind as c_int != 3 as c_int {
            return (*vd).vd.ridx as c_int + 1 as c_int;
        }
    }
    return 0 as c_int;
}

pub unsafe fn luaY_nvarstack(ls: *mut LexState, fs: *mut FuncState) -> c_int {
    return reglevel(ls, fs, (*fs).nactvar as c_int);
}

unsafe fn localdebuginfo(ls: *mut LexState, fs: *mut FuncState, vidx: c_int) -> *mut LocVar {
    let vd: *mut Vardesc = getlocalvardesc(ls, fs, vidx);
    if (*vd).vd.kind as c_int == 3 as c_int {
        return 0 as *mut LocVar;
    } else {
        let idx: c_int = (*vd).vd.pidx as c_int;
        return ((*(*fs).f).locvars).offset(idx as isize) as *mut LocVar;
    };
}

unsafe fn init_var(ls: *mut LexState, fs: *mut FuncState, e: *mut expdesc, vidx: c_int) {
    (*e).t = -(1 as c_int);
    (*e).f = (*e).t;
    (*e).k = VLOCAL;
    (*e).u.var.vidx = vidx as libc::c_ushort;
    (*e).u.var.ridx = (*getlocalvardesc(ls, fs, vidx)).vd.ridx;
}

unsafe fn check_readonly(ls: *mut LexState, e: *mut expdesc) -> Result<(), ParseError> {
    let fs: *mut FuncState = (*ls).fs;
    let mut varname = null();

    match (*e).k as libc::c_uint {
        11 => {
            varname = (*((*(*ls).dyd).actvar.arr).offset((*e).u.info as isize))
                .vd
                .name;
        }
        9 => {
            let vardesc: *mut Vardesc = getlocalvardesc(ls, fs, (*e).u.var.vidx as c_int);
            if (*vardesc).vd.kind as c_int != 0 as c_int {
                varname = (*vardesc).vd.name;
            }
        }
        10 => {
            let up: *mut Upvaldesc =
                &mut *((*(*fs).f).upvalues).offset((*e).u.info as isize) as *mut Upvaldesc;
            if (*up).kind as c_int != 0 as c_int {
                varname = (*up).name;
            }
        }
        _ => return Ok(()),
    }

    if !varname.is_null() {
        return Err(luaK_semerror(
            ls,
            format_args!(
                "attempt to assign to const variable '{}'",
                String::from_utf8_lossy((*varname).as_bytes()),
            ),
        ));
    }

    Ok(())
}

unsafe fn adjustlocalvars(ls: *mut LexState, nvars: c_int) -> Result<(), ParseError> {
    let fs: *mut FuncState = (*ls).fs;
    let mut reglevel_0: c_int = luaY_nvarstack(ls, fs);
    let mut i: c_int = 0;
    i = 0 as c_int;
    while i < nvars {
        let fresh6 = (*fs).nactvar;
        (*fs).nactvar = ((*fs).nactvar).wrapping_add(1);
        let vidx: c_int = fresh6 as c_int;
        let var: *mut Vardesc = getlocalvardesc(ls, fs, vidx);
        let fresh7 = reglevel_0;
        reglevel_0 = reglevel_0 + 1;
        (*var).vd.ridx = fresh7 as u8;
        (*var).vd.pidx = registerlocalvar(ls, fs, (*var).vd.name)? as libc::c_short;
        i += 1;
    }
    Ok(())
}

unsafe fn removevars(ls: *mut LexState, fs: *mut FuncState, tolevel: c_int) {
    (*(*ls).dyd).actvar.n -= (*fs).nactvar as c_int - tolevel;
    while (*fs).nactvar as c_int > tolevel {
        (*fs).nactvar = ((*fs).nactvar).wrapping_sub(1);
        let var: *mut LocVar = localdebuginfo(ls, fs, (*fs).nactvar as c_int);
        if !var.is_null() {
            (*var).endpc = (*fs).pc;
        }
    }
}

unsafe fn searchupvalue(fs: *mut FuncState, name: *const Str) -> c_int {
    let mut i: c_int = 0;
    let up: *mut Upvaldesc = (*(*fs).f).upvalues;
    i = 0 as c_int;
    while i < (*fs).nups as c_int {
        if (*up.offset(i as isize)).name == name {
            return i;
        }
        i += 1;
    }
    return -(1 as c_int);
}

unsafe fn allocupvalue(
    ls: *mut LexState,
    fs: *mut FuncState,
) -> Result<*mut Upvaldesc, ParseError> {
    let f: *mut Proto = (*fs).f;
    let mut oldsize: c_int = (*f).sizeupvalues;

    checklimit(
        ls,
        fs,
        (*fs).nups as c_int + 1 as c_int,
        255 as c_int,
        "upvalues",
    )?;

    (*f).upvalues = luaM_growaux_(
        &(*ls).g,
        (*f).upvalues as *mut libc::c_void,
        (*fs).nups as c_int,
        &mut (*f).sizeupvalues,
        ::core::mem::size_of::<Upvaldesc>() as libc::c_ulong as c_int,
        (if 255 as c_int as usize
            <= (!(0 as c_int as usize)).wrapping_div(::core::mem::size_of::<Upvaldesc>())
        {
            255 as c_int as libc::c_uint
        } else {
            (!(0 as c_int as usize)).wrapping_div(::core::mem::size_of::<Upvaldesc>())
                as libc::c_uint
        }) as c_int,
        "upvalues",
        (*ls).linenumber,
    )? as *mut Upvaldesc;
    while oldsize < (*f).sizeupvalues {
        let fresh8 = oldsize;
        oldsize = oldsize + 1;
        let ref mut fresh9 = (*((*f).upvalues).offset(fresh8 as isize)).name;
        *fresh9 = 0 as *mut Str;
    }
    let fresh10 = (*fs).nups;
    (*fs).nups = ((*fs).nups).wrapping_add(1);
    return Ok(&mut *((*f).upvalues).offset(fresh10 as isize) as *mut Upvaldesc);
}

unsafe fn newupvalue(
    ls: *mut LexState,
    fs: *mut FuncState,
    name: *const Str,
    v: *mut expdesc,
) -> Result<c_int, ParseError> {
    let up: *mut Upvaldesc = allocupvalue(ls, fs)?;
    let prev: *mut FuncState = (*fs).prev;

    if (*v).k as libc::c_uint == VLOCAL as c_int as libc::c_uint {
        (*up).instack = 1 as c_int as u8;
        (*up).idx = (*v).u.var.ridx;
        (*up).kind = (*getlocalvardesc(ls, prev, (*v).u.var.vidx.into())).vd.kind;
    } else {
        (*up).instack = 0 as c_int as u8;
        (*up).idx = (*v).u.info as u8;
        (*up).kind = (*((*(*prev).f).upvalues).offset((*v).u.info as isize)).kind;
    }

    (*up).name = name;

    if (*(*fs).f).hdr.marked.get() as c_int & (1 as c_int) << 5 as c_int != 0
        && (*name).hdr.marked.get() as c_int
            & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
            != 0
    {
        (*ls)
            .g
            .gc
            .barrier((*fs).f as *mut Object, name as *mut Object);
    }

    return Ok((*fs).nups as c_int - 1 as c_int);
}

unsafe fn searchvar(
    ls: *mut LexState,
    fs: *mut FuncState,
    n: *const Str,
    var: *mut expdesc,
) -> c_int {
    let mut i: c_int = 0;
    i = (*fs).nactvar as c_int - 1 as c_int;

    while i >= 0 as c_int {
        let vd: *mut Vardesc = getlocalvardesc(ls, fs, i);
        if n == (*vd).vd.name {
            if (*vd).vd.kind as c_int == 3 as c_int {
                init_exp(var, VCONST, (*fs).firstlocal + i);
            } else {
                init_var(ls, fs, var, i);
            }
            return (*var).k as c_int;
        }
        i -= 1;
    }

    return -(1 as c_int);
}

unsafe fn markupval(fs: *mut FuncState, level: c_int) {
    let mut bl: *mut BlockCnt = (*fs).bl;
    while (*bl).nactvar as c_int > level {
        bl = (*bl).previous;
    }
    (*bl).upval = 1 as c_int as u8;
    (*fs).needclose = 1 as c_int as u8;
}

unsafe fn marktobeclosed(fs: *mut FuncState) {
    let bl: *mut BlockCnt = (*fs).bl;
    (*bl).upval = 1 as c_int as u8;
    (*bl).insidetbc = 1 as c_int as u8;
    (*fs).needclose = 1 as c_int as u8;
}

unsafe fn singlevaraux(
    ls: *mut LexState,
    fs: *mut FuncState,
    n: *const Str,
    var: *mut expdesc,
    base: c_int,
) -> Result<(), ParseError> {
    if fs.is_null() {
        init_exp(var, VVOID, 0 as c_int);
    } else {
        let v: c_int = searchvar(ls, fs, n, var);
        if v >= 0 as c_int {
            if v == VLOCAL as c_int && base == 0 {
                markupval(fs, (*var).u.var.vidx as c_int);
            }
        } else {
            let mut idx: c_int = searchupvalue(fs, n);
            if idx < 0 as c_int {
                singlevaraux(ls, (*fs).prev, n, var, 0 as c_int)?;
                if (*var).k as libc::c_uint == VLOCAL as c_int as libc::c_uint
                    || (*var).k as libc::c_uint == VUPVAL as c_int as libc::c_uint
                {
                    idx = newupvalue(ls, fs, n, var)?;
                } else {
                    return Ok(());
                }
            }
            init_exp(var, VUPVAL, idx);
        }
    };
    Ok(())
}

unsafe fn singlevar(ls: *mut LexState, var: *mut expdesc) -> Result<(), ParseError> {
    let varname = str_checkname(ls)?;
    let fs: *mut FuncState = (*ls).fs;
    singlevaraux(ls, fs, varname, var, 1 as c_int)?;
    if (*var).k as libc::c_uint == VVOID as c_int as libc::c_uint {
        let mut key: expdesc = expdesc {
            k: VVOID,
            u: C2RustUnnamed_11 { ival: 0 },
            t: 0,
            f: 0,
        };
        singlevaraux(ls, fs, (*ls).envn, var, 1 as c_int)?;
        luaK_exp2anyregup(ls, fs, var)?;
        codestring(&mut key, varname);
        luaK_indexed(ls, fs, var, &mut key)?;
    }
    Ok(())
}

unsafe fn adjust_assign(
    ls: *mut LexState,
    nvars: c_int,
    nexps: c_int,
    e: *mut expdesc,
) -> Result<(), ParseError> {
    let fs: *mut FuncState = (*ls).fs;
    let needed: c_int = nvars - nexps;
    if (*e).k as libc::c_uint == VCALL as c_int as libc::c_uint
        || (*e).k as libc::c_uint == VVARARG as c_int as libc::c_uint
    {
        let mut extra: c_int = needed + 1 as c_int;
        if extra < 0 as c_int {
            extra = 0 as c_int;
        }
        luaK_setreturns(ls, fs, e, extra)?;
    } else {
        if (*e).k as libc::c_uint != VVOID as c_int as libc::c_uint {
            luaK_exp2nextreg(ls, fs, e)?;
        }
        if needed > 0 as c_int {
            luaK_nil(ls, fs, (*fs).freereg as c_int, needed)?;
        }
    }
    if needed > 0 as c_int {
        luaK_reserveregs(ls, fs, needed)?;
    } else {
        (*fs).freereg = ((*fs).freereg as c_int + needed) as u8;
    };
    Ok(())
}

unsafe fn jumpscopeerror(ls: *mut LexState, gt: *mut Labeldesc) -> ParseError {
    let varname = (*(*getlocalvardesc(ls, (*ls).fs, (*gt).nactvar.into()))
        .vd
        .name)
        .as_bytes();

    luaK_semerror(
        ls,
        format_args!(
            "<goto {}> at line {} jumps into the scope of local '{}'",
            String::from_utf8_lossy((*(*gt).name).as_bytes()),
            (*gt).line,
            String::from_utf8_lossy(varname),
        ),
    )
}

unsafe fn solvegoto(ls: *mut LexState, g: c_int, label: *mut Labeldesc) -> Result<(), ParseError> {
    let mut i: c_int = 0;
    let gl: *mut Labellist = &mut (*(*ls).dyd).gt;
    let gt: *mut Labeldesc = &mut *((*gl).arr).offset(g as isize) as *mut Labeldesc;
    if ((((*gt).nactvar as c_int) < (*label).nactvar as c_int) as c_int != 0 as c_int) as c_int
        as libc::c_long
        != 0
    {
        return Err(jumpscopeerror(ls, gt));
    }
    luaK_patchlist(ls, (*ls).fs, (*gt).pc, (*label).pc)?;
    i = g;
    while i < (*gl).n - 1 as c_int {
        *((*gl).arr).offset(i as isize) = *((*gl).arr).offset((i + 1 as c_int) as isize);
        i += 1;
    }
    (*gl).n -= 1;
    (*gl).n;
    Ok(())
}

unsafe fn findlabel(ls: *mut LexState, name: *const Str) -> *mut Labeldesc {
    let mut i: c_int = 0;
    let dyd: *mut Dyndata = (*ls).dyd;
    i = (*(*ls).fs).firstlabel;
    while i < (*dyd).label.n {
        let lb: *mut Labeldesc = &mut *((*dyd).label.arr).offset(i as isize) as *mut Labeldesc;
        if (*lb).name == name {
            return lb;
        }
        i += 1;
    }
    return 0 as *mut Labeldesc;
}

unsafe fn newlabelentry(
    ls: *mut LexState,
    l: *mut Labellist,
    name: *const Str,
    line: c_int,
    pc: c_int,
) -> Result<c_int, ParseError> {
    let n: c_int = (*l).n;
    (*l).arr = luaM_growaux_(
        &(*ls).g,
        (*l).arr as *mut libc::c_void,
        n,
        &mut (*l).size,
        ::core::mem::size_of::<Labeldesc>() as libc::c_ulong as c_int,
        (if 32767 as c_int as usize
            <= (!(0 as c_int as usize)).wrapping_div(::core::mem::size_of::<Labeldesc>())
        {
            32767 as c_int as libc::c_uint
        } else {
            (!(0 as c_int as usize)).wrapping_div(::core::mem::size_of::<Labeldesc>())
                as libc::c_uint
        }) as c_int,
        "labels/gotos",
        (*ls).linenumber,
    )? as *mut Labeldesc;
    let ref mut fresh11 = (*((*l).arr).offset(n as isize)).name;
    *fresh11 = name;
    (*((*l).arr).offset(n as isize)).line = line;
    (*((*l).arr).offset(n as isize)).nactvar = (*(*ls).fs).nactvar;
    (*((*l).arr).offset(n as isize)).close = 0 as c_int as u8;
    (*((*l).arr).offset(n as isize)).pc = pc;
    (*l).n = n + 1 as c_int;
    return Ok(n);
}

unsafe fn newgotoentry(
    ls: *mut LexState,
    name: *const Str,
    line: c_int,
    pc: c_int,
) -> Result<c_int, ParseError> {
    return newlabelentry(ls, &mut (*(*ls).dyd).gt, name, line, pc);
}

unsafe fn solvegotos(ls: *mut LexState, lb: *mut Labeldesc) -> Result<c_int, ParseError> {
    let gl: *mut Labellist = &mut (*(*ls).dyd).gt;
    let mut i: c_int = (*(*(*ls).fs).bl).firstgoto;
    let mut needsclose: c_int = 0 as c_int;
    while i < (*gl).n {
        if (*((*gl).arr).offset(i as isize)).name == (*lb).name {
            needsclose |= (*((*gl).arr).offset(i as isize)).close as c_int;
            solvegoto(ls, i, lb)?;
        } else {
            i += 1;
        }
    }
    return Ok(needsclose);
}

unsafe fn createlabel(
    ls: *mut LexState,
    name: *const Str,
    line: c_int,
    last: c_int,
) -> Result<c_int, ParseError> {
    let fs: *mut FuncState = (*ls).fs;
    let ll: *mut Labellist = &raw mut (*(*ls).dyd).label;
    let l: c_int = newlabelentry(ls, ll, name, line, luaK_getlabel(fs))?;
    if last != 0 {
        (*((*ll).arr).offset(l as isize)).nactvar = (*(*fs).bl).nactvar;
    }
    if solvegotos(ls, &mut *((*ll).arr).offset(l as isize))? != 0 {
        luaK_codeABCk(
            ls,
            fs,
            OP_CLOSE,
            luaY_nvarstack(ls, fs),
            0 as c_int,
            0 as c_int,
            0 as c_int,
        )?;
        return Ok(1 as c_int);
    }
    return Ok(0 as c_int);
}

unsafe fn movegotosout(ls: *mut LexState, fs: *mut FuncState, bl: *mut BlockCnt) {
    let mut i: c_int = 0;
    let gl: *mut Labellist = &raw mut (*(*ls).dyd).gt;
    i = (*bl).firstgoto;
    while i < (*gl).n {
        let gt: *mut Labeldesc = &mut *((*gl).arr).offset(i as isize) as *mut Labeldesc;
        if reglevel(ls, fs, (*gt).nactvar as c_int) > reglevel(ls, fs, (*bl).nactvar as c_int) {
            (*gt).close = ((*gt).close as c_int | (*bl).upval as c_int) as u8;
        }
        (*gt).nactvar = (*bl).nactvar;
        i += 1;
    }
}

unsafe fn enterblock(ls: *mut LexState, fs: *mut FuncState, bl: *mut BlockCnt, isloop: u8) {
    (*bl).isloop = isloop;
    (*bl).nactvar = (*fs).nactvar;
    (*bl).firstlabel = (*(*ls).dyd).label.n;
    (*bl).firstgoto = (*(*ls).dyd).gt.n;
    (*bl).upval = 0 as c_int as u8;
    (*bl).insidetbc = (!((*fs).bl).is_null() && (*(*fs).bl).insidetbc as c_int != 0) as c_int as u8;
    (*bl).previous = (*fs).bl;
    (*fs).bl = bl;
}

unsafe fn undefgoto(ls: *mut LexState, gt: *mut Labeldesc) -> ParseError {
    if (*gt).name == Str::from_str((*ls).g.deref(), "break") {
        luaK_semerror(
            ls,
            format_args!("break outside loop at line {}", (*gt).line),
        )
    } else {
        luaK_semerror(
            ls,
            format_args!(
                "no visible label '{}' for <goto> at line {}",
                String::from_utf8_lossy((*(*gt).name).as_bytes()),
                (*gt).line,
            ),
        )
    }
}

unsafe fn leaveblock(ls: *mut LexState, fs: *mut FuncState) -> Result<(), ParseError> {
    let bl: *mut BlockCnt = (*fs).bl;
    let mut hasclose: c_int = 0 as c_int;
    let stklevel: c_int = reglevel(ls, fs, (*bl).nactvar as c_int);
    removevars(ls, fs, (*bl).nactvar as c_int);
    if (*bl).isloop != 0 {
        hasclose = createlabel(
            ls,
            Str::from_str((*ls).g.deref(), "break"),
            0 as c_int,
            0 as c_int,
        )?;
    }
    if hasclose == 0 && !((*bl).previous).is_null() && (*bl).upval as c_int != 0 {
        luaK_codeABCk(ls, fs, OP_CLOSE, stklevel, 0 as c_int, 0 as c_int, 0)?;
    }
    (*fs).freereg = stklevel as u8;
    (*(*ls).dyd).label.n = (*bl).firstlabel;
    (*fs).bl = (*bl).previous;
    if !((*bl).previous).is_null() {
        movegotosout(ls, fs, bl);
    } else if (*bl).firstgoto < (*(*ls).dyd).gt.n {
        return Err(undefgoto(
            ls,
            &mut *((*(*ls).dyd).gt.arr).offset((*bl).firstgoto as isize),
        ));
    }
    Ok(())
}

unsafe fn addprototype(ls: *mut LexState) -> Result<*mut Proto, ParseError> {
    let mut clp: *mut Proto = 0 as *mut Proto;
    let g = (*ls).g.deref();
    let fs: *mut FuncState = (*ls).fs;
    let f: *mut Proto = (*fs).f;
    if (*fs).np >= (*f).sizep {
        let mut oldsize: c_int = (*f).sizep;
        (*f).p = luaM_growaux_(
            g,
            (*f).p as *mut libc::c_void,
            (*fs).np,
            &mut (*f).sizep,
            ::core::mem::size_of::<*mut Proto>() as libc::c_ulong as c_int,
            (if (((1 as c_int) << 8 as c_int + 8 as c_int + 1 as c_int) - 1 as c_int) as usize
                <= (!(0 as c_int as usize)).wrapping_div(::core::mem::size_of::<*mut Proto>())
            {
                (((1 as c_int) << 8 as c_int + 8 as c_int + 1 as c_int) - 1 as c_int)
                    as libc::c_uint
            } else {
                (!(0 as c_int as usize)).wrapping_div(::core::mem::size_of::<*mut Proto>())
                    as libc::c_uint
            }) as c_int,
            "functions",
            (*ls).linenumber,
        )? as *mut *mut Proto;
        while oldsize < (*f).sizep {
            let fresh12 = oldsize;
            oldsize = oldsize + 1;
            let ref mut fresh13 = *((*f).p).offset(fresh12 as isize);
            *fresh13 = 0 as *mut Proto;
        }
    }
    clp = luaF_newproto(g, ChunkInfo::default());
    let fresh14 = (*fs).np;
    (*fs).np = (*fs).np + 1;
    let ref mut fresh15 = *((*f).p).offset(fresh14 as isize);
    *fresh15 = clp;

    if (*f).hdr.marked.get() as c_int & (1 as c_int) << 5 as c_int != 0
        && (*clp).hdr.marked.get() as c_int
            & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
            != 0
    {
        g.gc.barrier(f as *mut Object, clp as *mut Object);
    }

    return Ok(clp);
}

unsafe fn codeclosure(ls: *mut LexState, v: *mut expdesc) -> Result<(), ParseError> {
    let fs: *mut FuncState = (*(*ls).fs).prev;
    init_exp(
        v,
        VRELOC,
        luaK_codeABx(
            ls,
            fs,
            OP_CLOSURE,
            0 as c_int,
            ((*fs).np - 1 as c_int) as libc::c_uint,
        )?,
    );
    luaK_exp2nextreg(ls, fs, v)
}

unsafe fn open_func(ls: *mut LexState, fs: *mut FuncState, bl: *mut BlockCnt) {
    let f: *mut Proto = (*fs).f;
    (*fs).prev = (*ls).fs;
    (*ls).fs = fs;
    (*fs).pc = 0 as c_int;
    (*fs).previousline = (*f).linedefined;
    (*fs).iwthabs = 0 as c_int as u8;
    (*fs).lasttarget = 0 as c_int;
    (*fs).freereg = 0 as c_int as u8;
    (*fs).nk = 0 as c_int;
    (*fs).nabslineinfo = 0 as c_int;
    (*fs).np = 0 as c_int;
    (*fs).nups = 0 as c_int as u8;
    (*fs).ndebugvars = 0 as c_int as libc::c_short;
    (*fs).nactvar = 0 as c_int as u8;
    (*fs).needclose = 0 as c_int as u8;
    (*fs).firstlocal = (*(*ls).dyd).actvar.n;
    (*fs).firstlabel = (*(*ls).dyd).label.n;
    (*fs).bl = 0 as *mut BlockCnt;
    (*f).chunk = (*ls).source.clone();
    (*f).maxstacksize = 2 as c_int as u8;
    enterblock(ls, fs, bl, 0 as c_int as u8);
}

unsafe fn close_func(ls: *mut LexState) -> Result<(), ParseError> {
    let fs: *mut FuncState = (*ls).fs;
    let f: *mut Proto = (*fs).f;
    luaK_ret(ls, fs, luaY_nvarstack(ls, fs), 0 as c_int)?;
    leaveblock(ls, fs)?;
    luaK_finish(ls, fs)?;
    (*f).code = luaM_shrinkvector_(
        (*ls).g.deref(),
        (*f).code as *mut libc::c_void,
        &mut (*f).sizecode,
        (*fs).pc,
        ::core::mem::size_of::<u32>() as libc::c_ulong as c_int,
    ) as *mut u32;
    (*f).lineinfo = luaM_shrinkvector_(
        (*ls).g.deref(),
        (*f).lineinfo as *mut libc::c_void,
        &mut (*f).sizelineinfo,
        (*fs).pc,
        ::core::mem::size_of::<i8>() as libc::c_ulong as c_int,
    ) as *mut i8;
    (*f).abslineinfo = luaM_shrinkvector_(
        (*ls).g.deref(),
        (*f).abslineinfo as *mut libc::c_void,
        &mut (*f).sizeabslineinfo,
        (*fs).nabslineinfo,
        ::core::mem::size_of::<AbsLineInfo>() as libc::c_ulong as c_int,
    ) as *mut AbsLineInfo;
    (*f).k = luaM_shrinkvector_(
        (*ls).g.deref(),
        (*f).k as *mut libc::c_void,
        &mut (*f).sizek,
        (*fs).nk,
        ::core::mem::size_of::<UnsafeValue>() as libc::c_ulong as c_int,
    ) as *mut UnsafeValue;
    (*f).p = luaM_shrinkvector_(
        (*ls).g.deref(),
        (*f).p as *mut libc::c_void,
        &mut (*f).sizep,
        (*fs).np,
        ::core::mem::size_of::<*mut Proto>() as libc::c_ulong as c_int,
    ) as *mut *mut Proto;
    (*f).locvars = luaM_shrinkvector_(
        (*ls).g.deref(),
        (*f).locvars as *mut libc::c_void,
        &mut (*f).sizelocvars,
        (*fs).ndebugvars as c_int,
        ::core::mem::size_of::<LocVar>() as libc::c_ulong as c_int,
    ) as *mut LocVar;
    (*f).upvalues = luaM_shrinkvector_(
        (*ls).g.deref(),
        (*f).upvalues as *mut libc::c_void,
        &mut (*f).sizeupvalues,
        (*fs).nups as c_int,
        ::core::mem::size_of::<Upvaldesc>() as libc::c_ulong as c_int,
    ) as *mut Upvaldesc;
    (*ls).fs = (*fs).prev;

    (*ls).g.gc.step();

    Ok(())
}

unsafe fn block_follow(ls: *mut LexState, withuntil: c_int) -> c_int {
    match (*ls).t.token {
        259 | 260 | 261 | 288 => return 1 as c_int,
        276 => return withuntil,
        _ => return 0 as c_int,
    };
}

unsafe fn statlist(ls: *mut LexState) -> Result<(), ParseError> {
    while block_follow(ls, 1 as c_int) == 0 {
        if (*ls).t.token == TK_RETURN as c_int {
            statement(ls)?;
            return Ok(());
        }
        statement(ls)?;
    }

    Ok(())
}

unsafe fn fieldsel(ls: *mut LexState, v: *mut expdesc) -> Result<(), ParseError> {
    let fs: *mut FuncState = (*ls).fs;
    let mut key: expdesc = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    luaK_exp2anyregup(ls, fs, v)?;
    luaX_next(ls)?;
    codename(ls, &mut key)?;
    luaK_indexed(ls, fs, v, &mut key)
}

unsafe fn yindex(ls: *mut LexState, v: *mut expdesc) -> Result<(), ParseError> {
    luaX_next(ls)?;
    expr(ls, v)?;
    luaK_exp2val(ls, (*ls).fs, v)?;
    checknext(ls, ']' as i32)?;
    Ok(())
}

unsafe fn recfield(ls: *mut LexState, cc: *mut ConsControl) -> Result<(), ParseError> {
    let fs: *mut FuncState = (*ls).fs;
    let reg: c_int = (*(*ls).fs).freereg as c_int;
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
    if (*ls).t.token == TK_NAME as c_int {
        codename(ls, &mut key)?;
    } else {
        yindex(ls, &mut key)?;
    }

    checklimit(ls, fs, (*cc).nh, 2147483647, "items in a constructor")?;

    (*cc).nh += 1;
    (*cc).nh;
    checknext(ls, '=' as i32)?;
    tab = *(*cc).t;
    luaK_indexed(ls, fs, &mut tab, &mut key)?;
    expr(ls, &mut val)?;
    luaK_storevar(ls, fs, &mut tab, &mut val)?;
    (*fs).freereg = reg as u8;
    Ok(())
}

unsafe fn closelistfield(
    ls: *mut LexState,
    fs: *mut FuncState,
    cc: *mut ConsControl,
) -> Result<(), ParseError> {
    if (*cc).v.k as libc::c_uint == VVOID as c_int as libc::c_uint {
        return Ok(());
    }
    luaK_exp2nextreg(ls, fs, &mut (*cc).v)?;
    (*cc).v.k = VVOID;
    if (*cc).tostore == 50 as c_int {
        luaK_setlist(ls, fs, (*(*cc).t).u.info, (*cc).na, (*cc).tostore)?;
        (*cc).na += (*cc).tostore;
        (*cc).tostore = 0 as c_int;
    }
    Ok(())
}

unsafe fn lastlistfield(
    ls: *mut LexState,
    fs: *mut FuncState,
    cc: *mut ConsControl,
) -> Result<(), ParseError> {
    if (*cc).tostore == 0 as c_int {
        return Ok(());
    }
    if (*cc).v.k as libc::c_uint == VCALL as c_int as libc::c_uint
        || (*cc).v.k as libc::c_uint == VVARARG as c_int as libc::c_uint
    {
        luaK_setreturns(ls, fs, &mut (*cc).v, -(1 as c_int))?;
        luaK_setlist(ls, fs, (*(*cc).t).u.info, (*cc).na, -(1 as c_int))?;
        (*cc).na -= 1;
        (*cc).na;
    } else {
        if (*cc).v.k as libc::c_uint != VVOID as c_int as libc::c_uint {
            luaK_exp2nextreg(ls, fs, &mut (*cc).v)?;
        }
        luaK_setlist(ls, fs, (*(*cc).t).u.info, (*cc).na, (*cc).tostore)?;
    }
    (*cc).na += (*cc).tostore;
    Ok(())
}

unsafe fn listfield(ls: *mut LexState, cc: *mut ConsControl) -> Result<(), ParseError> {
    expr(ls, &mut (*cc).v)?;
    (*cc).tostore += 1;
    (*cc).tostore;
    Ok(())
}

unsafe fn field(ls: *mut LexState, cc: *mut ConsControl) -> Result<(), ParseError> {
    match (*ls).t.token {
        291 => {
            if luaX_lookahead(ls)? != '=' as i32 {
                listfield(ls, cc)?;
            } else {
                recfield(ls, cc)?;
            }
        }
        91 => {
            recfield(ls, cc)?;
        }
        _ => {
            listfield(ls, cc)?;
        }
    };

    Ok(())
}

unsafe fn constructor(ls: *mut LexState, t: *mut expdesc) -> Result<(), ParseError> {
    let fs: *mut FuncState = (*ls).fs;
    let line: c_int = (*ls).linenumber;
    let pc: c_int = luaK_codeABCk(
        ls,
        fs,
        OP_NEWTABLE,
        0 as c_int,
        0 as c_int,
        0 as c_int,
        0 as c_int,
    )?;
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
    luaK_code(ls, fs, 0 as c_int as u32)?;
    cc.tostore = 0 as c_int;
    cc.nh = cc.tostore;
    cc.na = cc.nh;
    cc.t = t;
    init_exp(t, VNONRELOC, (*fs).freereg as c_int);
    luaK_reserveregs(ls, fs, 1 as c_int)?;
    init_exp(&mut cc.v, VVOID, 0 as c_int);
    checknext(ls, '{' as i32)?;
    while !((*ls).t.token == '}' as i32) {
        closelistfield(ls, fs, &mut cc)?;
        field(ls, &mut cc)?;
        if !(testnext(ls, ',' as i32)? != 0 || testnext(ls, ';' as i32)? != 0) {
            break;
        }
    }
    check_match(ls, '}' as i32, '{' as i32, line)?;
    lastlistfield(ls, fs, &mut cc)?;
    luaK_settablesize(fs, pc, (*t).u.info, cc.na, cc.nh);
    Ok(())
}

unsafe fn setvararg(
    ls: *mut LexState,
    fs: *mut FuncState,
    nparams: c_int,
) -> Result<(), ParseError> {
    (*(*fs).f).is_vararg = 1 as c_int as u8;
    luaK_codeABCk(
        ls,
        fs,
        OP_VARARGPREP,
        nparams,
        0 as c_int,
        0 as c_int,
        0 as c_int,
    )?;
    Ok(())
}

unsafe fn parlist(ls: *mut LexState) -> Result<(), ParseError> {
    let fs: *mut FuncState = (*ls).fs;
    let f: *mut Proto = (*fs).f;
    let mut nparams: c_int = 0 as c_int;
    let mut isvararg: c_int = 0 as c_int;
    if (*ls).t.token != ')' as i32 {
        loop {
            match (*ls).t.token {
                291 => {
                    new_localvar(ls, str_checkname(ls)?)?;
                    nparams += 1;
                }
                280 => {
                    luaX_next(ls)?;
                    isvararg = 1 as c_int;
                }
                _ => return Err(luaX_syntaxerror(ls, "<name> or '...' expected")),
            }
            if !(isvararg == 0 && testnext(ls, ',' as i32)? != 0) {
                break;
            }
        }
    }
    adjustlocalvars(ls, nparams)?;
    (*f).numparams = (*fs).nactvar;
    if isvararg != 0 {
        setvararg(ls, fs, (*f).numparams as c_int)?;
    }
    luaK_reserveregs(ls, fs, (*fs).nactvar as c_int)
}

unsafe fn body(
    ls: *mut LexState,
    e: *mut expdesc,
    ismethod: c_int,
    line: c_int,
) -> Result<(), ParseError> {
    let mut new_fs = FuncState::default();
    let mut bl: BlockCnt = BlockCnt {
        previous: 0 as *mut BlockCnt,
        firstlabel: 0,
        firstgoto: 0,
        nactvar: 0,
        upval: 0,
        isloop: 0,
        insidetbc: 0,
    };
    new_fs.f = addprototype(ls)?;
    (*new_fs.f).linedefined = line;
    open_func(ls, &mut new_fs, &mut bl);
    checknext(ls, '(' as i32)?;
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
        )?;
        adjustlocalvars(ls, 1 as c_int)?;
    }
    parlist(ls)?;
    checknext(ls, ')' as i32)?;
    statlist(ls)?;
    (*new_fs.f).lastlinedefined = (*ls).linenumber;
    check_match(ls, TK_END as c_int, TK_FUNCTION as c_int, line)?;
    codeclosure(ls, e)?;
    close_func(ls)?;
    Ok(())
}

unsafe fn explist(ls: *mut LexState, v: *mut expdesc) -> Result<c_int, ParseError> {
    let mut n: c_int = 1 as c_int;
    expr(ls, v)?;
    while testnext(ls, ',' as i32)? != 0 {
        luaK_exp2nextreg(ls, (*ls).fs, v)?;
        expr(ls, v)?;
        n += 1;
    }
    return Ok(n);
}

unsafe fn funcargs(ls: *mut LexState, f: *mut expdesc) -> Result<(), ParseError> {
    let fs: *mut FuncState = (*ls).fs;
    let mut args: expdesc = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    let mut base: c_int = 0;
    let mut nparams: c_int = 0;
    let line: c_int = (*ls).linenumber;
    match (*ls).t.token {
        40 => {
            luaX_next(ls)?;
            if (*ls).t.token == ')' as i32 {
                args.k = VVOID;
            } else {
                explist(ls, &mut args)?;
                if args.k as libc::c_uint == VCALL as c_int as libc::c_uint
                    || args.k as libc::c_uint == VVARARG as c_int as libc::c_uint
                {
                    luaK_setreturns(ls, fs, &mut args, -(1 as c_int))?;
                }
            }
            check_match(ls, ')' as i32, '(' as i32, line)?;
        }
        123 => constructor(ls, &mut args)?,
        292 => {
            codestring(&mut args, (*ls).t.seminfo.ts);
            luaX_next(ls)?;
        }
        _ => return Err(luaX_syntaxerror(ls, "function arguments expected")),
    }
    base = (*f).u.info;
    if args.k as libc::c_uint == VCALL as c_int as libc::c_uint
        || args.k as libc::c_uint == VVARARG as c_int as libc::c_uint
    {
        nparams = -(1 as c_int);
    } else {
        if args.k as libc::c_uint != VVOID as c_int as libc::c_uint {
            luaK_exp2nextreg(ls, fs, &mut args)?;
        }
        nparams = (*fs).freereg as c_int - (base + 1 as c_int);
    }
    init_exp(
        f,
        VCALL,
        luaK_codeABCk(
            ls,
            fs,
            OP_CALL,
            base,
            nparams + 1 as c_int,
            2 as c_int,
            0 as c_int,
        )?,
    );
    luaK_fixline(ls, fs, line)?;
    (*fs).freereg = (base + 1 as c_int) as u8;
    Ok(())
}

unsafe fn primaryexp(ls: *mut LexState, v: *mut expdesc) -> Result<(), ParseError> {
    match (*ls).t.token {
        40 => {
            let line: c_int = (*ls).linenumber;
            luaX_next(ls)?;
            expr(ls, v)?;
            check_match(ls, ')' as i32, '(' as i32, line)?;
            luaK_dischargevars(ls, (*ls).fs, v)?;
            Ok(())
        }
        291 => {
            singlevar(ls, v)?;
            Ok(())
        }
        _ => Err(luaX_syntaxerror(ls, "unexpected symbol")),
    }
}

unsafe fn suffixedexp(ls: *mut LexState, v: *mut expdesc) -> Result<(), ParseError> {
    let fs: *mut FuncState = (*ls).fs;
    primaryexp(ls, v)?;
    loop {
        match (*ls).t.token {
            46 => fieldsel(ls, v)?,
            91 => {
                let mut key: expdesc = expdesc {
                    k: VVOID,
                    u: C2RustUnnamed_11 { ival: 0 },
                    t: 0,
                    f: 0,
                };
                luaK_exp2anyregup(ls, fs, v)?;
                yindex(ls, &mut key)?;
                luaK_indexed(ls, fs, v, &mut key)?;
            }
            58 => {
                let mut key_0: expdesc = expdesc {
                    k: VVOID,
                    u: C2RustUnnamed_11 { ival: 0 },
                    t: 0,
                    f: 0,
                };
                luaX_next(ls)?;
                codename(ls, &mut key_0)?;
                luaK_self(ls, fs, v, &mut key_0)?;
                funcargs(ls, v)?;
            }
            40 | 292 | 123 => {
                luaK_exp2nextreg(ls, fs, v)?;
                funcargs(ls, v)?;
            }
            _ => return Ok(()),
        }
    }
}

unsafe fn simpleexp(ls: *mut LexState, v: *mut expdesc) -> Result<(), ParseError> {
    match (*ls).t.token as u32 {
        289 => {
            init_exp(v, VKFLT, 0 as c_int);
            (*v).u.nval = (*ls).t.seminfo.r;
        }
        290 => {
            init_exp(v, VKINT, 0 as c_int);
            (*v).u.ival = (*ls).t.seminfo.i;
        }
        292 => {
            codestring(v, (*ls).t.seminfo.ts);
        }
        TK_NIL => init_exp(v, VNIL, 0 as c_int),
        TK_TRUE => init_exp(v, VTRUE, 0 as c_int),
        TK_FALSE => init_exp(v, VFALSE, 0 as c_int),
        280 => {
            let fs: *mut FuncState = (*ls).fs;
            if (*(*fs).f).is_vararg == 0 {
                return Err(luaX_syntaxerror(
                    ls,
                    "cannot use '...' outside a vararg function",
                ));
            }
            init_exp(v, VVARARG, luaK_codeABCk(ls, fs, OP_VARARG, 0, 0, 1, 0)?);
        }
        123 => return constructor(ls, v),
        264 => {
            luaX_next(ls)?;
            body(ls, v, 0 as c_int, (*ls).linenumber)?;
            return Ok(());
        }
        _ => return suffixedexp(ls, v),
    }
    luaX_next(ls)
}

unsafe fn getunopr(op: c_int) -> UnOpr {
    match op as u32 {
        TK_NOT => return OPR_NOT,
        45 => return OPR_MINUS,
        126 => return OPR_BNOT,
        35 => return OPR_LEN,
        _ => return OPR_NOUNOPR,
    };
}

unsafe fn getbinopr(op: c_int) -> BinOpr {
    match op as u32 {
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
        TK_AND => return OPR_AND,
        TK_OR => return OPR_OR,
        _ => return OPR_NOBINOPR,
    };
}

static mut priority: [C2RustUnnamed_14; 21] = [
    {
        let init = C2RustUnnamed_14 {
            left: 10 as c_int as u8,
            right: 10 as c_int as u8,
        };
        init
    },
    {
        let init = C2RustUnnamed_14 {
            left: 10 as c_int as u8,
            right: 10 as c_int as u8,
        };
        init
    },
    {
        let init = C2RustUnnamed_14 {
            left: 11 as c_int as u8,
            right: 11 as c_int as u8,
        };
        init
    },
    {
        let init = C2RustUnnamed_14 {
            left: 11 as c_int as u8,
            right: 11 as c_int as u8,
        };
        init
    },
    {
        let init = C2RustUnnamed_14 {
            left: 14 as c_int as u8,
            right: 13 as c_int as u8,
        };
        init
    },
    {
        let init = C2RustUnnamed_14 {
            left: 11 as c_int as u8,
            right: 11 as c_int as u8,
        };
        init
    },
    {
        let init = C2RustUnnamed_14 {
            left: 11 as c_int as u8,
            right: 11 as c_int as u8,
        };
        init
    },
    {
        let init = C2RustUnnamed_14 {
            left: 6 as c_int as u8,
            right: 6 as c_int as u8,
        };
        init
    },
    {
        let init = C2RustUnnamed_14 {
            left: 4 as c_int as u8,
            right: 4 as c_int as u8,
        };
        init
    },
    {
        let init = C2RustUnnamed_14 {
            left: 5 as c_int as u8,
            right: 5 as c_int as u8,
        };
        init
    },
    {
        let init = C2RustUnnamed_14 {
            left: 7 as c_int as u8,
            right: 7 as c_int as u8,
        };
        init
    },
    {
        let init = C2RustUnnamed_14 {
            left: 7 as c_int as u8,
            right: 7 as c_int as u8,
        };
        init
    },
    {
        let init = C2RustUnnamed_14 {
            left: 9 as c_int as u8,
            right: 8 as c_int as u8,
        };
        init
    },
    {
        let init = C2RustUnnamed_14 {
            left: 3 as c_int as u8,
            right: 3 as c_int as u8,
        };
        init
    },
    {
        let init = C2RustUnnamed_14 {
            left: 3 as c_int as u8,
            right: 3 as c_int as u8,
        };
        init
    },
    {
        let init = C2RustUnnamed_14 {
            left: 3 as c_int as u8,
            right: 3 as c_int as u8,
        };
        init
    },
    {
        let init = C2RustUnnamed_14 {
            left: 3 as c_int as u8,
            right: 3 as c_int as u8,
        };
        init
    },
    {
        let init = C2RustUnnamed_14 {
            left: 3 as c_int as u8,
            right: 3 as c_int as u8,
        };
        init
    },
    {
        let init = C2RustUnnamed_14 {
            left: 3 as c_int as u8,
            right: 3 as c_int as u8,
        };
        init
    },
    {
        let init = C2RustUnnamed_14 {
            left: 2 as c_int as u8,
            right: 2 as c_int as u8,
        };
        init
    },
    {
        let init = C2RustUnnamed_14 {
            left: 1 as c_int as u8,
            right: 1 as c_int as u8,
        };
        init
    },
];

unsafe fn subexpr(ls: *mut LexState, v: *mut expdesc, limit: c_int) -> Result<BinOpr, ParseError> {
    // Check level.
    (*ls).level += 1;

    if (*ls).level >= 200 {
        return Err(ParseError::ItemLimit {
            name: "nested level",
            limit: 200,
            line: (*ls).linenumber,
        });
    }

    let mut op: BinOpr = OPR_ADD;
    let mut uop: UnOpr = OPR_MINUS;

    uop = getunopr((*ls).t.token);

    if uop as libc::c_uint != OPR_NOUNOPR as c_int as libc::c_uint {
        let line: c_int = (*ls).linenumber;
        luaX_next(ls)?;
        subexpr(ls, v, 12 as c_int)?;
        luaK_prefix(ls, (*ls).fs, uop, v, line)?;
    } else {
        simpleexp(ls, v)?;
    }
    op = getbinopr((*ls).t.token);
    while op as libc::c_uint != OPR_NOBINOPR as c_int as libc::c_uint
        && priority[op as usize].left as c_int > limit
    {
        let mut v2: expdesc = expdesc {
            k: VVOID,
            u: C2RustUnnamed_11 { ival: 0 },
            t: 0,
            f: 0,
        };
        let mut nextop: BinOpr = OPR_ADD;
        let line_0: c_int = (*ls).linenumber;
        luaX_next(ls)?;
        luaK_infix(ls, (*ls).fs, op, v)?;
        nextop = subexpr(ls, &mut v2, priority[op as usize].right as c_int)?;
        luaK_posfix(ls, (*ls).fs, op, v, &mut v2, line_0)?;
        op = nextop;
    }

    (*ls).level -= 1;

    return Ok(op);
}

unsafe fn expr(ls: *mut LexState, v: *mut expdesc) -> Result<(), ParseError> {
    subexpr(ls, v, 0 as c_int)?;
    Ok(())
}

unsafe fn block(ls: *mut LexState) -> Result<(), ParseError> {
    let fs: *mut FuncState = (*ls).fs;
    let mut bl: BlockCnt = BlockCnt {
        previous: 0 as *mut BlockCnt,
        firstlabel: 0,
        firstgoto: 0,
        nactvar: 0,
        upval: 0,
        isloop: 0,
        insidetbc: 0,
    };
    enterblock(ls, fs, &mut bl, 0 as c_int as u8);
    statlist(ls)?;
    leaveblock(ls, fs)
}

unsafe fn check_conflict(
    ls: *mut LexState,
    mut lh: *mut LHS_assign,
    v: *mut expdesc,
) -> Result<(), ParseError> {
    let fs: *mut FuncState = (*ls).fs;
    let extra: c_int = (*fs).freereg as c_int;
    let mut conflict: c_int = 0 as c_int;
    while !lh.is_null() {
        if VINDEXED as c_int as libc::c_uint <= (*lh).v.k as libc::c_uint
            && (*lh).v.k as libc::c_uint <= VINDEXSTR as c_int as libc::c_uint
        {
            if (*lh).v.k as libc::c_uint == VINDEXUP as c_int as libc::c_uint {
                if (*v).k as libc::c_uint == VUPVAL as c_int as libc::c_uint
                    && (*lh).v.u.ind.t as c_int == (*v).u.info
                {
                    conflict = 1 as c_int;
                    (*lh).v.k = VINDEXSTR;
                    (*lh).v.u.ind.t = extra as u8;
                }
            } else {
                if (*v).k as libc::c_uint == VLOCAL as c_int as libc::c_uint
                    && (*lh).v.u.ind.t as c_int == (*v).u.var.ridx as c_int
                {
                    conflict = 1 as c_int;
                    (*lh).v.u.ind.t = extra as u8;
                }
                if (*lh).v.k as libc::c_uint == VINDEXED as c_int as libc::c_uint
                    && (*v).k as libc::c_uint == VLOCAL as c_int as libc::c_uint
                    && (*lh).v.u.ind.idx as c_int == (*v).u.var.ridx as c_int
                {
                    conflict = 1 as c_int;
                    (*lh).v.u.ind.idx = extra as libc::c_short;
                }
            }
        }
        lh = (*lh).prev;
    }
    if conflict != 0 {
        if (*v).k as libc::c_uint == VLOCAL as c_int as libc::c_uint {
            luaK_codeABCk(
                ls,
                fs,
                OP_MOVE,
                extra,
                (*v).u.var.ridx as c_int,
                0 as c_int,
                0 as c_int,
            )?;
        } else {
            luaK_codeABCk(ls, fs, OP_GETUPVAL, extra, (*v).u.info, 0 as c_int, 0)?;
        }
        luaK_reserveregs(ls, fs, 1 as c_int)?;
    }
    Ok(())
}

unsafe fn restassign(
    ls: *mut LexState,
    lh: *mut LHS_assign,
    nvars: c_int,
) -> Result<(), ParseError> {
    let mut e: expdesc = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    if !(VLOCAL as c_int as libc::c_uint <= (*lh).v.k as libc::c_uint
        && (*lh).v.k as libc::c_uint <= VINDEXSTR as c_int as libc::c_uint)
    {
        return Err(luaX_syntaxerror(ls, "syntax error"));
    }
    check_readonly(ls, &mut (*lh).v)?;
    if testnext(ls, ',' as i32)? != 0 {
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
        suffixedexp(ls, &mut nv.v)?;
        if !(VINDEXED as c_int as libc::c_uint <= nv.v.k as libc::c_uint
            && nv.v.k as libc::c_uint <= VINDEXSTR as c_int as libc::c_uint)
        {
            check_conflict(ls, lh, &mut nv.v)?;
        }

        (*ls).level += 1;

        if (*ls).level >= 200 {
            return Err(ParseError::ItemLimit {
                name: "nested level",
                limit: 200,
                line: (*ls).linenumber,
            });
        }

        restassign(ls, &mut nv, nvars + 1 as c_int)?;

        (*ls).level -= 1;
    } else {
        let mut nexps: c_int = 0;
        checknext(ls, '=' as i32)?;
        nexps = explist(ls, &mut e)?;
        if nexps != nvars {
            adjust_assign(ls, nvars, nexps, &mut e)?;
        } else {
            luaK_setoneret((*ls).fs, &mut e);
            luaK_storevar(ls, (*ls).fs, &mut (*lh).v, &mut e)?;
            return Ok(());
        }
    }
    init_exp(&mut e, VNONRELOC, (*(*ls).fs).freereg as c_int - 1 as c_int);

    luaK_storevar(ls, (*ls).fs, &mut (*lh).v, &mut e)
}

unsafe fn cond(ls: *mut LexState) -> Result<c_int, ParseError> {
    let mut v: expdesc = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    expr(ls, &mut v)?;
    if v.k as libc::c_uint == VNIL as c_int as libc::c_uint {
        v.k = VFALSE;
    }
    luaK_goiftrue(ls, (*ls).fs, &mut v)?;
    return Ok(v.f);
}

unsafe fn gotostat(ls: *mut LexState) -> Result<(), ParseError> {
    let fs: *mut FuncState = (*ls).fs;
    let line: c_int = (*ls).linenumber;
    let name = str_checkname(ls)?;
    let lb: *mut Labeldesc = findlabel(ls, name);
    if lb.is_null() {
        newgotoentry(ls, name, line, luaK_jump(ls, fs)?)?;
    } else {
        let lblevel: c_int = reglevel(ls, fs, (*lb).nactvar as c_int);
        if luaY_nvarstack(ls, fs) > lblevel {
            luaK_codeABCk(ls, fs, OP_CLOSE, lblevel, 0 as c_int, 0 as c_int, 0)?;
        }
        luaK_patchlist(ls, fs, luaK_jump(ls, fs)?, (*lb).pc)?;
    };
    Ok(())
}

unsafe fn breakstat(ls: *mut LexState) -> Result<(), ParseError> {
    let line: c_int = (*ls).linenumber;
    luaX_next(ls)?;
    newgotoentry(
        ls,
        Str::from_str((*ls).g.deref(), "break"),
        line,
        luaK_jump(ls, (*ls).fs)?,
    )?;
    Ok(())
}

unsafe fn checkrepeated(ls: *mut LexState, name: *const Str) -> Result<(), ParseError> {
    let lb: *mut Labeldesc = findlabel(ls, name);
    if ((lb != 0 as *mut libc::c_void as *mut Labeldesc) as c_int != 0 as c_int) as c_int
        as libc::c_long
        != 0
    {
        return Err(luaK_semerror(
            ls,
            format_args!(
                "label '{}' already defined on line {}",
                String::from_utf8_lossy((*name).as_bytes()),
                (*lb).line
            ),
        ));
    }
    Ok(())
}

unsafe fn labelstat(ls: *mut LexState, name: *const Str, line: c_int) -> Result<(), ParseError> {
    checknext(ls, TK_DBCOLON as c_int)?;
    while (*ls).t.token == ';' as i32 || (*ls).t.token == TK_DBCOLON as c_int {
        statement(ls)?;
    }
    checkrepeated(ls, name)?;
    createlabel(ls, name, line, block_follow(ls, 0 as c_int))?;
    Ok(())
}

unsafe fn whilestat(ls: *mut LexState, line: c_int) -> Result<(), ParseError> {
    let fs: *mut FuncState = (*ls).fs;
    let mut whileinit: c_int = 0;
    let mut condexit: c_int = 0;
    let mut bl: BlockCnt = BlockCnt {
        previous: 0 as *mut BlockCnt,
        firstlabel: 0,
        firstgoto: 0,
        nactvar: 0,
        upval: 0,
        isloop: 0,
        insidetbc: 0,
    };
    luaX_next(ls)?;
    whileinit = luaK_getlabel(fs);
    condexit = cond(ls)?;
    enterblock(ls, fs, &mut bl, 1 as c_int as u8);
    checknext(ls, TK_DO as c_int)?;
    block(ls)?;
    luaK_patchlist(ls, fs, luaK_jump(ls, fs)?, whileinit)?;
    check_match(ls, TK_END as c_int, TK_WHILE as c_int, line)?;
    leaveblock(ls, fs)?;
    luaK_patchtohere(ls, fs, condexit)?;
    Ok(())
}

unsafe fn repeatstat(ls: *mut LexState, line: c_int) -> Result<(), ParseError> {
    let mut condexit: c_int = 0;
    let fs: *mut FuncState = (*ls).fs;
    let repeat_init: c_int = luaK_getlabel(fs);
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
    enterblock(ls, fs, &mut bl1, 1 as c_int as u8);
    enterblock(ls, fs, &mut bl2, 0 as c_int as u8);
    luaX_next(ls)?;
    statlist(ls)?;
    check_match(ls, TK_UNTIL as c_int, TK_REPEAT as c_int, line)?;
    condexit = cond(ls)?;
    leaveblock(ls, fs)?;
    if bl2.upval != 0 {
        let exit: c_int = luaK_jump(ls, fs)?;
        luaK_patchtohere(ls, fs, condexit)?;
        luaK_codeABCk(
            ls,
            fs,
            OP_CLOSE,
            reglevel(ls, fs, bl2.nactvar as c_int),
            0 as c_int,
            0 as c_int,
            0 as c_int,
        )?;
        condexit = luaK_jump(ls, fs)?;
        luaK_patchtohere(ls, fs, exit)?;
    }
    luaK_patchlist(ls, fs, condexit, repeat_init)?;
    leaveblock(ls, fs)?;
    Ok(())
}

unsafe fn exp1(ls: *mut LexState) -> Result<(), ParseError> {
    let mut e: expdesc = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };

    expr(ls, &mut e)?;
    luaK_exp2nextreg(ls, (*ls).fs, &mut e)
}

unsafe fn fixforjump(
    ls: *mut LexState,
    fs: *mut FuncState,
    pc: c_int,
    dest: c_int,
    back: c_int,
) -> Result<(), ParseError> {
    let jmp: *mut u32 = &mut *((*(*fs).f).code).offset(pc as isize) as *mut u32;
    let mut offset: c_int = dest - (pc + 1 as c_int);
    if back != 0 {
        offset = -offset;
    }
    if ((offset > ((1 as c_int) << 8 as c_int + 8 as c_int + 1 as c_int) - 1 as c_int) as c_int
        != 0 as c_int) as c_int as libc::c_long
        != 0
    {
        return Err(luaX_syntaxerror(ls, "control structure too long"));
    }
    *jmp = *jmp
        & !(!(!(0 as c_int as u32) << 8 as c_int + 8 as c_int + 1 as c_int)
            << 0 as c_int + 7 as c_int + 8 as c_int)
        | (offset as u32) << 0 as c_int + 7 as c_int + 8 as c_int
            & !(!(0 as c_int as u32) << 8 as c_int + 8 as c_int + 1 as c_int)
                << 0 as c_int + 7 as c_int + 8 as c_int;
    Ok(())
}

unsafe fn forbody(
    ls: *mut LexState,
    base: c_int,
    line: c_int,
    nvars: c_int,
    isgen: c_int,
) -> Result<(), ParseError> {
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
    let fs: *mut FuncState = (*ls).fs;
    let mut prep: c_int = 0;
    let mut endfor: c_int = 0;
    checknext(ls, TK_DO as c_int)?;
    prep = luaK_codeABx(
        ls,
        fs,
        forprep[isgen as usize],
        base,
        0 as c_int as libc::c_uint,
    )?;
    enterblock(ls, fs, &mut bl, 0 as c_int as u8);
    adjustlocalvars(ls, nvars)?;
    luaK_reserveregs(ls, fs, nvars)?;
    block(ls)?;
    leaveblock(ls, fs)?;
    fixforjump(ls, fs, prep, luaK_getlabel(fs), 0 as c_int)?;
    if isgen != 0 {
        luaK_codeABCk(ls, fs, OP_TFORCALL, base, 0 as c_int, nvars, 0 as c_int)?;
        luaK_fixline(ls, fs, line)?;
    }
    endfor = luaK_codeABx(
        ls,
        fs,
        forloop[isgen as usize],
        base,
        0 as c_int as libc::c_uint,
    )?;
    fixforjump(ls, fs, endfor, prep + 1 as c_int, 1 as c_int)?;
    luaK_fixline(ls, fs, line)
}

unsafe fn fornum(ls: *mut LexState, varname: *const Str, line: c_int) -> Result<(), ParseError> {
    let fs: *mut FuncState = (*ls).fs;
    let base: c_int = (*fs).freereg as c_int;
    new_localvar(
        ls,
        luaX_newstring(
            ls,
            b"(for state)\0" as *const u8 as *const libc::c_char,
            ::core::mem::size_of::<[libc::c_char; 12]>()
                .wrapping_div(::core::mem::size_of::<libc::c_char>())
                .wrapping_sub(1),
        ),
    )?;
    new_localvar(
        ls,
        luaX_newstring(
            ls,
            b"(for state)\0" as *const u8 as *const libc::c_char,
            ::core::mem::size_of::<[libc::c_char; 12]>()
                .wrapping_div(::core::mem::size_of::<libc::c_char>())
                .wrapping_sub(1),
        ),
    )?;
    new_localvar(
        ls,
        luaX_newstring(
            ls,
            b"(for state)\0" as *const u8 as *const libc::c_char,
            ::core::mem::size_of::<[libc::c_char; 12]>()
                .wrapping_div(::core::mem::size_of::<libc::c_char>())
                .wrapping_sub(1),
        ),
    )?;
    new_localvar(ls, varname)?;
    checknext(ls, '=' as i32)?;
    exp1(ls)?;
    checknext(ls, ',' as i32)?;
    exp1(ls)?;
    if testnext(ls, ',' as i32)? != 0 {
        exp1(ls)?;
    } else {
        luaK_int(ls, fs, (*fs).freereg as c_int, 1 as c_int as i64)?;
        luaK_reserveregs(ls, fs, 1 as c_int)?;
    }
    adjustlocalvars(ls, 3 as c_int)?;
    forbody(ls, base, line, 1 as c_int, 0 as c_int)?;

    Ok(())
}

unsafe fn forlist(ls: *mut LexState, indexname: *const Str) -> Result<(), ParseError> {
    let fs: *mut FuncState = (*ls).fs;
    let mut e: expdesc = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    let mut nvars: c_int = 5 as c_int;
    let mut line: c_int = 0;
    let base: c_int = (*fs).freereg as c_int;
    new_localvar(
        ls,
        luaX_newstring(
            ls,
            b"(for state)\0" as *const u8 as *const libc::c_char,
            ::core::mem::size_of::<[libc::c_char; 12]>()
                .wrapping_div(::core::mem::size_of::<libc::c_char>())
                .wrapping_sub(1),
        ),
    )?;
    new_localvar(
        ls,
        luaX_newstring(
            ls,
            b"(for state)\0" as *const u8 as *const libc::c_char,
            ::core::mem::size_of::<[libc::c_char; 12]>()
                .wrapping_div(::core::mem::size_of::<libc::c_char>())
                .wrapping_sub(1),
        ),
    )?;
    new_localvar(
        ls,
        luaX_newstring(
            ls,
            b"(for state)\0" as *const u8 as *const libc::c_char,
            ::core::mem::size_of::<[libc::c_char; 12]>()
                .wrapping_div(::core::mem::size_of::<libc::c_char>())
                .wrapping_sub(1),
        ),
    )?;
    new_localvar(
        ls,
        luaX_newstring(
            ls,
            b"(for state)\0" as *const u8 as *const libc::c_char,
            ::core::mem::size_of::<[libc::c_char; 12]>()
                .wrapping_div(::core::mem::size_of::<libc::c_char>())
                .wrapping_sub(1),
        ),
    )?;
    new_localvar(ls, indexname)?;
    while testnext(ls, ',' as i32)? != 0 {
        new_localvar(ls, str_checkname(ls)?)?;
        nvars += 1;
    }
    checknext(ls, TK_IN as c_int)?;
    line = (*ls).linenumber;
    adjust_assign(ls, 4 as c_int, explist(ls, &mut e)?, &mut e)?;
    adjustlocalvars(ls, 4 as c_int)?;
    marktobeclosed(fs);
    luaK_checkstack(ls, fs, 3 as c_int)?;
    forbody(ls, base, line, nvars - 4 as c_int, 1 as c_int)?;
    Ok(())
}

unsafe fn forstat(ls: *mut LexState, line: c_int) -> Result<(), ParseError> {
    let fs: *mut FuncState = (*ls).fs;
    let mut bl: BlockCnt = BlockCnt {
        previous: 0 as *mut BlockCnt,
        firstlabel: 0,
        firstgoto: 0,
        nactvar: 0,
        upval: 0,
        isloop: 0,
        insidetbc: 0,
    };
    enterblock(ls, fs, &mut bl, 1 as c_int as u8);
    luaX_next(ls)?;
    let varname = str_checkname(ls)?;
    match (*ls).t.token {
        61 => fornum(ls, varname, line)?,
        44 | 267 => forlist(ls, varname)?,
        _ => return Err(luaX_syntaxerror(ls, "'=' or 'in' expected")),
    }
    check_match(ls, TK_END as c_int, TK_FOR as c_int, line)?;
    leaveblock(ls, fs)?;
    Ok(())
}

unsafe fn test_then_block(ls: *mut LexState, escapelist: *mut c_int) -> Result<(), ParseError> {
    let mut bl: BlockCnt = BlockCnt {
        previous: 0 as *mut BlockCnt,
        firstlabel: 0,
        firstgoto: 0,
        nactvar: 0,
        upval: 0,
        isloop: 0,
        insidetbc: 0,
    };
    let fs: *mut FuncState = (*ls).fs;
    let mut v: expdesc = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    let mut jf: c_int = 0;
    luaX_next(ls)?;
    expr(ls, &mut v)?;
    checknext(ls, TK_THEN as c_int)?;
    if (*ls).t.token == TK_BREAK as c_int {
        let line: c_int = (*ls).linenumber;
        luaK_goiffalse(ls, (*ls).fs, &mut v)?;
        luaX_next(ls)?;
        enterblock(ls, fs, &mut bl, 0 as c_int as u8);
        newgotoentry(ls, Str::from_str((*ls).g.deref(), "break"), line, v.t)?;
        while testnext(ls, ';' as i32)? != 0 {}
        if block_follow(ls, 0 as c_int) != 0 {
            leaveblock(ls, fs)?;
            return Ok(());
        } else {
            jf = luaK_jump(ls, fs)?;
        }
    } else {
        luaK_goiftrue(ls, (*ls).fs, &mut v)?;
        enterblock(ls, fs, &mut bl, 0 as c_int as u8);
        jf = v.f;
    }
    statlist(ls)?;
    leaveblock(ls, fs)?;
    if (*ls).t.token == TK_ELSE as c_int || (*ls).t.token == TK_ELSEIF as c_int {
        luaK_concat(ls, fs, escapelist, luaK_jump(ls, fs)?)?;
    }
    luaK_patchtohere(ls, fs, jf)
}

unsafe fn ifstat(ls: *mut LexState, line: c_int) -> Result<(), ParseError> {
    let fs: *mut FuncState = (*ls).fs;
    let mut escapelist: c_int = -(1 as c_int);
    test_then_block(ls, &mut escapelist)?;
    while (*ls).t.token == TK_ELSEIF as c_int {
        test_then_block(ls, &mut escapelist)?;
    }
    if testnext(ls, TK_ELSE as c_int)? != 0 {
        block(ls)?;
    }
    check_match(ls, TK_END as c_int, TK_IF as c_int, line)?;
    luaK_patchtohere(ls, fs, escapelist)
}

unsafe fn localfunc(ls: *mut LexState) -> Result<(), ParseError> {
    let mut b: expdesc = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    let fs: *mut FuncState = (*ls).fs;
    let fvar: c_int = (*fs).nactvar as c_int;
    new_localvar(ls, str_checkname(ls)?)?;
    adjustlocalvars(ls, 1 as c_int)?;
    body(ls, &mut b, 0 as c_int, (*ls).linenumber)?;
    (*localdebuginfo(ls, fs, fvar)).startpc = (*fs).pc;
    Ok(())
}

unsafe fn getlocalattribute(ls: *mut LexState) -> Result<c_int, ParseError> {
    if testnext(ls, '<' as i32)? != 0 {
        let attr = (*str_checkname(ls)?).as_bytes();

        checknext(ls, '>' as i32)?;

        if attr == b"const" {
            return Ok(1 as c_int);
        } else if attr == b"close" {
            return Ok(2 as c_int);
        } else {
            return Err(luaK_semerror(
                ls,
                format_args!("unknown attribute '{}'", String::from_utf8_lossy(attr)),
            ));
        }
    }
    return Ok(0 as c_int);
}

unsafe fn checktoclose(
    ls: *mut LexState,
    fs: *mut FuncState,
    level: c_int,
) -> Result<(), ParseError> {
    if level != -(1 as c_int) {
        marktobeclosed(fs);
        luaK_codeABCk(
            ls,
            fs,
            OP_TBC,
            reglevel(ls, fs, level),
            0 as c_int,
            0 as c_int,
            0 as c_int,
        )?;
    }
    Ok(())
}

unsafe fn localstat(ls: *mut LexState) -> Result<(), ParseError> {
    let fs: *mut FuncState = (*ls).fs;
    let mut toclose: c_int = -(1 as c_int);
    let mut var: *mut Vardesc = 0 as *mut Vardesc;
    let mut vidx: c_int = 0;
    let mut kind: c_int = 0;
    let mut nvars: c_int = 0 as c_int;
    let mut nexps: c_int = 0;
    let mut e: expdesc = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    loop {
        vidx = new_localvar(ls, str_checkname(ls)?)?;
        kind = getlocalattribute(ls)?;
        (*getlocalvardesc(ls, fs, vidx)).vd.kind = kind as u8;
        if kind == 2 as c_int {
            if toclose != -(1 as c_int) {
                return Err(luaK_semerror(
                    ls,
                    "multiple to-be-closed variables in local list\0",
                ));
            }
            toclose = (*fs).nactvar as c_int + nvars;
        }
        nvars += 1;
        if !(testnext(ls, ',' as i32)? != 0) {
            break;
        }
    }
    if testnext(ls, '=' as i32)? != 0 {
        nexps = explist(ls, &mut e)?;
    } else {
        e.k = VVOID;
        nexps = 0 as c_int;
    }
    var = getlocalvardesc(ls, fs, vidx);
    if nvars == nexps
        && (*var).vd.kind as c_int == 1 as c_int
        && luaK_exp2const(ls, &mut e, &mut (*var).k) != 0
    {
        (*var).vd.kind = 3 as c_int as u8;
        adjustlocalvars(ls, nvars - 1 as c_int)?;
        (*fs).nactvar = ((*fs).nactvar).wrapping_add(1);
        (*fs).nactvar;
    } else {
        adjust_assign(ls, nvars, nexps, &mut e)?;
        adjustlocalvars(ls, nvars)?;
    }
    checktoclose(ls, fs, toclose)?;
    Ok(())
}

unsafe fn funcname(ls: *mut LexState, v: *mut expdesc) -> Result<c_int, ParseError> {
    let mut ismethod: c_int = 0 as c_int;
    singlevar(ls, v)?;
    while (*ls).t.token == '.' as i32 {
        fieldsel(ls, v)?;
    }
    if (*ls).t.token == ':' as i32 {
        ismethod = 1 as c_int;
        fieldsel(ls, v)?;
    }
    return Ok(ismethod);
}

unsafe fn funcstat(ls: *mut LexState, line: c_int) -> Result<(), ParseError> {
    let mut ismethod: c_int = 0;
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
    luaX_next(ls)?;
    ismethod = funcname(ls, &mut v)?;
    body(ls, &mut b, ismethod, line)?;
    check_readonly(ls, &mut v)?;
    luaK_storevar(ls, (*ls).fs, &mut v, &mut b)?;
    luaK_fixline(ls, (*ls).fs, line)
}

unsafe fn exprstat(ls: *mut LexState) -> Result<(), ParseError> {
    let fs: *mut FuncState = (*ls).fs;
    let mut v: LHS_assign = LHS_assign {
        prev: 0 as *mut LHS_assign,
        v: expdesc {
            k: VVOID,
            u: C2RustUnnamed_11 { ival: 0 },
            t: 0,
            f: 0,
        },
    };
    suffixedexp(ls, &mut v.v)?;
    if (*ls).t.token == '=' as i32 || (*ls).t.token == ',' as i32 {
        v.prev = 0 as *mut LHS_assign;
        restassign(ls, &mut v, 1 as c_int)?;
    } else {
        let mut inst: *mut u32 = 0 as *mut u32;
        if !(v.v.k as libc::c_uint == VCALL as c_int as libc::c_uint) {
            return Err(luaX_syntaxerror(ls, "syntax error"));
        }
        inst = &mut *((*(*fs).f).code).offset(v.v.u.info as isize) as *mut u32;
        *inst = *inst
            & !(!(!(0 as c_int as u32) << 8 as c_int)
                << 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int)
            | (1 as c_int as u32) << 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int
                & !(!(0 as c_int as u32) << 8 as c_int)
                    << 0 as c_int + 7 as c_int + 8 as c_int + 1 as c_int + 8 as c_int;
    };
    Ok(())
}

unsafe fn retstat(ls: *mut LexState) -> Result<(), ParseError> {
    let fs: *mut FuncState = (*ls).fs;
    let mut e: expdesc = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    let mut nret: c_int = 0;
    let mut first: c_int = luaY_nvarstack(ls, fs);
    if block_follow(ls, 1 as c_int) != 0 || (*ls).t.token == ';' as i32 {
        nret = 0 as c_int;
    } else {
        nret = explist(ls, &mut e)?;
        if e.k as libc::c_uint == VCALL as c_int as libc::c_uint
            || e.k as libc::c_uint == VVARARG as c_int as libc::c_uint
        {
            luaK_setreturns(ls, fs, &mut e, -(1 as c_int))?;
            if e.k as libc::c_uint == VCALL as c_int as libc::c_uint
                && nret == 1 as c_int
                && (*(*fs).bl).insidetbc == 0
            {
                *((*(*fs).f).code).offset(e.u.info as isize) = *((*(*fs).f).code)
                    .offset(e.u.info as isize)
                    & !(!(!(0 as c_int as u32) << 7 as c_int) << 0 as c_int)
                    | (OP_TAILCALL as c_int as u32) << 0 as c_int
                        & !(!(0 as c_int as u32) << 7 as c_int) << 0 as c_int;
            }
            nret = -(1 as c_int);
        } else if nret == 1 as c_int {
            first = luaK_exp2anyreg(ls, fs, &mut e)?;
        } else {
            luaK_exp2nextreg(ls, fs, &mut e)?;
        }
    }
    luaK_ret(ls, fs, first, nret)?;
    testnext(ls, ';' as i32)?;
    Ok(())
}

unsafe fn statement(ls: *mut LexState) -> Result<(), ParseError> {
    let line: c_int = (*ls).linenumber;

    (*ls).level += 1;

    if (*ls).level >= 200 {
        return Err(ParseError::ItemLimit {
            name: "nested level",
            limit: 200,
            line: (*ls).linenumber,
        });
    }

    match (*ls).t.token as u32 {
        59 => luaX_next(ls)?,
        266 => ifstat(ls, line)?,
        277 => whilestat(ls, line)?,
        258 => {
            luaX_next(ls)?;
            block(ls)?;
            check_match(ls, TK_END as c_int, TK_DO as c_int, line)?;
        }
        263 => forstat(ls, line)?,
        272 => repeatstat(ls, line)?,
        264 => funcstat(ls, line)?,
        TK_LOCAL => {
            luaX_next(ls)?;
            if testnext(ls, TK_FUNCTION as c_int)? != 0 {
                localfunc(ls)?;
            } else {
                localstat(ls)?;
            }
        }
        287 => {
            luaX_next(ls)?;
            labelstat(ls, str_checkname(ls)?, line)?;
        }
        273 => {
            luaX_next(ls)?;
            retstat(ls)?;
        }
        257 => breakstat(ls)?,
        TK_GOTO => {
            luaX_next(ls)?;
            gotostat(ls)?;
        }
        _ => {
            exprstat(ls)?;
        }
    }

    (*(*ls).fs).freereg = luaY_nvarstack(ls, (*ls).fs) as u8;
    (*ls).level -= 1;

    Ok(())
}

unsafe fn mainfunc(ls: &mut LexState, fs: &mut FuncState) -> Result<(), ParseError> {
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
    setvararg(ls, fs, 0 as c_int)?;
    env = allocupvalue(ls, fs)?;
    (*env).instack = 1 as c_int as u8;
    (*env).idx = 0 as c_int as u8;
    (*env).kind = 0 as c_int as u8;
    (*env).name = (*ls).envn;

    if (*(*fs).f).hdr.marked.get() as c_int & (1 as c_int) << 5 as c_int != 0
        && (*(*env).name).hdr.marked.get() as c_int
            & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
            != 0
    {
        (*ls)
            .g
            .gc
            .barrier((*fs).f as *mut Object, (*env).name as *mut Object);
    }

    luaX_next(ls)?;
    statlist(ls)?;
    check(ls, TK_EOS as c_int)?;
    close_func(ls)?;
    Ok(())
}

pub unsafe fn luaY_parser(
    g: &Lua,
    z: *mut ZIO,
    buff: *mut Mbuffer,
    dyd: *mut Dyndata,
    info: ChunkInfo,
    firstchar: c_int,
) -> Result<Ref<LuaFn>, ParseError> {
    let mut funcstate = FuncState::default();
    let cl = Ref::new(luaF_newLclosure(g, 1));
    let mut lexstate = LexState {
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
        g: g.to_rc(),
        z: 0 as *mut ZIO,
        buff: 0 as *mut Mbuffer,
        h: Ref::new(Table::new(g)),
        dyd: 0 as *mut Dyndata,
        source: info.clone(),
        envn: 0 as *mut Str,
        level: 0,
    };

    (*cl).p.set(luaF_newproto(g, info));
    funcstate.f = (*cl).p.get();

    if (*cl).hdr.marked.get() as c_int & (1 as c_int) << 5 as c_int != 0
        && (*(*cl).p.get()).hdr.marked.get() as c_int
            & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
            != 0
    {
        g.gc.barrier(&cl.hdr, (*cl).p.get().cast());
    }

    lexstate.buff = buff;
    lexstate.dyd = dyd;
    (*dyd).label.n = 0 as c_int;
    (*dyd).gt.n = (*dyd).label.n;
    (*dyd).actvar.n = (*dyd).gt.n;
    luaX_setinput(&mut lexstate, z, firstchar);
    mainfunc(&mut lexstate, &mut funcstate)?;

    return Ok(cl);
}
