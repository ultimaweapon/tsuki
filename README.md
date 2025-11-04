# Tsuki
[![Crates.io Version](https://img.shields.io/crates/v/tsuki)](https://crates.io/crates/tsuki)

Tsuki is a port of Lua 5.4 to Rust. This is a port, not binding; which mean all code are Rust and can be using without C compiler[^1]. The initial works was done by [C2Rust](https://github.com/immunant/c2rust). Note that this port was done **without** compatibility with the previous version. You can see a list of the differences [here](https://www.lua.org/manual/5.4/manual.html#8).

> [!WARNING]
> Tsuki currently in a pre-1.0 so prepare for a lot of breaking changes!

## Status

The VM to run Lua code is fully working almost exactly as vanilla Lua (see some of differences below). Some functions on Lua standard library are still missing.

> [!IMPORTANT]
> All types in Tsuki does not implement `Send` and `Sync` and no plan to support this at the moment.

## Safety

All public API of Tsuki should provide 100% safety as long as you don't use unsafe API incorrectly.

Tsuki was not designed to run untrusted Lua script. Although you can limit what Lua script can do by not expose a function to it but there is no way to limit amount of memory or execution time used by Lua script. The meaning of this is Lua script can cause a panic due to out of memory or never return the control back to Rust with infinite loop.

## Performance

### Interpreter

Tsuki is slower than Lua about 60%. The only possibility for Tsuki to be faster than Lua with computed goto is JIT since computed goto does not available on Rust.

### Async

A call to async function without any suspend on Tsuki is faster than mlua about 3.5x. For 1 suspend Tsuki it faster about 3x. For 8 suspend Tsuki is faster about 2x.

## Features

- 100% Rust code.
  - [libc](https://crates.io/crates/libc) is required at the moment.
- Support both synchronous and asynchronous.
- Safe, ergonomic and low overhead API.
- Strongly typed registry.
- Rust collections to store Lua values (e.g. [BTreeMap](https://doc.rust-lang.org/alloc/collections/btree_map/struct.BTreeMap.html)).
- Any error propagated to the caller via Rust `Result` instead of a long jump.
- `core::any::Any` as Lua userdata and can be created without the need to define its metatable.
- Metatable for a userdata is lookup with `core::any::TypeId` instead of a string.
- Property system on userdata to store per-object values for fast access from Lua.

## Differences from Lua

### VM and Language

- Binary chunk is not supported.
- Hook functions is not supported.
- Panic when memory allocation is failed without retry (same as Rust).
- GC has only one mode and cannot control from outside.
- Chunk name does not have a prefix (e.g. `@`).
- Second argument to `__close` metamethod always `nil`.
- `__gc` metamethod is not supported.
- `__name` metavalue must be UTF-8 string.
- `__tostring` metamethod must return a UTF-8 string.
- Float to string conversion does not truncate precision (Lua limit to 14 digits by default).
- Float literal does not accept hexadecimal format.
- U+000B VERTICAL TAB is not considered as a whitespace.
- C locale is ignored (once `libc` has been completely removed).

### Standard library

- No `_VERSION`, `collectgarbage`, `dofile`, `loadfile`, `warn`, `xpcall`, `string.dump` and debug library.
- Second argument of `assert` accept only a UTF-8 string.
- Arguments of `error`:
  - First argument accept only a UTF-8 string.
  - Second argument is not supported and it is always assume 1.
- Arguments of `load`:
  - First argument accept only a string.
  - Second argument accept only a UTF-8 string and will be empty when absent.
  - Third argument must be `nil` or `"t"`.
- `string.format` requires UTF-8 string for both format string and format value.
- `string.find` and `string.gsub` does not support class `z`.
- Native module is not supported.
- Environment variable `LUA_PATH` and `LUA_PATH_5_4` is ignored.
- `LUA_NOENV` in registry is ignored.

## Non-goals

- Become a superset of Lua (e.g. Luau).
- C API compatibility.
- Stand-alone mode.
- 16-bit systems.

## Roadmap

- Complete Lua standard library.
- Remove libc.
- JIT using Cranelift.

## Breaking changes in 0.3

- `TryCall` has been removed and `Context::try_forward` was merged with `Context::forward`.
- Return type of `Context::forward` has been changed.
- `Arg::get_metatable` was renamed to `Arg::metatable`.
- `Arg::as_str` has parameter to allow converting a number to string.
- `Arg::to_str` and `Arg::to_nilable_str` now convert a number to string in-place.
- `Arg::to_float` and `Arg::to_nilable_float` now return `Float` instead of `f64`.
- `Number` No longer implement `PartialEq`.
- `Value::Float` and `Number::Float` value is changed from `f64` to `Float`.
- `Value::Fp` value is changed from `fn` to `Fp`.
- `Value::AsyncFp` value is changed from `fn` to `AsyncFp`.
- `Value::Bool` has been replaced with `Value::False` and `Value::True`.
- `Thread::async_call` now accept only `LuaFn`.
- `Module::Instance` was renamed to `Module::Inst`.
- `StringLib` was renamed to `StrLib`.
- `Table::contains_str_key` now requires `AsRef<str>` on the key. Use `Table::contains_bytes_key` if you want old requirements.
- `Table::get_str_key` now requires `AsRef<str>` on the key. Use `Table::get_bytes_key` if you want old requirements.
- `Ops` now a private type.
- `Context` and its related types now live in `context` module.
- Float to string conversion does not truncate precision (Lua limit this to 14 digits by default).
- Float literal no longer accept hexadecimal format.
- U+000B VERTICAL TAB no longer considered as a whitespace.

## License

Same as Lua, which is MIT.

[^1]: On Windows, a proxy to `sprintf` written in C++ is required at the moment. This proxy will be removed when we replace `sprintf` calls with Rust equivalent.
