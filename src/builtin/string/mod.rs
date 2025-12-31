//! Implementation of [string library](https://www.lua.org/manual/5.4/manual.html#6.4).
use crate::context::{Arg, Args, Context, Ret};
use crate::{Buffer, Float, Fp, LuaFn, Nil, Number, Str, Table, Type, Value};
use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::cmp::min;
use core::ffi::{c_double, c_float, c_int, c_long, c_short};
use core::fmt::Write;
use core::iter::Peekable;
use core::num::NonZero;
use memchr::memchr;

/// Implementation of `__add` metamethod for string.
pub fn add<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    arith(cx, "__add", |cx, lhs, rhs| {
        cx.push_add(lhs, rhs).map(|_| ())
    })
}

/// Implementation of [string.byte](https://www.lua.org/manual/5.4/manual.html#pdf-string.byte).
pub fn byte<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    let s = cx.arg(1).to_str()?.as_bytes();
    let pi = cx.arg(2).to_nilable_int(false)?.unwrap_or(1);
    let pose = cx.arg(3).to_nilable_int(false)?.unwrap_or(pi);
    let l = s.len() as i64;
    let posi = posrelatI(pi, l);
    let pose = getendpos(pose, l);

    if posi.get() > pose {
        return Ok(cx.into());
    }

    // Reserve stack.
    let posi = posi.get() as usize; // posi guarantee to not exceed the length of string.
    let pose = pose as usize; // Same here.
    let n = pose - posi + 1;

    if cx.reserve(n).is_err() {
        return Err("string slice too long".into());
    }

    for i in 0..n {
        cx.push(s[posi - 1 + i])?;
    }

    Ok(cx.into())
}

/// Implementation of [string.char](https://www.lua.org/manual/5.4/manual.html#pdf-string.char).
pub fn char<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    let n = cx.args();
    let mut b = Vec::with_capacity(n);

    for i in 1..=n {
        let arg = cx.arg(i);
        let val = arg.to_int()? as u64;
        let val = val
            .try_into()
            .map_err(|_| arg.error("value out of range"))?;

        b.push(val);
    }

    cx.push_bytes(b)?;

    Ok(cx.into())
}

/// Implementation of `__div` metamethod for string.
pub fn div<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    arith(cx, "__div", |cx, lhs, rhs| {
        let r = cx.thread().div(lhs, rhs)?;

        cx.push(r)?;

        Ok(())
    })
}

/// Implementation of [string.find](https://www.lua.org/manual/5.4/manual.html#pdf-string.find).
///
/// Note that class `z` is not supported.
pub fn find<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    str_find_aux(cx, true)
}

/// Implementation of [string.format](https://www.lua.org/manual/5.4/manual.html#pdf-string.format).
///
/// The main differences from Lua is:
///
/// - First argument accept only a UTF-8 string.
/// - `a`, `A`, `e`, `E`, `g` and `G` format is not supported.
/// - `q` format requires string value to be UTF-8 and will use decimal notation instead of
///   hexadecimal exponent notation for float.
/// - Format have unlimited length.
pub fn format<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    // Get format.
    let mut next = 1;
    let arg = cx.arg(next);
    let fmt = arg.to_str()?;
    let fmt = fmt
        .as_utf8()
        .ok_or_else(|| arg.error("expect UTF-8 string"))?;

    // Parse format.
    let mut res = Vec::with_capacity(fmt.len() * 2);
    let mut buf = Buffer::default();
    let mut iter = fmt.chars();

    while let Some(ch) = iter.next() {
        // Check if '%'.
        if ch != '%' {
            res.extend_from_slice(ch.encode_utf8(&mut [0; 4]).as_bytes());
            continue;
        }

        // Check if '%%.
        let start = iter.as_str();
        let mut len = 1;
        let mut ch = match iter.next() {
            Some('%') => {
                res.push(b'%');
                continue;
            }
            Some(v) => v,
            None => return Err("invalid conversion '%' to 'format'".into()),
        };

        // Parse flags.
        let mut left = false;
        let mut sign = Sign::Negative;
        let mut alt = false;
        let mut lz = false;

        loop {
            match ch {
                '-' => left = true,
                '+' => sign = Sign::Always,
                ' ' => match sign {
                    Sign::Always => (),
                    Sign::SpaceIfPositive => (),
                    Sign::Negative => sign = Sign::SpaceIfPositive,
                },
                '#' => alt = true,
                '0' => lz = true,
                _ => break,
            };

            ch = match iter.next() {
                Some(v) => v,
                None => return Err(format!("invalid conversion '%{start}' to 'format'").into()),
            };

            len += 1;
        }

        // Parse width. Lua does not support '*'.
        let width = if ch.is_ascii_digit() {
            // The value is never be '0' when we are here.
            let mut v = (ch as u8) - b'0';

            ch = match iter.next() {
                Some(v) => v,
                None => return Err(format!("invalid conversion '%{start}' to 'format'").into()),
            };

            len += 1;

            // Lua limit 2 digits for width.
            if ch.is_ascii_digit() {
                v *= 10;
                v += (ch as u8) - b'0';

                ch = match iter.next() {
                    Some(v) => v,
                    None => return Err(format!("invalid conversion '%{start}' to 'format'").into()),
                };

                len += 1;
            }

            Some(NonZero::new(v).unwrap())
        } else {
            None
        };

        // Parse precision.
        let precision = if ch == '.' {
            ch = match iter.next() {
                Some(v) => v,
                None => return Err(format!("invalid conversion '%{start}' to 'format'").into()),
            };

            len += 1;

            // Lua does not support '*'.
            if ch.is_ascii_digit() {
                let mut p = (ch as u8) - b'0';

                ch = match iter.next() {
                    Some(v) => v,
                    None => return Err(format!("invalid conversion '%{start}' to 'format'").into()),
                };

                len += 1;

                // Lua limit 2 digits for width.
                if ch.is_ascii_digit() {
                    p *= 10;
                    p += (ch as u8) - b'0';

                    ch = match iter.next() {
                        Some(v) => v,
                        None => {
                            return Err(format!("invalid conversion '%{start}' to 'format'").into());
                        }
                    };

                    len += 1;
                }

                Some(p.into())
            } else {
                Some(0)
            }
        } else {
            None
        };

        // Check if argument exists.
        next += 1;

        if next > cx.args() {
            return Err(cx.arg(next).error("no value"));
        }

        // Parse format.
        let arg = cx.arg(next);
        let fmt = &start[..len];

        buf.clear();

        match ch {
            'c' if sign == Sign::Negative && !alt && !lz && precision.is_none() => {
                buf.push(arg.to_int()? as u8);
            }
            'd' | 'i' if !alt => {
                let n = arg.to_int()?;
                let w = width.map(|v| v.get()).unwrap_or(0).into();
                let lz = if left || w == 0 { false } else { lz };

                match precision {
                    Some(p) if p == 0 && n == 0 => {
                        // Produce empty data.
                    }
                    Some(p) => match sign {
                        Sign::Always => {
                            buf.push(if n >= 0 { b'+' } else { b'-' });

                            write!(buf, "{:01$}", n.unsigned_abs(), p).unwrap();
                        }
                        Sign::SpaceIfPositive => {
                            buf.push(if n >= 0 { b' ' } else { b'-' });

                            write!(buf, "{:01$}", n.unsigned_abs(), p).unwrap();
                        }
                        Sign::Negative => {
                            if n < 0 {
                                buf.push(b'-');
                            }

                            write!(buf, "{:01$}", n.unsigned_abs(), p).unwrap();
                        }
                    },
                    None => match (sign, lz) {
                        (Sign::Always, true) => write!(buf, "{:+01$}", n, w).unwrap(),
                        (Sign::Always, false) => write!(buf, "{:+}", n).unwrap(),
                        (Sign::SpaceIfPositive, true) => match n >= 0 {
                            true => write!(buf, " {:01$}", n, w).unwrap(),
                            false => write!(buf, "{:01$}", n, w).unwrap(),
                        },
                        (Sign::SpaceIfPositive, false) => match n >= 0 {
                            true => write!(buf, " {}", n).unwrap(),
                            false => write!(buf, "{}", n).unwrap(),
                        },
                        (Sign::Negative, true) => write!(buf, "{:01$}", n, w).unwrap(),
                        (Sign::Negative, false) => write!(buf, "{}", n).unwrap(),
                    },
                }
            }
            'u' if sign == Sign::Negative && !alt => {
                let n = arg.to_int()? as u64;
                let w = width.map(|v| v.get()).unwrap_or(0).into();
                let lz = if left || w == 0 { false } else { lz };

                match precision {
                    Some(p) if p == 0 && n == 0 => {
                        // Produce empty data.
                    }
                    Some(p) => write!(buf, "{:01$}", n, p).unwrap(),
                    None => match lz {
                        true => write!(buf, "{:01$}", n, w).unwrap(),
                        false => write!(buf, "{}", n).unwrap(),
                    },
                }
            }
            'o' if sign == Sign::Negative => {
                let n = arg.to_int()? as u64;
                let w = width.map(|v| v.get()).unwrap_or(0).into();
                let lz = if left || w == 0 { false } else { lz };

                match precision {
                    Some(p) if p == 0 && n == 0 => {
                        if alt {
                            buf.push(b'0');
                        }
                    }
                    Some(p) => write!(buf, "{:01$o}", n, p).unwrap(),
                    None => match lz {
                        true => write!(buf, "{:01$o}", n, w).unwrap(),
                        false => write!(buf, "{:o}", n).unwrap(),
                    },
                }

                if alt && buf[0] != b'0' {
                    buf.insert(0, b'0');
                }
            }
            'x' if sign == Sign::Negative => {
                let n = arg.to_int()? as u64;
                let w = width.map(|v| v.get()).unwrap_or(0).into();
                let lz = if left || w == 0 { false } else { lz };

                if n != 0 && alt {
                    buf.extend_from_slice(b"0x");
                }

                match precision {
                    Some(p) if p == 0 && n == 0 => {
                        // Produce empty data.
                    }
                    Some(p) => write!(buf, "{:01$x}", n, p).unwrap(),
                    None => match lz {
                        true => write!(buf, "{:01$x}", n, w).unwrap(),
                        false => write!(buf, "{:x}", n).unwrap(),
                    },
                }
            }
            'X' if sign == Sign::Negative => {
                let n = arg.to_int()? as u64;
                let w = width.map(|v| v.get()).unwrap_or(0).into();
                let lz = if left || w == 0 { false } else { lz };

                if n != 0 && alt {
                    buf.extend_from_slice(b"0X");
                }

                match precision {
                    Some(p) if p == 0 && n == 0 => {
                        // Produce empty data.
                    }
                    Some(p) => write!(buf, "{:01$X}", n, p).unwrap(),
                    None => match lz {
                        true => write!(buf, "{:01$X}", n, w).unwrap(),
                        false => write!(buf, "{:X}", n).unwrap(),
                    },
                }
            }
            'f' => {
                let Float(n) = arg.to_float()?;
                let p = precision.unwrap_or(6);
                let w = width.map(|v| v.get()).unwrap_or(0).into();
                let lz = if left || w == 0 { false } else { lz };

                match (sign, lz) {
                    (Sign::Always, true) => write!(buf, "{:+02$.1$}", n, p, w).unwrap(),
                    (Sign::Always, false) => write!(buf, "{:+.1$}", n, p).unwrap(),
                    (Sign::SpaceIfPositive, true) => {
                        if n >= 0.0 {
                            buf.push(b' ');
                        }

                        write!(buf, "{:02$.1$}", n, p, w).unwrap()
                    }
                    (Sign::SpaceIfPositive, false) => {
                        if n >= 0.0 {
                            buf.push(b' ');
                        }

                        write!(buf, "{:.1$}", n, p).unwrap()
                    }
                    (Sign::Negative, true) => write!(buf, "{:02$.1$}", n, p, w).unwrap(),
                    (Sign::Negative, false) => write!(buf, "{:.1$}", n, p).unwrap(),
                }

                if alt {}
            }
            'p' if sign == Sign::Negative && !alt && !lz && precision.is_none() => {
                let p = arg.as_ptr();

                match p.is_null() {
                    true => buf.extend_from_slice(b"(null)"),
                    false => write!(buf, "{:p}", p).unwrap(),
                }
            }
            'q' => {
                if len != 1 {
                    return Err("specifier '%q' cannot have modifiers".into());
                }

                match arg.ty() {
                    Some(Type::String) => {
                        let s = arg
                            .get_str()?
                            .as_utf8()
                            .ok_or_else(|| arg.error("specifier '%q' requires UTF-8 string"))?;
                        let mut iter = s.chars().peekable();

                        buf.push(b'"');

                        while let Some(ch) = iter.next() {
                            if ch == '"' || ch == '\\' || ch == '\n' {
                                buf.push(b'\\');
                                buf.extend_from_slice(ch.encode_utf8(&mut [0; 4]).as_bytes());
                            } else if ch.is_ascii_control() {
                                if iter.peek().is_none_or(|&b| !b.is_ascii_digit()) {
                                    write!(buf, "\\{}", ch as i32).unwrap();
                                } else {
                                    write!(buf, "\\{:03}", ch as i32).unwrap();
                                }
                            } else {
                                buf.extend_from_slice(ch.encode_utf8(&mut [0; 4]).as_bytes());
                            }
                        }

                        buf.push(b'"');
                    }
                    Some(Type::Number) => match arg.as_int(false) {
                        Some(n) if n == i64::MIN => write!(buf, "{:#x}", n).unwrap(),
                        Some(n) => write!(buf, "{}", n).unwrap(),
                        None => {
                            let Float(n) = arg.to_float()?;

                            if n == f64::INFINITY {
                                buf.extend_from_slice(b"1e9999");
                            } else if n == -f64::INFINITY {
                                buf.extend_from_slice(b"-1e9999");
                            } else if n != n {
                                buf.extend_from_slice(b"(0/0)");
                            } else {
                                write!(buf, "{}", n).unwrap();
                            }
                        }
                    },
                    Some(Type::Nil) | Some(Type::Boolean) => {
                        // Use display() to honor metatable (if any).
                        let s = arg.display()?;

                        buf.extend_from_slice(s.as_bytes());
                    }
                    _ => return Err(arg.error("value has no literal form")),
                }
            }
            's' if sign == Sign::Negative && !alt && !lz => {
                let s = arg.display()?;
                let v = s.as_bytes();

                if len == 1 {
                    buf.extend_from_slice(v);
                } else if v.contains(&0) {
                    return Err(arg.error("string contains zeros"));
                } else if let Some(p) = precision {
                    let l = min(p, v.len());

                    buf.extend_from_slice(&v[..l]);
                } else {
                    buf.extend_from_slice(v);
                }
            }
            _ => return Err(format!("invalid conversion '%{fmt}' to 'format'").into()),
        }

        // Apply width.
        match width {
            Some(w) if buf.len() < w.get().into() => match left {
                true => {
                    res.extend_from_slice(&buf);

                    for _ in 0..(usize::from(w.get()) - buf.len()) {
                        res.push(b' ');
                    }
                }
                false => {
                    for _ in 0..(usize::from(w.get()) - buf.len()) {
                        res.push(b' ');
                    }

                    res.extend_from_slice(&buf);
                }
            },
            _ => res.extend_from_slice(&buf),
        }
    }

    cx.push_bytes(res)?;

    Ok(cx.into())
}

/// Implementation of [string.gsub](https://www.lua.org/manual/5.4/manual.html#pdf-string.gsub).
///
/// Note that class `z` is not supported.
pub fn gsub<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    let arg1 = cx.arg(1);
    let src = arg1.to_str()?.as_bytes();
    let mut p = cx.arg(2).to_str()?.as_bytes();
    let tr = cx.arg(3);
    let max_s = cx
        .arg(4)
        .to_nilable_int(false)?
        .unwrap_or((src.len() + 1) as i64);
    let anchor = p.first().copied() == Some(b'^');
    let mut n = 0;
    let mut changed = false;
    let mut b = Vec::new();
    let mut ms = MatchState::prepstate(src);
    let mut off = 0;
    let mut lastmatch = usize::MAX;
    let tr = if let Some(v) = tr.as_str(true) {
        Replacement::Str(v)
    } else if let Some(v) = tr.as_table() {
        Replacement::Table(v)
    } else if let Some(v) = tr.as_fp() {
        Replacement::Fp(v)
    } else if let Some(v) = tr.as_lua_fn() {
        Replacement::LuaFn(v)
    } else {
        return Err(tr.invalid_type("string/function/table"));
    };

    if anchor {
        p = &p[1..];
    }

    while n < max_s {
        ms.reprepstate();

        if let Some(e) = ms.match_0(off, p)?
            && e != lastmatch
        {
            n += 1;
            changed |= ms.add_value(&cx, &mut b, off, e, &tr)?;
            lastmatch = e;
            off = lastmatch;
        } else {
            if !(off < src.len()) {
                break;
            }

            b.push(src[off]);
            off += 1;
        }

        if anchor {
            break;
        }
    }

    if !changed {
        cx.push(arg1)?;
    } else {
        b.extend_from_slice(&src[off..]);
        cx.push_bytes(b)?;
    }

    cx.push(n)?;

    Ok(cx.into())
}

/// Implementation of [string.len](https://www.lua.org/manual/5.4/manual.html#pdf-string.len).
pub fn len<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    let l = cx.arg(1).to_str()?.len();

    cx.push(l as i64)?;

    Ok(cx.into())
}

/// Implementation of [string.lower](https://www.lua.org/manual/5.4/manual.html#pdf-string.lower).
pub fn lower<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    let mut s = cx.arg(1).to_str()?.as_bytes().to_vec();

    for b in &mut s {
        b.make_ascii_lowercase();
    }

    cx.push_bytes(s)?;

    Ok(cx.into())
}

/// Implementation of [string.match](https://www.lua.org/manual/5.4/manual.html#pdf-string.match).
pub fn r#match<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    str_find_aux(cx, false)
}

/// Implementation of `__mul` metamethod for string.
pub fn mul<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    arith(cx, "__mul", |cx, lhs, rhs| {
        let r = cx.thread().mul(lhs, rhs)?;

        cx.push(r)?;

        Ok(())
    })
}

/// Implementation of `__unm` metamethod for string.
pub fn negate<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    arith(cx, "__unm", |cx, v, _| cx.push_neg(v).map(|_| ()))
}

/// Implementation of [string.packsize](https://www.lua.org/manual/5.4/manual.html#pdf-string.packsize).
///
/// Note that first argumebnt must be UTF-8 and format error always report as argument error.
pub fn packsize<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    // Check if UTF-8.
    let arg = cx.arg(1);
    let fmt = arg
        .to_str()?
        .as_utf8()
        .ok_or_else(|| arg.error("expect UTF-8 string"))?;

    // Parse.
    let mut fmt = fmt.chars().take_while(|&c| c != '\0').peekable();
    let mut h = Header::new();
    let mut totalsize = 0;

    while let Some(first) = fmt.next() {
        let mut ntoalign = 0;
        let mut size = match h
            .getdetails(totalsize, first, &mut fmt, &mut ntoalign)
            .map_err(|e| arg.error(e))?
        {
            Pack::Signed(v) => v,
            Pack::Unsigned(v) => v,
            Pack::Float(v) => v,
            Pack::Double(v) => v,
            Pack::Number(v) => v,
            Pack::Char(v) => v,
            Pack::Str(_) => return Err(arg.error("variable-length format")),
            Pack::Padding(v) => v,
            Pack::PadAlign => 0,
            Pack::Nop => 0,
        };

        size += ntoalign;

        if totalsize > 2147483647usize.wrapping_sub(size) {
            return Err(arg.error("format result too large"));
        }

        totalsize = totalsize.wrapping_add(size);
    }

    cx.push(totalsize as i64)?;

    Ok(cx.into())
}

/// Implementation of `__pow` metamethod for string.
pub fn pow<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    arith(cx, "__pow", |cx, lhs, rhs| {
        cx.push_pow(lhs, rhs).map(|_| ())
    })
}

/// Implementation of `__mod` metamethod for string.
pub fn rem<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    arith(cx, "__mod", |cx, lhs, rhs| {
        cx.push_rem(lhs, rhs).map(|_| ())
    })
}

/// Implementation of [string.rep](https://www.lua.org/manual/5.4/manual.html#pdf-string.rep).
pub fn rep<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    // Check n.
    let n = cx.arg(2).to_int()?;

    if n <= 0 {
        cx.push_str("")?;

        return Ok(cx.into());
    }

    // Check length.
    let s = cx.arg(1).to_str()?;
    let sep = cx.arg(3).to_nilable_str(false)?;
    let lsep = sep.map(|v| v.len()).unwrap_or(0);

    if !s
        .len()
        .checked_add(lsep)
        .is_some_and(move |v| v <= (0x7FFFFFFF / n) as usize)
    {
        return Err("resulting string too large".into());
    }

    // Build string.
    let len = n as usize * s.len() + (n - 1) as usize * lsep;

    match (s.as_utf8(), sep.map(|v| v.as_utf8()).unwrap_or(Some(""))) {
        (Some(s), Some(sep)) => {
            let mut b = String::with_capacity(len);

            for _ in 0..(n - 1) {
                b.push_str(s);
                b.push_str(sep);
            }

            b.push_str(s);

            cx.push_str(b)?;
        }
        _ => {
            let s = s.as_bytes();
            let sep = sep.map(|v| v.as_bytes()).unwrap_or(b"");
            let mut b = Vec::with_capacity(len);

            for _ in 0..(n - 1) {
                b.extend_from_slice(s);
                b.extend_from_slice(sep);
            }

            b.extend_from_slice(s);

            cx.push_bytes(b)?;
        }
    }

    Ok(cx.into())
}

/// Implementation of
/// [string.reverse](https://www.lua.org/manual/5.4/manual.html#pdf-string.reverse).
pub fn reverse<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    let mut s = cx.arg(1).to_str()?.as_bytes().to_vec();

    s.reverse();

    cx.push_bytes(s)?;

    Ok(cx.into())
}

/// Implementation of [string.sub](https://www.lua.org/manual/5.4/manual.html#pdf-string.sub).
pub fn sub<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    let s = cx.arg(1).to_str()?;
    let s = s.as_bytes();
    let start = cx.arg(2).to_int()?;
    let start = posrelatI(start, s.len().try_into().unwrap()).get();
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

    cx.push_bytes(s)?;

    Ok(cx.into())
}

/// Implementation of `__sub` metamethod for string.
pub fn subtract<D>(cx: Context<D, Args>) -> Result<Context<D, Ret>, Box<dyn core::error::Error>> {
    arith(cx, "__sub", |cx, lhs, rhs| {
        cx.push_sub(lhs, rhs).map(|_| ())
    })
}

/// Implementation of [string.upper](https://www.lua.org/manual/5.4/manual.html#pdf-string.upper).
pub fn upper<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    let s = cx.arg(1).to_str()?;
    let mut s = s.as_bytes().to_vec();

    for b in &mut s {
        b.make_ascii_uppercase();
    }

    cx.push_bytes(s)?;

    Ok(cx.into())
}

fn arith<'a, A>(
    cx: Context<'a, A, Args>,
    mt: &str,
    f: impl FnOnce(&Context<'a, A, Args>, Number, Number) -> Result<(), Box<dyn core::error::Error>>,
) -> Result<Context<'a, A, Ret>, Box<dyn core::error::Error>> {
    // Get first operand.
    let lhs = cx.arg(1);
    let lhs = match tonum(&lhs) {
        Some(v) => v,
        None => return trymt(cx, mt),
    };

    // Get second operand.
    let rhs = cx.arg(2);
    let rhs = match tonum(&rhs) {
        Some(v) => v,
        None => return trymt(cx, mt),
    };

    f(&cx, lhs, rhs)?;

    Ok(cx.into())
}

fn tonum<A>(arg: &Arg<A>) -> Option<Number> {
    if let Some(v) = arg.as_num() {
        Some(v)
    } else if let Some(v) = arg.as_str(false) {
        v.to_num()
    } else {
        None
    }
}

fn trymt<'a, A>(
    cx: Context<'a, A, Args>,
    name: &str,
) -> Result<Context<'a, A, Ret>, Box<dyn core::error::Error>> {
    // Get metamethod.
    let lhs = cx.arg(1);
    let rhs = cx.arg(2);
    let mt = (rhs.ty() != Some(Type::String)).then(|| rhs.metatable().unwrap());
    let mt = match mt
        .as_ref()
        .and_then(|t| t.as_ref())
        .and_then(|t| match t.get_str_key(name) {
            Value::Nil => None,
            v => Some(v),
        }) {
        Some(v) => v,
        None => {
            let e = format!(
                "attempt to {} a '{}' with a '{}'",
                &name[2..],
                lhs.ty().unwrap(),
                rhs.ty().unwrap()
            );

            return Err(e.into());
        }
    };

    // Prepare to call metamethod.
    cx.push(mt)?;
    cx.push(lhs)?;
    cx.push(rhs)?;

    // Call metamethod.
    let mut cx = match cx.forward(-3) {
        (_, Some(e)) => return Err(e),
        (cx, _) => cx,
    };

    cx.truncate(1);

    Ok(cx)
}

fn str_find_aux<A>(
    cx: Context<A, Args>,
    find: bool,
) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    let s = cx.arg(1).to_str()?;
    let s = s.as_bytes();
    let p = cx.arg(2).to_str()?;
    let mut p = p.as_bytes();
    let init = cx.arg(3).to_nilable_int(false)?.unwrap_or(1);
    let ls = s.len().try_into().unwrap();
    let lp = p.len();
    let init = posrelatI(init, ls).get() - 1;

    if init > ls as u64 {
        cx.push(Nil)?;

        return Ok(cx.into());
    }

    // When we are here init guarantee to fit in usize.
    let mut init = init as usize;

    if find && (cx.arg(4).to_bool() == Some(true) || nospecials(p)) {
        if let Some(i) = memchr::memmem::find(&s[init..], p) {
            let i = init + i;

            cx.push((i + 1) as i64)?;
            cx.push((i + lp) as i64)?;

            return Ok(cx.into());
        }
    } else {
        let mut ms = MatchState::prepstate(s);
        let anchor = p.first().copied() == Some(b'^');

        if anchor {
            p = &p[1..];
        }

        loop {
            ms.reprepstate();

            if let Some(res) = ms.match_0(init, p)? {
                if find {
                    cx.push((init + 1) as i64)?;
                    cx.push(res as i64)?;

                    ms.push_captures(&cx, None, None)?;
                } else {
                    ms.push_captures(&cx, Some(init), Some(res))?;
                }

                return Ok(cx.into());
            }

            let fresh4 = init;

            init += 1;

            if !(fresh4 < s.len() && !anchor) {
                break;
            }
        }
    }

    cx.push(Nil)?;

    Ok(cx.into())
}

fn nospecials(p: &[u8]) -> bool {
    let m = |b| {
        b == b'^'
            || b == b'$'
            || b == b'*'
            || b == b'+'
            || b == b'?'
            || b == b'.'
            || b == b'('
            || b == b'['
            || b == b'%'
            || b == b'-'
    };

    if p.iter().copied().any(m) {
        return false;
    }

    true
}

// TODO: Find a better name.
#[allow(non_snake_case)]
fn posrelatI(pos: i64, len: i64) -> NonZero<u64> {
    let r = if pos > 0 {
        pos as u64
    } else if pos == 0 {
        1
    } else if pos < -len {
        1
    } else {
        (len + pos + 1).try_into().unwrap()
    };

    NonZero::new(r).unwrap()
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

struct MatchState<'a> {
    src: &'a [u8],
    matchdepth: i32,
    capture: Vec<MatchCapture>,
}

impl<'a> MatchState<'a> {
    fn prepstate(s: &'a [u8]) -> Self {
        Self {
            src: s,
            matchdepth: 200,
            capture: Vec::with_capacity(32),
        }
    }

    fn reprepstate(&mut self) {
        self.capture.clear();
    }

    fn match_0(
        &mut self,
        mut off: usize,
        mut p: &[u8],
    ) -> Result<Option<usize>, Box<dyn core::error::Error>> {
        // Check depth.
        if self.matchdepth == 0 {
            return Err("pattern too complex".into());
        }

        self.matchdepth -= 1;

        // Match.
        let current_block: u64;
        let mut ep_0 = 0;
        let mut res = loop {
            // Check first character.
            let first = match p.first().copied() {
                Some(v) => v,
                None => {
                    current_block = 6476622998065200121;
                    break Some(off);
                }
            };

            match first {
                b'(' => {
                    let r = if p.get(1).copied() == Some(b')') {
                        self.start_capture(off, &p[2..], -2)?
                    } else {
                        self.start_capture(off, &p[1..], -1)?
                    };

                    current_block = 6476622998065200121;
                    break r;
                }
                b')' => {
                    let r = self.end_capture(off, &p[1..])?;
                    current_block = 6476622998065200121;
                    break r;
                }
                b'$' => {
                    if !p.get(1).is_some() {
                        let r = if off == self.src.len() {
                            Some(off)
                        } else {
                            None
                        };
                        current_block = 6476622998065200121;
                        break r;
                    }
                }
                b'%' => match p.get(1) {
                    Some(b'b') => match self.matchbalance(off, &p[2..])? {
                        Some(v) => {
                            off = v;
                            p = &p[4..];
                            continue;
                        }
                        None => {
                            current_block = 6476622998065200121;
                            break None;
                        }
                    },
                    Some(b'f') => {
                        p = &p[2..];

                        if p.first().copied().is_none_or(|b| b != b'[') {
                            return Err("missing '[' after '%f' in pattern".into());
                        }

                        let ep = Self::classend(p)?;
                        let previous = if off == 0 { 0 } else { self.src[off - 1] };

                        if !Self::matchbracketclass(previous, p, ep - 1)
                            && Self::matchbracketclass(self.src[off], p, ep - 1)
                        {
                            p = &p[ep..];
                            continue;
                        } else {
                            current_block = 6476622998065200121;
                            break None;
                        }
                    }
                    Some(b'0') | Some(b'1') | Some(b'2') | Some(b'3') | Some(b'4') | Some(b'5')
                    | Some(b'6') | Some(b'7') | Some(b'8') | Some(b'9') => {
                        off = match self.match_capture(off, p[1])? {
                            Some(v) => v,
                            None => {
                                current_block = 6476622998065200121;
                                break None;
                            }
                        };

                        p = &p[2..];
                        continue;
                    }
                    _ => {}
                },
                _ => {}
            }

            ep_0 = Self::classend(p)?;

            if !self.singlematch(off, p, ep_0) {
                if p[ep_0] == b'*' || p[ep_0] == b'?' || p[ep_0] == b'-' {
                    p = &p[(ep_0 + 1)..];
                } else {
                    current_block = 6476622998065200121;
                    break None;
                }
            } else {
                match p.get(ep_0) {
                    Some(b'?') => match self.match_0(off + 1, &p[(ep_0 + 1)..])? {
                        Some(v) => {
                            current_block = 6476622998065200121;
                            break Some(v);
                        }
                        None => p = &p[(ep_0 + 1)..],
                    },
                    Some(b'+') => {
                        current_block = 5161946086944071447;
                        break Some(off + 1);
                    }
                    Some(b'*') => {
                        current_block = 5161946086944071447;
                        break Some(off);
                    }
                    Some(b'-') => {
                        current_block = 6476622998065200121;
                        break self.min_expand(off, p, ep_0)?;
                    }
                    _ => {
                        off += 1;
                        p = &p[ep_0..];
                    }
                }
            }
        };

        match current_block {
            5161946086944071447 => {
                res = self.max_expand(off, p, ep_0)?;
            }
            _ => {}
        }

        self.matchdepth += 1;

        Ok(res)
    }

    fn start_capture(
        &mut self,
        off: usize,
        p: &[u8],
        what: isize,
    ) -> Result<Option<usize>, Box<dyn core::error::Error>> {
        if self.capture.len() >= 32 {
            return Err("too many captures".into());
        }

        self.capture.push(MatchCapture { off, len: what });

        let res = self.match_0(off, p)?;

        if res.is_none() {
            self.capture.pop();
        }

        Ok(res)
    }

    fn end_capture(
        &mut self,
        off: usize,
        p: &[u8],
    ) -> Result<Option<usize>, Box<dyn core::error::Error>> {
        let l = self.capture_to_close()?;

        self.capture[l].len = (off - self.capture[l].off) as isize;

        let res = self.match_0(off, p)?;

        if res.is_none() {
            self.capture[l].len = -1;
        }

        Ok(res)
    }

    fn capture_to_close(&self) -> Result<usize, Box<dyn core::error::Error>> {
        for (l, c) in self.capture.iter().enumerate().rev() {
            if c.len == -1 {
                return Ok(l);
            }
        }

        Err("invalid pattern capture".into())
    }

    fn matchbalance(
        &self,
        mut off: usize,
        p: &[u8],
    ) -> Result<Option<usize>, Box<dyn core::error::Error>> {
        let mut iter = p.iter().copied();
        let first = match iter.next() {
            Some(v) => v,
            None => return Err("malformed pattern (missing arguments to '%b')".into()),
        };

        if self.src[off] != first {
            return Ok(None);
        } else {
            let e = iter.next();
            let mut cont = 1;

            loop {
                off += 1;

                if !(off < self.src.len()) {
                    break;
                }

                if Some(self.src[off]) == e {
                    cont -= 1;

                    if cont == 0 {
                        return Ok(Some(off + 1));
                    }
                } else if self.src[off] == first {
                    cont += 1;
                }
            }
        }

        Ok(None)
    }

    fn classend(p: &[u8]) -> Result<usize, Box<dyn core::error::Error>> {
        let mut p = p.iter();

        match p.next().copied() {
            Some(b'%') => {
                if p.next().is_none() {
                    return Err("malformed pattern (ends with '%')".into());
                }

                Ok(2)
            }
            Some(b'[') => {
                let mut o = 1;

                if p.as_slice().first().copied() == Some(b'^') {
                    p.next();
                    o += 1;
                }

                loop {
                    let fresh2 = match p.next().copied() {
                        Some(v) => v,
                        None => return Err("malformed pattern (missing ']')".into()),
                    };

                    o += 1;

                    if fresh2 == b'%' && !p.as_slice().is_empty() {
                        p.next();
                        o += 1;
                    }

                    if !(p.as_slice().first().copied() != Some(b']')) {
                        break;
                    }
                }

                p.next();
                o += 1;

                Ok(o)
            }
            _ => Ok(1),
        }
    }

    fn singlematch(&self, off: usize, p: &[u8], ep: usize) -> bool {
        let c = match self.src.get(off).copied() {
            Some(v) => v,
            None => return false,
        };

        match p.first().copied() {
            Some(b'.') => true,
            Some(b'%') => Self::match_class(c, p[1]),
            Some(b'[') => Self::matchbracketclass(c, p, ep - 1),
            _ => p[0] == c,
        }
    }

    fn matchbracketclass(c: u8, p: &[u8], ec: usize) -> bool {
        let mut sig = true;
        let mut i = 0;

        if p[1] == b'^' {
            sig = false;
            i = 1;
        }

        loop {
            i += 1;

            if i >= ec {
                break;
            }

            if p[i] == b'%' {
                i += 1;

                if Self::match_class(c, p[i]) {
                    return sig;
                }
            } else if p[i + 1] == b'-' && (i + 2) < ec {
                i += 2;

                if p[i - 2] <= c && c <= p[i] {
                    return sig;
                }
            } else if p[i] == c {
                return sig;
            }
        }

        sig == false
    }

    fn match_class(c: u8, cl: u8) -> bool {
        let res = match cl.to_ascii_lowercase() {
            b'a' => c.is_ascii_alphabetic(),
            b'c' => c.is_ascii_control(),
            b'd' => c.is_ascii_digit(),
            b'g' => c.is_ascii_graphic(),
            b'l' => c.is_ascii_lowercase(),
            b'p' => c.is_ascii_punctuation(),
            b's' => c == 0x20 || c == 0x0c || c == 0x0a || c == 0x0d || c == 0x09 || c == 0x0b,
            b'u' => c.is_ascii_uppercase(),
            b'w' => c.is_ascii_alphanumeric(),
            b'x' => c.is_ascii_hexdigit(),
            _ => return cl == c,
        };

        if cl.is_ascii_lowercase() {
            res
        } else {
            res == false
        }
    }

    fn match_capture(
        &self,
        off: usize,
        l: u8,
    ) -> Result<Option<usize>, Box<dyn core::error::Error>> {
        let l = self.check_capture(l)?;
        let len = usize::try_from(self.capture[l].len).unwrap();
        let c = self.capture[l].off;
        let c = &self.src[c..];
        let c = &c[..len];
        let s = &self.src[off..];
        let s = match s.get(..len) {
            Some(v) => v,
            None => return Ok(None),
        };

        if s == c {
            Ok(Some(off + len))
        } else {
            Ok(None)
        }
    }

    fn check_capture(&self, l: u8) -> Result<usize, Box<dyn core::error::Error>> {
        let mut l = isize::from(l);

        l -= isize::from(b'1');

        if l < 0 || self.capture.get(l as usize).is_none_or(|c| c.len == -1) {
            return Err(format!("invalid capture index %{}", l + 1).into());
        }

        Ok(l as usize)
    }

    fn min_expand(
        &mut self,
        mut off: usize,
        p: &[u8],
        ep: usize,
    ) -> Result<Option<usize>, Box<dyn core::error::Error>> {
        loop {
            if let Some(v) = self.match_0(off, &p[(ep + 1)..])? {
                return Ok(Some(v));
            }

            if self.singlematch(off, p, ep) {
                off += 1;
            } else {
                return Ok(None);
            }
        }
    }

    fn max_expand(
        &mut self,
        off: usize,
        p: &[u8],
        ep: usize,
    ) -> Result<Option<usize>, Box<dyn core::error::Error>> {
        let mut i = 0;

        while self.singlematch(off + i, p, ep) {
            i += 1;
        }

        for i in (0..=i).rev() {
            if let Some(v) = self.match_0(off + i, &p[(ep + 1)..])? {
                return Ok(Some(v));
            }
        }

        Ok(None)
    }

    fn captures_to_values<'b, A>(
        &self,
        cx: &Context<'b, A, Args>,
        off: Option<usize>,
        e: Option<usize>,
    ) -> Result<Vec<Value<'b, A>>, Box<dyn core::error::Error>> {
        let nlevels = if self.capture.is_empty() && off.is_some() {
            1
        } else {
            self.capture.len()
        };

        // Create values.
        let mut values = Vec::with_capacity(nlevels);

        for i in 0..nlevels {
            let v = match self.get_onecapture(i, off, e)? {
                CaptureValue::Num(v) => Value::Int(v),
                CaptureValue::Str(v) => Value::Str(cx.create_bytes(v)),
            };

            values.push(v);
        }

        Ok(values)
    }

    fn push_captures<A>(
        self,
        cx: &Context<A, Args>,
        off: Option<usize>,
        e: Option<usize>,
    ) -> Result<(), Box<dyn core::error::Error>> {
        let nlevels = if self.capture.is_empty() && off.is_some() {
            1
        } else {
            self.capture.len()
        };

        if cx.reserve(nlevels).is_err() {
            return Err("too many captures".into());
        }

        for i in 0..nlevels {
            self.push_onecapture(&cx, i, off, e)?;
        }

        Ok(())
    }

    fn push_onecapture<A>(
        &self,
        cx: &Context<A, Args>,
        i: usize,
        off: Option<usize>,
        e: Option<usize>,
    ) -> Result<(), Box<dyn core::error::Error>> {
        match self.get_onecapture(i, off, e)? {
            CaptureValue::Num(v) => cx.push(v)?,
            CaptureValue::Str(v) => cx.push_bytes(v)?,
        }

        Ok(())
    }

    fn get_onecapture<'b>(
        &self,
        i: usize,
        off: Option<usize>,
        e: Option<usize>,
    ) -> Result<CaptureValue<'a>, Box<dyn core::error::Error>> {
        let cap = match self.capture.get(i) {
            Some(v) => v,
            None => {
                return match i {
                    0 => Ok(CaptureValue::Str(&self.src[off.unwrap()..e.unwrap()])),
                    _ => Err(format!("invalid capture index %{}", i.wrapping_add(1)).into()),
                };
            }
        };

        match cap.len {
            -1 => Err("unfinished capture".into()),
            -2 => Ok(CaptureValue::Num((cap.off + 1) as i64)),
            l => {
                let l = usize::try_from(l).unwrap();

                Ok(CaptureValue::Str(&self.src[cap.off..(cap.off + l)]))
            }
        }
    }

    fn add_value<A>(
        &self,
        cx: &Context<A, Args>,
        b: &mut Vec<u8>,
        off: usize,
        e: usize,
        tr: &Replacement<A>,
    ) -> Result<bool, Box<dyn core::error::Error>> {
        let r = match tr {
            Replacement::Fp(f) => self
                .captures_to_values(cx, Some(off), Some(e))
                .and_then(move |args| cx.call(*f, args))?,
            Replacement::Str(v) => {
                self.add_s(b, off, e, v.as_bytes())?;

                return Ok(true);
            }
            Replacement::Table(t) => match self.get_onecapture(0, Some(off), Some(e))? {
                CaptureValue::Num(v) => t.get(v),
                CaptureValue::Str(v) => t.get_bytes_key(v),
            },
            Replacement::LuaFn(f) => self
                .captures_to_values(cx, Some(off), Some(e))
                .and_then(move |args| cx.call(*f, args))?,
        };

        match r {
            Value::Nil | Value::False => {
                b.extend_from_slice(&self.src[off..e]);

                return Ok(false);
            }
            Value::Int(v) => b.extend_from_slice(v.to_string().as_bytes()),
            Value::Float(v) => b.extend_from_slice(v.to_string().as_bytes()),
            Value::Str(v) => b.extend_from_slice(v.as_bytes()),
            v => return Err(format!("invalid replacement value (a {})", cx.type_name(v)).into()),
        }

        Ok(true)
    }

    fn add_s(
        &self,
        b: &mut Vec<u8>,
        off: usize,
        e: usize,
        mut tr: &[u8],
    ) -> Result<(), Box<dyn core::error::Error>> {
        loop {
            let mut p = match memchr(b'%', tr) {
                Some(v) => v,
                None => break,
            };

            b.extend_from_slice(&tr[..p]);
            p += 1;

            match tr.get(p).copied() {
                Some(b'%') => b.push(b'%'),
                Some(v) if v.is_ascii_digit() => {
                    match self.get_onecapture(
                        v.checked_sub(b'1').map(usize::from).unwrap_or(usize::MAX),
                        Some(off),
                        Some(e),
                    )? {
                        CaptureValue::Num(v) => b.extend_from_slice(v.to_string().as_bytes()),
                        CaptureValue::Str(v) => b.extend_from_slice(v),
                    }
                }
                Some(_) => return Err("invalid use of '%' in replacement string".into()),
                None => b.extend_from_slice(&self.src[off..e]),
            }

            tr = &tr[(p + 1)..];
        }

        b.extend_from_slice(tr);

        Ok(())
    }
}

struct MatchCapture {
    off: usize,
    len: isize,
}

enum CaptureValue<'a> {
    Num(i64),
    Str(&'a [u8]),
}

enum Replacement<'a, A> {
    Fp(Fp<A>),
    Str(&'a Str<A>),
    Table(&'a Table<A>),
    LuaFn(&'a LuaFn<A>),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sign {
    Always,
    SpaceIfPositive,
    Negative,
}

struct Header {
    islittle: bool,
    maxalign: usize,
}

impl Header {
    fn new() -> Self {
        Self {
            islittle: cfg!(target_endian = "little"),
            maxalign: 1,
        }
    }

    fn getdetails<F: Iterator<Item = char>>(
        &mut self,
        totalsize: usize,
        first: char,
        fmt: &mut Peekable<F>,
        ntoalign: &mut usize,
    ) -> Result<Pack, Box<dyn core::error::Error>> {
        let opt = self.getoption(first, fmt)?;
        let mut align = match opt {
            Pack::Signed(v) => v,
            Pack::Unsigned(v) => v,
            Pack::Float(v) => v,
            Pack::Double(v) => v,
            Pack::Number(v) => v,
            Pack::Char(v) => v,
            Pack::Str(v) => v.unwrap_or(0),
            Pack::Padding(v) => v,
            Pack::PadAlign => match fmt.next().map(|c| self.getoption(c, fmt)).transpose()? {
                Some(Pack::Signed(v))
                | Some(Pack::Unsigned(v))
                | Some(Pack::Float(v))
                | Some(Pack::Double(v))
                | Some(Pack::Number(v))
                | Some(Pack::Str(Some(v)))
                | Some(Pack::Padding(v))
                    if v != 0 =>
                {
                    v
                }
                _ => return Err("invalid next option for option 'X'".into()),
            },
            Pack::Nop => 0,
        };

        if align <= 1 || matches!(opt, Pack::Char(_)) {
            *ntoalign = 0;
        } else {
            if align > self.maxalign {
                align = self.maxalign;
            }

            if align & align - 1 != 0 {
                return Err("format asks for alignment not power of 2".into());
            }

            *ntoalign = align - (totalsize & (align - 1)) & align - 1;
        }

        Ok(opt)
    }

    fn getoption<F: Iterator<Item = char>>(
        &mut self,
        opt: char,
        fmt: &mut Peekable<F>,
    ) -> Result<Pack, Box<dyn core::error::Error>> {
        match opt {
            'b' => return Ok(Pack::Signed(1)),
            'B' => return Ok(Pack::Unsigned(1)),
            'h' => return Ok(Pack::Signed(size_of::<c_short>())),
            'H' => return Ok(Pack::Unsigned(size_of::<c_short>())),
            'l' => return Ok(Pack::Signed(size_of::<c_long>())),
            'L' => return Ok(Pack::Unsigned(size_of::<c_long>())),
            'j' => return Ok(Pack::Signed(8)),
            'J' => return Ok(Pack::Unsigned(8)),
            'T' => return Ok(Pack::Unsigned(size_of::<usize>())),
            'f' => return Ok(Pack::Float(size_of::<c_float>())),
            'n' => return Ok(Pack::Number(size_of::<f64>())),
            'd' => return Ok(Pack::Double(size_of::<c_double>())),
            'i' => return Self::getnumlimit(fmt, size_of::<c_int>()).map(Pack::Signed),
            'I' => return Self::getnumlimit(fmt, size_of::<c_int>()).map(Pack::Unsigned),
            's' => return Self::getnumlimit(fmt, size_of::<usize>()).map(|v| Pack::Str(Some(v))),
            'c' => match Self::getnum(fmt) {
                Some(v) => return Ok(Pack::Char(v)),
                None => return Err("missing size for format option 'c'".into()),
            },
            'z' => return Ok(Pack::Str(None)),
            'x' => return Ok(Pack::Padding(1)),
            'X' => return Ok(Pack::PadAlign),
            ' ' => (),
            '<' => self.islittle = true,
            '>' => self.islittle = false,
            '=' => self.islittle = cfg!(target_endian = "little"),
            '!' => self.maxalign = Self::getnumlimit(fmt, size_of::<usize>())?,
            _ => return Err(format!("invalid format option '{opt}'",).into()),
        }

        Ok(Pack::Nop)
    }

    fn getnumlimit<F: Iterator<Item = char>>(
        fmt: &mut Peekable<F>,
        df: usize,
    ) -> Result<usize, Box<dyn core::error::Error>> {
        let sz = Self::getnum(fmt).unwrap_or(df);

        if sz > 16 || sz <= 0 {
            return Err(format!("integral size ({sz}) out of limits [1,16]").into());
        }

        Ok(sz)
    }

    fn getnum<F: Iterator<Item = char>>(fmt: &mut Peekable<F>) -> Option<usize> {
        let mut b = fmt.next_if(|b| b.is_ascii_digit())? as u8;
        let mut a = 0;

        loop {
            a = a * 10 + usize::from(b - b'0');

            b = match fmt.next_if(move |b| b.is_ascii_digit() && a <= (2147483647 - 9) / 10) {
                Some(v) => v as u8,
                None => break,
            };
        }

        Some(a)
    }
}

#[derive(Clone, Copy)]
enum Pack {
    Signed(usize),
    Unsigned(usize),
    Float(usize),
    Double(usize),
    Number(usize),
    Char(usize),
    Str(Option<usize>),
    Padding(usize),
    PadAlign,
    Nop,
}
