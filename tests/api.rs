use std::ops::Deref;
use std::path::PathBuf;
use std::ptr::null;
use tsuki::{Builder, lua_load};

#[test]
fn dump() {
    // Load.
    let path = PathBuf::from_iter(["lua", "testes", "api.lua"]);
    let chunk = std::fs::read(path).unwrap();
    let lua = Builder::new().build();
    let th = lua.spawn();

    unsafe { lua_load(th.deref(), null(), chunk).unwrap() };
}
