use crate::{Args, Context, Ret, Type};
use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use libc::snprintf;

/// Implementation of [string.format](https://www.lua.org/manual/5.4/manual.html#pdf-string.format).
pub fn format(cx: Context<Args>) -> Result<Context<Ret>, Box<dyn core::error::Error>> {
    let mut arg = 1;
    let fmt = cx.arg(arg).get_str()?.as_bytes();
    let mut buf = Vec::with_capacity(fmt.len() * 2);
    let mut iter = fmt.into_iter();
    let mut form = Vec::with_capacity(32);

    while let Some(b) = iter.next().copied() {
        // Check if '%'.
        if b != b'%' {
            buf.push(b);
            continue;
        }

        // Check next character.
        let mut b = match iter.next().copied() {
            Some(b'%') => {
                buf.push(b'%');
                continue;
            }
            v => v,
        };

        // Check if argument exists. The reason we need to do it here is to match with Lua behavior.
        arg += 1;

        if arg > cx.args() {
            return Err(cx.arg(arg).error("no value"));
        }

        // Create null-terminated format.
        form.clear();
        form.push(b'%');

        while let Some(v) = b {
            form.push(v);

            if form.len() >= (32 - 10) {
                return Err("invalid format (too long)".into());
            } else if v.is_ascii_digit()
                || v == b'-'
                || v == b'+'
                || v == b'#'
                || v == b' '
                || v == b'.'
            {
                b = iter.next().copied();
            } else {
                break;
            }
        }

        // Format.
        let arg = cx.arg(arg);
        let mut flags = None::<&[u8]>;
        let mut buff = [0u8; 418];
        let mut nb: libc::c_int = 0 as libc::c_int;

        match b {
            Some(99) => unsafe {
                checkformat(&form, b"-", false)?;

                form.push(0);

                nb = snprintf(
                    buff.as_mut_ptr().cast(),
                    buff.len(),
                    form.as_ptr().cast(),
                    arg.to_int()? as i32,
                );
            },
            Some(100) | Some(105) => flags = Some(b"-+0 "),
            Some(117) => flags = Some(b"-0"),
            Some(111) | Some(120) | Some(88) => flags = Some(b"-#0"),
            Some(b'a') | Some(b'A') => unsafe {
                checkformat(&form, b"-+#0 ", true)?;

                form.push(0);

                nb = snprintf(
                    buff.as_mut_ptr().cast(),
                    buff.len(),
                    form.as_ptr().cast(),
                    arg.to_num()?,
                );
            },
            Some(b'f') | Some(101) | Some(69) | Some(103) | Some(71) => unsafe {
                let n_0 = arg.to_num()?;

                checkformat(&form, b"-+#0 ", true)?;
                form.push(0);

                nb = snprintf(
                    buff.as_mut_ptr().cast(),
                    buff.len(),
                    form.as_ptr().cast(),
                    n_0,
                );
            },
            Some(b'p') => unsafe {
                let mut p = arg.as_ptr();

                checkformat(&form, b"-", false)?;

                if p.is_null() {
                    p = c"(null)".as_ptr().cast();
                    form.pop();
                    form.push(b's');
                }

                form.push(0);

                nb = snprintf(
                    buff.as_mut_ptr().cast(),
                    buff.len(),
                    form.as_ptr().cast(),
                    p,
                );
            },
            Some(b'q') => {
                if form.len() != 2 {
                    return Err("specifier '%q' cannot have modifiers".into());
                }

                match arg.ty() {
                    Some(Type::String) => unsafe {
                        let s = arg.get_str()?;
                        let mut iter = s.as_bytes().iter().copied().peekable();

                        buf.push(b'"');

                        while let Some(b) = iter.next() {
                            if b == b'"' || b == b'\\' || b == b'\n' {
                                buf.push(b'\\');
                                buf.push(b);
                            } else if b.is_ascii_control() {
                                let mut buff = [0; 10];
                                let l = if iter.peek().is_none_or(|&b| !b.is_ascii_digit()) {
                                    snprintf(
                                        buff.as_mut_ptr().cast(),
                                        10,
                                        c"\\%d".as_ptr(),
                                        b as i32,
                                    )
                                } else {
                                    snprintf(
                                        buff.as_mut_ptr().cast(),
                                        10,
                                        c"\\%03d".as_ptr(),
                                        b as i32,
                                    )
                                };

                                buf.extend_from_slice(&buff[..(l as usize)]);
                            } else {
                                buf.push(b);
                            }
                        }

                        buf.push(b'"');
                    },
                    Some(Type::Number) => {
                        let mut buff = [0; 120];
                        let nb = if arg.is_int() == Some(true) {
                            let n = arg.to_int()?;
                            let f = if n == i64::MIN {
                                c"0x%llx".as_ptr()
                            } else {
                                c"%lld".as_ptr()
                            };

                            unsafe { snprintf(buff.as_mut_ptr().cast(), 120, f, n) }
                        } else {
                            let n = arg.to_num()?;
                            let f = if n == f64::INFINITY {
                                c"1e9999".as_ptr()
                            } else if n == -f64::INFINITY {
                                c"-1e9999".as_ptr()
                            } else if n != n {
                                c"(0/0)".as_ptr()
                            } else {
                                c"%a".as_ptr()
                            };

                            unsafe { snprintf(buff.as_mut_ptr().cast(), 120, f, n) }
                        };

                        buf.extend_from_slice(&buff[..nb as usize]);
                    }
                    Some(Type::Nil) | Some(Type::Boolean) => {
                        let s = arg.display()?;

                        buf.extend_from_slice(s.as_bytes());
                    }
                    _ => return Err(arg.error("value has no literal form")),
                }
            }
            Some(b's') => unsafe {
                let s = arg.display()?;
                let v = s.as_bytes();

                if form.len() == 2 {
                    buf.extend_from_slice(v);
                } else if v.contains(&0) {
                    return Err(arg.error("string contains zeros"));
                } else {
                    checkformat(&form, b"-", true)?;

                    form.push(0);

                    if !form.contains(&b'.') && v.len() >= 100 {
                        buf.extend_from_slice(v);
                    } else {
                        nb = snprintf(
                            buff.as_mut_ptr().cast(),
                            buff.len(),
                            form.as_ptr().cast(),
                            s.as_ptr(),
                        );
                    }
                }
            },
            _ => {
                return Err(format!(
                    "invalid conversion '{}' to 'format'",
                    String::from_utf8_lossy(&form)
                )
                .into());
            }
        }

        if let Some(flags) = flags {
            let n = arg.to_int()?;

            checkformat(&form, flags, true)?;

            // Prefix format with ll.
            let f = form.pop();

            form.extend_from_slice(b"ll");

            if let Some(v) = f {
                form.push(v);
            }

            form.push(0);

            nb = unsafe {
                snprintf(
                    buff.as_mut_ptr().cast(),
                    buff.len(),
                    form.as_ptr().cast(),
                    n,
                )
            };
        }

        buf.extend_from_slice(&buff[..nb as usize]);
    }

    // Check if result is UTF-8.
    match String::from_utf8(buf) {
        Ok(v) => cx.push_str(v)?,
        Err(e) => cx.push_bytes(e.into_bytes())?,
    }

    Ok(cx.into())
}

/// Implementation of [string.sub](https://www.lua.org/manual/5.4/manual.html#pdf-string.sub).
pub fn sub(cx: Context<Args>) -> Result<Context<Ret>, Box<dyn core::error::Error>> {
    let s = cx.arg(1).get_str()?.as_bytes();
    let start = cx.arg(2).to_int()?;
    let start = posrelatI(start, s.len().try_into().unwrap());
    let end = cx.arg(3).to_nilable_int(false)?.unwrap_or(-1);
    let end = getendpos(end, s.len().try_into().unwrap());
    let s = if start <= end {
        let start = usize::try_from(start).unwrap();
        let end = usize::try_from(end).unwrap();
        let len = end - start + 1;

        &s[(start - 1)..][..len]
    } else {
        b""
    };

    match core::str::from_utf8(s) {
        Ok(v) => cx.push_str(v)?,
        Err(_) => cx.push_bytes(s)?,
    }

    Ok(cx.into())
}

fn checkformat(
    form: &[u8],
    flags: &[u8],
    precision: bool,
) -> Result<(), Box<dyn core::error::Error>> {
    let mut spec = form[1..].into_iter();
    let mut b = spec.next().copied();

    while let Some(v) = b {
        if !flags.contains(&v) {
            break;
        }

        b = spec.next().copied();
    }

    if let Some(v) = b.filter(|&b| b != b'0') {
        if v.is_ascii_digit() {
            b = spec.next().copied();

            if b.is_some_and(|b| b.is_ascii_digit()) {
                b = spec.next().copied();
            }
        }

        if b.is_some_and(|b| b == b'.') && precision {
            b = spec.next().copied();

            if b.is_some_and(|b| b.is_ascii_digit()) {
                b = spec.next().copied();

                if b.is_some_and(|b| b.is_ascii_digit()) {
                    b = spec.next().copied();
                }
            }
        }
    }

    if b.is_none_or(|b| !b.is_ascii_alphabetic()) {
        return Err(format!(
            "invalid conversion specification: '{}'",
            String::from_utf8_lossy(form)
        )
        .into());
    }

    Ok(())
}

// TODO: Find a better name.
#[allow(non_snake_case)]
fn posrelatI(pos: i64, len: i64) -> u64 {
    if pos > 0 {
        pos as u64
    } else if pos == 0 {
        1
    } else if pos < -len {
        1
    } else {
        (len + pos + 1).try_into().unwrap()
    }
}

fn getendpos(pos: i64, len: i64) -> u64 {
    if pos > len {
        len.try_into().unwrap()
    } else if pos >= 0 {
        pos as u64
    } else if pos < -len {
        0
    } else {
        (len + pos + 1).try_into().unwrap()
    }
}
