use std::path::PathBuf;
use tsuki::Lua;

#[test]
fn load() {
    let path = PathBuf::from_iter(["lua", "testes", "api.lua"]);
    let chunk = std::fs::read(path).unwrap();
    let lua = Lua::new(());

    lua.load("api.lua", chunk).unwrap();
}
