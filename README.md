# Tsuki
[![Crates.io Version](https://img.shields.io/crates/v/tsuki)](https://crates.io/crates/tsuki)

Tsuki is a port of Lua 5.4 to Rust. This is porting, not binding; which mean all code are Rust and can be using without C compiler[^1]. The initial works was done by [C2Rust](https://github.com/immunant/c2rust). Note that this port was done **without** compatibility with the previous version. You can see a list of the differences [here](https://www.lua.org/manual/5.4/manual.html#8).

> [!IMPORTANT]
> All types in Tsuki does not implement `Send` and `Sync` and no plan to support this at the moment.

## Status

The VM to run Lua code is fully working almost exactly as vanilla Lua (see some of differences below). Some functions on Lua standard library are still missing.

## Safety

All public API of Tsuki should provide 100% safety as long as you don't use unsafe API incorrectly.

Tsuki is not designed to run untrusted Lua script. Although you can limit what Lua script can do by not expose a function to it but there is no way to limit amount of memory or execution time used by Lua script. The meaning of this is Lua script can cause a panic due to out of memory or never return the control back to Rust with infinite loop.

## Performance

### VM

On platform that Lua cannot use computed goto (e.g. Windows with MSVC) Tsuki VM is faster than Lua about 10% otherwise Lua is faster about 30%. The only possibility for Tsuki to be faster than Lua with computed goto is JIT since computed goto does not available on Rust. See issue [18](https://github.com/ultimaweapon/tsuki/issues/18) for more details.

### Async

A call to async function without any suspend on Tsuki is faster than mlua about 3.5x. For 1 suspend Tsuki it faster about 3x. For 8 suspend Tsuki is faster about 2x.

## Features

- 100% Rust code.
  - [libc](https://crates.io/crates/libc) is required at the moment.
- Support both synchronous and asynchronous.
- Safe, ergonomic and low overhead API.
- Any error propagated to the caller via Rust `Result` instead of a long jump.
- `core::any::Any` as Lua userdata and can be created without the need to define its metatable.
- Metatable for a userdata is lookup with `core::any::TypeId` instead of a string.

## Differences from Lua

### VM and Language

- Binary chunk is not supported.
- Panic when memory allocation is failed without retry (Rust behavior).
- Chunk name does not have a prefix (e.g. `@`).
- Second argument to `__close` metamethod always `nil`.
- `__gc` metamethod is not supported.
- `__name` metavalue must be UTF-8 string.
- `__tostring` metamethod must return a UTF-8 string.
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

## Breaking changes in 0.2

- `Lua::with_seed` has parameters swapped.
- `Lua::setup_base` has been replaced with `BaseLib`.
- `Lua::setup_string` has been replaced with `StringLib`.
- `Lua::setup_table` has been replaced with `TableLib`.
- `Lua::setup_math` has been replaced with `MathLib`.
- `Lua::setup_coroutine` has been replaced with `CoroLib`.
- `Lua::load` and `Context::load` accept `Into<ChunkInfo>` instead of `ChunkInfo`.
- `Arg::get_str` and `Arg::get_nilable_str` no longer accept a number. Use `Arg::to_str` or `Arg::to_nilable_str` instead of you want old behavior.
- `Arg::to_num` renamed to `Arg::to_float`.
- `Arg::to_nilable_num` renamed to `Arg::to_nilable_float`.
- `Arg::len` is removed in favor of `Context::get_value_len`.
- `Arg::lt` is removed in favor of `Context::is_value_lt`.
- `Value::Num` is renamed to `Value::Float`.
- `ChunkInfo` no longer implement `Default`.
- `Str::is_utf8` and `Str::as_str` now lazy evaluate the content to see if data is UTF-8.

## License

Same as Lua, which is MIT.

[^1]: On Windows, a proxy to `sprintf` written in C++ is required at the moment. This proxy will be removed when we replace `sprintf` calls with Rust equivalent.
