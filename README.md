# Tsuki

Tsuki is a port of vanilla Lua 5.4 to Rust. This is porting, not binding; which mean all code are Rust and can be using without C compiler. The initial works was done by [C2Rust](https://github.com/immunant/c2rust). Note that the port was done **without** compatibility with the previous version. You can see a list of the differences [here](https://www.lua.org/manual/5.4/manual.html#8).

> [!WARNING]
> Tsuki APIs are highly unstable.

## Differences from vanilla

- Execution can only yield with Rust await.
- No `xpcall` in basic library.
- C locale is ignored.

## Non-goals

- Becoming a superset of Lua (e.g. Luau).
- C API compatibility.
- Stand-alone mode.

## License

MIT
