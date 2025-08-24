# Tsuki

Tsuki is a port of Lua 5.4 to Rust. This is porting, not binding; which mean all code are Rust and can be using without C compiler. The initial works was done by [C2Rust](https://github.com/immunant/c2rust). Note that this port was done **without** compatibility with the previous version. You can see a list of the differences [here](https://www.lua.org/manual/5.4/manual.html#8).

> [!IMPORTANT]
> Tsuki does not support multi-threading and no plan to support this at the moment.

## Status

Almost ready for release. Everything are working as expected and Lua test cases are passed. The developer want to revise the API that return Lua object before release. Note that the first release will have **incomplete** Lua standard library.

## Features

- 100% Rust code.
  - [libc](https://crates.io/crates/libc) is required at the moment.
- Support both synchronous and asynchronous.
- Safe and low overhead API.
- Any error propagated to the caller via Rust `Result` instead of a long jump.
- `core::any::Any` as Lua userdata and can be created without the need to define its metatable.
- Metatable for a userdata is lookup with `core::any::TypeId` instead of a string.

## Differences from Lua

### Language

- Binary chunk is not supported.
- Panic when memory allocation is failed without retry (Rust behavior).
- Chunk name does not have a prefix (e.g. `@`).
- Second argument to `__close` metamethod always `nil`.
- `__gc` metamethod is not supported.
- `__name` metavalue must be UTF-8 string.
- `__tostring` metamethod must return a UTF-8 string.
- C locale is ignored (once `libc` has been completely removed).

### Standard library

- No `_VERSION`, `collectgarbage`, `dofile`, `loadfile`, `xpcall`, `string.dump` and debug library.
- Second argument of `assert` accept only a UTF-8 string.
- Arguments of `error`:
  - First argument accept only a UTF-8 string.
  - Second argument is not supported and it is always assume 1.
- Arguments of `load`:
  - First argument accept only a string.
  - Second argument accept only a UTF-8 string and will be empty when absent.
  - Third argument must be `nil` or `"t"`.
- `warn` is enabled by default without message prefixes and does not support control message.
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

- Support Windows.
- Remove libc.
- JIT using Cranelift.

## License

MIT
