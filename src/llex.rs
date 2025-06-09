#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments
)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::gc::luaC_fix;
use crate::lctype::luai_ctype_;
use crate::lmem::luaM_saferealloc_;
use crate::lobject::{luaO_hexavalue, luaO_str2num, luaO_utf8esc};
use crate::lparser::{Dyndata, FuncState};
use crate::lstring::luaS_newlstr;
use crate::lzio::{Mbuffer, ZIO};
use crate::table::{luaH_finishset, luaH_getstr};
use crate::value::{UnsafeValue, UntaggedValue};
use crate::{ChunkInfo, Lua, Node, Object, ParseError, Ref, Str, Table};
use alloc::borrow::Cow;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::ToString;
use core::ffi::CStr;
use core::fmt::Display;
use core::ops::Deref;
use core::pin::Pin;

pub type RESERVED = libc::c_uint;
pub const TK_STRING: RESERVED = 292;
pub const TK_NAME: RESERVED = 291;
pub const TK_INT: RESERVED = 290;
pub const TK_FLT: RESERVED = 289;
pub const TK_EOS: RESERVED = 288;
pub const TK_DBCOLON: RESERVED = 287;
pub const TK_SHR: RESERVED = 286;
pub const TK_SHL: RESERVED = 285;
pub const TK_NE: RESERVED = 284;
pub const TK_LE: RESERVED = 283;
pub const TK_GE: RESERVED = 282;
pub const TK_EQ: RESERVED = 281;
pub const TK_DOTS: RESERVED = 280;
pub const TK_CONCAT: RESERVED = 279;
pub const TK_IDIV: RESERVED = 278;
pub const TK_WHILE: RESERVED = 277;
pub const TK_UNTIL: RESERVED = 276;
pub const TK_TRUE: RESERVED = 275;
pub const TK_THEN: RESERVED = 274;
pub const TK_RETURN: RESERVED = 273;
pub const TK_REPEAT: RESERVED = 272;
pub const TK_OR: RESERVED = 271;
pub const TK_NOT: RESERVED = 270;
pub const TK_NIL: RESERVED = 269;
pub const TK_LOCAL: RESERVED = 268;
pub const TK_IN: RESERVED = 267;
pub const TK_IF: RESERVED = 266;
pub const TK_GOTO: RESERVED = 265;
pub const TK_FUNCTION: RESERVED = 264;
pub const TK_FOR: RESERVED = 263;
pub const TK_FALSE: RESERVED = 262;
pub const TK_END: RESERVED = 261;
pub const TK_ELSEIF: RESERVED = 260;
pub const TK_ELSE: RESERVED = 259;
pub const TK_DO: RESERVED = 258;
pub const TK_BREAK: RESERVED = 257;
pub const TK_AND: RESERVED = 256;

#[derive(Copy, Clone)]
#[repr(C)]
pub union SemInfo {
    pub r: f64,
    pub i: i64,
    pub ts: *const Str,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Token {
    pub token: libc::c_int,
    pub seminfo: SemInfo,
}

pub struct LexState {
    pub current: libc::c_int,
    pub linenumber: libc::c_int,
    pub lastline: libc::c_int,
    pub t: Token,
    pub lookahead: Token,
    pub fs: *mut FuncState,
    pub g: Pin<Rc<Lua>>,
    pub z: *mut ZIO,
    pub buff: *mut Mbuffer,
    pub h: Ref<Table>,
    pub dyd: *mut Dyndata,
    pub source: ChunkInfo,
    pub envn: *const Str,
    pub level: usize,
}

const luaX_tokens: [&str; 37] = [
    "and",
    "break",
    "do",
    "else",
    "elseif",
    "end",
    "false",
    "for",
    "function",
    "goto",
    "if",
    "in",
    "local",
    "nil",
    "not",
    "or",
    "repeat",
    "return",
    "then",
    "true",
    "until",
    "while",
    "//",
    "..",
    "...",
    "==",
    ">=",
    "<=",
    "~=",
    "<<",
    ">>",
    "::",
    "<eof>",
    "<number>",
    "<integer>",
    "<name>",
    "<string>",
];

unsafe fn save(ls: *mut LexState, c: libc::c_int) {
    let b: *mut Mbuffer = (*ls).buff;
    if ((*b).n).wrapping_add(1 as libc::c_int as usize) > (*b).buffsize {
        let newsize = (*b).buffsize * 2 as libc::c_int as usize;

        (*b).buffer = luaM_saferealloc_(
            (*ls).g.deref(),
            (*b).buffer as *mut libc::c_void,
            ((*b).buffsize).wrapping_mul(::core::mem::size_of::<libc::c_char>()),
            newsize.wrapping_mul(::core::mem::size_of::<libc::c_char>()),
        ) as *mut libc::c_char;

        (*b).buffsize = newsize;
    }
    let fresh0 = (*b).n;
    (*b).n = ((*b).n).wrapping_add(1);
    *((*b).buffer).offset(fresh0 as isize) = c as libc::c_char;
}

pub unsafe fn luaX_init(g: *const Lua) {
    let mut i: libc::c_int = 0;
    let e = luaS_newlstr(
        g,
        b"_ENV\0" as *const u8 as *const libc::c_char,
        ::core::mem::size_of::<[libc::c_char; 5]>()
            .wrapping_div(::core::mem::size_of::<libc::c_char>())
            .wrapping_sub(1),
    );
    luaC_fix(&*g, e.cast());
    i = 0 as libc::c_int;
    while i < TK_WHILE as libc::c_int - (255 as libc::c_int + 1 as libc::c_int) + 1 as libc::c_int {
        let ts = luaS_newlstr(
            g,
            luaX_tokens[i as usize].as_ptr().cast(),
            luaX_tokens[i as usize].len(),
        );
        luaC_fix(&*g, ts.cast());
        (*ts).extra.set((i + 1 as libc::c_int) as u8);
        i += 1;
    }
}

pub unsafe fn luaX_token2str(token: libc::c_int) -> Cow<'static, str> {
    if token < 255 as libc::c_int + 1 as libc::c_int {
        if luai_ctype_[(token + 1 as libc::c_int) as usize] as libc::c_int
            & (1 as libc::c_int) << 2 as libc::c_int
            != 0
        {
            return format!("'{}'", char::from_u32(token as _).unwrap()).into();
        } else {
            return format!("'<\\{token}>'").into();
        }
    } else {
        let s = luaX_tokens[(token - (255 as libc::c_int + 1 as libc::c_int)) as usize];
        if token < TK_EOS as libc::c_int {
            return format!("'{s}'").into();
        } else {
            return s.into();
        }
    };
}

unsafe fn txtToken(ls: *mut LexState, token: libc::c_int) -> Cow<'static, str> {
    match token {
        291 | 292 | 289 | 290 => {
            save(ls, '\0' as i32);

            format!(
                "'{}'",
                CStr::from_ptr((*(*ls).buff).buffer).to_string_lossy()
            )
            .into()
        }
        _ => luaX_token2str(token),
    }
}

unsafe fn lexerror(ls: *mut LexState, msg: impl Display, token: libc::c_int) -> ParseError {
    let token = if token != 0 {
        Some(txtToken(ls, token))
    } else {
        None
    };

    ParseError::Source {
        reason: msg.to_string(),
        token,
        line: (*ls).linenumber,
    }
}

pub unsafe fn luaX_syntaxerror(ls: *mut LexState, msg: impl Display) -> ParseError {
    lexerror(ls, msg, (*ls).t.token)
}

pub unsafe fn luaX_newstring(ls: *mut LexState, str: *const libc::c_char, l: usize) -> *const Str {
    let mut ts = luaS_newlstr((*ls).g.deref(), str, l);
    let o: *const UnsafeValue = luaH_getstr((*ls).h.deref(), ts);

    if !((*o).tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int) {
        ts = (*(o as *mut Node)).u.key_val.gc as *mut Str;
    } else {
        let ts = Ref::new((*ls).g.clone(), ts);
        let stv = UnsafeValue {
            value_: UntaggedValue { gc: &ts.hdr },
            tt_: ((*ts).hdr.tt as libc::c_int | (1 as libc::c_int) << 6 as libc::c_int) as u8,
        };

        luaH_finishset((*ls).h.deref(), &stv, o, &stv).unwrap(); // This should never fails.

        if (&(*ls).g).gc.debt() > 0 as libc::c_int as isize {
            crate::gc::step((*ls).g.deref());
        }
    }

    ts
}

unsafe fn inclinenumber(ls: *mut LexState) {
    let old: libc::c_int = (*ls).current;
    let fresh2 = (*(*ls).z).n;
    (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
    (*ls).current = if fresh2 > 0 as libc::c_int as usize {
        let fresh3 = (*(*ls).z).p;
        (*(*ls).z).p = ((*(*ls).z).p).offset(1);
        *fresh3 as libc::c_uchar as libc::c_int
    } else {
        -1
    };
    if ((*ls).current == '\n' as i32 || (*ls).current == '\r' as i32) && (*ls).current != old {
        let fresh4 = (*(*ls).z).n;
        (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
        (*ls).current = if fresh4 > 0 as libc::c_int as usize {
            let fresh5 = (*(*ls).z).p;
            (*(*ls).z).p = ((*(*ls).z).p).offset(1);
            *fresh5 as libc::c_uchar as libc::c_int
        } else {
            -1
        };
    }
    (*ls).linenumber = (*ls).linenumber.checked_add(1).unwrap();
}

pub unsafe fn luaX_setinput(ls: &mut LexState, z: *mut ZIO, firstchar: libc::c_int) {
    (*ls).t.token = 0 as libc::c_int;
    (*ls).current = firstchar;
    (*ls).lookahead.token = TK_EOS as libc::c_int;
    (*ls).z = z;
    (*ls).fs = 0 as *mut FuncState;
    (*ls).linenumber = 1 as libc::c_int;
    (*ls).lastline = 1 as libc::c_int;
    (*ls).envn = luaS_newlstr(
        (*ls).g.deref(),
        b"_ENV\0" as *const u8 as *const libc::c_char,
        ::core::mem::size_of::<[libc::c_char; 5]>()
            .wrapping_div(::core::mem::size_of::<libc::c_char>())
            .wrapping_sub(1),
    );
    (*(*ls).buff).buffer = luaM_saferealloc_(
        (*ls).g.deref(),
        (*(*ls).buff).buffer as *mut libc::c_void,
        ((*(*ls).buff).buffsize).wrapping_mul(::core::mem::size_of::<libc::c_char>()),
        32usize.wrapping_mul(::core::mem::size_of::<libc::c_char>()),
    ) as *mut libc::c_char;
    (*(*ls).buff).buffsize = 32 as libc::c_int as usize;
}

unsafe fn check_next1(ls: *mut LexState, c: libc::c_int) -> libc::c_int {
    if (*ls).current == c {
        let fresh6 = (*(*ls).z).n;
        (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
        (*ls).current = if fresh6 > 0 as libc::c_int as usize {
            let fresh7 = (*(*ls).z).p;
            (*(*ls).z).p = ((*(*ls).z).p).offset(1);
            *fresh7 as libc::c_uchar as libc::c_int
        } else {
            -1
        };
        return 1 as libc::c_int;
    } else {
        return 0 as libc::c_int;
    };
}

unsafe fn check_next2(ls: *mut LexState, set: *const libc::c_char) -> libc::c_int {
    if (*ls).current == *set.offset(0 as libc::c_int as isize) as libc::c_int
        || (*ls).current == *set.offset(1 as libc::c_int as isize) as libc::c_int
    {
        save(ls, (*ls).current);
        let fresh8 = (*(*ls).z).n;
        (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
        (*ls).current = if fresh8 > 0 as libc::c_int as usize {
            let fresh9 = (*(*ls).z).p;
            (*(*ls).z).p = ((*(*ls).z).p).offset(1);
            *fresh9 as libc::c_uchar as libc::c_int
        } else {
            -1
        };
        return 1 as libc::c_int;
    } else {
        return 0 as libc::c_int;
    };
}

unsafe fn read_numeral(
    ls: *mut LexState,
    seminfo: *mut SemInfo,
) -> Result<libc::c_int, ParseError> {
    let mut obj: UnsafeValue = UnsafeValue {
        value_: UntaggedValue {
            gc: 0 as *mut Object,
        },
        tt_: 0,
    };
    let mut expo: *const libc::c_char = b"Ee\0" as *const u8 as *const libc::c_char;
    let first: libc::c_int = (*ls).current;
    save(ls, (*ls).current);
    let fresh10 = (*(*ls).z).n;
    (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
    (*ls).current = if fresh10 > 0 as libc::c_int as usize {
        let fresh11 = (*(*ls).z).p;
        (*(*ls).z).p = ((*(*ls).z).p).offset(1);
        *fresh11 as libc::c_uchar as libc::c_int
    } else {
        -1
    };
    if first == '0' as i32 && check_next2(ls, b"xX\0" as *const u8 as *const libc::c_char) != 0 {
        expo = b"Pp\0" as *const u8 as *const libc::c_char;
    }
    loop {
        if check_next2(ls, expo) != 0 {
            check_next2(ls, b"-+\0" as *const u8 as *const libc::c_char);
        } else {
            if !(luai_ctype_[((*ls).current + 1 as libc::c_int) as usize] as libc::c_int
                & (1 as libc::c_int) << 4 as libc::c_int
                != 0
                || (*ls).current == '.' as i32)
            {
                break;
            }
            save(ls, (*ls).current);
            let fresh12 = (*(*ls).z).n;
            (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
            (*ls).current = if fresh12 > 0 as libc::c_int as usize {
                let fresh13 = (*(*ls).z).p;
                (*(*ls).z).p = ((*(*ls).z).p).offset(1);
                *fresh13 as libc::c_uchar as libc::c_int
            } else {
                -1
            };
        }
    }
    if luai_ctype_[((*ls).current + 1 as libc::c_int) as usize] as libc::c_int
        & (1 as libc::c_int) << 0 as libc::c_int
        != 0
    {
        save(ls, (*ls).current);
        let fresh14 = (*(*ls).z).n;
        (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
        (*ls).current = if fresh14 > 0 as libc::c_int as usize {
            let fresh15 = (*(*ls).z).p;
            (*(*ls).z).p = ((*(*ls).z).p).offset(1);
            *fresh15 as libc::c_uchar as libc::c_int
        } else {
            -1
        };
    }
    save(ls, '\0' as i32);
    if luaO_str2num((*(*ls).buff).buffer, &mut obj) == 0 as libc::c_int as usize {
        return Err(lexerror(ls, "malformed number", TK_FLT as libc::c_int));
    }
    if obj.tt_ as libc::c_int == 3 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int {
        (*seminfo).i = obj.value_.i;
        return Ok(TK_INT as libc::c_int);
    } else {
        (*seminfo).r = obj.value_.n;
        return Ok(TK_FLT as libc::c_int);
    };
}

unsafe fn skip_sep(ls: *mut LexState) -> usize {
    let mut count: usize = 0 as libc::c_int as usize;
    let s: libc::c_int = (*ls).current;
    save(ls, (*ls).current);
    let fresh16 = (*(*ls).z).n;
    (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
    (*ls).current = if fresh16 > 0 as libc::c_int as usize {
        let fresh17 = (*(*ls).z).p;
        (*(*ls).z).p = ((*(*ls).z).p).offset(1);
        *fresh17 as libc::c_uchar as libc::c_int
    } else {
        -1
    };
    while (*ls).current == '=' as i32 {
        save(ls, (*ls).current);
        let fresh18 = (*(*ls).z).n;
        (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
        (*ls).current = if fresh18 > 0 as libc::c_int as usize {
            let fresh19 = (*(*ls).z).p;
            (*(*ls).z).p = ((*(*ls).z).p).offset(1);
            *fresh19 as libc::c_uchar as libc::c_int
        } else {
            -1
        };
        count = count.wrapping_add(1);
    }
    return if (*ls).current == s {
        count.wrapping_add(2 as libc::c_int as usize)
    } else {
        (if count == 0 as libc::c_int as usize {
            1 as libc::c_int
        } else {
            0 as libc::c_int
        }) as usize
    };
}

unsafe fn read_long_string(
    ls: *mut LexState,
    seminfo: *mut SemInfo,
    sep: usize,
) -> Result<(), ParseError> {
    let line: libc::c_int = (*ls).linenumber;
    save(ls, (*ls).current);
    let fresh20 = (*(*ls).z).n;
    (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
    (*ls).current = if fresh20 > 0 as libc::c_int as usize {
        let fresh21 = (*(*ls).z).p;
        (*(*ls).z).p = ((*(*ls).z).p).offset(1);
        *fresh21 as libc::c_uchar as libc::c_int
    } else {
        -1
    };
    if (*ls).current == '\n' as i32 || (*ls).current == '\r' as i32 {
        inclinenumber(ls);
    }
    loop {
        match (*ls).current {
            -1 => {
                let what = if !seminfo.is_null() {
                    "string"
                } else {
                    "comment"
                };

                return Err(lexerror(
                    ls,
                    format_args!("unfinished long {what} (starting at line {line})"),
                    TK_EOS as libc::c_int,
                ));
            }
            93 => {
                if !(skip_sep(ls) == sep) {
                    continue;
                }
                save(ls, (*ls).current);
                let fresh22 = (*(*ls).z).n;
                (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
                (*ls).current = if fresh22 > 0 as libc::c_int as usize {
                    let fresh23 = (*(*ls).z).p;
                    (*(*ls).z).p = ((*(*ls).z).p).offset(1);
                    *fresh23 as libc::c_uchar as libc::c_int
                } else {
                    -1
                };
                break;
            }
            10 | 13 => {
                save(ls, '\n' as i32);
                inclinenumber(ls);
                if seminfo.is_null() {
                    (*(*ls).buff).n = 0 as libc::c_int as usize;
                }
            }
            _ => {
                if !seminfo.is_null() {
                    save(ls, (*ls).current);
                    let fresh24 = (*(*ls).z).n;
                    (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
                    (*ls).current = if fresh24 > 0 as libc::c_int as usize {
                        let fresh25 = (*(*ls).z).p;
                        (*(*ls).z).p = ((*(*ls).z).p).offset(1);
                        *fresh25 as libc::c_uchar as libc::c_int
                    } else {
                        -1
                    };
                } else {
                    let fresh26 = (*(*ls).z).n;
                    (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
                    (*ls).current = if fresh26 > 0 as libc::c_int as usize {
                        let fresh27 = (*(*ls).z).p;
                        (*(*ls).z).p = ((*(*ls).z).p).offset(1);
                        *fresh27 as libc::c_uchar as libc::c_int
                    } else {
                        -1
                    };
                }
            }
        }
    }
    if !seminfo.is_null() {
        (*seminfo).ts = luaX_newstring(
            ls,
            ((*(*ls).buff).buffer).offset(sep as isize),
            ((*(*ls).buff).n).wrapping_sub(2 as libc::c_int as usize * sep),
        );
    }

    Ok(())
}

unsafe fn esccheck(ls: *mut LexState, c: libc::c_int, msg: impl Display) -> Result<(), ParseError> {
    if c == 0 {
        if (*ls).current != -(1 as libc::c_int) {
            save(ls, (*ls).current);
            let fresh28 = (*(*ls).z).n;
            (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
            (*ls).current = if fresh28 > 0 as libc::c_int as usize {
                let fresh29 = (*(*ls).z).p;
                (*(*ls).z).p = ((*(*ls).z).p).offset(1);
                *fresh29 as libc::c_uchar as libc::c_int
            } else {
                -1
            };
        }

        return Err(lexerror(ls, msg, TK_STRING as libc::c_int));
    }
    Ok(())
}

unsafe fn gethexa(ls: *mut LexState) -> Result<libc::c_int, ParseError> {
    save(ls, (*ls).current);
    let fresh30 = (*(*ls).z).n;
    (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
    (*ls).current = if fresh30 > 0 as libc::c_int as usize {
        let fresh31 = (*(*ls).z).p;
        (*(*ls).z).p = ((*(*ls).z).p).offset(1);
        *fresh31 as libc::c_uchar as libc::c_int
    } else {
        -1
    };
    esccheck(
        ls,
        luai_ctype_[((*ls).current + 1 as libc::c_int) as usize] as libc::c_int
            & (1 as libc::c_int) << 4 as libc::c_int,
        "hexadecimal digit expected",
    )?;
    return Ok(luaO_hexavalue((*ls).current));
}

unsafe fn readhexaesc(ls: *mut LexState) -> Result<libc::c_int, ParseError> {
    let mut r: libc::c_int = gethexa(ls)?;
    r = (r << 4 as libc::c_int) + gethexa(ls)?;
    (*(*ls).buff).n = ((*(*ls).buff).n).wrapping_sub(2 as libc::c_int as usize);
    return Ok(r);
}

unsafe fn readutf8esc(ls: *mut LexState) -> Result<libc::c_ulong, ParseError> {
    let mut r: libc::c_ulong = 0;
    let mut i: libc::c_int = 4 as libc::c_int;
    save(ls, (*ls).current);
    let fresh32 = (*(*ls).z).n;
    (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
    (*ls).current = if fresh32 > 0 as libc::c_int as usize {
        let fresh33 = (*(*ls).z).p;
        (*(*ls).z).p = ((*(*ls).z).p).offset(1);
        *fresh33 as libc::c_uchar as libc::c_int
    } else {
        -1
    };
    esccheck(
        ls,
        ((*ls).current == '{' as i32) as libc::c_int,
        "missing '{'",
    )?;
    r = gethexa(ls)? as libc::c_ulong;
    loop {
        save(ls, (*ls).current);
        let fresh34 = (*(*ls).z).n;
        (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
        (*ls).current = if fresh34 > 0 as libc::c_int as usize {
            let fresh35 = (*(*ls).z).p;
            (*(*ls).z).p = ((*(*ls).z).p).offset(1);
            *fresh35 as libc::c_uchar as libc::c_int
        } else {
            -1
        };
        if !(luai_ctype_[((*ls).current + 1 as libc::c_int) as usize] as libc::c_int
            & (1 as libc::c_int) << 4 as libc::c_int
            != 0)
        {
            break;
        }
        i += 1;
        esccheck(
            ls,
            (r <= (0x7fffffff as libc::c_uint >> 4 as libc::c_int) as libc::c_ulong) as libc::c_int,
            "UTF-8 value too large",
        )?;
        r = (r << 4 as libc::c_int).wrapping_add(luaO_hexavalue((*ls).current) as libc::c_ulong);
    }
    esccheck(
        ls,
        ((*ls).current == '}' as i32) as libc::c_int,
        "missing '}'",
    )?;
    let fresh36 = (*(*ls).z).n;
    (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
    (*ls).current = if fresh36 > 0 as libc::c_int as usize {
        let fresh37 = (*(*ls).z).p;
        (*(*ls).z).p = ((*(*ls).z).p).offset(1);
        *fresh37 as libc::c_uchar as libc::c_int
    } else {
        -1
    };
    (*(*ls).buff).n = ((*(*ls).buff).n).wrapping_sub(i as usize);
    return Ok(r);
}

unsafe fn utf8esc(ls: *mut LexState) -> Result<(), ParseError> {
    let mut buff: [libc::c_char; 8] = [0; 8];
    let mut n: libc::c_int = luaO_utf8esc(buff.as_mut_ptr(), readutf8esc(ls)?);
    while n > 0 as libc::c_int {
        save(ls, buff[(8 as libc::c_int - n) as usize] as libc::c_int);
        n -= 1;
    }
    Ok(())
}

unsafe fn readdecesc(ls: *mut LexState) -> Result<libc::c_int, ParseError> {
    let mut i: libc::c_int = 0;
    let mut r: libc::c_int = 0 as libc::c_int;
    i = 0 as libc::c_int;
    while i < 3 as libc::c_int
        && luai_ctype_[((*ls).current + 1 as libc::c_int) as usize] as libc::c_int
            & (1 as libc::c_int) << 1 as libc::c_int
            != 0
    {
        r = 10 as libc::c_int * r + (*ls).current - '0' as i32;
        save(ls, (*ls).current);
        let fresh38 = (*(*ls).z).n;
        (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
        (*ls).current = if fresh38 > 0 as libc::c_int as usize {
            let fresh39 = (*(*ls).z).p;
            (*(*ls).z).p = ((*(*ls).z).p).offset(1);
            *fresh39 as libc::c_uchar as libc::c_int
        } else {
            -1
        };
        i += 1;
    }
    esccheck(
        ls,
        (r <= 255 as libc::c_int) as libc::c_int,
        "decimal escape too large",
    )?;
    (*(*ls).buff).n = ((*(*ls).buff).n).wrapping_sub(i as usize);
    return Ok(r);
}

unsafe fn read_string(
    ls: *mut LexState,
    del: libc::c_int,
    seminfo: *mut SemInfo,
) -> Result<(), ParseError> {
    let mut current_block: u64;
    save(ls, (*ls).current);
    let fresh40 = (*(*ls).z).n;
    (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
    (*ls).current = if fresh40 > 0 as libc::c_int as usize {
        let fresh41 = (*(*ls).z).p;
        (*(*ls).z).p = ((*(*ls).z).p).offset(1);
        *fresh41 as libc::c_uchar as libc::c_int
    } else {
        -1
    };
    while (*ls).current != del {
        match (*ls).current {
            -1 => return Err(lexerror(ls, "unfinished string", TK_EOS as libc::c_int)),
            10 | 13 => return Err(lexerror(ls, "unfinished string", TK_STRING as libc::c_int)),
            92 => {
                let mut c: libc::c_int = 0;
                save(ls, (*ls).current);
                let fresh42 = (*(*ls).z).n;
                (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
                (*ls).current = if fresh42 > 0 as libc::c_int as usize {
                    let fresh43 = (*(*ls).z).p;
                    (*(*ls).z).p = ((*(*ls).z).p).offset(1);
                    *fresh43 as libc::c_uchar as libc::c_int
                } else {
                    -1
                };
                match (*ls).current {
                    97 => {
                        c = '\u{7}' as i32;
                        current_block = 1042351266007549468;
                    }
                    98 => {
                        c = '\u{8}' as i32;
                        current_block = 1042351266007549468;
                    }
                    102 => {
                        c = '\u{c}' as i32;
                        current_block = 1042351266007549468;
                    }
                    110 => {
                        c = '\n' as i32;
                        current_block = 1042351266007549468;
                    }
                    114 => {
                        c = '\r' as i32;
                        current_block = 1042351266007549468;
                    }
                    116 => {
                        c = '\t' as i32;
                        current_block = 1042351266007549468;
                    }
                    118 => {
                        c = '\u{b}' as i32;
                        current_block = 1042351266007549468;
                    }
                    120 => {
                        c = readhexaesc(ls)?;
                        current_block = 1042351266007549468;
                    }
                    117 => {
                        utf8esc(ls)?;
                        continue;
                    }
                    10 | 13 => {
                        inclinenumber(ls);
                        c = '\n' as i32;
                        current_block = 1600088076184679856;
                    }
                    92 | 34 | 39 => {
                        c = (*ls).current;
                        current_block = 1042351266007549468;
                    }
                    -1 => {
                        continue;
                    }
                    122 => {
                        (*(*ls).buff).n = ((*(*ls).buff).n).wrapping_sub(1 as libc::c_int as usize);
                        let fresh44 = (*(*ls).z).n;
                        (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
                        (*ls).current = if fresh44 > 0 as libc::c_int as usize {
                            let fresh45 = (*(*ls).z).p;
                            (*(*ls).z).p = ((*(*ls).z).p).offset(1);
                            *fresh45 as libc::c_uchar as libc::c_int
                        } else {
                            -1
                        };
                        while luai_ctype_[((*ls).current + 1 as libc::c_int) as usize]
                            as libc::c_int
                            & (1 as libc::c_int) << 3 as libc::c_int
                            != 0
                        {
                            if (*ls).current == '\n' as i32 || (*ls).current == '\r' as i32 {
                                inclinenumber(ls);
                            } else {
                                let fresh46 = (*(*ls).z).n;
                                (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
                                (*ls).current = if fresh46 > 0 as libc::c_int as usize {
                                    let fresh47 = (*(*ls).z).p;
                                    (*(*ls).z).p = ((*(*ls).z).p).offset(1);
                                    *fresh47 as libc::c_uchar as libc::c_int
                                } else {
                                    -1
                                };
                            }
                        }
                        continue;
                    }
                    _ => {
                        esccheck(
                            ls,
                            luai_ctype_[((*ls).current + 1 as libc::c_int) as usize] as libc::c_int
                                & (1 as libc::c_int) << 1 as libc::c_int,
                            "invalid escape sequence",
                        )?;
                        c = readdecesc(ls)?;
                        current_block = 1600088076184679856;
                    }
                }
                match current_block {
                    1042351266007549468 => {
                        let fresh48 = (*(*ls).z).n;
                        (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
                        (*ls).current = if fresh48 > 0 as libc::c_int as usize {
                            let fresh49 = (*(*ls).z).p;
                            (*(*ls).z).p = ((*(*ls).z).p).offset(1);
                            *fresh49 as libc::c_uchar as libc::c_int
                        } else {
                            -1
                        };
                    }
                    _ => {}
                }
                (*(*ls).buff).n = ((*(*ls).buff).n).wrapping_sub(1 as libc::c_int as usize);
                save(ls, c);
            }
            _ => {
                save(ls, (*ls).current);
                let fresh50 = (*(*ls).z).n;
                (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
                (*ls).current = if fresh50 > 0 as libc::c_int as usize {
                    let fresh51 = (*(*ls).z).p;
                    (*(*ls).z).p = ((*(*ls).z).p).offset(1);
                    *fresh51 as libc::c_uchar as libc::c_int
                } else {
                    -1
                };
            }
        }
    }
    save(ls, (*ls).current);
    let fresh52 = (*(*ls).z).n;
    (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
    (*ls).current = if fresh52 > 0 as libc::c_int as usize {
        let fresh53 = (*(*ls).z).p;
        (*(*ls).z).p = ((*(*ls).z).p).offset(1);
        *fresh53 as libc::c_uchar as libc::c_int
    } else {
        -1
    };
    (*seminfo).ts = luaX_newstring(
        ls,
        ((*(*ls).buff).buffer).offset(1 as libc::c_int as isize),
        ((*(*ls).buff).n).wrapping_sub(2 as libc::c_int as usize),
    );
    Ok(())
}

unsafe fn llex(ls: *mut LexState, seminfo: *mut SemInfo) -> Result<libc::c_int, ParseError> {
    (*(*ls).buff).n = 0 as libc::c_int as usize;
    loop {
        let current_block_85: u64;
        match (*ls).current {
            10 | 13 => inclinenumber(ls),
            32 | 12 | 9 | 11 => {
                let fresh54 = (*(*ls).z).n;
                (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
                (*ls).current = if fresh54 > 0 as libc::c_int as usize {
                    let fresh55 = (*(*ls).z).p;
                    (*(*ls).z).p = ((*(*ls).z).p).offset(1);
                    *fresh55 as libc::c_uchar as libc::c_int
                } else {
                    -1
                };
            }
            45 => {
                let fresh56 = (*(*ls).z).n;
                (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
                (*ls).current = if fresh56 > 0 as libc::c_int as usize {
                    let fresh57 = (*(*ls).z).p;
                    (*(*ls).z).p = ((*(*ls).z).p).offset(1);
                    *fresh57 as libc::c_uchar as libc::c_int
                } else {
                    -1
                };
                if (*ls).current != '-' as i32 {
                    return Ok('-' as i32);
                }
                let fresh58 = (*(*ls).z).n;
                (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
                (*ls).current = if fresh58 > 0 as libc::c_int as usize {
                    let fresh59 = (*(*ls).z).p;
                    (*(*ls).z).p = ((*(*ls).z).p).offset(1);
                    *fresh59 as libc::c_uchar as libc::c_int
                } else {
                    -1
                };
                if (*ls).current == '[' as i32 {
                    let sep: usize = skip_sep(ls);
                    (*(*ls).buff).n = 0 as libc::c_int as usize;
                    if sep >= 2 as libc::c_int as usize {
                        read_long_string(ls, 0 as *mut SemInfo, sep)?;
                        (*(*ls).buff).n = 0 as libc::c_int as usize;
                        current_block_85 = 10512632378975961025;
                    } else {
                        current_block_85 = 3512920355445576850;
                    }
                } else {
                    current_block_85 = 3512920355445576850;
                }
                match current_block_85 {
                    10512632378975961025 => {}
                    _ => {
                        while !((*ls).current == '\n' as i32 || (*ls).current == '\r' as i32)
                            && (*ls).current != -(1 as libc::c_int)
                        {
                            let fresh60 = (*(*ls).z).n;
                            (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
                            (*ls).current = if fresh60 > 0 as libc::c_int as usize {
                                let fresh61 = (*(*ls).z).p;
                                (*(*ls).z).p = ((*(*ls).z).p).offset(1);
                                *fresh61 as libc::c_uchar as libc::c_int
                            } else {
                                -1
                            };
                        }
                    }
                }
            }
            91 => {
                let sep_0: usize = skip_sep(ls);
                if sep_0 >= 2 as libc::c_int as usize {
                    read_long_string(ls, seminfo, sep_0)?;
                    return Ok(TK_STRING as libc::c_int);
                } else if sep_0 == 0 as libc::c_int as usize {
                    return Err(lexerror(
                        ls,
                        "invalid long string delimiter",
                        TK_STRING as libc::c_int,
                    ));
                }
                return Ok('[' as i32);
            }
            61 => {
                let fresh62 = (*(*ls).z).n;
                (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
                (*ls).current = if fresh62 > 0 as libc::c_int as usize {
                    let fresh63 = (*(*ls).z).p;
                    (*(*ls).z).p = ((*(*ls).z).p).offset(1);
                    *fresh63 as libc::c_uchar as libc::c_int
                } else {
                    -1
                };
                if check_next1(ls, '=' as i32) != 0 {
                    return Ok(TK_EQ as libc::c_int);
                } else {
                    return Ok('=' as i32);
                }
            }
            60 => {
                let fresh64 = (*(*ls).z).n;
                (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
                (*ls).current = if fresh64 > 0 as libc::c_int as usize {
                    let fresh65 = (*(*ls).z).p;
                    (*(*ls).z).p = ((*(*ls).z).p).offset(1);
                    *fresh65 as libc::c_uchar as libc::c_int
                } else {
                    -1
                };
                if check_next1(ls, '=' as i32) != 0 {
                    return Ok(TK_LE as libc::c_int);
                } else if check_next1(ls, '<' as i32) != 0 {
                    return Ok(TK_SHL as libc::c_int);
                } else {
                    return Ok('<' as i32);
                }
            }
            62 => {
                let fresh66 = (*(*ls).z).n;
                (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
                (*ls).current = if fresh66 > 0 as libc::c_int as usize {
                    let fresh67 = (*(*ls).z).p;
                    (*(*ls).z).p = ((*(*ls).z).p).offset(1);
                    *fresh67 as libc::c_uchar as libc::c_int
                } else {
                    -1
                };
                if check_next1(ls, '=' as i32) != 0 {
                    return Ok(TK_GE as libc::c_int);
                } else if check_next1(ls, '>' as i32) != 0 {
                    return Ok(TK_SHR as libc::c_int);
                } else {
                    return Ok('>' as i32);
                }
            }
            47 => {
                let fresh68 = (*(*ls).z).n;
                (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
                (*ls).current = if fresh68 > 0 as libc::c_int as usize {
                    let fresh69 = (*(*ls).z).p;
                    (*(*ls).z).p = ((*(*ls).z).p).offset(1);
                    *fresh69 as libc::c_uchar as libc::c_int
                } else {
                    -1
                };
                if check_next1(ls, '/' as i32) != 0 {
                    return Ok(TK_IDIV as libc::c_int);
                } else {
                    return Ok('/' as i32);
                }
            }
            126 => {
                let fresh70 = (*(*ls).z).n;
                (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
                (*ls).current = if fresh70 > 0 as libc::c_int as usize {
                    let fresh71 = (*(*ls).z).p;
                    (*(*ls).z).p = ((*(*ls).z).p).offset(1);
                    *fresh71 as libc::c_uchar as libc::c_int
                } else {
                    -1
                };
                if check_next1(ls, '=' as i32) != 0 {
                    return Ok(TK_NE as libc::c_int);
                } else {
                    return Ok('~' as i32);
                }
            }
            58 => {
                let fresh72 = (*(*ls).z).n;
                (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
                (*ls).current = if fresh72 > 0 as libc::c_int as usize {
                    let fresh73 = (*(*ls).z).p;
                    (*(*ls).z).p = ((*(*ls).z).p).offset(1);
                    *fresh73 as libc::c_uchar as libc::c_int
                } else {
                    -1
                };
                if check_next1(ls, ':' as i32) != 0 {
                    return Ok(TK_DBCOLON as libc::c_int);
                } else {
                    return Ok(':' as i32);
                }
            }
            34 | 39 => {
                read_string(ls, (*ls).current, seminfo)?;
                return Ok(TK_STRING as libc::c_int);
            }
            46 => {
                save(ls, (*ls).current);
                let fresh74 = (*(*ls).z).n;
                (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
                (*ls).current = if fresh74 > 0 as libc::c_int as usize {
                    let fresh75 = (*(*ls).z).p;
                    (*(*ls).z).p = ((*(*ls).z).p).offset(1);
                    *fresh75 as libc::c_uchar as libc::c_int
                } else {
                    -1
                };
                if check_next1(ls, '.' as i32) != 0 {
                    if check_next1(ls, '.' as i32) != 0 {
                        return Ok(TK_DOTS as libc::c_int);
                    } else {
                        return Ok(TK_CONCAT as libc::c_int);
                    }
                } else if luai_ctype_[((*ls).current + 1 as libc::c_int) as usize] as libc::c_int
                    & (1 as libc::c_int) << 1 as libc::c_int
                    == 0
                {
                    return Ok('.' as i32);
                } else {
                    return read_numeral(ls, seminfo);
                }
            }
            48 | 49 | 50 | 51 | 52 | 53 | 54 | 55 | 56 | 57 => {
                return read_numeral(ls, seminfo);
            }
            -1 => return Ok(TK_EOS as libc::c_int),
            _ => {
                if luai_ctype_[((*ls).current + 1 as libc::c_int) as usize] as libc::c_int
                    & (1 as libc::c_int) << 0 as libc::c_int
                    != 0
                {
                    loop {
                        save(ls, (*ls).current);
                        let fresh76 = (*(*ls).z).n;
                        (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
                        (*ls).current = if fresh76 > 0 as libc::c_int as usize {
                            let fresh77 = (*(*ls).z).p;
                            (*(*ls).z).p = ((*(*ls).z).p).offset(1);
                            *fresh77 as libc::c_uchar as libc::c_int
                        } else {
                            -1
                        };
                        if !(luai_ctype_[((*ls).current + 1 as libc::c_int) as usize]
                            as libc::c_int
                            & ((1 as libc::c_int) << 0 as libc::c_int
                                | (1 as libc::c_int) << 1 as libc::c_int)
                            != 0)
                        {
                            break;
                        }
                    }

                    let ts = luaX_newstring(ls, (*(*ls).buff).buffer, (*(*ls).buff).n);
                    (*seminfo).ts = ts;
                    if (*ts).hdr.tt as libc::c_int
                        == 4 as libc::c_int | (0 as libc::c_int) << 4 as libc::c_int
                        && (*ts).extra.get() as libc::c_int > 0 as libc::c_int
                    {
                        return Ok((*ts).extra.get() as libc::c_int - 1 as libc::c_int
                            + (255 as libc::c_int + 1 as libc::c_int));
                    } else {
                        return Ok(TK_NAME as libc::c_int);
                    }
                } else {
                    let c: libc::c_int = (*ls).current;
                    let fresh78 = (*(*ls).z).n;
                    (*(*ls).z).n = ((*(*ls).z).n).wrapping_sub(1);
                    (*ls).current = if fresh78 > 0 as libc::c_int as usize {
                        let fresh79 = (*(*ls).z).p;
                        (*(*ls).z).p = ((*(*ls).z).p).offset(1);
                        *fresh79 as libc::c_uchar as libc::c_int
                    } else {
                        -1
                    };
                    return Ok(c);
                }
            }
        }
    }
}

pub unsafe fn luaX_next(ls: *mut LexState) -> Result<(), ParseError> {
    (*ls).lastline = (*ls).linenumber;
    if (*ls).lookahead.token != TK_EOS as libc::c_int {
        (*ls).t = (*ls).lookahead;
        (*ls).lookahead.token = TK_EOS as libc::c_int;
    } else {
        (*ls).t.token = llex(ls, &mut (*ls).t.seminfo)?;
    };
    Ok(())
}

pub unsafe fn luaX_lookahead(ls: *mut LexState) -> Result<libc::c_int, ParseError> {
    (*ls).lookahead.token = llex(ls, &mut (*ls).lookahead.seminfo)?;
    return Ok((*ls).lookahead.token);
}
