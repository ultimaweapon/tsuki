use std::path::PathBuf;
use tsuki::{Builder, ChunkInfo};

#[test]
fn load() {
    let path = PathBuf::from_iter(["lua", "testes", "api.lua"]);
    let chunk = std::fs::read(path).unwrap();
    let lua = Builder::new().build(());
    let info = ChunkInfo::new("api.lua");

    lua.load(info, chunk).unwrap();
}
