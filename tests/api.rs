use std::path::PathBuf;
use tsuki::{ChunkInfo, Lua};

#[test]
fn dump() {
    // Load.
    let path = PathBuf::from_iter(["lua", "testes", "api.lua"]);
    let chunk = std::fs::read(path).unwrap();
    let lua = Lua::new();
    let info = ChunkInfo::default();

    lua.load(info, chunk).unwrap();
}
