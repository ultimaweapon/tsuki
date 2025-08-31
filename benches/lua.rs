use core::ops::Deref;
use criterion::{Criterion, criterion_group, criterion_main};
use std::path::PathBuf;

criterion_main!(benches);
criterion_group!(benches, fannkuch_redux);

fn fannkuch_redux(c: &mut Criterion) {
    let mut g = c.benchmark_group("fannkuch-redux");
    let src = read_source();

    {
        let lua = tsuki::Lua::new(());
        let chunk = lua
            .load(tsuki::ChunkInfo::new("fannkuch-redux.lua"), &src)
            .unwrap();
        let f: tsuki::Value<_> = lua.call(chunk, ()).unwrap();
        let f = match f {
            tsuki::Value::LuaFn(v) => v,
            _ => unreachable!(),
        };

        g.bench_function("tsuki", |b| {
            b.iter(|| lua.call::<()>(f.deref(), 6).unwrap())
        });
    }

    {
        let lua = mlua::Lua::new();
        let f = lua.load(&src).call::<mlua::Function>(()).unwrap();

        g.bench_function("mlua", |b| b.iter(|| f.call::<()>(6).unwrap()));
    }

    g.finish();
}

fn read_source() -> Vec<u8> {
    let path = PathBuf::from_iter(["benches", "fannkuch-redux.lua"]);

    std::fs::read(path).unwrap()
}
