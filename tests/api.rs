use std::path::PathBuf;
use tsuki::{ChunkInfo, Lua};

#[test]
fn load() {
    let path = PathBuf::from_iter(["lua", "testes", "api.lua"]);
    let chunk = std::fs::read(path).unwrap();
    let lua = Lua::new(());
    let info = ChunkInfo::new("api.lua");

    lua.load(info, chunk).unwrap();
}
