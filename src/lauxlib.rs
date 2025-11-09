#![allow(non_camel_case_types, non_snake_case, unused_assignments)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::ldebug::{lua_getinfo, lua_getstack};
use crate::lstate::{CallInfo, lua_Debug};
use crate::value::UnsafeValue;
use crate::vm::luaV_equalobj;
use crate::{Nil, Str, Table, Thread};
use alloc::borrow::Cow;
use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::ffi::{CStr, c_char};
use core::fmt::{Display, Formatter};
use core::num::NonZero;
use libc::strcmp;

type c_int = i32;

unsafe fn findfield<A>(
    names: &mut Vec<*const Str<A>>,
    g: *const Table<A>,
    t: UnsafeValue<A>,
    f: *const UnsafeValue<A>,
    level: usize,
) -> Result<bool, Box<dyn core::error::Error>> {
    let t = if level == 0 || (t.tt_ & 0xf) != 5 {
        return Ok(false);
    } else {
        t.value_.gc.cast::<Table<A>>()
    };

    // Enumerate table.
    let mut key = UnsafeValue::from(Nil);

    while let Some([k, v]) = (*t).next_raw(&key)? {
        key = k;

        // Skip global table.
        if v.tt_ & 1 << 6 != 0 && v.value_.gc.cast() == g {
            continue;
        }

        // Check if string key.
        let name = match k.tt_ & 0xf {
            4 => k.value_.gc.cast::<Str<A>>(),
            _ => continue,
        };

        names.push(name);

        if luaV_equalobj(None, &v, f)? || findfield(names, g, v, f, level - 1)? {
            return Ok(true);
        }

        names.pop();
    }

    Ok(false)
}

unsafe fn pushglobalfuncname<A>(
    L: *const Thread<A>,
    ci: *mut CallInfo,
) -> Result<Vec<u8>, Box<dyn core::error::Error>> {
    // Search global table first so we don't found a global function in _G module.
    let mut names = Vec::with_capacity(2);
    let func = (*L).stack.get().add((*ci).func);
    let g = (*L).hdr.global().global();

    findfield(&mut names, g, g.into(), func.cast(), 1)?;

    if let Some(v) = names.first().copied() {
        return Ok((*v).as_bytes().into());
    }

    // Search function from all modules.
    let t = (*L).hdr.global().modules();

    findfield(&mut names, g, t.into(), func.cast(), 2)?;

    // Build full name.
    let mut buf = Vec::new();
    let mut iter = names.iter().copied();
    let first = match iter.next() {
        Some(v) => (*v).as_bytes(),
        None => return Ok(buf),
    };

    buf.extend_from_slice(first);

    for name in iter {
        buf.push(b'.');
        buf.extend_from_slice((*name).as_bytes());
    }

    Ok(buf)
}

/// `arg` is used only for display.
#[inline(never)]
pub unsafe fn luaL_argerror<D>(
    L: *const Thread<D>,
    mut arg: NonZero<usize>,
    reason: impl Into<Box<dyn core::error::Error>>,
) -> Box<dyn core::error::Error> {
    let mut ar = lua_Debug::default();

    if lua_getstack(L, 0 as c_int, &mut ar) == 0 {
        return Box::new(ArgError {
            message: format!("bad argument #{arg}"),
            reason: reason.into(),
        });
    }

    lua_getinfo(L, b"n\0" as *const u8 as *const c_char, &mut ar);

    if strcmp(ar.namewhat, b"method\0" as *const u8 as *const c_char) == 0 as c_int {
        arg = match NonZero::new(arg.get() - 1) {
            Some(v) => v,
            None => {
                return Box::new(ArgError {
                    message: format!(
                        "calling '{}' on bad self",
                        CStr::from_ptr(ar.name).to_string_lossy()
                    ),
                    reason: reason.into(),
                });
            }
        };
    }

    // Get name.
    let name = match ar.name.is_null() {
        true => match pushglobalfuncname(L, ar.i_ci) {
            Ok(v) if v.is_empty() => b"?".into(),
            Ok(v) => Cow::Owned(v),
            Err(e) => return e,
        },
        false => Cow::Borrowed(CStr::from_ptr(ar.name).to_bytes()),
    };

    Box::new(ArgError {
        message: format!(
            "bad argument #{} to '{}'",
            arg,
            String::from_utf8_lossy(&name)
        ),
        reason: reason.into(),
    })
}

/// Represents an error when argument to Rust function is not valid.
#[derive(Debug)]
struct ArgError {
    message: String,
    reason: Box<dyn core::error::Error>,
}

impl core::error::Error for ArgError {
    #[inline(always)]
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        Some(self.reason.as_ref())
    }
}

impl Display for ArgError {
    #[inline(always)]
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        self.message.fmt(f)
    }
}
