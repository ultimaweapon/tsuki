use crate::libc::snprintf;
use crate::{Args, Context, Ret, Type};
use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

/// Implementation of [string.format](https://www.lua.org/manual/5.4/manual.html#pdf-string.format).
pub fn format<D>(cx: Context<D, Args>) -> Result<Context<D, Ret>, Box<dyn core::error::Error>> {
    // Get format.
    let mut arg = 1;
    let fmt = cx.arg(arg);
    let fmt = fmt
        .get_str()?
        .as_str()
        .ok_or_else(|| fmt.error("expect UTF-8 string"))?;

    // Parse format.
    let mut buf = String::with_capacity(fmt.len() * 2);
    let mut iter = fmt.chars();
    let mut form = Vec::with_capacity(32);

    while let Some(ch) = iter.next() {
        // Check if '%'.
        if ch != '%' {
            buf.push(ch);
            continue;
        }

        // Check next character.
        let mut ch = match iter.next() {
            Some('%') => {
                buf.push('%');
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

        while let Some(v) = ch {
            form.extend_from_slice(v.encode_utf8(&mut [0; 4]).as_bytes());

            if form.len() >= (32 - 10) {
                return Err("invalid format (too long)".into());
            } else if v.is_ascii_digit() || v == '-' || v == '+' || v == '#' || v == ' ' || v == '.'
            {
                ch = iter.next();
            } else {
                break;
            }
        }

        // Format.
        let arg = cx.arg(arg);
        let mut flags = None::<&[u8]>;
        let mut buff = [0u8; 418];
        let mut nb = 0;

        match ch {
            Some('c') => unsafe {
                checkformat(&form, b"-", false)?;

                form.push(0);

                nb = snprintf(
                    buff.as_mut_ptr().cast(),
                    buff.len(),
                    form.as_ptr().cast(),
                    arg.to_int()? as i32, // Preserve Lua behavior.
                );
            },
            Some('d') | Some('i') => flags = Some(b"-+0 "),
            Some('u') => flags = Some(b"-0"),
            Some('o') | Some('x') | Some('X') => flags = Some(b"-#0"),
            Some('a') | Some('A') => unsafe {
                checkformat(&form, b"-+#0 ", true)?;

                form.push(0);

                nb = snprintf(
                    buff.as_mut_ptr().cast(),
                    buff.len(),
                    form.as_ptr().cast(),
                    arg.to_float()?,
                );
            },
            Some('f') | Some('e') | Some('E') | Some('g') | Some('G') => unsafe {
                let n_0 = arg.to_float()?;

                checkformat(&form, b"-+#0 ", true)?;
                form.push(0);

                nb = snprintf(
                    buff.as_mut_ptr().cast(),
                    buff.len(),
                    form.as_ptr().cast(),
                    n_0,
                );
            },
            Some('p') => unsafe {
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
            Some('q') => {
                if form.len() != 2 {
                    return Err("specifier '%q' cannot have modifiers".into());
                }

                match arg.ty() {
                    Some(Type::String) => unsafe {
                        let s = arg
                            .get_str()?
                            .as_str()
                            .ok_or_else(|| arg.error("specifier '%q' requires UTF-8 string"))?;
                        let mut iter = s.chars().peekable();

                        buf.push('"');

                        while let Some(ch) = iter.next() {
                            if ch == '"' || ch == '\\' || ch == '\n' {
                                buf.push('\\');
                                buf.push(ch);
                            } else if ch.is_ascii_control() {
                                let mut buff = [0; 10];
                                let l = if iter.peek().is_none_or(|&b| !b.is_ascii_digit()) {
                                    snprintf(
                                        buff.as_mut_ptr().cast(),
                                        10,
                                        c"\\%d".as_ptr(),
                                        ch as i32,
                                    )
                                } else {
                                    snprintf(
                                        buff.as_mut_ptr().cast(),
                                        10,
                                        c"\\%03d".as_ptr(),
                                        ch as i32,
                                    )
                                };

                                buf.push_str(core::str::from_utf8(&buff[..(l as usize)]).unwrap());
                            } else {
                                buf.push(ch);
                            }
                        }

                        buf.push('"');
                    },
                    Some(Type::Number) => {
                        nb = if arg.is_int() == Some(true) {
                            let n = arg.to_int()?;
                            let f = if n == i64::MIN {
                                c"0x%llx".as_ptr()
                            } else {
                                c"%lld".as_ptr()
                            };

                            unsafe { snprintf(buff.as_mut_ptr().cast(), buff.len(), f, n) }
                        } else {
                            let n = arg.to_float()?;
                            let f = if n == f64::INFINITY {
                                c"1e9999".as_ptr()
                            } else if n == -f64::INFINITY {
                                c"-1e9999".as_ptr()
                            } else if n != n {
                                c"(0/0)".as_ptr()
                            } else {
                                c"%a".as_ptr()
                            };

                            unsafe { snprintf(buff.as_mut_ptr().cast(), buff.len(), f, n) }
                        };
                    }
                    Some(Type::Nil) | Some(Type::Boolean) => {
                        // Use display() to honor metatable (if any).
                        let s = arg.display()?;

                        buf.push_str(s.as_str().unwrap());
                    }
                    _ => return Err(arg.error("value has no literal form")),
                }
            }
            Some('s') => unsafe {
                let s = arg.display()?;
                let v = s.as_str().unwrap();

                if form.len() == 2 {
                    buf.push_str(v);
                } else if v.contains('\0') {
                    return Err(arg.error("string contains zeros"));
                } else {
                    checkformat(&form, b"-", true)?;

                    form.push(0);

                    if !form.contains(&b'.') && v.len() >= 100 {
                        buf.push_str(v);
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
                    core::str::from_utf8(&form).unwrap()
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

        buf.push_str(core::str::from_utf8(&buff[..nb as usize]).unwrap());
    }

    cx.push_str(buf)?;

    Ok(cx.into())
}

/// Implementation of [string.sub](https://www.lua.org/manual/5.4/manual.html#pdf-string.sub).
pub fn sub<D>(cx: Context<D, Args>) -> Result<Context<D, Ret>, Box<dyn core::error::Error>> {
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
