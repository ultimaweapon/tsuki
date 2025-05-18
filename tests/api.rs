use std::path::PathBuf;
use std::ptr::null;
use tsuki::{lua_close, lua_newstate, luaL_loadbufferx};

#[test]
fn dump() {
    // Load.
    let path = PathBuf::from_iter(["lua", "testes", "api.lua"]);
    let chunk = std::fs::read(path).unwrap();
    let td = unsafe { lua_newstate() };

    unsafe { luaL_loadbufferx(td, chunk, c"".as_ptr(), null()).unwrap() };
    unsafe { lua_close(td) };
}
