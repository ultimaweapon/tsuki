use proc_macro::TokenStream;
use syn::{Error, ItemEnum, ItemImpl, parse_macro_input};

mod attribute;
mod derive;

#[proc_macro_attribute]
pub fn class(arg: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemImpl);
    let mut args = self::attribute::class::Args::default();
    let parser = syn::meta::parser(|m| args.parse(m));

    parse_macro_input!(arg with parser);

    self::attribute::class::parse(item, args)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

#[proc_macro_derive(FromStr)]
pub fn derive_from_str(item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemEnum);

    self::derive::from_str(item)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}
