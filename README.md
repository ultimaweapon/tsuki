# Tsuki
[![Crates.io Version](https://img.shields.io/crates/v/tsuki)](https://crates.io/crates/tsuki)

Tsuki is a port of Lua 5.4 to Rust. This is a port, not binding; which mean all code are Rust and can be using without C compiler. The initial works was done by [C2Rust](https://github.com/immunant/c2rust). Note that this port was done **without** compatibility with the previous version. You can see a list of the differences [here](https://www.lua.org/manual/5.4/manual.html#8).

> [!WARNING]
> Tsuki currently in a pre-1.0 so prepare for a lot of breaking changes!

## Status

The VM to run Lua code is fully working almost exactly as vanilla Lua (see some of differences below). Some functions on Lua standard library are still missing.

> [!IMPORTANT]
> All types in Tsuki does not implement `Send` and `Sync` and no plan to support this at the moment.

## Safety

All public API of Tsuki should provide 100% safety as long as you don't use unsafe API incorrectly.

Tsuki was not designed to run untrusted Lua script. Although you can limit what Lua script can do by not expose a function to it but there is no way to limit amount of memory or execution time used by Lua script. The meaning of this is Lua script can cause a panic due to out of memory or never return the control back to Rust with infinite loop. It is also possible for Lua script to cause stack overflow on Rust.

## Performance

### Interpreter

Tsuki is slower than Lua about 40%. The only possibility for Tsuki to be faster than Lua with computed goto is JIT since computed goto does not available on Rust.

### Async

A call to async function without any suspend on Tsuki is faster than mlua about 2.8x. For 1 suspend Tsuki it faster about 2.4x. For 8 suspend Tsuki is faster about 1.9x.

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
- Light userdata is not supported.
- Panic when memory allocation is failed without retry (same as Rust).
- No recursion checks on a call to Rust function.
- Rust function cannot yield the values back to Lua in the middle.
- Function called by Rust function cannot yield.
- GC has only one mode and cannot control from outside.
- Chunk name does not have a prefix (e.g. `@`).
- Second argument to `__close` metamethod always `nil`.
- `__gc` metamethod is not supported.
- `__name` metavalue must be UTF-8 string.
- `__tostring` metamethod must return a UTF-8 string.
- Float to string conversion does not truncate precision (Lua limit to 14 digits by default).
- Float literal does not accept hexadecimal format.
- String literal must be a UTF-8 string.
  - The result string can be non-UTF-8, only the literal need to be UTF-8.
- Comment must be a UTF-8 string.
- U+000B VERTICAL TAB is not considered as a whitespace.
- C locale is ignored (once `libc` has been completely removed).

### Standard library

- No `_VERSION`, `collectgarbage`, `dofile`, `loadfile`, `warn`, `xpcall`, `string.dump` and debug library.
- Second argument of `assert` accept only a UTF-8 string.
- `coroutine.isyieldable` does not accept any arguments.
- Arguments of `error`:
  - First argument accept only a UTF-8 string.
  - Second argument is not supported and it is always assume 1.
- Arguments of `load`:
  - First argument accept only a string.
  - Second argument accept only a UTF-8 string and will be empty when absent.
  - Third argument must be `nil` or `"t"`.
- `pcall` can produce up to 4 results on error and the message does not have a prefix.
  - 3rd result is a chunk name.
  - 4th result is a line number.
  - 3rd and 4th results will be absent if the called function is a Rust function.
- `coroutine.resume` can produce up to 4 results on error the same as `pcall`.
- `string.format` requires format string to be UTF-8.
  - `a`, `A`, `e`, `E`, `g` and `G` format is not supported.
  - `q` format requires string value to be UTF-8 and will use decimal notation instead of hexadecimal exponent notation for float.
  - Format have unlimited length.
- `string.find` and `string.gsub` does not support class `z`.
- `string.packsize` requires UTF-8 string for format string and format error always argument error.
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

## Breaking changes in 0.4

- `YieldFp` has been added to `Value`.
- `Float` methods has been removed.
- `BadInst` has been removed.
- `ChunkInfo` was replaced with `String`.
- `Context::push_from_index` and `Context::push_from_index_with_int` has been replaced with `Thread::index`.
- `Str::as_str` has been renamed to `Str::as_utf8`.
- `Arg::as_int` has additional parameter.
- `Str`, `Table`, `LuaFn` and `UserData` no longer implement `Unpin`.
- `DynamicInputs::push_num` has been renamed to `DynamicInputs::push_float`.
- `DynamicInputs::push_fp` now accept `Fp` instead of function pointer.
- `pcall` now produce up to 4 results on error and the message does not have a prefix.
- `string.format` now implemented in Rust with some breaking changes.
- `string.rep` now have the same result limit as Lua.
- Main thread has been removed.
- Recursion checks on a call to Rust function has been removed.
- String literal now required a UTF-8 string.
- Comment now required a UTF-8 string.

## Frequently Asked Questions

### Can we have zero-based indexing?

This requires too much changes to the language so the answer is no. See #16 for more details.

## License

Same as Lua, which is MIT.
