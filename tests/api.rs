use std::ops::Deref;
use std::path::PathBuf;
use tsuki::{Builder, ChunkInfo, lua_load};

#[test]
fn dump() {
    // Load.
    let path = PathBuf::from_iter(["lua", "testes", "api.lua"]);
    let chunk = std::fs::read(path).unwrap();
    let lua = Builder::new().build();
    let th = lua.spawn();
    let info = ChunkInfo::default();

    unsafe { lua_load(th.deref(), info, chunk).unwrap() };
}
