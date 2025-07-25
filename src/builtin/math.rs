use crate::{ArgNotFound, Args, Context, Nil, Ret, Type, Value};
use alloc::boxed::Box;

/// Implementation of [math.floor](https://www.lua.org/manual/5.4/manual.html#pdf-math.floor).
pub fn floor(cx: Context<Args>) -> Result<Context<Ret>, Box<dyn core::error::Error>> {
    let v = cx.arg(1);
    let r = if v.is_int() == Some(true) {
        let mut r = cx.into_results(1);

        r.truncate(1);
        r
    } else {
        cx.push(pushnumint(v.to_num()?.floor()))?;
        cx.into()
    };

    Ok(r)
}

/// Implementation of [math.log](https://www.lua.org/manual/5.4/manual.html#pdf-math.log).
pub fn log(cx: Context<Args>) -> Result<Context<Ret>, Box<dyn core::error::Error>> {
    let v = cx.arg(1).to_num()?;

    match cx.arg(2).to_nilable_num(false)? {
        Some(2.0) => cx.push(v.log2())?,
        Some(10.0) => cx.push(v.log10())?,
        Some(b) => cx.push(v.log(b))?,
        None => cx.push(libm::log(v))?,
    }

    Ok(cx.into())
}

/// Implementation of [math.max](https://www.lua.org/manual/5.4/manual.html#pdf-math.max).
pub fn max(cx: Context<Args>) -> Result<Context<Ret>, Box<dyn core::error::Error>> {
    let mut r = cx.arg(1).exists()?;

    for i in 2..=cx.args() {
        let v = cx.arg(i);

        if r.lt(&v)? == Some(true) {
            r = v;
        }
    }

    cx.push(r)?;

    Ok(cx.into())
}

/// Implementation of [math.sin](https://www.lua.org/manual/5.4/manual.html#pdf-math.sin).
pub fn sin(cx: Context<Args>) -> Result<Context<Ret>, Box<dyn core::error::Error>> {
    let v = cx.arg(1).to_num()?;

    cx.push(v.sin())?;

    Ok(cx.into())
}

/// Implementation of [math.type](https://www.lua.org/manual/5.4/manual.html#pdf-math.type).
pub fn r#type(cx: Context<Args>) -> Result<Context<Ret>, Box<dyn core::error::Error>> {
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

#[inline(always)]
fn pushnumint(d: f64) -> Value {
    // TODO: This does not seems right even on Lua implementation. Lua said MININTEGER always has an
    // exact representation as a float but it does not.
    if d >= i64::MIN as f64 && d <= i64::MAX as f64 {
        Value::Int(d as i64)
    } else {
        Value::Num(d)
    }
}
