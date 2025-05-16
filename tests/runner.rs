use std::ptr::null;
use tsuki::{
    lua_close, lua_pcallk, lua_pop, luaL_loadbufferx, luaL_newstate, luaL_requiref, luaopen_base,
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

    // Load.
    let status = unsafe { luaL_loadbufferx(lua, content, c"".as_ptr(), null()) };

    assert_eq!(status, 0);

    // Run.
    assert_eq!(unsafe { lua_pcallk(lua, 0, 0, 0, 0, None).unwrap() }, 0);

    unsafe { lua_close(lua) };
}
