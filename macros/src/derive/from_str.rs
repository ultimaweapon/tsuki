use proc_macro2::TokenStream;
use quote::quote;
use syn::{Error, Fields, ItemEnum};

pub fn from_str(item: ItemEnum) -> syn::Result<TokenStream> {
    // Parse variants.
    let ident = item.ident;
    let mut arms = TokenStream::new();

    for v in item.variants {
        if !matches!(v.fields, Fields::Unit) {
            return Err(Error::new_spanned(v, "non-unit variant is not supported"));
        }

        // Generate match arm.
        let ident = v.ident;
        let mut name = ident.to_string();

        name.make_ascii_lowercase();

        arms.extend(quote! {
            #name => Self::#ident,
        });
    }

    // Generate error type.
    let et = if cfg!(feature = "std") {
        quote! {
            ::std::boxed::Box<dyn ::std::error::Error>
        }
    } else {
        quote! {
            ::alloc::boxed::Box<dyn ::core::error::Error>
        }
    };

    // Generate error message.
    let em = if cfg!(feature = "std") {
        quote! {
            ::std::format!("invalid option '{v}'").into()
        }
    } else {
        quote! {
            ::alloc::format!("invalid option '{v}'").into()
        }
    };

    Ok(quote! {
        impl ::core::str::FromStr for #ident {
            type Err = #et;

            fn from_str(s: &str) -> ::core::result::Result<Self, Self::Err> {
                let v = match s {
                    #arms
                    v => return ::core::result::Result::Err(#em),
                };

                #[allow(unreachable_code)]
                ::core::result::Result::Ok(v)
            }
        }
    })
}
