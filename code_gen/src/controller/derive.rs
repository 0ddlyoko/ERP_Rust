use proc_macro2::TokenStream;
use quote::quote;
use syn::spanned::Spanned;
use syn::{Data, DeriveInput, Error, Fields, LitStr, Result};

/// `#[derive(Controller)]`: the controller id a struct contributes to, and how to make one.
///
/// A controller carries no state, so the struct has no fields: it is an identity, and every
/// struct naming the same id extends the same controller.
pub fn derive(input: &DeriveInput) -> Result<TokenStream> {
    let ident = &input.ident;
    if !input.generics.params.is_empty() {
        return Err(Error::new(
            input.generics.span(),
            "A controller takes no generic parameters",
        ));
    }
    let stateless = "A controller carries no state: every call gets the environment and the \
                     request. Declare it as `struct Name;`";
    let construct = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Unit => quote! { #ident },
            Fields::Named(fields) if fields.named.is_empty() => quote! { #ident {} },
            fields => return Err(Error::new(fields.span(), stateless)),
        },
        _ => return Err(Error::new(ident.span(), stateless)),
    };

    let mut id: Option<LitStr> = None;
    for attr in input
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident("erp"))
    {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("id") {
                id = Some(meta.value()?.parse()?);
                Ok(())
            } else {
                Err(meta.error("Unknown key. The only key on a controller is: id"))
            }
        })?;
    }
    let Some(id) = id else {
        return Err(Error::new(
            ident.span(),
            "A controller needs #[erp(id = \"...\")]: the id every struct extending it names",
        ));
    };

    Ok(quote! {
        impl erp::http::Controller for #ident {
            fn controller_name() -> &'static str {
                #id
            }

            fn instance() -> Self {
                #construct
            }
        }
    })
}
