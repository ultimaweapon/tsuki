//! Implementation of [table library](https://www.lua.org/manual/5.4/manual.html#6.6).
use crate::context::{Arg, Args, Context, Ret};
use crate::{Buffer, Type, Value};
use alloc::boxed::Box;
use alloc::format;

/// Implementation of [table.concat](https://www.lua.org/manual/5.4/manual.html#pdf-table.concat).
pub fn concat<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    // Check if table.
    let t = cx.arg(1);

    checktab(&t, 1 | 4)?;

    // Load arguments.
    let sep = cx
        .arg(2)
        .to_nilable_str(false)?
        .map(|v| v.as_bytes())
        .unwrap_or(b"");
    let mut i = cx.arg(3).to_nilable_int(false)?.unwrap_or(1);
    let last = match cx.arg(4).to_nilable_int(false)? {
        Some(v) => v,
        None => cx.get_value_len(&t)?,
    };

    // Concat.
    let mut b = Buffer::default();

    while i < last {
        addfield(&cx, &t, &mut b, i)?;

        b.extend_from_slice(sep);
        i += 1;
    }

    if i == last {
        addfield(&cx, &t, &mut b, i)?;
    }

    cx.push_bytes(b)?;

    Ok(cx.into())
}

/// Implementation of [table.unpack](https://www.lua.org/manual/5.4/manual.html#pdf-table.unpack).
pub fn unpack<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
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
        let v = cx.thread().index(&l, i)?;

        cx.push(v)?;
    }

    Ok(cx.into())
}

fn checktab<A>(arg: &Arg<A>, what: u8) -> Result<(), Box<dyn core::error::Error>> {
    if arg.ty() == Some(Type::Table) {
        return Ok(());
    }

    arg.metatable()
        .flatten()
        .filter(move |mt| what & 1 == 0 || mt.contains_str_key("__index"))
        .filter(move |mt| what & 2 == 0 || mt.contains_str_key("__newindex"))
        .filter(move |mt| what & 4 == 0 || mt.contains_str_key("__len"))
        .ok_or_else(|| arg.invalid_type("table"))?;

    Ok(())
}

fn addfield<A>(
    cx: &Context<A, Args>,
    t: &Arg<A>,
    b: &mut Buffer,
    i: i64,
) -> Result<(), Box<dyn core::error::Error>> {
    use core::fmt::Write;

    match cx.thread().index(t, i)? {
        Value::Int(v) => write!(b, "{v}").unwrap(),
        Value::Float(v) => write!(b, "{v}").unwrap(),
        Value::Str(v) => b.extend_from_slice(v.as_bytes()),
        v => {
            return Err(format!(
                "invalid value ({}) at index {} in table for 'concat'",
                v.ty(),
                i
            )
            .into());
        }
    }

    Ok(())
}
