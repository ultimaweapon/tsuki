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
use crate::lzio::ZIO;
use crate::value::{UnsafeValue, UntaggedValue};
use crate::vm::{
    OP_CALL, OP_CLOSE, OP_CLOSURE, OP_FORLOOP, OP_FORPREP, OP_GETUPVAL, OP_MOVE, OP_NEWTABLE,
    OP_TAILCALL, OP_TBC, OP_TFORCALL, OP_TFORLOOP, OP_TFORPREP, OP_VARARG, OP_VARARGPREP, OpCode,
};
use crate::{Float, Lua, LuaFn, ParseError, Ref, Str, Table};
use alloc::borrow::Cow;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use core::convert::identity;
use core::ffi::{c_char, c_void};
use core::fmt::Display;
use core::ptr::{null, null_mut};

type c_short = i16;
type c_ushort = u16;
type c_int = i32;
type c_uint = u32;
type c_long = i64;
type c_ulong = u64;

#[repr(C)]
pub struct Dyndata<D> {
    pub actvar: C2RustUnnamed_9<D>,
    pub gt: Labellist<D>,
    pub label: Labellist<D>,
}

impl<D> Clone for Dyndata<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for Dyndata<D> {}

#[repr(C)]
pub struct Labellist<D> {
    pub arr: *mut Labeldesc<D>,
    pub n: c_int,
    pub size: c_int,
}

impl<D> Clone for Labellist<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for Labellist<D> {}

#[repr(C)]
pub struct Labeldesc<D> {
    pub name: *const Str<D>,
    pub pc: c_int,
    pub line: c_int,
    pub nactvar: u8,
    pub close: u8,
}

impl<D> Clone for Labeldesc<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for Labeldesc<D> {}

#[repr(C)]
pub struct C2RustUnnamed_9<D> {
    pub arr: *mut Vardesc<D>,
    pub n: c_int,
    pub size: c_int,
}

impl<D> Clone for C2RustUnnamed_9<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for C2RustUnnamed_9<D> {}

#[repr(C)]
pub union Vardesc<D> {
    pub vd: C2RustUnnamed_10<D>,
    pub k: UnsafeValue<D>,
}

impl<D> Clone for Vardesc<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for Vardesc<D> {}

#[repr(C)]
pub struct C2RustUnnamed_10<D> {
    pub tt_: u8,
    pub kind: u8,
    pub ridx: u8,
    pub pidx: c_short,
    pub value_: UntaggedValue<D>,
    pub name: *const Str<D>,
}

impl<D> Clone for C2RustUnnamed_10<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for C2RustUnnamed_10<D> {}

#[repr(C)]
pub struct FuncState<D> {
    pub f: *mut Proto<D>,
    pub prev: *mut Self,
    pub bl: *mut BlockCnt,
    pub pc: c_int,
    pub lasttarget: c_int,
    pub previousline: c_int,
    pub nk: c_int,
    pub np: c_int,
    pub nabslineinfo: c_int,
    pub firstlocal: c_int,
    pub firstlabel: c_int,
    pub ndebugvars: c_short,
    pub nactvar: u8,
    pub nups: u8,
    pub freereg: u8,
    pub iwthabs: u8,
    pub needclose: u8,
}

impl<D> Default for FuncState<D> {
    fn default() -> Self {
        Self {
            f: null_mut(),
            prev: null_mut(),
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

pub type expkind = c_uint;
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

#[repr(C)]
pub struct expdesc<D> {
    pub k: expkind,
    pub u: C2RustUnnamed_11<D>,
    pub t: c_int,
    pub f: c_int,
}

impl<D> Clone for expdesc<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for expdesc<D> {}

#[repr(C)]
pub union C2RustUnnamed_11<D> {
    pub ival: i64,
    pub nval: Float,
    pub strval: *const Str<D>,
    pub info: c_int,
    pub ind: C2RustUnnamed_13,
    pub var: C2RustUnnamed_12,
}

impl<D> Clone for C2RustUnnamed_11<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for C2RustUnnamed_11<D> {}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct C2RustUnnamed_12 {
    pub ridx: u8,
    pub vidx: c_ushort,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct C2RustUnnamed_13 {
    pub idx: c_short,
    pub t: u8,
}

#[repr(C)]
pub struct LHS_assign<D> {
    pub prev: *mut Self,
    pub v: expdesc<D>,
}

impl<D> Clone for LHS_assign<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for LHS_assign<D> {}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct C2RustUnnamed_14 {
    pub left: u8,
    pub right: u8,
}

#[repr(C)]
pub struct ConsControl<D> {
    pub v: expdesc<D>,
    pub t: *mut expdesc<D>,
    pub nh: c_int,
    pub na: c_int,
    pub tostore: c_int,
}

impl<D> Clone for ConsControl<D> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<D> Copy for ConsControl<D> {}

unsafe fn error_expected<D>(ls: *mut LexState<D>, token: c_int) -> ParseError {
    luaX_syntaxerror(ls, format_args!("{} expected", luaX_token2str(token)))
}

unsafe fn errorlimit<D>(
    ls: *mut LexState<D>,
    fs: *mut FuncState<D>,
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

unsafe fn checklimit<D>(
    ls: *mut LexState<D>,
    fs: *mut FuncState<D>,
    v: c_int,
    l: c_int,
    what: impl Display,
) -> Result<(), ParseError> {
    if v > l {
        return Err(errorlimit(ls, fs, l, what));
    }
    Ok(())
}

unsafe fn testnext<D>(ls: *mut LexState<D>, c: c_int) -> Result<c_int, ParseError> {
    if (*ls).t.token == c {
        luaX_next(ls)?;
        return Ok(1 as c_int);
    } else {
        return Ok(0 as c_int);
    };
}

unsafe fn check<D>(ls: *mut LexState<D>, c: c_int) -> Result<(), ParseError> {
    if (*ls).t.token != c {
        return Err(error_expected(ls, c));
    }
    Ok(())
}

unsafe fn checknext<D>(ls: *mut LexState<D>, c: c_int) -> Result<(), ParseError> {
    check(ls, c)?;
    luaX_next(ls)
}

unsafe fn check_match<D>(
    ls: *mut LexState<D>,
    what: c_int,
    who: c_int,
    where_0: c_int,
) -> Result<(), ParseError> {
    if ((testnext(ls, what)? == 0) as c_int != 0 as c_int) as c_int as c_long != 0 {
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

unsafe fn str_checkname<D>(ls: *mut LexState<D>) -> Result<*const Str<D>, ParseError> {
    check(ls, TK_NAME as c_int)?;
    let ts = (*ls).t.seminfo.ts;
    luaX_next(ls)?;
    return Ok(ts);
}

unsafe fn init_exp<D>(e: *mut expdesc<D>, k: expkind, i: c_int) {
    (*e).t = -(1 as c_int);
    (*e).f = (*e).t;
    (*e).k = k;
    (*e).u.info = i;
}

unsafe fn codestring<D>(e: *mut expdesc<D>, s: *const Str<D>) {
    (*e).t = -(1 as c_int);
    (*e).f = (*e).t;
    (*e).k = VKSTR;
    (*e).u.strval = s;
}

unsafe fn codename<D>(ls: *mut LexState<D>, e: *mut expdesc<D>) -> Result<(), ParseError> {
    codestring(e, str_checkname(ls)?);
    Ok(())
}

unsafe fn registerlocalvar<D>(
    ls: *mut LexState<D>,
    fs: *mut FuncState<D>,
    varname: *const Str<D>,
) -> Result<c_int, ParseError> {
    let f = (*fs).f;
    let mut oldsize: c_int = (*f).sizelocvars;
    (*f).locvars = luaM_growaux_(
        (*f).locvars as *mut c_void,
        (*fs).ndebugvars as c_int,
        &mut (*f).sizelocvars,
        ::core::mem::size_of::<LocVar<D>>() as c_ulong as c_int,
        (if 32767 as c_int as usize
            <= (!(0 as c_int as usize)).wrapping_div(::core::mem::size_of::<LocVar<D>>())
        {
            32767 as c_int as c_uint
        } else {
            (!(0 as c_int as usize)).wrapping_div(::core::mem::size_of::<LocVar<D>>()) as c_uint
        }) as c_int,
        "local variables",
        (*ls).linenumber,
    )? as *mut LocVar<D>;
    while oldsize < (*f).sizelocvars {
        let fresh0 = oldsize;
        oldsize = oldsize + 1;
        let ref mut fresh1 = (*((*f).locvars).offset(fresh0 as isize)).varname;
        *fresh1 = 0 as *mut Str<D>;
    }
    let ref mut fresh2 = (*((*f).locvars).offset((*fs).ndebugvars as isize)).varname;
    *fresh2 = varname;
    (*((*f).locvars).offset((*fs).ndebugvars as isize)).startpc = (*fs).pc;

    if (*f).hdr.marked.get() as c_int & (1 as c_int) << 5 as c_int != 0
        && (*varname).hdr.marked.get() as c_int
            & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
            != 0
    {
        (&(*ls).g).gc.barrier(f.cast(), varname.cast());
    }

    let fresh3 = (*fs).ndebugvars;
    (*fs).ndebugvars = (*fs).ndebugvars + 1;
    return Ok(fresh3 as c_int);
}

unsafe fn new_localvar<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
    name: *const Str<A>,
) -> Result<c_int, ParseError> {
    let dyd = (*ls).dyd;

    checklimit(
        ls,
        fs,
        (*dyd).actvar.n + 1 as c_int - (*fs).firstlocal,
        200 as c_int,
        "local variables",
    )?;

    (*dyd).actvar.arr = luaM_growaux_(
        (*dyd).actvar.arr as *mut c_void,
        (*dyd).actvar.n + 1 as c_int,
        &raw mut (*dyd).actvar.size,
        size_of::<Vardesc<A>>() as c_ulong as c_int,
        i16::MAX.into(),
        "local variables",
        (*ls).linenumber,
    )? as *mut Vardesc<A>;
    let fresh4 = (*dyd).actvar.n;
    (*dyd).actvar.n = (*dyd).actvar.n + 1;
    let var = ((*dyd).actvar.arr).offset(fresh4 as isize) as *mut Vardesc<A>;
    (*var).vd.kind = 0 as c_int as u8;
    (*var).vd.name = name;
    return Ok((*dyd).actvar.n - 1 as c_int - (*fs).firstlocal);
}

unsafe fn getlocalvardesc<D>(
    ls: *mut LexState<D>,
    fs: *mut FuncState<D>,
    vidx: c_int,
) -> *mut Vardesc<D> {
    return ((*(*ls).dyd).actvar.arr).offset(((*fs).firstlocal + vidx) as isize) as *mut Vardesc<D>;
}

unsafe fn reglevel<D>(ls: *mut LexState<D>, fs: *mut FuncState<D>, mut nvar: c_int) -> c_int {
    loop {
        let fresh5 = nvar;
        nvar = nvar - 1;
        if !(fresh5 > 0 as c_int) {
            break;
        }
        let vd = getlocalvardesc(ls, fs, nvar);
        if (*vd).vd.kind as c_int != 3 as c_int {
            return (*vd).vd.ridx as c_int + 1 as c_int;
        }
    }
    return 0 as c_int;
}

pub unsafe fn luaY_nvarstack<D>(ls: *mut LexState<D>, fs: *mut FuncState<D>) -> c_int {
    return reglevel(ls, fs, (*fs).nactvar as c_int);
}

unsafe fn localdebuginfo<D>(
    ls: *mut LexState<D>,
    fs: *mut FuncState<D>,
    vidx: c_int,
) -> *mut LocVar<D> {
    let vd = getlocalvardesc(ls, fs, vidx);
    if (*vd).vd.kind as c_int == 3 as c_int {
        return null_mut();
    } else {
        let idx: c_int = (*vd).vd.pidx as c_int;
        return ((*(*fs).f).locvars).offset(idx as isize) as *mut LocVar<D>;
    };
}

unsafe fn init_var<D>(
    ls: *mut LexState<D>,
    fs: *mut FuncState<D>,
    e: *mut expdesc<D>,
    vidx: c_int,
) {
    (*e).t = -(1 as c_int);
    (*e).f = (*e).t;
    (*e).k = VLOCAL;
    (*e).u.var.vidx = vidx as c_ushort;
    (*e).u.var.ridx = (*getlocalvardesc(ls, fs, vidx)).vd.ridx;
}

unsafe fn check_readonly<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
    e: *mut expdesc<A>,
) -> Result<(), ParseError> {
    let mut varname = null();

    match (*e).k as c_uint {
        11 => {
            varname = (*((*(*ls).dyd).actvar.arr).offset((*e).u.info as isize))
                .vd
                .name;
        }
        9 => {
            let vardesc = getlocalvardesc(ls, fs, (*e).u.var.vidx as c_int);
            if (*vardesc).vd.kind as c_int != 0 as c_int {
                varname = (*vardesc).vd.name;
            }
        }
        10 => {
            let up = ((*(*fs).f).upvalues).offset((*e).u.info as isize) as *mut Upvaldesc<A>;
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

unsafe fn adjustlocalvars<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
    nvars: c_int,
) -> Result<(), ParseError> {
    let mut reglevel_0: c_int = luaY_nvarstack(ls, fs);
    let mut i: c_int = 0;
    i = 0 as c_int;
    while i < nvars {
        let fresh6 = (*fs).nactvar;
        (*fs).nactvar = ((*fs).nactvar).wrapping_add(1);
        let vidx: c_int = fresh6 as c_int;
        let var = getlocalvardesc(ls, fs, vidx);
        let fresh7 = reglevel_0;
        reglevel_0 = reglevel_0 + 1;
        (*var).vd.ridx = fresh7 as u8;
        (*var).vd.pidx = registerlocalvar(ls, fs, (*var).vd.name)? as c_short;
        i += 1;
    }
    Ok(())
}

unsafe fn removevars<D>(ls: *mut LexState<D>, fs: *mut FuncState<D>, tolevel: c_int) {
    (*(*ls).dyd).actvar.n -= (*fs).nactvar as c_int - tolevel;
    while (*fs).nactvar as c_int > tolevel {
        (*fs).nactvar = ((*fs).nactvar).wrapping_sub(1);
        let var = localdebuginfo(ls, fs, (*fs).nactvar as c_int);
        if !var.is_null() {
            (*var).endpc = (*fs).pc;
        }
    }
}

unsafe fn searchupvalue<D>(fs: *mut FuncState<D>, name: *const Str<D>) -> c_int {
    let mut i: c_int = 0;
    let up = (*(*fs).f).upvalues;
    i = 0 as c_int;
    while i < (*fs).nups as c_int {
        if (*up.offset(i as isize)).name == name {
            return i;
        }
        i += 1;
    }
    return -(1 as c_int);
}

unsafe fn allocupvalue<D>(
    ls: *mut LexState<D>,
    fs: *mut FuncState<D>,
) -> Result<*mut Upvaldesc<D>, ParseError> {
    let f = (*fs).f;
    let mut oldsize: c_int = (*f).sizeupvalues;

    checklimit(
        ls,
        fs,
        (*fs).nups as c_int + 1 as c_int,
        255 as c_int,
        "upvalues",
    )?;

    (*f).upvalues = luaM_growaux_(
        (*f).upvalues as *mut c_void,
        (*fs).nups as c_int,
        &mut (*f).sizeupvalues,
        ::core::mem::size_of::<Upvaldesc<D>>() as c_ulong as c_int,
        (if 255 as c_int as usize
            <= (!(0 as c_int as usize)).wrapping_div(::core::mem::size_of::<Upvaldesc<D>>())
        {
            255 as c_int as c_uint
        } else {
            (!(0 as c_int as usize)).wrapping_div(::core::mem::size_of::<Upvaldesc<D>>()) as c_uint
        }) as c_int,
        "upvalues",
        (*ls).linenumber,
    )? as *mut Upvaldesc<D>;
    while oldsize < (*f).sizeupvalues {
        let fresh8 = oldsize;
        oldsize = oldsize + 1;
        let ref mut fresh9 = (*((*f).upvalues).offset(fresh8 as isize)).name;
        *fresh9 = null_mut();
    }
    let fresh10 = (*fs).nups;
    (*fs).nups = ((*fs).nups).wrapping_add(1);
    return Ok(((*f).upvalues).offset(fresh10 as isize) as *mut Upvaldesc<D>);
}

unsafe fn newupvalue<D>(
    ls: *mut LexState<D>,
    fs: *mut FuncState<D>,
    name: *const Str<D>,
    v: *mut expdesc<D>,
) -> Result<c_int, ParseError> {
    let up = allocupvalue(ls, fs)?;
    let prev = (*fs).prev;

    if (*v).k as c_uint == VLOCAL as c_int as c_uint {
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
        (&(*ls).g).gc.barrier((*fs).f.cast(), name.cast());
    }

    return Ok((*fs).nups as c_int - 1 as c_int);
}

unsafe fn searchvar<D>(
    ls: *mut LexState<D>,
    fs: *mut FuncState<D>,
    n: *const Str<D>,
    var: *mut expdesc<D>,
) -> c_int {
    let mut i: c_int = 0;
    i = (*fs).nactvar as c_int - 1 as c_int;

    while i >= 0 as c_int {
        let vd = getlocalvardesc(ls, fs, i);
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

unsafe fn markupval<D>(fs: *mut FuncState<D>, level: c_int) {
    let mut bl: *mut BlockCnt = (*fs).bl;
    while (*bl).nactvar as c_int > level {
        bl = (*bl).previous;
    }
    (*bl).upval = 1 as c_int as u8;
    (*fs).needclose = 1 as c_int as u8;
}

unsafe fn marktobeclosed<D>(fs: *mut FuncState<D>) {
    let bl: *mut BlockCnt = (*fs).bl;
    (*bl).upval = 1 as c_int as u8;
    (*bl).insidetbc = 1 as c_int as u8;
    (*fs).needclose = 1 as c_int as u8;
}

unsafe fn singlevaraux<D>(
    ls: *mut LexState<D>,
    fs: *mut FuncState<D>,
    n: *const Str<D>,
    var: *mut expdesc<D>,
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
                if (*var).k as c_uint == VLOCAL as c_int as c_uint
                    || (*var).k as c_uint == VUPVAL as c_int as c_uint
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

unsafe fn singlevar<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
    var: *mut expdesc<A>,
) -> Result<(), ParseError> {
    let varname = str_checkname(ls)?;

    singlevaraux(ls, fs, varname, var, 1 as c_int)?;
    if (*var).k as c_uint == VVOID as c_int as c_uint {
        let mut key = expdesc {
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

unsafe fn adjust_assign<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
    nvars: c_int,
    nexps: c_int,
    e: *mut expdesc<A>,
) -> Result<(), ParseError> {
    let needed: c_int = nvars - nexps;
    if (*e).k as c_uint == VCALL as c_int as c_uint
        || (*e).k as c_uint == VVARARG as c_int as c_uint
    {
        let mut extra: c_int = needed + 1 as c_int;
        if extra < 0 as c_int {
            extra = 0 as c_int;
        }
        luaK_setreturns(ls, fs, e, extra)?;
    } else {
        if (*e).k as c_uint != VVOID as c_int as c_uint {
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

unsafe fn jumpscopeerror<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
    gt: *mut Labeldesc<A>,
) -> ParseError {
    let varname = (*(*getlocalvardesc(ls, fs, (*gt).nactvar.into())).vd.name).as_bytes();

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

unsafe fn solvegoto<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
    g: c_int,
    label: *mut Labeldesc<A>,
) -> Result<(), ParseError> {
    let mut i: c_int = 0;
    let gl = &raw mut (*(*ls).dyd).gt;
    let gt = ((*gl).arr).offset(g as isize) as *mut Labeldesc<A>;
    if ((((*gt).nactvar as c_int) < (*label).nactvar as c_int) as c_int != 0 as c_int) as c_int
        as c_long
        != 0
    {
        return Err(jumpscopeerror(ls, fs, gt));
    }
    luaK_patchlist(ls, fs, (*gt).pc, (*label).pc)?;
    i = g;
    while i < (*gl).n - 1 as c_int {
        *((*gl).arr).offset(i as isize) = *((*gl).arr).offset((i + 1 as c_int) as isize);
        i += 1;
    }
    (*gl).n -= 1;
    (*gl).n;
    Ok(())
}

unsafe fn findlabel<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
    name: *const Str<A>,
) -> *mut Labeldesc<A> {
    let mut i: c_int = 0;
    let dyd = (*ls).dyd;

    i = (*fs).firstlabel;
    while i < (*dyd).label.n {
        let lb = ((*dyd).label.arr).offset(i as isize) as *mut Labeldesc<A>;
        if (*lb).name == name {
            return lb;
        }
        i += 1;
    }

    null_mut()
}

unsafe fn newlabelentry<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
    l: *mut Labellist<A>,
    name: *const Str<A>,
    line: c_int,
    pc: c_int,
) -> Result<c_int, ParseError> {
    let n: c_int = (*l).n;
    (*l).arr = luaM_growaux_(
        (*l).arr as *mut c_void,
        n,
        &mut (*l).size,
        ::core::mem::size_of::<Labeldesc<A>>() as c_ulong as c_int,
        (if 32767 as c_int as usize
            <= (!(0 as c_int as usize)).wrapping_div(::core::mem::size_of::<Labeldesc<A>>())
        {
            32767 as c_int as c_uint
        } else {
            (!(0 as c_int as usize)).wrapping_div(::core::mem::size_of::<Labeldesc<A>>()) as c_uint
        }) as c_int,
        "labels/gotos",
        (*ls).linenumber,
    )? as *mut Labeldesc<A>;
    let ref mut fresh11 = (*((*l).arr).offset(n as isize)).name;
    *fresh11 = name;
    (*((*l).arr).offset(n as isize)).line = line;
    (*((*l).arr).offset(n as isize)).nactvar = (*fs).nactvar;
    (*((*l).arr).offset(n as isize)).close = 0 as c_int as u8;
    (*((*l).arr).offset(n as isize)).pc = pc;
    (*l).n = n + 1 as c_int;
    return Ok(n);
}

unsafe fn newgotoentry<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
    name: *const Str<A>,
    line: c_int,
    pc: c_int,
) -> Result<c_int, ParseError> {
    return newlabelentry(ls, fs, &mut (*(*ls).dyd).gt, name, line, pc);
}

unsafe fn solvegotos<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
    lb: *mut Labeldesc<A>,
) -> Result<c_int, ParseError> {
    let gl = &raw mut (*(*ls).dyd).gt;
    let mut i: c_int = (*(*fs).bl).firstgoto;
    let mut needsclose: c_int = 0 as c_int;
    while i < (*gl).n {
        if (*((*gl).arr).offset(i as isize)).name == (*lb).name {
            needsclose |= (*((*gl).arr).offset(i as isize)).close as c_int;
            solvegoto(ls, fs, i, lb)?;
        } else {
            i += 1;
        }
    }
    return Ok(needsclose);
}

unsafe fn createlabel<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
    name: *const Str<A>,
    line: c_int,
    last: c_int,
) -> Result<c_int, ParseError> {
    let ll = &raw mut (*(*ls).dyd).label;
    let l: c_int = newlabelentry(ls, fs, ll, name, line, luaK_getlabel(fs))?;
    if last != 0 {
        (*((*ll).arr).offset(l as isize)).nactvar = (*(*fs).bl).nactvar;
    }
    if solvegotos(ls, fs, &mut *((*ll).arr).offset(l as isize))? != 0 {
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

unsafe fn movegotosout<D>(ls: *mut LexState<D>, fs: *mut FuncState<D>, bl: *mut BlockCnt) {
    let mut i: c_int = 0;
    let gl = &raw mut (*(*ls).dyd).gt;
    i = (*bl).firstgoto;
    while i < (*gl).n {
        let gt = ((*gl).arr).offset(i as isize) as *mut Labeldesc<D>;
        if reglevel(ls, fs, (*gt).nactvar as c_int) > reglevel(ls, fs, (*bl).nactvar as c_int) {
            (*gt).close = ((*gt).close as c_int | (*bl).upval as c_int) as u8;
        }
        (*gt).nactvar = (*bl).nactvar;
        i += 1;
    }
}

unsafe fn enterblock<D>(
    ls: *mut LexState<D>,
    fs: *mut FuncState<D>,
    bl: *mut BlockCnt,
    isloop: u8,
) {
    (*bl).isloop = isloop;
    (*bl).nactvar = (*fs).nactvar;
    (*bl).firstlabel = (*(*ls).dyd).label.n;
    (*bl).firstgoto = (*(*ls).dyd).gt.n;
    (*bl).upval = 0 as c_int as u8;
    (*bl).insidetbc = (!((*fs).bl).is_null() && (*(*fs).bl).insidetbc as c_int != 0) as c_int as u8;
    (*bl).previous = (*fs).bl;
    (*fs).bl = bl;
}

unsafe fn undefgoto<D>(ls: *mut LexState<D>, gt: *mut Labeldesc<D>) -> ParseError {
    if (*gt).name == Str::from_str((*ls).g, "break").unwrap_or_else(identity) {
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

unsafe fn leaveblock<D>(ls: *mut LexState<D>, fs: *mut FuncState<D>) -> Result<(), ParseError> {
    let bl: *mut BlockCnt = (*fs).bl;
    let mut hasclose: c_int = 0 as c_int;
    let stklevel: c_int = reglevel(ls, fs, (*bl).nactvar as c_int);
    removevars(ls, fs, (*bl).nactvar as c_int);
    if (*bl).isloop != 0 {
        hasclose = createlabel(
            ls,
            fs,
            Str::from_str((*ls).g, "break").unwrap_or_else(identity),
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

unsafe fn addprototype<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
) -> Result<*mut Proto<A>, ParseError> {
    let g = (*ls).g;
    let f = (*fs).f;

    if (*fs).np >= (*f).sizep {
        let mut oldsize: c_int = (*f).sizep;
        (*f).p = luaM_growaux_(
            (*f).p as *mut c_void,
            (*fs).np,
            &mut (*f).sizep,
            ::core::mem::size_of::<*mut Proto<A>>() as c_ulong as c_int,
            (if (((1 as c_int) << 8 as c_int + 8 as c_int + 1 as c_int) - 1 as c_int) as usize
                <= (!(0 as c_int as usize)).wrapping_div(::core::mem::size_of::<*mut Proto<A>>())
            {
                (((1 as c_int) << 8 as c_int + 8 as c_int + 1 as c_int) - 1 as c_int) as c_uint
            } else {
                (!(0 as c_int as usize)).wrapping_div(::core::mem::size_of::<*mut Proto<A>>())
                    as c_uint
            }) as c_int,
            "functions",
            (*ls).linenumber,
        )? as *mut *mut Proto<A>;
        while oldsize < (*f).sizep {
            let fresh12 = oldsize;
            oldsize = oldsize + 1;
            let ref mut fresh13 = *((*f).p).offset(fresh12 as isize);
            *fresh13 = null_mut();
        }
    }

    let clp = luaF_newproto(g, (*ls).chunk.clone());
    let fresh14 = (*fs).np;
    (*fs).np = (*fs).np + 1;
    let ref mut fresh15 = *((*f).p).offset(fresh14 as isize);
    *fresh15 = clp;

    if (*f).hdr.marked.get() as c_int & (1 as c_int) << 5 as c_int != 0
        && (*clp).hdr.marked.get() as c_int
            & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
            != 0
    {
        g.gc.barrier(f.cast(), clp.cast());
    }

    return Ok(clp);
}

unsafe fn codeclosure<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
    v: *mut expdesc<A>,
) -> Result<(), ParseError> {
    let fs = (*fs).prev;

    init_exp(
        v,
        VRELOC,
        luaK_codeABx(
            ls,
            fs,
            OP_CLOSURE,
            0 as c_int,
            ((*fs).np - 1 as c_int) as c_uint,
        )?,
    );
    luaK_exp2nextreg(ls, fs, v)
}

unsafe fn open_func<A>(ls: *mut LexState<A>, fs: *mut FuncState<A>, bl: *mut BlockCnt) {
    let f = (*fs).f;

    (*fs).pc = 0 as c_int;
    (*fs).previousline = (*f).linedefined;
    (*fs).iwthabs = 0 as c_int as u8;
    (*fs).lasttarget = 0 as c_int;
    (*fs).freereg = 0 as c_int as u8;
    (*fs).nk = 0 as c_int;
    (*fs).nabslineinfo = 0 as c_int;
    (*fs).np = 0 as c_int;
    (*fs).nups = 0 as c_int as u8;
    (*fs).ndebugvars = 0 as c_int as c_short;
    (*fs).nactvar = 0 as c_int as u8;
    (*fs).needclose = 0 as c_int as u8;
    (*fs).firstlocal = (*(*ls).dyd).actvar.n;
    (*fs).firstlabel = (*(*ls).dyd).label.n;
    (*fs).bl = 0 as *mut BlockCnt;
    (*f).maxstacksize = 2 as c_int as u8;
    enterblock(ls, fs, bl, 0 as c_int as u8);
}

unsafe fn close_func<A>(ls: *mut LexState<A>, fs: *mut FuncState<A>) -> Result<(), ParseError> {
    let f = (*fs).f;

    luaK_ret(ls, fs, luaY_nvarstack(ls, fs), 0 as c_int)?;
    leaveblock(ls, fs)?;
    luaK_finish(ls, fs)?;
    (*f).code = luaM_shrinkvector_(
        (*ls).g,
        (*f).code as *mut c_void,
        &mut (*f).sizecode,
        (*fs).pc,
        ::core::mem::size_of::<u32>() as c_ulong as c_int,
    ) as *mut u32;
    (*f).lineinfo = luaM_shrinkvector_(
        (*ls).g,
        (*f).lineinfo as *mut c_void,
        &mut (*f).sizelineinfo,
        (*fs).pc,
        ::core::mem::size_of::<i8>() as c_ulong as c_int,
    ) as *mut i8;
    (*f).abslineinfo = luaM_shrinkvector_(
        (*ls).g,
        (*f).abslineinfo as *mut c_void,
        &mut (*f).sizeabslineinfo,
        (*fs).nabslineinfo,
        ::core::mem::size_of::<AbsLineInfo>() as c_ulong as c_int,
    ) as *mut AbsLineInfo;
    (*f).k = luaM_shrinkvector_(
        (*ls).g,
        (*f).k as *mut c_void,
        &mut (*f).sizek,
        (*fs).nk,
        ::core::mem::size_of::<UnsafeValue<A>>() as c_ulong as c_int,
    ) as *mut UnsafeValue<A>;
    (*f).p = luaM_shrinkvector_(
        (*ls).g,
        (*f).p as *mut c_void,
        &mut (*f).sizep,
        (*fs).np,
        ::core::mem::size_of::<*mut Proto<A>>() as c_ulong as c_int,
    ) as *mut *mut Proto<A>;
    (*f).locvars = luaM_shrinkvector_(
        (*ls).g,
        (*f).locvars as *mut c_void,
        &mut (*f).sizelocvars,
        (*fs).ndebugvars as c_int,
        ::core::mem::size_of::<LocVar<A>>() as c_ulong as c_int,
    ) as *mut LocVar<A>;
    (*f).upvalues = luaM_shrinkvector_(
        (*ls).g,
        (*f).upvalues as *mut c_void,
        &mut (*f).sizeupvalues,
        (*fs).nups as c_int,
        ::core::mem::size_of::<Upvaldesc<A>>() as c_ulong as c_int,
    ) as *mut Upvaldesc<A>;

    (*ls).g.gc.step();

    Ok(())
}

unsafe fn block_follow<D>(ls: *mut LexState<D>, withuntil: c_int) -> c_int {
    match (*ls).t.token {
        259 | 260 | 261 | 288 => return 1 as c_int,
        276 => return withuntil,
        _ => return 0 as c_int,
    };
}

unsafe fn statlist<A>(ls: *mut LexState<A>, fs: *mut FuncState<A>) -> Result<(), ParseError> {
    while block_follow(ls, 1 as c_int) == 0 {
        if (*ls).t.token == TK_RETURN as c_int {
            statement(ls, fs)?;
            return Ok(());
        }
        statement(ls, fs)?;
    }

    Ok(())
}

unsafe fn fieldsel<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
    v: *mut expdesc<A>,
) -> Result<(), ParseError> {
    let mut key = expdesc {
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

unsafe fn yindex<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
    v: *mut expdesc<A>,
) -> Result<(), ParseError> {
    luaX_next(ls)?;
    expr(ls, fs, v)?;
    luaK_exp2val(ls, fs, v)?;
    checknext(ls, ']' as i32)?;
    Ok(())
}

unsafe fn recfield<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
    cc: *mut ConsControl<A>,
) -> Result<(), ParseError> {
    let reg: c_int = (*fs).freereg as c_int;
    let mut tab = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    let mut key = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    let mut val = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    if (*ls).t.token == TK_NAME as c_int {
        codename(ls, &mut key)?;
    } else {
        yindex(ls, fs, &mut key)?;
    }

    checklimit(ls, fs, (*cc).nh, 2147483647, "items in a constructor")?;

    (*cc).nh += 1;
    (*cc).nh;
    checknext(ls, '=' as i32)?;
    tab = *(*cc).t;
    luaK_indexed(ls, fs, &mut tab, &mut key)?;
    expr(ls, fs, &mut val)?;
    luaK_storevar(ls, fs, &mut tab, &mut val)?;
    (*fs).freereg = reg as u8;
    Ok(())
}

unsafe fn closelistfield<D>(
    ls: *mut LexState<D>,
    fs: *mut FuncState<D>,
    cc: *mut ConsControl<D>,
) -> Result<(), ParseError> {
    if (*cc).v.k as c_uint == VVOID as c_int as c_uint {
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

unsafe fn lastlistfield<D>(
    ls: *mut LexState<D>,
    fs: *mut FuncState<D>,
    cc: *mut ConsControl<D>,
) -> Result<(), ParseError> {
    if (*cc).tostore == 0 as c_int {
        return Ok(());
    }
    if (*cc).v.k as c_uint == VCALL as c_int as c_uint
        || (*cc).v.k as c_uint == VVARARG as c_int as c_uint
    {
        luaK_setreturns(ls, fs, &mut (*cc).v, -(1 as c_int))?;
        luaK_setlist(ls, fs, (*(*cc).t).u.info, (*cc).na, -(1 as c_int))?;
        (*cc).na -= 1;
        (*cc).na;
    } else {
        if (*cc).v.k as c_uint != VVOID as c_int as c_uint {
            luaK_exp2nextreg(ls, fs, &mut (*cc).v)?;
        }
        luaK_setlist(ls, fs, (*(*cc).t).u.info, (*cc).na, (*cc).tostore)?;
    }
    (*cc).na += (*cc).tostore;
    Ok(())
}

unsafe fn listfield<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
    cc: *mut ConsControl<A>,
) -> Result<(), ParseError> {
    expr(ls, fs, &mut (*cc).v)?;

    (*cc).tostore += 1;
    (*cc).tostore;
    Ok(())
}

unsafe fn field<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
    cc: *mut ConsControl<A>,
) -> Result<(), ParseError> {
    match (*ls).t.token {
        291 => {
            if luaX_lookahead(ls)? != '=' as i32 {
                listfield(ls, fs, cc)?;
            } else {
                recfield(ls, fs, cc)?;
            }
        }
        91 => {
            recfield(ls, fs, cc)?;
        }
        _ => listfield(ls, fs, cc)?,
    };

    Ok(())
}

unsafe fn constructor<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
    t: *mut expdesc<A>,
) -> Result<(), ParseError> {
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
    let mut cc = ConsControl {
        v: expdesc {
            k: VVOID,
            u: C2RustUnnamed_11 { ival: 0 },
            t: 0,
            f: 0,
        },
        t: null_mut(),
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
        field(ls, fs, &mut cc)?;
        if !(testnext(ls, ',' as i32)? != 0 || testnext(ls, ';' as i32)? != 0) {
            break;
        }
    }
    check_match(ls, '}' as i32, '{' as i32, line)?;
    lastlistfield(ls, fs, &mut cc)?;
    luaK_settablesize(fs, pc, (*t).u.info, cc.na, cc.nh);
    Ok(())
}

unsafe fn setvararg<D>(
    ls: *mut LexState<D>,
    fs: *mut FuncState<D>,
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

unsafe fn parlist<A>(ls: *mut LexState<A>, fs: *mut FuncState<A>) -> Result<(), ParseError> {
    let f = (*fs).f;
    let mut nparams: c_int = 0 as c_int;
    let mut isvararg: c_int = 0 as c_int;
    if (*ls).t.token != ')' as i32 {
        loop {
            match (*ls).t.token {
                291 => {
                    new_localvar(ls, fs, str_checkname(ls)?)?;
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
    adjustlocalvars(ls, fs, nparams)?;
    (*f).numparams = (*fs).nactvar;
    if isvararg != 0 {
        setvararg(ls, fs, (*f).numparams as c_int)?;
    }
    luaK_reserveregs(ls, fs, (*fs).nactvar as c_int)
}

unsafe fn body<D>(
    ls: *mut LexState<D>,
    fs: *mut FuncState<D>,
    e: *mut expdesc<D>,
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

    new_fs.f = addprototype(ls, fs)?;
    (*new_fs.f).linedefined = line;
    new_fs.prev = fs;

    open_func(ls, &mut new_fs, &mut bl);
    checknext(ls, '(' as i32)?;
    if ismethod != 0 {
        new_localvar(
            ls,
            &mut new_fs,
            luaX_newstring(
                ls,
                b"self\0" as *const u8 as *const c_char,
                ::core::mem::size_of::<[c_char; 5]>()
                    .wrapping_div(::core::mem::size_of::<c_char>())
                    .wrapping_sub(1),
            ),
        )?;
        adjustlocalvars(ls, &mut new_fs, 1 as c_int)?;
    }
    parlist(ls, &mut new_fs)?;
    checknext(ls, ')' as i32)?;
    statlist(ls, &mut new_fs)?;
    (*new_fs.f).lastlinedefined = (*ls).linenumber;
    check_match(ls, TK_END as c_int, TK_FUNCTION as c_int, line)?;
    codeclosure(ls, &mut new_fs, e)?;
    close_func(ls, &mut new_fs)?;

    Ok(())
}

unsafe fn explist<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
    v: *mut expdesc<A>,
) -> Result<c_int, ParseError> {
    let mut n: c_int = 1 as c_int;

    expr(ls, fs, v)?;

    while testnext(ls, ',' as i32)? != 0 {
        luaK_exp2nextreg(ls, fs, v)?;
        expr(ls, fs, v)?;
        n += 1;
    }
    return Ok(n);
}

unsafe fn funcargs<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
    f: *mut expdesc<A>,
) -> Result<(), ParseError> {
    let mut args = expdesc {
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
                explist(ls, fs, &mut args)?;
                if args.k as c_uint == VCALL as c_int as c_uint
                    || args.k as c_uint == VVARARG as c_int as c_uint
                {
                    luaK_setreturns(ls, fs, &mut args, -(1 as c_int))?;
                }
            }
            check_match(ls, ')' as i32, '(' as i32, line)?;
        }
        123 => constructor(ls, fs, &mut args)?,
        292 => {
            codestring(&mut args, (*ls).t.seminfo.ts);
            luaX_next(ls)?;
        }
        _ => return Err(luaX_syntaxerror(ls, "function arguments expected")),
    }
    base = (*f).u.info;
    if args.k as c_uint == VCALL as c_int as c_uint
        || args.k as c_uint == VVARARG as c_int as c_uint
    {
        nparams = -(1 as c_int);
    } else {
        if args.k as c_uint != VVOID as c_int as c_uint {
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

unsafe fn primaryexp<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
    v: *mut expdesc<A>,
) -> Result<(), ParseError> {
    match (*ls).t.token {
        40 => {
            let line: c_int = (*ls).linenumber;
            luaX_next(ls)?;
            expr(ls, fs, v)?;
            check_match(ls, ')' as i32, '(' as i32, line)?;
            luaK_dischargevars(ls, fs, v)?;
            Ok(())
        }
        291 => {
            singlevar(ls, fs, v)?;
            Ok(())
        }
        _ => Err(luaX_syntaxerror(ls, "unexpected symbol")),
    }
}

unsafe fn suffixedexp<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
    v: *mut expdesc<A>,
) -> Result<(), ParseError> {
    primaryexp(ls, fs, v)?;

    loop {
        match (*ls).t.token {
            46 => fieldsel(ls, fs, v)?,
            91 => {
                let mut key = expdesc {
                    k: VVOID,
                    u: C2RustUnnamed_11 { ival: 0 },
                    t: 0,
                    f: 0,
                };
                luaK_exp2anyregup(ls, fs, v)?;
                yindex(ls, fs, &mut key)?;
                luaK_indexed(ls, fs, v, &mut key)?;
            }
            58 => {
                let mut key_0 = expdesc {
                    k: VVOID,
                    u: C2RustUnnamed_11 { ival: 0 },
                    t: 0,
                    f: 0,
                };
                luaX_next(ls)?;
                codename(ls, &mut key_0)?;
                luaK_self(ls, fs, v, &mut key_0)?;
                funcargs(ls, fs, v)?;
            }
            40 | 292 | 123 => {
                luaK_exp2nextreg(ls, fs, v)?;
                funcargs(ls, fs, v)?;
            }
            _ => return Ok(()),
        }
    }
}

unsafe fn simpleexp<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
    v: *mut expdesc<A>,
) -> Result<(), ParseError> {
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
            if (*(*fs).f).is_vararg == 0 {
                return Err(luaX_syntaxerror(
                    ls,
                    "cannot use '...' outside a vararg function",
                ));
            }
            init_exp(v, VVARARG, luaK_codeABCk(ls, fs, OP_VARARG, 0, 0, 1, 0)?);
        }
        123 => return constructor(ls, fs, v),
        264 => {
            luaX_next(ls)?;
            body(ls, fs, v, 0 as c_int, (*ls).linenumber)?;
            return Ok(());
        }
        _ => return suffixedexp(ls, fs, v),
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

unsafe fn subexpr<D>(
    ls: *mut LexState<D>,
    fs: *mut FuncState<D>,
    v: *mut expdesc<D>,
    limit: c_int,
) -> Result<BinOpr, ParseError> {
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

    if uop as c_uint != OPR_NOUNOPR as c_int as c_uint {
        let line: c_int = (*ls).linenumber;
        luaX_next(ls)?;
        subexpr(ls, fs, v, 12 as c_int)?;
        luaK_prefix(ls, fs, uop, v, line)?;
    } else {
        simpleexp(ls, fs, v)?;
    }
    op = getbinopr((*ls).t.token);
    while op as c_uint != OPR_NOBINOPR as c_int as c_uint
        && priority[op as usize].left as c_int > limit
    {
        let mut v2 = expdesc {
            k: VVOID,
            u: C2RustUnnamed_11 { ival: 0 },
            t: 0,
            f: 0,
        };
        let mut nextop: BinOpr = OPR_ADD;
        let line_0: c_int = (*ls).linenumber;
        luaX_next(ls)?;
        luaK_infix(ls, fs, op, v)?;
        nextop = subexpr(ls, fs, &mut v2, priority[op as usize].right as c_int)?;
        luaK_posfix(ls, fs, op, v, &mut v2, line_0)?;
        op = nextop;
    }

    (*ls).level -= 1;

    return Ok(op);
}

unsafe fn expr<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
    v: *mut expdesc<A>,
) -> Result<(), ParseError> {
    subexpr(ls, fs, v, 0 as c_int)?;
    Ok(())
}

unsafe fn block<A>(ls: *mut LexState<A>, fs: *mut FuncState<A>) -> Result<(), ParseError> {
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
    statlist(ls, fs)?;
    leaveblock(ls, fs)
}

unsafe fn check_conflict<D>(
    ls: *mut LexState<D>,
    fs: *mut FuncState<D>,
    mut lh: *mut LHS_assign<D>,
    v: *mut expdesc<D>,
) -> Result<(), ParseError> {
    let extra: c_int = (*fs).freereg as c_int;
    let mut conflict: c_int = 0 as c_int;
    while !lh.is_null() {
        if VINDEXED as c_int as c_uint <= (*lh).v.k as c_uint
            && (*lh).v.k as c_uint <= VINDEXSTR as c_int as c_uint
        {
            if (*lh).v.k as c_uint == VINDEXUP as c_int as c_uint {
                if (*v).k as c_uint == VUPVAL as c_int as c_uint
                    && (*lh).v.u.ind.t as c_int == (*v).u.info
                {
                    conflict = 1 as c_int;
                    (*lh).v.k = VINDEXSTR;
                    (*lh).v.u.ind.t = extra as u8;
                }
            } else {
                if (*v).k as c_uint == VLOCAL as c_int as c_uint
                    && (*lh).v.u.ind.t as c_int == (*v).u.var.ridx as c_int
                {
                    conflict = 1 as c_int;
                    (*lh).v.u.ind.t = extra as u8;
                }
                if (*lh).v.k as c_uint == VINDEXED as c_int as c_uint
                    && (*v).k as c_uint == VLOCAL as c_int as c_uint
                    && (*lh).v.u.ind.idx as c_int == (*v).u.var.ridx as c_int
                {
                    conflict = 1 as c_int;
                    (*lh).v.u.ind.idx = extra as c_short;
                }
            }
        }
        lh = (*lh).prev;
    }
    if conflict != 0 {
        if (*v).k as c_uint == VLOCAL as c_int as c_uint {
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

unsafe fn restassign<D>(
    ls: *mut LexState<D>,
    fs: *mut FuncState<D>,
    lh: *mut LHS_assign<D>,
    nvars: c_int,
) -> Result<(), ParseError> {
    let mut e = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    if !(VLOCAL as c_int as c_uint <= (*lh).v.k as c_uint
        && (*lh).v.k as c_uint <= VINDEXSTR as c_int as c_uint)
    {
        return Err(luaX_syntaxerror(ls, "syntax error"));
    }
    check_readonly(ls, fs, &mut (*lh).v)?;
    if testnext(ls, ',' as i32)? != 0 {
        let mut nv = LHS_assign {
            prev: null_mut(),
            v: expdesc {
                k: VVOID,
                u: C2RustUnnamed_11 { ival: 0 },
                t: 0,
                f: 0,
            },
        };
        nv.prev = lh;
        suffixedexp(ls, fs, &mut nv.v)?;
        if !(VINDEXED as c_int as c_uint <= nv.v.k as c_uint
            && nv.v.k as c_uint <= VINDEXSTR as c_int as c_uint)
        {
            check_conflict(ls, fs, lh, &mut nv.v)?;
        }

        (*ls).level += 1;

        if (*ls).level >= 200 {
            return Err(ParseError::ItemLimit {
                name: "nested level",
                limit: 200,
                line: (*ls).linenumber,
            });
        }

        restassign(ls, fs, &mut nv, nvars + 1 as c_int)?;

        (*ls).level -= 1;
    } else {
        let mut nexps: c_int = 0;
        checknext(ls, '=' as i32)?;
        nexps = explist(ls, fs, &mut e)?;
        if nexps != nvars {
            adjust_assign(ls, fs, nvars, nexps, &mut e)?;
        } else {
            luaK_setoneret(fs, &mut e);
            luaK_storevar(ls, fs, &mut (*lh).v, &mut e)?;
            return Ok(());
        }
    }
    init_exp(&mut e, VNONRELOC, (*fs).freereg as c_int - 1 as c_int);

    luaK_storevar(ls, fs, &mut (*lh).v, &mut e)
}

unsafe fn cond<A>(ls: *mut LexState<A>, fs: *mut FuncState<A>) -> Result<c_int, ParseError> {
    let mut v = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    expr(ls, fs, &mut v)?;
    if v.k as c_uint == VNIL as c_int as c_uint {
        v.k = VFALSE;
    }

    luaK_goiftrue(ls, fs, &mut v)?;

    return Ok(v.f);
}

unsafe fn gotostat<A>(ls: *mut LexState<A>, fs: *mut FuncState<A>) -> Result<(), ParseError> {
    let line: c_int = (*ls).linenumber;
    let name = str_checkname(ls)?;
    let lb = findlabel(ls, fs, name);

    if lb.is_null() {
        newgotoentry(ls, fs, name, line, luaK_jump(ls, fs)?)?;
    } else {
        let lblevel: c_int = reglevel(ls, fs, (*lb).nactvar as c_int);
        if luaY_nvarstack(ls, fs) > lblevel {
            luaK_codeABCk(ls, fs, OP_CLOSE, lblevel, 0 as c_int, 0 as c_int, 0)?;
        }
        luaK_patchlist(ls, fs, luaK_jump(ls, fs)?, (*lb).pc)?;
    };
    Ok(())
}

unsafe fn breakstat<A>(ls: *mut LexState<A>, fs: *mut FuncState<A>) -> Result<(), ParseError> {
    let line: c_int = (*ls).linenumber;
    luaX_next(ls)?;
    newgotoentry(
        ls,
        fs,
        Str::from_str((*ls).g, "break").unwrap_or_else(identity),
        line,
        luaK_jump(ls, fs)?,
    )?;
    Ok(())
}

unsafe fn checkrepeated<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
    name: *const Str<A>,
) -> Result<(), ParseError> {
    let lb = findlabel(ls, fs, name);

    if !lb.is_null() {
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

unsafe fn labelstat<D>(
    ls: *mut LexState<D>,
    fs: *mut FuncState<D>,
    name: *const Str<D>,
    line: c_int,
) -> Result<(), ParseError> {
    checknext(ls, TK_DBCOLON as c_int)?;
    while (*ls).t.token == ';' as i32 || (*ls).t.token == TK_DBCOLON as c_int {
        statement(ls, fs)?;
    }

    checkrepeated(ls, fs, name)?;
    createlabel(ls, fs, name, line, block_follow(ls, 0 as c_int))?;

    Ok(())
}

unsafe fn whilestat<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
    line: c_int,
) -> Result<(), ParseError> {
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
    condexit = cond(ls, fs)?;
    enterblock(ls, fs, &mut bl, 1 as c_int as u8);
    checknext(ls, TK_DO as c_int)?;
    block(ls, fs)?;
    luaK_patchlist(ls, fs, luaK_jump(ls, fs)?, whileinit)?;
    check_match(ls, TK_END as c_int, TK_WHILE as c_int, line)?;
    leaveblock(ls, fs)?;
    luaK_patchtohere(ls, fs, condexit)?;
    Ok(())
}

unsafe fn repeatstat<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
    line: c_int,
) -> Result<(), ParseError> {
    let mut condexit: c_int = 0;
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
    statlist(ls, fs)?;
    check_match(ls, TK_UNTIL as c_int, TK_REPEAT as c_int, line)?;
    condexit = cond(ls, fs)?;
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

unsafe fn exp1<A>(ls: *mut LexState<A>, fs: *mut FuncState<A>) -> Result<(), ParseError> {
    let mut e = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };

    expr(ls, fs, &mut e)?;
    luaK_exp2nextreg(ls, fs, &mut e)
}

unsafe fn fixforjump<D>(
    ls: *mut LexState<D>,
    fs: *mut FuncState<D>,
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
        != 0 as c_int) as c_int as c_long
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

unsafe fn forbody<D>(
    ls: *mut LexState<D>,
    fs: *mut FuncState<D>,
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
    let mut prep: c_int = 0;
    let mut endfor: c_int = 0;
    checknext(ls, TK_DO as c_int)?;
    prep = luaK_codeABx(ls, fs, forprep[isgen as usize], base, 0 as c_int as c_uint)?;
    enterblock(ls, fs, &mut bl, 0 as c_int as u8);
    adjustlocalvars(ls, fs, nvars)?;
    luaK_reserveregs(ls, fs, nvars)?;
    block(ls, fs)?;
    leaveblock(ls, fs)?;
    fixforjump(ls, fs, prep, luaK_getlabel(fs), 0 as c_int)?;
    if isgen != 0 {
        luaK_codeABCk(ls, fs, OP_TFORCALL, base, 0 as c_int, nvars, 0 as c_int)?;
        luaK_fixline(ls, fs, line)?;
    }
    endfor = luaK_codeABx(ls, fs, forloop[isgen as usize], base, 0 as c_int as c_uint)?;
    fixforjump(ls, fs, endfor, prep + 1 as c_int, 1 as c_int)?;
    luaK_fixline(ls, fs, line)
}

unsafe fn fornum<D>(
    ls: *mut LexState<D>,
    fs: *mut FuncState<D>,
    varname: *const Str<D>,
    line: c_int,
) -> Result<(), ParseError> {
    let base: c_int = (*fs).freereg as c_int;
    new_localvar(
        ls,
        fs,
        luaX_newstring(
            ls,
            b"(for state)\0" as *const u8 as *const c_char,
            ::core::mem::size_of::<[c_char; 12]>()
                .wrapping_div(::core::mem::size_of::<c_char>())
                .wrapping_sub(1),
        ),
    )?;
    new_localvar(
        ls,
        fs,
        luaX_newstring(
            ls,
            b"(for state)\0" as *const u8 as *const c_char,
            ::core::mem::size_of::<[c_char; 12]>()
                .wrapping_div(::core::mem::size_of::<c_char>())
                .wrapping_sub(1),
        ),
    )?;
    new_localvar(
        ls,
        fs,
        luaX_newstring(
            ls,
            b"(for state)\0" as *const u8 as *const c_char,
            ::core::mem::size_of::<[c_char; 12]>()
                .wrapping_div(::core::mem::size_of::<c_char>())
                .wrapping_sub(1),
        ),
    )?;
    new_localvar(ls, fs, varname)?;
    checknext(ls, '=' as i32)?;
    exp1(ls, fs)?;
    checknext(ls, ',' as i32)?;
    exp1(ls, fs)?;
    if testnext(ls, ',' as i32)? != 0 {
        exp1(ls, fs)?;
    } else {
        luaK_int(ls, fs, (*fs).freereg as c_int, 1 as c_int as i64)?;
        luaK_reserveregs(ls, fs, 1 as c_int)?;
    }
    adjustlocalvars(ls, fs, 3 as c_int)?;
    forbody(ls, fs, base, line, 1 as c_int, 0 as c_int)?;

    Ok(())
}

unsafe fn forlist<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
    indexname: *const Str<A>,
) -> Result<(), ParseError> {
    let mut e = expdesc {
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
        fs,
        luaX_newstring(
            ls,
            b"(for state)\0" as *const u8 as *const c_char,
            ::core::mem::size_of::<[c_char; 12]>()
                .wrapping_div(::core::mem::size_of::<c_char>())
                .wrapping_sub(1),
        ),
    )?;
    new_localvar(
        ls,
        fs,
        luaX_newstring(
            ls,
            b"(for state)\0" as *const u8 as *const c_char,
            ::core::mem::size_of::<[c_char; 12]>()
                .wrapping_div(::core::mem::size_of::<c_char>())
                .wrapping_sub(1),
        ),
    )?;
    new_localvar(
        ls,
        fs,
        luaX_newstring(
            ls,
            b"(for state)\0" as *const u8 as *const c_char,
            ::core::mem::size_of::<[c_char; 12]>()
                .wrapping_div(::core::mem::size_of::<c_char>())
                .wrapping_sub(1),
        ),
    )?;
    new_localvar(
        ls,
        fs,
        luaX_newstring(
            ls,
            b"(for state)\0" as *const u8 as *const c_char,
            ::core::mem::size_of::<[c_char; 12]>()
                .wrapping_div(::core::mem::size_of::<c_char>())
                .wrapping_sub(1),
        ),
    )?;
    new_localvar(ls, fs, indexname)?;

    while testnext(ls, ',' as i32)? != 0 {
        new_localvar(ls, fs, str_checkname(ls)?)?;
        nvars += 1;
    }
    checknext(ls, TK_IN as c_int)?;
    line = (*ls).linenumber;
    adjust_assign(ls, fs, 4 as c_int, explist(ls, fs, &mut e)?, &mut e)?;
    adjustlocalvars(ls, fs, 4 as c_int)?;
    marktobeclosed(fs);
    luaK_checkstack(ls, fs, 3 as c_int)?;
    forbody(ls, fs, base, line, nvars - 4 as c_int, 1 as c_int)?;

    Ok(())
}

unsafe fn forstat<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
    line: c_int,
) -> Result<(), ParseError> {
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
        61 => fornum(ls, fs, varname, line)?,
        44 | 267 => forlist(ls, fs, varname)?,
        _ => return Err(luaX_syntaxerror(ls, "'=' or 'in' expected")),
    }
    check_match(ls, TK_END as c_int, TK_FOR as c_int, line)?;
    leaveblock(ls, fs)?;
    Ok(())
}

unsafe fn test_then_block<D>(
    ls: *mut LexState<D>,
    fs: *mut FuncState<D>,
    escapelist: *mut c_int,
) -> Result<(), ParseError> {
    let mut bl: BlockCnt = BlockCnt {
        previous: 0 as *mut BlockCnt,
        firstlabel: 0,
        firstgoto: 0,
        nactvar: 0,
        upval: 0,
        isloop: 0,
        insidetbc: 0,
    };
    let mut v = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    let mut jf: c_int = 0;
    luaX_next(ls)?;
    expr(ls, fs, &mut v)?;
    checknext(ls, TK_THEN as c_int)?;
    if (*ls).t.token == TK_BREAK as c_int {
        let line: c_int = (*ls).linenumber;
        luaK_goiffalse(ls, fs, &mut v)?;
        luaX_next(ls)?;
        enterblock(ls, fs, &mut bl, 0 as c_int as u8);
        newgotoentry(
            ls,
            fs,
            Str::from_str((*ls).g, "break").unwrap_or_else(identity),
            line,
            v.t,
        )?;
        while testnext(ls, ';' as i32)? != 0 {}
        if block_follow(ls, 0 as c_int) != 0 {
            leaveblock(ls, fs)?;
            return Ok(());
        } else {
            jf = luaK_jump(ls, fs)?;
        }
    } else {
        luaK_goiftrue(ls, fs, &mut v)?;
        enterblock(ls, fs, &mut bl, 0 as c_int as u8);
        jf = v.f;
    }
    statlist(ls, fs)?;
    leaveblock(ls, fs)?;
    if (*ls).t.token == TK_ELSE as c_int || (*ls).t.token == TK_ELSEIF as c_int {
        luaK_concat(ls, fs, escapelist, luaK_jump(ls, fs)?)?;
    }
    luaK_patchtohere(ls, fs, jf)
}

unsafe fn ifstat<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
    line: c_int,
) -> Result<(), ParseError> {
    let mut escapelist: c_int = -(1 as c_int);
    test_then_block(ls, fs, &mut escapelist)?;
    while (*ls).t.token == TK_ELSEIF as c_int {
        test_then_block(ls, fs, &mut escapelist)?;
    }
    if testnext(ls, TK_ELSE as c_int)? != 0 {
        block(ls, fs)?;
    }
    check_match(ls, TK_END as c_int, TK_IF as c_int, line)?;
    luaK_patchtohere(ls, fs, escapelist)
}

unsafe fn localfunc<A>(ls: *mut LexState<A>, fs: *mut FuncState<A>) -> Result<(), ParseError> {
    let mut b = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    let fvar: c_int = (*fs).nactvar as c_int;
    new_localvar(ls, fs, str_checkname(ls)?)?;
    adjustlocalvars(ls, fs, 1 as c_int)?;
    body(ls, fs, &mut b, 0 as c_int, (*ls).linenumber)?;
    (*localdebuginfo(ls, fs, fvar)).startpc = (*fs).pc;
    Ok(())
}

unsafe fn getlocalattribute<D>(ls: *mut LexState<D>) -> Result<c_int, ParseError> {
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

unsafe fn checktoclose<D>(
    ls: *mut LexState<D>,
    fs: *mut FuncState<D>,
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

unsafe fn localstat<A>(ls: *mut LexState<A>, fs: *mut FuncState<A>) -> Result<(), ParseError> {
    let mut toclose: c_int = -(1 as c_int);
    let mut vidx: c_int = 0;
    let mut kind: c_int = 0;
    let mut nvars: c_int = 0 as c_int;
    let mut nexps: c_int = 0;
    let mut e = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    loop {
        vidx = new_localvar(ls, fs, str_checkname(ls)?)?;
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
        nexps = explist(ls, fs, &mut e)?;
    } else {
        e.k = VVOID;
        nexps = 0 as c_int;
    }
    let var = getlocalvardesc(ls, fs, vidx);
    if nvars == nexps
        && (*var).vd.kind as c_int == 1 as c_int
        && luaK_exp2const(ls, &mut e, &mut (*var).k) != 0
    {
        (*var).vd.kind = 3 as c_int as u8;
        adjustlocalvars(ls, fs, nvars - 1 as c_int)?;
        (*fs).nactvar = ((*fs).nactvar).wrapping_add(1);
        (*fs).nactvar;
    } else {
        adjust_assign(ls, fs, nvars, nexps, &mut e)?;
        adjustlocalvars(ls, fs, nvars)?;
    }
    checktoclose(ls, fs, toclose)?;
    Ok(())
}

unsafe fn funcname<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
    v: *mut expdesc<A>,
) -> Result<c_int, ParseError> {
    let mut ismethod: c_int = 0 as c_int;

    singlevar(ls, fs, v)?;

    while (*ls).t.token == '.' as i32 {
        fieldsel(ls, fs, v)?;
    }
    if (*ls).t.token == ':' as i32 {
        ismethod = 1 as c_int;
        fieldsel(ls, fs, v)?;
    }
    return Ok(ismethod);
}

unsafe fn funcstat<A>(
    ls: *mut LexState<A>,
    fs: *mut FuncState<A>,
    line: c_int,
) -> Result<(), ParseError> {
    let mut ismethod: c_int = 0;
    let mut v = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    let mut b = expdesc {
        k: VVOID,
        u: C2RustUnnamed_11 { ival: 0 },
        t: 0,
        f: 0,
    };
    luaX_next(ls)?;

    ismethod = funcname(ls, fs, &mut v)?;

    body(ls, fs, &mut b, ismethod, line)?;
    check_readonly(ls, fs, &mut v)?;
    luaK_storevar(ls, fs, &mut v, &mut b)?;
    luaK_fixline(ls, fs, line)
}

unsafe fn exprstat<A>(ls: *mut LexState<A>, fs: *mut FuncState<A>) -> Result<(), ParseError> {
    let mut v = LHS_assign {
        prev: null_mut(),
        v: expdesc {
            k: VVOID,
            u: C2RustUnnamed_11 { ival: 0 },
            t: 0,
            f: 0,
        },
    };
    suffixedexp(ls, fs, &mut v.v)?;
    if (*ls).t.token == '=' as i32 || (*ls).t.token == ',' as i32 {
        v.prev = null_mut();
        restassign(ls, fs, &mut v, 1 as c_int)?;
    } else {
        let mut inst: *mut u32 = 0 as *mut u32;
        if !(v.v.k as c_uint == VCALL as c_int as c_uint) {
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

unsafe fn retstat<A>(ls: *mut LexState<A>, fs: *mut FuncState<A>) -> Result<(), ParseError> {
    let mut e = expdesc {
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
        nret = explist(ls, fs, &mut e)?;
        if e.k as c_uint == VCALL as c_int as c_uint || e.k as c_uint == VVARARG as c_int as c_uint
        {
            luaK_setreturns(ls, fs, &mut e, -(1 as c_int))?;
            if e.k as c_uint == VCALL as c_int as c_uint
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

unsafe fn statement<A>(ls: *mut LexState<A>, fs: *mut FuncState<A>) -> Result<(), ParseError> {
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
        266 => ifstat(ls, fs, line)?,
        277 => whilestat(ls, fs, line)?,
        258 => {
            luaX_next(ls)?;
            block(ls, fs)?;
            check_match(ls, TK_END as c_int, TK_DO as c_int, line)?;
        }
        263 => forstat(ls, fs, line)?,
        272 => repeatstat(ls, fs, line)?,
        264 => funcstat(ls, fs, line)?,
        TK_LOCAL => {
            luaX_next(ls)?;
            if testnext(ls, TK_FUNCTION as c_int)? != 0 {
                localfunc(ls, fs)?;
            } else {
                localstat(ls, fs)?;
            }
        }
        287 => {
            luaX_next(ls)?;
            labelstat(ls, fs, str_checkname(ls)?, line)?;
        }
        273 => {
            luaX_next(ls)?;
            retstat(ls, fs)?;
        }
        257 => breakstat(ls, fs)?,
        TK_GOTO => {
            luaX_next(ls)?;
            gotostat(ls, fs)?;
        }
        _ => exprstat(ls, fs)?,
    }

    (*fs).freereg = luaY_nvarstack(ls, fs) as u8;
    (*ls).level -= 1;

    Ok(())
}

unsafe fn mainfunc<D>(ls: &mut LexState<D>, fs: &mut FuncState<D>) -> Result<(), ParseError> {
    let mut bl: BlockCnt = BlockCnt {
        previous: 0 as *mut BlockCnt,
        firstlabel: 0,
        firstgoto: 0,
        nactvar: 0,
        upval: 0,
        isloop: 0,
        insidetbc: 0,
    };

    open_func(ls, fs, &mut bl);
    setvararg(ls, fs, 0 as c_int)?;
    let env = allocupvalue(ls, fs)?;
    (*env).instack = 1 as c_int as u8;
    (*env).idx = 0 as c_int as u8;
    (*env).kind = 0 as c_int as u8;
    (*env).name = (*ls).envn;

    if (*(*fs).f).hdr.marked.get() as c_int & (1 as c_int) << 5 as c_int != 0
        && (*(*env).name).hdr.marked.get() as c_int
            & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
            != 0
    {
        (*ls).g.gc.barrier((*fs).f.cast(), (*env).name.cast());
    }

    luaX_next(ls)?;
    statlist(ls, fs)?;
    check(ls, TK_EOS as c_int)?;
    close_func(ls, fs)?;
    Ok(())
}

pub unsafe fn luaY_parser<D>(
    g: &Lua<D>,
    z: *mut ZIO,
    dyd: *mut Dyndata<D>,
    name: Rc<String>,
    firstchar: c_int,
) -> Result<Ref<'_, LuaFn<D>>, ParseError> {
    let mut funcstate = FuncState::default();
    let cl = Ref::new(luaF_newLclosure(g, 1));
    let mut lexstate = LexState {
        current: 0,
        linenumber: 0,
        lastline: 0,
        t: Token {
            token: 0,
            seminfo: SemInfo {
                r: Float::default(),
            },
        },
        lookahead: Token {
            token: 0,
            seminfo: SemInfo {
                r: Float::default(),
            },
        },
        g,
        z: 0 as *mut ZIO,
        buf: Default::default(),
        h: Ref::new(Table::new(g)),
        dyd: null_mut(),
        chunk: name.clone(),
        envn: null(),
        level: 0,
    };

    (*cl).p.set(luaF_newproto(g, name));
    funcstate.f = (*cl).p.get();

    if (*cl).hdr.marked.get() as c_int & (1 as c_int) << 5 as c_int != 0
        && (*(*cl).p.get()).hdr.marked.get() as c_int
            & ((1 as c_int) << 3 as c_int | (1 as c_int) << 4 as c_int)
            != 0
    {
        g.gc.barrier(&cl.hdr, (*cl).p.get().cast());
    }

    lexstate.dyd = dyd;
    (*dyd).label.n = 0 as c_int;
    (*dyd).gt.n = (*dyd).label.n;
    (*dyd).actvar.n = (*dyd).gt.n;
    luaX_setinput(&mut lexstate, z, firstchar);
    mainfunc(&mut lexstate, &mut funcstate)?;

    return Ok(cl);
}
