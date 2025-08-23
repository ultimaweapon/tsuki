use std::path::PathBuf;
use std::sync::LazyLock;
use tsuki::{Args, CallError, ChunkInfo, Context, Fp, Lua, Ret};

#[test]
fn badkey() {
    run("badkey.lua", |_| {}).unwrap();
}

#[test]
fn close() {
    run("close.lua", |_| {}).unwrap();
}

#[test]
fn closure() {
    run("closure.lua", |_| {}).unwrap();
}

#[test]
#[ignore = "need Lua standard library"]
fn coroutine() {
    run("coroutine.lua", |_| {}).unwrap();
}

#[test]
fn error() {
    let e = run("error.lua", |_| {})
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
    run("errors.lua", |_| {}).unwrap();
}

#[test]
#[ignore = "need Lua standard library"]
fn events() {
    run("events.lua", |_| {}).unwrap();
}

#[test]
#[ignore = "need Lua standard library"]
fn math() {
    run("math.lua", |_| {}).unwrap();
}

#[test]
fn print() {
    run("print.lua", |_| {}).unwrap();
}

#[test]
#[ignore = "need Lua standard library"]
fn tpack() {
    run("tpack.lua", |_| {}).unwrap();
}

#[test]
#[ignore = "need Lua standard library"]
fn strings() {
    run("strings.lua", |_| {}).unwrap();
}

#[test]
fn userdata() {
    struct MyUd(String);

    impl Drop for MyUd {
        fn drop(&mut self) {
            println!("{}", self.0);
        }
    }

    fn f(cx: Context<(), Args>) -> Result<Context<(), Ret>, Box<dyn core::error::Error>> {
        let ud = cx.create_ud(MyUd(String::from("abc")));

        cx.push(ud)?;

        Ok(cx.into())
    }

    run("userdata.lua", |lua| {
        lua.global().set_str_key("createud", Fp(f))
    })
    .unwrap()
}

#[test]
fn vararg() {
    run("vararg.lua", |_| {}).unwrap();
}

fn run(file: &str, setup: impl FnOnce(&Lua<()>)) -> Result<(), Box<dyn std::error::Error>> {
    // Get path.
    let mut path = ROOT.join("tests");

    path.push("cases");
    path.push(file);

    // Setup Lua.
    let content = std::fs::read(&path).unwrap();
    let lua = Lua::new(());

    lua.setup_base();
    lua.setup_string();
    lua.setup_table();
    lua.setup_math();
    lua.setup_coroutine();

    setup(&lua);

    // Run.
    let chunk = lua.load(ChunkInfo::new(path.to_string_lossy().into_owned()), content)?;

    lua.call::<()>(chunk, ())?;

    Ok(())
}

static ROOT: LazyLock<PathBuf> = LazyLock::new(|| std::env::current_dir().unwrap());
