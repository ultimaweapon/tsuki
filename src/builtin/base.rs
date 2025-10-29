//! Implementation of [basic library](https://www.lua.org/manual/5.4/manual.html#6.1).
use crate::{ArgNotFound, Args, Context, Nil, Ret, TryCall, Type, fp};
use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt::Write;

/// Implementation of [assert](https://www.lua.org/manual/5.4/manual.html#pdf-assert) function.
///
/// Note that second argument accept only a string.
pub fn assert<D>(cx: Context<D, Args>) -> Result<Context<D, Ret>, Box<dyn core::error::Error>> {
    // Check condition.
    let c = cx.arg(1);

    if c.to_bool().ok_or_else(|| c.error(ArgNotFound))? {
        return Ok(cx.into_results(1));
    }

    // Raise error.
    if cx.args() > 1 {
        let a = cx.arg(2);
        let m = a.to_str()?;
        let m = m.as_str().ok_or_else(|| a.error("expect UTF-8 string"))?;

        Err(m.into())
    } else {
        Err("assertion failed!".into())
    }
}

/// Implementation of [error](https://www.lua.org/manual/5.4/manual.html#pdf-error) function.
///
/// Note that first argument accept only a string and second argument is not supported.
pub fn error<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    let arg = cx.arg(1);
    let msg = arg.to_str()?;
    let msg = msg
        .as_str()
        .ok_or_else(|| arg.error("expect UTF-8 string"))?;

    if cx.args() > 1 {
        return Err("second argument of 'error' is not supported".into());
    }

    Err(msg.into())
}

/// Implementation of [getmetatable](https://www.lua.org/manual/5.4/manual.html#pdf-getmetatable).
pub fn getmetatable<A>(
    cx: Context<A, Args>,
) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    // Get metatable.
    let arg = cx.arg(1);
    let mt = arg.metatable().ok_or_else(|| arg.error(ArgNotFound))?;
    let mt = match mt {
        Some(v) => v,
        None => {
            cx.push(Nil)?;
            return Ok(cx.into());
        }
    };

    // Get __metatable from metatable.
    if cx.push_from_str_key(&mt, "__metatable")? == Type::Nil {
        cx.push(mt)?;
        Ok(cx.into_results(-1))
    } else {
        Ok(cx.into())
    }
}

/// Implementation of [load](https://www.lua.org/manual/5.4/manual.html#pdf-load).
///
/// The main differences from Lua is:
///
/// - First argument accept only a string.
/// - Second argument accept only a UTF-8 string and will be empty when absent.
/// - Third argument must be `nil` or `"t"`.
pub fn load<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    let s = cx.arg(1).to_str()?;

    // Get name.
    let a = cx.arg(2);
    let name = a.to_nilable_str(false)?;
    let name = match &name {
        Some(v) => v.as_str().ok_or_else(|| a.error("expect UTF-8 string"))?,
        None => "",
    };

    // Get mode.
    let mode = cx.arg(3);

    if let Some(v) = mode.to_nilable_str(false)? {
        if v.ne("t") {
            return Err(mode.error("mode other than 't' is not supported"));
        }
    }

    // Load.
    let f = match cx.load(name, s.as_bytes()) {
        Ok(v) => v,
        Err(e) => {
            cx.push(Nil)?;
            cx.push_str(format!("{}:{}: {}", name, e.line(), e))?;

            return Ok(cx.into());
        }
    };

    // Set environment.
    if let Some(env) = cx.arg(4).get() {
        drop(f.set_upvalue(1, env));
    }

    cx.push(f)?;

    Ok(cx.into())
}

/// Implementation of [next](https://www.lua.org/manual/5.4/manual.html#pdf-next).
pub fn next<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    let t = cx.arg(1).get_table()?;
    let k = cx.arg(2);

    if !cx.push_next(t, k)? {
        cx.push(Nil)?;
    }

    Ok(cx.into())
}

/// Implementation of [pairs](https://www.lua.org/manual/5.4/manual.html#pdf-pairs).
pub fn pairs<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    let t = cx.arg(1);
    let m = t.metatable().ok_or_else(|| t.error(ArgNotFound))?;

    match m
        .as_ref()
        .map(|m| m.get_str_key("__pairs"))
        .filter(|v| !v.is_nil())
    {
        Some(f) => {
            cx.push(f)?;
            cx.push(t)?;
            cx.forward(-2)
        }
        None => {
            cx.push(fp!(next))?;
            cx.push(t)?;
            cx.push(Nil)?;

            Ok(cx.into())
        }
    }
}

/// Implementation of [pcall](https://www.lua.org/manual/5.4/manual.html#pdf-pcall).
pub fn pcall<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    let r = match cx.try_forward(1)? {
        TryCall::Ok(r) => {
            r.insert(1, true)?;
            r
        }
        TryCall::Err(cx, e) => {
            use core::error::Error;

            // Write first error.
            let mut m = String::with_capacity(128);

            if let Some((s, l)) = e.location() {
                write!(m, "{s}:{l}: ").unwrap();
            }

            write!(m, "{e}").unwrap();

            // Write nested errors.
            let mut e = e.source();

            while let Some(v) = e {
                write!(m, " -> {v}").unwrap();
                e = v.source();
            }

            // Push results.
            cx.push(false)?;
            cx.push_str(m)?;
            cx.into()
        }
    };

    Ok(r)
}

/// Implementation of [print](https://www.lua.org/manual/5.4/manual.html#pdf-print).
#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
pub fn print<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    use std::io::Write;

    // We can't print while converting the arguments to string since it can call into arbitrary
    // function, which may lock stdout.
    let mut args = Vec::with_capacity(cx.args());

    for i in 1..=cx.args() {
        args.push(cx.arg(i).display()?);
    }

    // Print.
    let mut stdout = std::io::stdout().lock();

    for (i, arg) in args.into_iter().enumerate() {
        if i > 0 {
            stdout.write_all(b"\t")?;
        }

        stdout.write_all(arg.as_bytes())?;
    }

    writeln!(stdout)?;

    Ok(cx.into())
}

/// Implementation of [rawequal](https://www.lua.org/manual/5.4/manual.html#pdf-rawequal).
pub fn rawequal<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    let a = cx.arg(1).exists()?;
    let b = cx.arg(2).exists()?;
    let r = cx.is_value_eq(a, b, false)?;

    cx.push(r)?;

    Ok(cx.into())
}

/// Implementation of [rawget](https://www.lua.org/manual/5.4/manual.html#pdf-rawget).
pub fn rawget<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    let t = cx.arg(1).get_table()?;
    let k = cx.arg(2).exists()?;

    cx.push_from_table(t, k)?;

    Ok(cx.into())
}

/// Implementation of [rawlen](https://www.lua.org/manual/5.4/manual.html#pdf-rawlen).
pub fn rawlen<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    let v = cx.arg(1);
    let l = if let Some(v) = v.as_str(false) {
        v.len() as i64
    } else if let Some(v) = v.as_table() {
        v.len()
    } else {
        return Err(v.invalid_type("table or string"));
    };

    cx.push(l)?;

    Ok(cx.into())
}

/// Implementation of [rawset](https://www.lua.org/manual/5.4/manual.html#pdf-rawset).
pub fn rawset<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    let t = cx.arg(1).get_table()?;
    let k = cx.arg(2).exists()?;
    let v = cx.arg(3).exists()?;

    // SAFETY: t, k and v passed from Lua, which mean it is guarantee to be created from the same
    // Lua instance.
    unsafe { t.set_unchecked(k, v)? };
    unsafe { cx.push_unchecked(t)? };

    Ok(cx.into())
}

/// Implementation of [select](https://www.lua.org/manual/5.4/manual.html#pdf-select).
pub fn select<D>(cx: Context<D, Args>) -> Result<Context<D, Ret>, Box<dyn core::error::Error>> {
    // Check if first argument is '#'. We check only first byte to match with Lua behavior.
    let n = cx.args();
    let i = cx.arg(1);

    if i.ty() == Some(Type::String) && i.get_str()?.as_bytes().starts_with(b"#") {
        cx.push((n - 1) as i64)?;
        return Ok(cx.into());
    }

    // Adjust index.
    let i = i
        .to_int()?
        .try_into()
        .ok()
        .and_then(move |i: isize| {
            if i < 0 {
                if i.unsigned_abs() >= n { None } else { Some(i) }
            } else if i == 0 || i > n as isize {
                None
            } else {
                Some(1 + i)
            }
        })
        .ok_or_else(|| i.error("index out of range"))?;

    Ok(cx.into_results(i))
}

/// Implementation of [setmetatable](https://www.lua.org/manual/5.4/manual.html#pdf-setmetatable).
pub fn setmetatable<A>(
    cx: Context<A, Args>,
) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    let t = cx.arg(1).get_table()?;
    let mt = cx.arg(2).get_nilable_table(true)?;

    if t.metatable()
        .is_some_and(|v| v.contains_str_key("__metatable"))
    {
        return Err("cannot change a protected metatable".into());
    }

    match mt {
        Some(v) => t.set_metatable(v)?,
        None => t.remove_metatable(),
    }

    // Remove metatable and return the table.
    let mut cx = cx.into_results(1);

    cx.pop();

    Ok(cx)
}

/// Implementation of [tonumber](https://www.lua.org/manual/5.4/manual.html#pdf-tonumber).
pub fn tonumber<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    let e = cx.arg(1);
    let b = cx.arg(2);

    match b.to_nilable_int(false)? {
        Some(base) => {
            let s = e.get_str()?;

            if !(2 <= base && base <= 36) {
                return Err(b.error("base out of range"));
            } else if let Some(v) = b_str2int(s.as_bytes(), base as u8) {
                cx.push(v)?;

                return Ok(cx.into());
            }
        }
        None => {
            if let Some(v) = e.as_num() {
                cx.push(v)?;

                return Ok(cx.into());
            } else if let Some(v) = e.as_str(false).and_then(|v| v.to_num()) {
                cx.push(v)?;

                return Ok(cx.into());
            }

            e.exists()?;
        }
    }

    cx.push(Nil)?;

    Ok(cx.into())
}

/// Implementation of [tostring](https://www.lua.org/manual/5.4/manual.html#pdf-tostring).
pub fn tostring<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    let v = cx.arg(1).exists()?;

    cx.push(v.display()?)?;

    Ok(cx.into())
}

/// Implementation of [type](https://www.lua.org/manual/5.4/manual.html#pdf-type).
pub fn r#type<D>(cx: Context<D, Args>) -> Result<Context<D, Ret>, Box<dyn core::error::Error>> {
    let v = cx.arg(1);
    let t = v.ty().ok_or_else(|| v.error(ArgNotFound))?;

    cx.push_str(t.to_string())?;

    Ok(cx.into())
}

fn b_str2int(s: &[u8], base: u8) -> Option<i64> {
    // Skip initial spaces.
    let mut s = s.iter();
    let mut b = s.next().copied()?;
    let is_space =
        |b: u8| b == b' ' || b == 0x0C || b == b'\n' || b == b'\r' || b == b'\t' || b == 0x0B;

    while is_space(b) {
        b = s.next().copied()?;
    }

    // Check negative.
    let neg = match b {
        b'-' => {
            b = s.next().copied()?;
            true
        }
        b'+' => {
            b = s.next().copied()?;
            false
        }
        _ => false,
    };

    if !b.is_ascii_alphanumeric() {
        return None;
    }

    // Parse.
    let mut n: u64 = 0;

    'top: loop {
        let digit = match b.is_ascii_digit() {
            true => b - b'0',
            false => b.to_ascii_uppercase() - b'A' + 10,
        };

        if digit >= base {
            return None;
        }

        n = n.wrapping_mul(base.into()).wrapping_add(digit.into());
        b = match s.next().copied() {
            Some(v) => v,
            None => break,
        };

        if !b.is_ascii_alphanumeric() {
            while is_space(b) {
                b = match s.next().copied() {
                    Some(v) => v,
                    None => break 'top,
                };
            }

            return None;
        }
    }

    // Convert.
    if neg {
        n = 0u64.wrapping_sub(n);
    }

    Some(n as i64)
}
