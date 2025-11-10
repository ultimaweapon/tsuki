//! Implementation of [coroutine library](https://www.lua.org/manual/5.4/manual.html#6.2).
use crate::context::{Args, Context, Ret};
use alloc::boxed::Box;

/// Implementation of
/// [coroutine.running](https://www.lua.org/manual/5.4/manual.html#pdf-coroutine.running).
pub fn running<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    cx.push(cx.thread())?;
    cx.push(cx.is_main_thread())?;

    Ok(cx.into())
}
