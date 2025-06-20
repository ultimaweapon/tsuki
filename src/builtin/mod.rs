use crate::{Args, Context, Ret, TryCall};
use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

pub fn assert(cx: Context<Args>) -> Result<Context<Ret>, Box<dyn core::error::Error>> {
    // Check condition.
    if cx.arg(1).to_bool() {
        return Ok(cx.into_results(1));
    }

    cx.arg(1).exists()?;

    // Raise error.
    let m = if cx.args() > 1 {
        let m = cx.arg(2).get_str(true)?;

        String::from_utf8_lossy(m.as_bytes()).into()
    } else {
        "assertion failed!".into()
    };

    Err(m)
}

pub fn error(cx: Context<Args>) -> Result<Context<Ret>, Box<dyn core::error::Error>> {
    let msg = cx.arg(1).get_str(true)?;

    if cx.args() > 1 {
        return Err("second argument of 'error' is not supported".into());
    }

    Err(String::from_utf8_lossy(msg.as_bytes()).into())
}

pub fn pcall(cx: Context<Args>) -> Result<Context<Ret>, Box<dyn core::error::Error>> {
    let r = match cx.try_forward(1)? {
        TryCall::Ok(r) => {
            r.insert(1, true)?;
            r
        }
        TryCall::Err(cx, chunk, e) => {
            cx.push(false)?;
            cx.push_str(match chunk {
                Some((s, l)) => format!("{s}:{l}: {e}"),
                None => e.to_string(),
            })?;
            cx.into()
        }
    };

    Ok(r)
}

#[cfg(feature = "std")]
pub fn print(cx: Context<Args>) -> Result<Context<Ret>, Box<dyn core::error::Error>> {
    use std::io::Write;

    // We can't print while converting the arguments to string since it can call into arbitrary
    // function, which may lock stdout.
    let mut args = Vec::with_capacity(cx.args());

    for i in 1..=cx.args() {
        args.push(cx.arg(i).to_str()?);
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
