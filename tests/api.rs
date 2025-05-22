use std::path::PathBuf;
use std::ptr::null;
use tsuki::{Lua, lua_closethread, luaL_loadbufferx};

#[test]
fn dump() {
    // Load.
    let path = PathBuf::from_iter(["lua", "testes", "api.lua"]);
    let chunk = std::fs::read(path).unwrap();
    let lua = Lua::new().unwrap();
    let td = lua.spawn();

    unsafe { luaL_loadbufferx(td, chunk, c"".as_ptr(), null()).unwrap() };
    unsafe { lua_closethread(td).unwrap() };
}
