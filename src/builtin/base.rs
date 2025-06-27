use crate::{ArgNotFound, Args, ChunkInfo, Context, Nil, Ret, TryCall, Type};
use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt::Write;

/// Implementation of [assert](https://www.lua.org/manual/5.4/manual.html#pdf-assert) function.
///
/// Note that second argument accept only a string.
pub fn assert(cx: Context<Args>) -> Result<Context<Ret>, Box<dyn core::error::Error>> {
    // Check condition.
    let c = cx.arg(1);

    if c.to_bool().ok_or_else(|| c.error(ArgNotFound))? {
        return Ok(cx.into_results(1));
    }

    // Raise error.
    let m = if cx.args() > 1 {
        let m = cx.arg(2);

        m.get_str()?
            .as_str()
            .ok_or_else(|| m.error("expect UTF-8 string"))?
    } else {
        "assertion failed!"
    };

    Err(m.into())
}

/// Implementation of [error](https://www.lua.org/manual/5.4/manual.html#pdf-error) function.
///
/// Note that first argument accept only a string and second argument is not supported.
pub fn error(cx: Context<Args>) -> Result<Context<Ret>, Box<dyn core::error::Error>> {
    let msg = cx.arg(1);
    let msg = msg
        .get_str()?
        .as_str()
        .ok_or_else(|| msg.error("expect UTF-8 string"))?;

    if cx.args() > 1 {
        return Err("second argument of 'error' is not supported".into());
    }

    Err(msg.into())
}

/// Implementation of [getmetatable](https://www.lua.org/manual/5.4/manual.html#pdf-getmetatable).
pub fn getmetatable(cx: Context<Args>) -> Result<Context<Ret>, Box<dyn core::error::Error>> {
    // Get metatable.
    let mt = cx.arg(1);
    let mt = mt.get_metatable().ok_or_else(|| mt.error(ArgNotFound))?;
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
pub fn load(cx: Context<Args>) -> Result<Context<Ret>, Box<dyn core::error::Error>> {
    let s = cx.arg(1).get_str()?;

    // Get name.
    let name = cx.arg(2);
    let name = match name.get_nilable_str(false)? {
        Some(v) => v
            .as_str()
            .ok_or_else(|| name.error("expect UTF-8 string"))?,
        None => "",
    };

    // Get mode.
    let mode = cx.arg(3);

    if let Some(v) = mode.get_nilable_str(false)? {
        if v != "t" {
            return Err(mode.error("mode other than 't' is not supported"));
        }
    }

    // Load.
    let f = match cx.load(ChunkInfo::new(name), s.as_bytes()) {
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

/// Implementation of [pcall](https://www.lua.org/manual/5.4/manual.html#pdf-pcall).
pub fn pcall(cx: Context<Args>) -> Result<Context<Ret>, Box<dyn core::error::Error>> {
    let r = match cx.try_forward(1)? {
        TryCall::Ok(r) => {
            r.insert(1, true)?;
            r
        }
        TryCall::Err(cx, chunk, e) => {
            // Write first error.
            let mut m = String::with_capacity(128);

            if let Some((s, l)) = chunk {
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
pub fn print(cx: Context<Args>) -> Result<Context<Ret>, Box<dyn core::error::Error>> {
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

/// Implementation of [rawget](https://www.lua.org/manual/5.4/manual.html#pdf-rawget).
pub fn rawget(cx: Context<Args>) -> Result<Context<Ret>, Box<dyn core::error::Error>> {
    let t = cx.arg(1).get_table()?;
    let k = cx.arg(2);

    if !k.is_exists() {
        return Err(k.error(ArgNotFound));
    }

    cx.push_from_key(t, k)?;

    Ok(cx.into())
}

/// Implementation of [rawset](https://www.lua.org/manual/5.4/manual.html#pdf-rawset).
pub fn rawset(cx: Context<Args>) -> Result<Context<Ret>, Box<dyn core::error::Error>> {
    let t = cx.arg(1).get_table()?;
    let k = cx.arg(2);
    let v = cx.arg(3);

    if !k.is_exists() {
        return Err(k.error(ArgNotFound));
    }

    if !v.is_exists() {
        return Err(v.error(ArgNotFound));
    }

    // SAFETY: t, k and v passed from Lua, which mean it is guarantee to be created from the same
    // Lua instance.
    unsafe { t.set_unchecked(k, v)? };
    unsafe { cx.push_unchecked(t)? };

    Ok(cx.into())
}

/// Implementation of [select](https://www.lua.org/manual/5.4/manual.html#pdf-select).
pub fn select(cx: Context<Args>) -> Result<Context<Ret>, Box<dyn core::error::Error>> {
    // Check if first argument is '#'. We check only first byte to match with Lua behavior.
    let n = cx.args().wrapping_sub(1);
    let i = cx.arg(1);

    if i.ty() == Some(Type::String) && i.get_str()?.as_bytes().starts_with(b"#") {
        cx.push(n as i64)?;
        return Ok(cx.into());
    }

    // Adjust index.
    let i = i
        .to_int()?
        .try_into()
        .ok()
        .and_then(move |i: isize| {
            if i < 0 {
                if i.unsigned_abs() > n { None } else { Some(i) }
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
pub fn setmetatable(cx: Context<Args>) -> Result<Context<Ret>, Box<dyn core::error::Error>> {
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

/// Implementation of [tostring](https://www.lua.org/manual/5.4/manual.html#pdf-tostring).
pub fn tostring(cx: Context<Args>) -> Result<Context<Ret>, Box<dyn core::error::Error>> {
    let v = cx.arg(1);

    if !v.is_exists() {
        return Err(v.error(ArgNotFound));
    }

    cx.push(v.display()?)?;

    Ok(cx.into())
}

/// Implementation of [type](https://www.lua.org/manual/5.4/manual.html#pdf-type).
pub fn r#type(cx: Context<Args>) -> Result<Context<Ret>, Box<dyn core::error::Error>> {
    let v = cx.arg(1);
    let t = v.ty().ok_or_else(|| v.error(ArgNotFound))?;

    cx.push_str(t.to_string())?;

    Ok(cx.into())
}
