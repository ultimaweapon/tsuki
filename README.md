# Tsuki

Tsuki is a port of Lua 5.4 to Rust. This is porting, not binding; which mean all code are Rust and can be using without C compiler. The initial works was done by [C2Rust](https://github.com/immunant/c2rust). Note that the port was done **without** compatibility with the previous version. You can see a list of the differences [here](https://www.lua.org/manual/5.4/manual.html#8).

> [!IMPORTANT]
> Tsuki does not support multi-threading and no plan to support this.

## Features

- Support both synchronous and asynchronous.
- Safe and low overhead API.
- Any error propagated to the caller via Rust `Result` instead of a long jump.
- All values owned by Rust will exempt from GC automatically (no need to move it to Lua registry).

## Differences from Lua

- Binary chunk is not supported.
- Panic when memory allocation is failed without retry (Rust behavior).
- Chunk name does not have a prefix (e.g. `@`).
- No `_VERSION`, `collectgarbage`, `dofile`, `loadfile`, `xpcall`, `string.dump` and debug library.
- No main thread and second result of `coroutine.running` always `false`.
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
- Second argument to `__close` metamethod always `nil`.
- `__gc` metamethod is not supported.
- `__name` metavalue must be UTF-8 string.
- `__tostring` metamethod must return a UTF-8 string.
- Native module is not supported.
- Environment variable `LUA_PATH` and `LUA_PATH_5_4` is ignored.
- `LUA_NOENV` in registry is ignored.
- C locale is ignored.

## Non-goals

- Becoming a superset of Lua (e.g. Luau).
- C API compatibility.
- Stand-alone mode.
- 16-bit systems.

## License

MIT
