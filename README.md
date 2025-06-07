# Tsuki

Tsuki is a port of vanilla Lua 5.4 to Rust. This is porting, not binding; which mean all code are Rust and can be using without C compiler. The initial works was done by [C2Rust](https://github.com/immunant/c2rust). Note that the port was done **without** compatibility with the previous version. You can see a list of the differences [here](https://www.lua.org/manual/5.4/manual.html#8).

> [!WARNING]
> Tsuki currently in alpha stage. Do not use it on production!

## Features

- Any error propagated to the caller via Rust `Result` instead of a long jump.
- All values owned by Rust will exempt from GC automatically (no need to move it to Lua registry).

## Differences from vanilla Lua

- Binary chunk is not supported and third argument of `load` must be `nil`.
- Panic when memory allocation is failed without retry.
- First argument of `error` and `load` accept only a string.
- Chunk name does not have a prefix (e.g. `@`) and second argument of `load` will be empty when absent.
- No `_VERSION`, `collectgarbage`, `dofile`, `loadfile`, `xpcall` and `string.dump`.
- `warn` is enabled by default without message prefixes and does not support control message.
- Second argument to `__close` metamethod always `nil`.
- `__gc` metamethod is not supported.
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
