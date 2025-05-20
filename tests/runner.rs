use std::path::PathBuf;
use std::ptr::null;
use std::sync::LazyLock;
use tsuki::{
    lua_close, lua_newstate, lua_pcall, lua_pop, luaL_loadbufferx, luaL_requiref, luaopen_base,
    luaopen_math, luaopen_string, luaopen_table,
};

#[test]
fn close() {
    run("close.lua").unwrap();
}

#[test]
fn error() {
    assert!(
        run("error.lua")
            .unwrap_err()
            .to_string()
            .ends_with("error.lua:2: oh no")
    );
}

#[test]
fn gc() {
    run("gc.lua").unwrap();
}

#[test]
fn math() {
    run("math.lua").unwrap();
}

#[test]
fn print() {
    run("print.lua").unwrap();
}

#[test]
fn strings() {
    run("strings.lua").unwrap();
}

fn run(file: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Get path.
    let mut path = ROOT.join("tests");

    path.push("cases");
    path.push(file);

    // Setup Lua.
    let content = std::fs::read(&path).unwrap();
    let lua = unsafe { lua_newstate() };

    unsafe { luaL_requiref(lua, c"_G".as_ptr(), luaopen_base, 0).unwrap() };
    unsafe { lua_pop(lua, 1).unwrap() };
    unsafe { luaL_requiref(lua, c"math".as_ptr(), luaopen_math, 1).unwrap() };
    unsafe { lua_pop(lua, 1).unwrap() };
    unsafe { luaL_requiref(lua, c"string".as_ptr(), luaopen_string, 1).unwrap() };
    unsafe { lua_pop(lua, 1).unwrap() };
    unsafe { luaL_requiref(lua, c"table".as_ptr(), luaopen_table, 1).unwrap() };
    unsafe { lua_pop(lua, 1).unwrap() };

    // Build chunk name.
    let mut name = String::with_capacity(1 + path.as_os_str().len() + 1);

    name.push('@');
    name.push_str(path.to_str().unwrap());
    name.push('\0');

    // Run.
    let mut r = unsafe { luaL_loadbufferx(lua, content, name.as_ptr().cast(), null()) };

    if r.is_ok() {
        r = unsafe { lua_pcall(lua, 0, 0) };
    }

    unsafe { lua_close(lua) };
    r
}

static ROOT: LazyLock<PathBuf> = LazyLock::new(|| std::env::current_dir().unwrap());
