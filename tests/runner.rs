use std::path::PathBuf;
use std::sync::LazyLock;
use tsuki::{CallError, ChunkInfo, Lua};

#[test]
fn badkey() {
    run("badkey.lua").unwrap();
}

#[test]
fn close() {
    run("close.lua").unwrap();
}

#[test]
fn closure() {
    run("closure.lua").unwrap();
}

#[test]
fn error() {
    let e = run("error.lua")
        .unwrap_err()
        .downcast::<CallError>()
        .unwrap();
    let (f, l) = e.location().unwrap();

    assert!(f.ends_with("error.lua"));
    assert_eq!(l, 2);
    assert_eq!(e.to_string(), "oh no");
}

#[test]
#[ignore = "need Lua standard library"]
fn errors() {
    run("errors.lua").unwrap();
}

#[test]
#[ignore = "need Lua standard library"]
fn events() {
    run("events.lua").unwrap();
}

#[test]
#[ignore = "need Lua standard library"]
fn math() {
    run("math.lua").unwrap();
}

#[test]
fn print() {
    run("print.lua").unwrap();
}

#[test]
#[ignore = "need Lua standard library"]
fn tpack() {
    run("tpack.lua").unwrap();
}

#[test]
#[ignore = "need Lua standard library"]
fn strings() {
    run("strings.lua").unwrap();
}

#[test]
#[ignore = "need Lua standard library"]
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
    let lua = Lua::new();

    lua.setup_base();
    lua.setup_string();
    lua.setup_table();
    lua.setup_math();

    // Run.
    let chunk = lua.load(ChunkInfo::new(path.to_string_lossy().into_owned()), content)?;
    let th = lua.spawn();

    th.call::<()>(chunk, ())?;

    Ok(())
}

static ROOT: LazyLock<PathBuf> = LazyLock::new(|| std::env::current_dir().unwrap());
