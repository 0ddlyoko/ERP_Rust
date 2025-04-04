use proc_macro2::TokenStream;
use quote::quote;
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::{Expr, Result};

pub fn derive(items: &Punctuated<Expr, Comma>) -> Result<TokenStream> {
    if items.is_empty() {
        return Ok(quote! {
            erp::search::SearchType::Nothing;
        });
    }

    let result = quote! {
        erp::search::SearchType::Nothing;
    };

    Ok(result)
}