use std::ptr::null;
use tsuki::{
    lua_close, lua_pcall, lua_pop, luaL_loadbufferx, luaL_newstate, luaL_requiref, luaopen_base,
};

#[test]
fn basic_print() {
    run("basic", "print.lua");
}

fn run(cat: &str, file: &str) {
    // Get path.
    let mut path = std::env::current_dir().unwrap();

    path.push("tests");
    path.push(cat);
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
