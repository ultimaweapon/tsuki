use std::path::PathBuf;
use std::ptr::null;
use std::sync::LazyLock;
use tsuki::{
    lua_close, lua_pcall, lua_pop, luaL_loadbufferx, luaL_newstate, luaL_requiref, luaopen_base,
};

#[test]
fn close() {
    run("close.lua");
}

#[test]
fn print() {
    run("print.lua");
}

fn run(file: &str) {
    // Get path.
    let mut path = ROOT.join("tests");

    path.push("cases");
    path.push(file);

    // Setup Lua.
    let content = std::fs::read(path).unwrap();
    let lua = unsafe { luaL_newstate() };

    unsafe { luaL_requiref(lua, c"_G".as_ptr(), luaopen_base, 0).unwrap() };
    unsafe { lua_pop(lua, 1).unwrap() };

    unsafe { luaL_loadbufferx(lua, content, c"".as_ptr(), null()).unwrap() };
    unsafe { lua_pcall(lua, 0, 0).unwrap() };

    unsafe { lua_close(lua) };
}

static ROOT: LazyLock<PathBuf> = LazyLock::new(|| std::env::current_dir().unwrap());
