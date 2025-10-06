//! Implementation of [table library](https://www.lua.org/manual/5.4/manual.html#6.6).
use crate::{Args, Context, Ret};
use alloc::boxed::Box;

/// Implementation of [table.unpack](https://www.lua.org/manual/5.4/manual.html#pdf-table.unpack).
pub fn unpack<D>(cx: Context<D, Args>) -> Result<Context<D, Ret>, Box<dyn core::error::Error>> {
    // Check if start index greater than end index.
    let l = cx.arg(1);
    let i = cx.arg(2).to_nilable_int(false)?.unwrap_or(1);
    let e = match cx.arg(3).to_nilable_int(false)? {
        Some(v) => v,
        None => cx.get_value_len(&l)?,
    };

    if i > e {
        return Ok(cx.into());
    }

    // Reserve stack.
    if e.checked_sub(i)
        .and_then(|v| v.checked_add(1))
        .and_then(|v| v.try_into().ok())
        .and_then(|v| cx.reserve(v).ok())
        .is_none()
    {
        return Err("too many results to unpack".into());
    }

    // Get value and push to results.
    for i in i..=e {
        cx.push_from_index_with_int(&l, i)?;
    }

    Ok(cx.into())
}
