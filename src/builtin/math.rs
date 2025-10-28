//! Implementation of [mathematical library](https://www.lua.org/manual/5.4/manual.html#6.7).
use crate::{ArgNotFound, Args, Context, Float, Nil, Number, Ret, Type};
use alloc::boxed::Box;
use libm::fabs;

/// Implementation of [math.abs](https://www.lua.org/manual/5.4/manual.html#pdf-math.abs).
pub fn abs<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    let arg = cx.arg(1);

    match arg.as_int() {
        Some(mut n) => {
            if n < 0 {
                n = 0u64.wrapping_sub(n as u64) as i64;
            }

            cx.push(n)?;
        }
        None => {
            let n = arg.to_float()?;

            cx.push(fabs(n.into()))?;
        }
    }

    Ok(cx.into())
}

/// Implementation of [math.floor](https://www.lua.org/manual/5.4/manual.html#pdf-math.floor).
pub fn floor<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    let v = cx.arg(1);
    let r = if v.is_int() == Some(true) {
        let mut r = cx.into_results(1);

        r.truncate(1);
        r
    } else {
        cx.push(pushnumint(v.to_float()?.floor()))?;
        cx.into()
    };

    Ok(r)
}

/// Implementation of [math.log](https://www.lua.org/manual/5.4/manual.html#pdf-math.log).
pub fn log<D>(cx: Context<D, Args>) -> Result<Context<D, Ret>, Box<dyn core::error::Error>> {
    let v = cx.arg(1).to_float()?;

    match cx.arg(2).to_nilable_float(false)? {
        Some(Float(2.0)) => cx.push(v.log2())?,
        Some(Float(10.0)) => cx.push(v.log10())?,
        Some(b) => cx.push(v.log(b))?,
        None => cx.push(libm::log(v.into()))?,
    }

    Ok(cx.into())
}

/// Implementation of [math.max](https://www.lua.org/manual/5.4/manual.html#pdf-math.max).
pub fn max<D>(cx: Context<D, Args>) -> Result<Context<D, Ret>, Box<dyn core::error::Error>> {
    let mut r = cx.arg(1).exists()?;

    for i in 2..=cx.args() {
        let v = cx.arg(i);

        if cx.is_value_lt(&r, &v)? {
            r = v;
        }
    }

    cx.push(r)?;

    Ok(cx.into())
}

/// Implementation of [math.modf](https://www.lua.org/manual/5.4/manual.html#pdf-math.modf).
pub fn modf<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    let v = cx.arg(1);

    if let Some(v) = v.as_int() {
        cx.push(v)?;
        cx.push(0.0)?;
    } else {
        let n = v.to_float()?;
        let ip = if n < 0.0 { n.ceil() } else { n.floor() };

        cx.push(pushnumint(ip))?;
        cx.push(if n == ip { Float::default() } else { n - ip })?;
    }

    Ok(cx.into())
}

/// Implementation of [math.sin](https://www.lua.org/manual/5.4/manual.html#pdf-math.sin).
pub fn sin<D>(cx: Context<D, Args>) -> Result<Context<D, Ret>, Box<dyn core::error::Error>> {
    let v = cx.arg(1).to_float()?;

    cx.push(v.sin())?;

    Ok(cx.into())
}

/// Implementation of [math.type](https://www.lua.org/manual/5.4/manual.html#pdf-math.type).
pub fn r#type<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    let v = cx.arg(1);

    if v.ty().ok_or_else(|| v.error(ArgNotFound))? == Type::Number {
        if v.is_int() == Some(true) {
            cx.push_str("integer")?;
        } else {
            cx.push_str("float")?;
        }
    } else {
        cx.push(Nil)?;
    }

    Ok(cx.into())
}

/// Implementation of [math.ult](https://www.lua.org/manual/5.4/manual.html#pdf-math.ult).
pub fn ult<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    let a = cx.arg(1).to_int()?;
    let b = cx.arg(2).to_int()?;

    cx.push((a as u64) < (b as u64))?;

    Ok(cx.into())
}

#[inline(always)]
fn pushnumint(d: Float) -> Number {
    // TODO: This does not seems right even on Lua implementation. Lua said MININTEGER always has an
    // exact representation as a float but it does not.
    if d >= i64::MIN as f64 && d <= i64::MAX as f64 {
        Number::Int(f64::from(d) as i64)
    } else {
        Number::Float(d)
    }
}
