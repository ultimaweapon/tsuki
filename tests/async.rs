use core::error::Error;
use core::time::Duration;
use tokio::task::{JoinSet, LocalSet};
use tsuki::builtin::{BaseLib, CoroLib, IoLib, MathLib, StringLib, TableLib, Utf8Lib};
use tsuki::{Args, ChunkInfo, Context, Lua, LuaFn, Ref, RegKey, Ret, fp};

#[test]
fn async_call() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let exec = LocalSet::new();
    let lua = Lua::new(());
    let chunk = lua.load(ChunkInfo::new("async.lua"), "sleep()").unwrap();

    lua.use_module(None, true, BaseLib).unwrap();
    lua.use_module(None, true, CoroLib).unwrap();
    lua.use_module(None, true, IoLib).unwrap();
    lua.use_module(None, true, MathLib).unwrap();
    lua.use_module(None, true, StringLib).unwrap();
    lua.use_module(None, true, TableLib).unwrap();
    lua.use_module(None, true, Utf8Lib).unwrap();

    lua.set_registry::<Chunk>(chunk);

    lua.global().set_str_key("sleep", fp!(sleep as async));

    exec.block_on(&rt, async move {
        let mut tasks = JoinSet::new();

        for _ in 0..10 {
            let lua = lua.clone();

            tasks.spawn_local(async move {
                let th = lua.create_thread();
                let chunk = lua.registry::<Chunk>().unwrap();

                th.async_call::<()>(chunk, ()).await.unwrap();
            });
        }

        tasks.join_all().await;
    })
}

async fn sleep(cx: Context<'_, (), Args>) -> Result<Context<'_, (), Ret>, Box<dyn Error>> {
    tokio::time::sleep(Duration::from_secs(5)).await;
    Ok(cx.into())
}

struct Chunk;

impl<A> RegKey<A> for Chunk {
    type Value<'a>
        = Ref<'a, LuaFn<A>>
    where
        A: 'a;
}
