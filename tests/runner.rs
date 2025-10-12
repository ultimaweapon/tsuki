use std::path::PathBuf;
use std::sync::LazyLock;
use tsuki::builtin::{BaseLib, CoroLib, IoLib, MathLib, StringLib, TableLib, Utf8Lib};
use tsuki::{Args, CallError, ChunkInfo, Context, Lua, Ret, fp};

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

    fn createud(cx: Context<(), Args>) -> Result<Context<(), Ret>, Box<dyn core::error::Error>> {
        let ud = cx.create_ud(MyUd(String::from("abc")));

        cx.push(ud)?;

        Ok(cx.into())
    }

    fn method1(cx: Context<(), Args>) -> Result<Context<(), Ret>, Box<dyn core::error::Error>> {
        let ud = cx.arg(1).get_ud::<MyUd>()?;

        cx.push_str(ud.value().0.as_str())?;

        Ok(cx.into())
    }

    run("userdata.lua", |lua| {
        let mt = lua.create_table();
        let methods = lua.create_table();

        methods.set_str_key("method1", fp!(method1));

        mt.set_str_key("__index", methods);

        lua.register_metatable::<MyUd>(&mt);
        lua.global().set_str_key("createud", fp!(createud))
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

    lua.use_module(None, true, BaseLib).unwrap();
    lua.use_module(None, true, CoroLib).unwrap();
    lua.use_module(None, true, IoLib).unwrap();
    lua.use_module(None, true, MathLib).unwrap();
    lua.use_module(None, true, StringLib).unwrap();
    lua.use_module(None, true, TableLib).unwrap();
    lua.use_module(None, true, Utf8Lib).unwrap();

    setup(&lua);

    // Run.
    let chunk = lua.load(ChunkInfo::new(path.to_string_lossy().into_owned()), content)?;

    lua.call::<()>(chunk, ())?;

    Ok(())
}

static ROOT: LazyLock<PathBuf> = LazyLock::new(|| std::env::current_dir().unwrap());
