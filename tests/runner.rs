use std::path::PathBuf;
use std::sync::LazyLock;
use tsuki::{Builder, ChunkInfo};

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
    assert!(
        run("error.lua")
            .unwrap_err()
            .to_string()
            .ends_with("error.lua:2: oh no")
    );
}

#[test]
fn errors() {
    run("errors.lua").unwrap();
}

#[test]
fn events() {
    run("events.lua").unwrap();
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
    let lua = Builder::default().enable_all().build();
    let chunk = lua.load(ChunkInfo::new(path.to_string_lossy().into_owned()), content)?;

    // Run.
    let th = lua.spawn();

    pollster::block_on(async move { th.call(&chunk, ()).await })?;

    Ok(())
}

static ROOT: LazyLock<PathBuf> = LazyLock::new(|| std::env::current_dir().unwrap());
