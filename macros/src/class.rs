use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::meta::ParseNestedMeta;
use syn::{Error, FnArg, ImplItem, ItemImpl, Meta, Path, Type};

pub fn parse(mut ib: ItemImpl, args: Args) -> syn::Result<TokenStream> {
    // Get required arguments.
    let associated_data = match args.associated_data {
        Some(v) => v,
        None => {
            return Err(Error::new(
                Span::call_site(),
                "missing `associated_data` option",
            ));
        }
    };

    // Don't allow trait implementation.
    if let Some((_, t, _)) = ib.trait_ {
        return Err(Error::new_spanned(
            t,
            "trait implementation is not supported",
        ));
    }

    // Parse items.
    let mut close = TokenStream::new();
    let mut index = TokenStream::new();

    for item in &mut ib.items {
        let item = match item {
            ImplItem::Fn(v) => v,
            i => return Err(Error::new_spanned(i, "unsupported item")),
        };

        // Get first parameter.
        let sig = &item.sig;
        let ident = &sig.ident;
        let mut inputs = sig.inputs.iter();
        let first = match inputs.next() {
            Some(v) => v,
            None => return Err(Error::new_spanned(ident, "unsupported signature")),
        };

        // Check first parameter.
        let value = match (first, sig.asyncness.is_some()) {
            #[cfg(feature = "std")]
            (FnArg::Receiver(_), true) => quote! {
                ::tsuki::AsyncFp::new(|cx| ::std::boxed::Box::pin(async move {
                    let ud = cx.arg(1).get_ud::<Self>()?;

                    Self::#ident(ud.value(), &cx).await?;

                    Ok(cx.into())
                }))
            },
            #[cfg(not(feature = "std"))]
            (FnArg::Receiver(_), true) => quote! {
                ::tsuki::AsyncFp::new(|cx| ::alloc::boxed::Box::pin(async move {
                    let ud = cx.arg(1).get_ud::<Self>()?;

                    Self::#ident(ud.value(), &cx).await?;

                    Ok(cx.into())
                }))
            },
            (FnArg::Receiver(_), false) => quote! {
                ::tsuki::Fp::new(|cx| {
                    let ud = cx.arg(1).get_ud::<Self>()?;

                    Self::#ident(ud.value(), &cx)?;

                    Ok(cx.into())
                })
            },
            #[cfg(feature = "std")]
            (FnArg::Typed(_), true) => quote! {
                ::tsuki::AsyncFp::new(|cx| ::std::boxed::Box::pin(Self::#ident(cx)))
            },
            #[cfg(not(feature = "std"))]
            (FnArg::Typed(_), true) => quote! {
                ::tsuki::AsyncFp::new(|cx| ::alloc::boxed::Box::pin(Self::#ident(cx)))
            },
            (FnArg::Typed(_), false) => quote! {
                ::tsuki::Fp::new(Self::#ident)
            },
        };

        // Parse attributes.
        let mut t = None;
        let mut i = 0;

        while i < item.attrs.len() {
            let a = &item.attrs[i];

            if a.path().is_ident("close") {
                if t.is_some() {
                    return Err(Error::new_spanned(a, "multiple type is not supported"));
                }

                // Check if hidden.
                let hidden = if let Meta::Path(_) = &a.meta {
                    false
                } else {
                    let p = a.parse_args::<Path>()?;

                    if p.is_ident("hidden") {
                        true
                    } else {
                        return Err(Error::new_spanned(p, "unknown option"));
                    }
                };

                t = Some(Event::Close { hidden });
            } else {
                i += 1;
                continue;
            }

            item.attrs.remove(i);
        }

        // Check type.
        match t {
            Some(Event::Close { hidden }) => {
                if !close.is_empty() {
                    return Err(Error::new_spanned(
                        ident,
                        "multiple function with #[close] is not supported",
                    ));
                }

                close = value.clone();

                if hidden {
                    continue;
                }
            }
            None => (),
        }

        // Add to index.
        let name = ident.to_string();

        index.extend(quote!(mt.set_str_key(#name, #value);));
    }

    if !close.is_empty() {
        close = quote!(mt.set_str_key("__close", #close););
    }

    if !index.is_empty() {
        index.extend(quote!(mt.set_str_key("__index", &*mt);));
    }

    // Compose.
    let generics = &ib.generics;
    let ty = &ib.self_ty;

    Ok(quote! {
        #ib

        impl #generics ::tsuki::Class<#associated_data> for #ty {
            fn create_metatable(lua: &::tsuki::Lua<#associated_data>) -> ::tsuki::Ref<'_, ::tsuki::Table<#associated_data>> {
                let mt = lua.create_table();

                #index
                #close

                mt
            }
        }
    })
}

#[derive(Default)]
pub struct Args {
    associated_data: Option<Type>,
}

impl Args {
    pub fn parse(&mut self, m: ParseNestedMeta) -> syn::Result<()> {
        if m.path.is_ident("associated_data") {
            self.associated_data = Some(m.value()?.parse()?);
        }

        Ok(())
    }
}

enum Event {
    Close { hidden: bool },
}
