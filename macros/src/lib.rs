use proc_macro::TokenStream;
use syn::{Error, ItemEnum, parse_macro_input};

mod derive;

/// Generate [core::str::FromStr] implementation for enum to parse Lua
/// [option](https://www.lua.org/manual/5.4/manual.html#luaL_checkoption).
///
/// Only enum with unit variants is supported. The name to map will be the same as Lua convention,
/// which is lower-cased without separators:
///
/// ```
/// use tsuki::FromStr;
///
/// #[derive(FromStr)]
/// enum MyOption {
///     Foo,
///     FooBar,
/// }
/// ```
///
/// Will map `foo` to `MyOption::Foo` and `foobar` to `MyOption::FooBar`.
#[proc_macro_derive(FromStr)]
pub fn derive_from_str(item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemEnum);

    self::derive::from_str(item)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}
