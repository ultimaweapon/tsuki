use std::ops::Deref;
use std::path::PathBuf;
use std::sync::LazyLock;
use tsuki::{Builder, ChunkInfo, lua_load, lua_pcall};

#[test]
fn badkey() {
    run("badkey.lua").unwrap();
}

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
fn math() {
    run("math.lua").unwrap();
}

#[test]
fn print() {
    run("print.lua").unwrap();
}

#[test]
fn tpack() {
    run("tpack.lua").unwrap();
}

#[test]
fn strings() {
    run("strings.lua").unwrap();
}

#[test]
fn vararg() {
    run("vararg.lua").unwrap();
}

fn run(file: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Get path.
    let mut path = ROOT.join("tests");

    path.push("cases");
    path.push(file);

    // Setup Lua.
    let content = std::fs::read(&path).unwrap();
    let lua = Builder::new().enable_all().build();
    let lua = lua.spawn();
    let info = ChunkInfo {
        name: path.to_string_lossy().into_owned(),
    };

    // Run.
    let mut r = unsafe { lua_load(lua.deref(), info, content) };

    if r.is_ok() {
        r = unsafe { lua_pcall(lua.deref(), 0, 0) };
    }

    r
}

static ROOT: LazyLock<PathBuf> = LazyLock::new(|| std::env::current_dir().unwrap());
