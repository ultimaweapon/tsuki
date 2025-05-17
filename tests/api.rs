use std::path::PathBuf;
use std::ptr::null;
use tsuki::{lua_close, luaL_loadbufferx, luaL_newstate};

#[test]
fn dump() {
    // Load.
    let path = PathBuf::from_iter(["lua", "testes", "api.lua"]);
    let chunk = std::fs::read(path).unwrap();
    let td = unsafe { luaL_newstate() };

    unsafe { luaL_loadbufferx(td, chunk, c"".as_ptr(), null()).unwrap() };
    unsafe { lua_close(td) };
}
