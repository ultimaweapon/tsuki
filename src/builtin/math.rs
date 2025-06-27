use crate::{Args, Context, Ret};
use alloc::boxed::Box;

/// Implementation of [math.sin](https://www.lua.org/manual/5.4/manual.html#pdf-math.sin).
pub fn sin(cx: Context<Args>) -> Result<Context<Ret>, Box<dyn core::error::Error>> {
    let v = cx.arg(1).to_num()?;

    cx.push(libm::sin(v))?;

    Ok(cx.into())
}
