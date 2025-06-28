use crate::{Args, Context, Ret};
use alloc::boxed::Box;

/// Implementation of [math.log](https://www.lua.org/manual/5.4/manual.html#pdf-math.log).
pub fn log(cx: Context<Args>) -> Result<Context<Ret>, Box<dyn core::error::Error>> {
    let v = cx.arg(1).to_num()?;

    match cx.arg(2).to_nilable_num(false)? {
        Some(2.0) => cx.push(libm::log2(v))?,
        Some(10.0) => cx.push(libm::log10(v))?,
        Some(b) => cx.push(libm::log(v) / libm::log(b))?,
        None => cx.push(libm::log(v))?,
    }

    Ok(cx.into())
}

/// Implementation of [math.sin](https://www.lua.org/manual/5.4/manual.html#pdf-math.sin).
pub fn sin(cx: Context<Args>) -> Result<Context<Ret>, Box<dyn core::error::Error>> {
    let v = cx.arg(1).to_num()?;

    cx.push(libm::sin(v))?;

    Ok(cx.into())
}
