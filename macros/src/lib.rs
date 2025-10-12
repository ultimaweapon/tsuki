use proc_macro::TokenStream;
use syn::{Error, ItemEnum, parse_macro_input};

mod derive;

#[proc_macro_derive(FromStr)]
pub fn derive_from_str(item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemEnum);

    self::derive::from_str(item)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}
