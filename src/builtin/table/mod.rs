//! Implementation of [table library](https://www.lua.org/manual/5.4/manual.html#6.6).
use crate::context::{Arg, Args, Context, Ret};
use crate::{Buffer, Fp, LuaFn, Type, Value};
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

/// Implementation of [table.sort](https://www.lua.org/manual/5.4/manual.html#pdf-table.sort).
#[cfg(feature = "rand")]
#[cfg_attr(docsrs, doc(cfg(feature = "rand")))]
pub fn sort<A>(cx: Context<A, Args>) -> Result<Context<A, Ret>, Box<dyn core::error::Error>> {
    // Check if table.
    let tab = cx.arg(1);

    checktab(&tab, 1 | 2 | 4)?;

    // Check if empty.
    let n = cx.get_value_len(&tab)?;

    if n <= 1 {
        return Ok(cx.into());
    } else if n >= 2147483647 {
        return Err(tab.error("array too big"));
    }

    // Check comparer.
    let cmp = cx.arg(2);
    let cmp = if let Some(v) = cmp.as_lua_fn() {
        Comparer::LuaFn(v)
    } else if let Some(v) = cmp.as_fp() {
        Comparer::Fp(v)
    } else if matches!(cmp.ty(), Some(Type::Nil) | None) {
        Comparer::Default
    } else {
        return Err(cmp.invalid_type("function"));
    };

    auxsort(&cx, &tab, &cmp, 1, n as u32, 0)?;

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

#[cfg(feature = "rand")]
fn auxsort<A>(
    cx: &Context<A, Args>,
    tab: &Arg<A>,
    cmp: &Comparer<A>,
    mut lo: u32,
    mut up: u32,
    mut rnd: u32,
) -> Result<(), Box<dyn core::error::Error>> {
    while lo < up {
        let a = cx.thread().index(tab, up)?;
        let b = cx.thread().index(tab, lo)?;

        if sort_comp(cx, &a, &b, cmp)? {
            cx.thread().set(tab, lo, a)?;
            cx.thread().set(tab, up, b)?;
        }

        if (up - lo) == 1 {
            return Ok(());
        }

        // Get Pivot index.
        let p = if (up - lo) < 100 || rnd == 0 {
            lo.wrapping_add(up) / 2
        } else {
            choose_pivot(lo, up, rnd)
        };

        let a = cx.thread().index(tab, p)?;
        let b = cx.thread().index(tab, lo)?;

        if sort_comp(cx, &a, &b, cmp)? {
            cx.thread().set(tab, p, b)?;
            cx.thread().set(tab, lo, a)?;
        } else {
            let b = cx.thread().index(tab, up)?;

            if sort_comp(cx, &b, &a, cmp)? {
                cx.thread().set(tab, p, b)?;
                cx.thread().set(tab, up, a)?;
            }
        }

        if (up - lo) == 2 {
            return Ok(());
        }

        let a = cx.thread().index(tab, p)?;
        let b = cx.thread().index(tab, up.wrapping_sub(1))?;

        cx.thread().set(tab, p, b)?;
        cx.thread().set(tab, up.wrapping_sub(1), &a)?;

        let p = partition(cx, tab, a, lo, up, cmp)?;
        let n;

        if p.wrapping_sub(lo) < up.wrapping_sub(p) {
            auxsort(cx, tab, cmp, lo, p.wrapping_sub(1), rnd)?;
            n = p.wrapping_sub(lo);
            lo = p.wrapping_add(1);
        } else {
            auxsort(cx, tab, cmp, p.wrapping_add(1), up, rnd)?;
            n = up.wrapping_sub(p);
            up = p.wrapping_sub(1);
        }

        if up.wrapping_sub(lo) / 128 > n {
            rnd = rand::random();
        }
    }

    Ok(())
}

fn sort_comp<A>(
    cx: &Context<A, Args>,
    a: &Value<A>,
    b: &Value<A>,
    cmp: &Comparer<A>,
) -> Result<bool, Box<dyn core::error::Error>> {
    let r: Value<_> = match cmp {
        Comparer::Default => return cx.is_value_lt(a, b),
        Comparer::Fp(f) => cx.thread().call(*f, (a, b))?,
        Comparer::LuaFn(f) => cx.thread().call(*f, (a, b))?,
    };

    Ok(r.to_bool())
}

fn choose_pivot(lo: u32, up: u32, rnd: u32) -> u32 {
    let r4 = up.wrapping_sub(lo) / 4;

    rnd.wrapping_rem(r4 * 2).wrapping_add(lo.wrapping_add(r4))
}

fn partition<A>(
    cx: &Context<A, Args>,
    tab: &Arg<A>,
    a: Value<A>,
    lo: u32,
    up: u32,
    cmp: &Comparer<A>,
) -> Result<u32, Box<dyn core::error::Error>> {
    let mut i = lo;
    let mut j = up.wrapping_sub(1);

    loop {
        let b = loop {
            i = i.wrapping_add(1);

            let b = cx.thread().index(tab, i)?;

            if !sort_comp(cx, &b, &a, cmp)? {
                break b;
            }

            if i == up.wrapping_sub(1) {
                return Err("invalid order function for sorting".into());
            }
        };

        let c = loop {
            j = j.wrapping_sub(1);

            let c = cx.thread().index(tab, j)?;

            if !sort_comp(cx, &a, &c, cmp)? {
                break c;
            }

            if j < i {
                return Err("invalid order function for sorting".into());
            }
        };

        if j < i {
            cx.thread().set(tab, up.wrapping_sub(1), b)?;
            cx.thread().set(tab, i, a)?;

            return Ok(i);
        }

        cx.thread().set(tab, i, c)?;
        cx.thread().set(tab, j, b)?;
    }
}

enum Comparer<'a, A> {
    Default,
    Fp(Fp<A>),
    LuaFn(&'a LuaFn<A>),
}
