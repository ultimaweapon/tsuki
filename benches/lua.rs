use core::ops::Deref;
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use std::path::PathBuf;
use tokio::task::{LocalSet, yield_now};

criterion_main!(benches);
criterion_group!(benches, fannkuch_redux, async_call);

fn fannkuch_redux(c: &mut Criterion) {
    let mut g = c.benchmark_group("fannkuch-redux");
    let src = read_source();

    {
        let lua = tsuki::Lua::new(());
        let td = lua.create_thread();
        let chunk = lua
            .load(tsuki::ChunkInfo::new("fannkuch-redux.lua"), &src)
            .unwrap();
        let f = match td.call(chunk, ()).unwrap() {
            tsuki::Value::LuaFn(v) => v,
            _ => unreachable!(),
        };

        g.bench_function("tsuki", |b| b.iter(|| td.call::<()>(f.deref(), 6).unwrap()));
    }

    {
        let lua = mlua::Lua::new();
        let f = lua.load(&src).call::<mlua::Function>(()).unwrap();

        g.bench_function("mlua", |b| b.iter(|| f.call::<()>(6).unwrap()));
    }

    g.finish();
}

fn async_call(c: &mut Criterion) {
    let mut g = c.benchmark_group("async-call");
    let yields = [0, 1, 2, 4, 8];

    {
        let lua = tsuki::Lua::new(());
        let th = lua.create_thread();
        let chunk = lua
            .load(
                tsuki::ChunkInfo::new("async-call.lua"),
                "return asyncfn(...)",
            )
            .unwrap();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let exec = LocalSet::new();

        lua.global()
            .set_str_key("asyncfn", tsuki::fp!(asyncfn as async));

        for yc in &yields {
            g.bench_with_input(BenchmarkId::new("tsuki", yc), yc, |b, &yc| {
                b.iter(|| {
                    exec.block_on(&rt, async {
                        tokio::task::spawn_local(async {
                            loop {
                                yield_now().await;
                            }
                        });

                        th.async_call::<()>(chunk.deref(), yc).await.unwrap()
                    })
                });
            });
        }

        async fn asyncfn(
            cx: tsuki::context::Context<'_, (), tsuki::context::Args>,
        ) -> Result<tsuki::context::Context<'_, (), tsuki::context::Ret>, Box<dyn std::error::Error>>
        {
            let yc = cx.arg(1).to_int()?;

            for _ in 0..yc {
                yield_now().await;
            }

            Ok(cx.into())
        }
    }

    {
        let lua = mlua::Lua::new();
        let chunk = lua.load("return asyncfn(...)").into_function().unwrap();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let exec = LocalSet::new();
        let f = lua
            .create_async_function(|_, yc: i64| async move {
                for _ in 0..yc {
                    yield_now().await;
                }

                Ok(())
            })
            .unwrap();

        lua.globals().set("asyncfn", f).unwrap();

        for yc in &yields {
            g.bench_with_input(BenchmarkId::new("mlua", yc), yc, |b, &yc| {
                b.iter(|| {
                    exec.block_on(&rt, async {
                        tokio::task::spawn_local(async {
                            loop {
                                yield_now().await;
                            }
                        });

                        chunk.call_async::<()>(yc).await.unwrap();
                    })
                });
            });
        }
    }

    g.finish();
}

fn read_source() -> Vec<u8> {
    let path = PathBuf::from_iter(["benches", "fannkuch-redux.lua"]);

    std::fs::read(path).unwrap()
}
