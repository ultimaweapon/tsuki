use std::path::PathBuf;
use std::ptr::null;
use tsuki::{Lua, lua_closethread, lua_load};

#[test]
fn dump() {
    // Load.
    let path = PathBuf::from_iter(["lua", "testes", "api.lua"]);
    let chunk = std::fs::read(path).unwrap();
    let lua = Lua::new().unwrap();
    let td = lua.spawn();

    unsafe { lua_load(td, null(), null(), chunk).unwrap() };
    unsafe { lua_closethread(td).unwrap() };
}
