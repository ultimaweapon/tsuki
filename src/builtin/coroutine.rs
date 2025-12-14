//! Implementation of [coroutine library](https://www.lua.org/manual/5.4/manual.html#6.2).
use crate::context::{Args, Context, Ret};
use crate::{Coroutine, DynamicInputs, Thread, Value};
use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::vec::Vec;
use erdp::ErrorDisplay;

/// Implementation of
/// [coroutine.resume](https://www.lua.org/manual/5.4/manual.html#pdf-coroutine.resume).
pub fn resume<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    let co = cx.arg(1).get_thread()?;

    match auxresume(&cx, co, 2) {
        Ok(r) => match cx.reserve(r.len() + 1) {
            Ok(_) => {
                cx.push(true)?;

                for v in r {
                    cx.push(v)?;
                }
            }
            Err(_) => {
                cx.push(false)?;
                cx.push_str("too many results to resume")?;
            }
        },
        Err(e) => {
            cx.push(false)?;
            cx.push_str(e.display().to_string())?;
        }
    }

    Ok(cx.into())
}

/// Implementation of
/// [coroutine.running](https://www.lua.org/manual/5.4/manual.html#pdf-coroutine.running).
pub fn running<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    cx.push(cx.thread())?;
    cx.push(false)?;

    Ok(cx.into())
}

fn auxresume<'a, A>(
    cx: &'a Context<A, Args>,
    co: &'a Thread<A>,
    first: usize,
) -> Result<Vec<Value<'a, A>>, Box<dyn core::error::Error>> {
    // Reserve stack now to mimic Lua behavior.
    let narg = cx.args() + 1 - first;

    if co.reserve(narg).is_err() {
        return Err("too many arguments to resume".into());
    }

    // Get arguments.
    let mut args = DynamicInputs::with_capacity(narg);

    for i in first..=cx.args() {
        args.push_arg(cx.arg(i));
    }

    match co.resume::<Vec<Value<A>>>(args)? {
        Coroutine::Suspended(v) => Ok(v),
        Coroutine::Finished(v) => Ok(v),
    }
}
