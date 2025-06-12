use std::path::PathBuf;
use std::sync::LazyLock;
use tsuki::{Builder, ChunkInfo};

#[test]
#[ignore = "need Lua standard library"]
fn badkey() {
    run("badkey.lua").unwrap();
}

#[test]
#[ignore = "need Lua standard library"]
fn close() {
    run("close.lua").unwrap();
}

#[test]
#[ignore = "need Lua standard library"]
fn closure() {
    run("closure.lua").unwrap();
}

#[test]
#[ignore = "need Lua standard library"]
fn error() {
    assert!(
        run("error.lua")
            .unwrap_err()
            .to_string()
            .ends_with("error.lua:2: oh no")
    );
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
#[ignore = "need Lua standard library"]
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
    let lua = Builder::default().build();
    let chunk = lua.load(ChunkInfo::new(path.to_string_lossy().into_owned()), content)?;

    // Run.
    let th = lua.spawn();

    pollster::block_on(async move { th.call(&chunk, ()).await })?;

    Ok(())
}

static ROOT: LazyLock<PathBuf> = LazyLock::new(|| std::env::current_dir().unwrap());
