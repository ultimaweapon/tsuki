# Tsuki

Tsuki is a port of vanilla Lua 5.4 to Rust. This is porting, not binding; which mean all code are Rust and can be using without C compiler. The initial works was done by [C2Rust](https://github.com/immunant/c2rust). Note that the port was done **without** compatibility with the previous version. You can see a list of the differences [here](https://www.lua.org/manual/5.4/manual.html#8).

> [!WARNING]
> Tsuki currently in alpha stage. Do not use it on production!

## Features

- Any error propagated to the caller via Rust `Result` instead of a long jump.

## Differences from vanilla Lua

- Panic when memory allocation is failed without retry.
- First argument of `error` accept only a string.
- No `xpcall` in basic library.
- `warn` is enabled by default without message prefixes and does not support control message.
- Second argument to `__close` metamethod always `nil`.
- `__gc` metamethod is ignored.
- C locale is ignored.

## Non-goals

- Becoming a superset of Lua (e.g. Luau).
- C API compatibility.
- Stand-alone mode.

## License

MIT
