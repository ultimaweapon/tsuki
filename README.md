# Tsuki

Tsuki is a port of vanilla Lua 5.4 to Rust. This is porting, not binding; which mean all code are Rust and can be using without C compiler. The initial works was done by [C2Rust](https://github.com/immunant/c2rust). Note that the port was done **without** compatibility with the previous version. You can see a list of the differences [here](https://www.lua.org/manual/5.4/manual.html#8).

> [!WARNING]
> Tsuki currently in alpha stage. Do not use it on production!

## Features

- Support both synchronous and asynchronous.
  - Coroutine can only yield within async context.
  - All metamethod and iterator function cannot be async and cannot yield.
  - Each call into Rust async function from Lua always incur one heap allocation.
- Safe and low overhead API.
  - Direct access to Lua object instead of access it via Lua stack.
  - A call to small function get inlined due to all code are Rust.
  - Fast path to get Rust string from Lua string without checking if UTF-8 on every access.
- Any error propagated to the caller via Rust `Result` instead of a long jump.
- All values owned by Rust will exempt from GC automatically (no need to move it to Lua registry).

## Differences from vanilla Lua

- Binary chunk is not supported.
- Panic when memory allocation is failed without retry.
- Chunk name does not have a prefix (e.g. `@`).
- No `_VERSION`, `collectgarbage`, `dofile`, `loadfile`, `xpcall` and `string.dump`.
- Second argument of `assert` accept only a UTF-8 string.
- Arguments of `error`:
  - First argument accept only a UTF-8 string.
  - Second argument is not supported and it is always assume 1.
- Arguments of `load`:
  - First argument accept only a string.
  - Second argument will be an empty string when absent.
  - Third argument must be `nil`.
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
