use std::path::PathBuf;
use tsuki::{Builder, ChunkInfo};

#[test]
fn dump() {
    // Load.
    let path = PathBuf::from_iter(["lua", "testes", "api.lua"]);
    let chunk = std::fs::read(path).unwrap();
    let lua = Builder::default().build();
    let info = ChunkInfo::default();

    lua.load(info, chunk).unwrap();
}
