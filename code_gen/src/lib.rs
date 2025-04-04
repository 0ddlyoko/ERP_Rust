extern crate proc_macro;

mod domain;
mod model;

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput, ExprArray};

#[proc_macro]
pub fn derive_domain(input: TokenStream) -> TokenStream {
    domain::domain_gen::derive(&parse_macro_input!(input as ExprArray).elems)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(Model, attributes(erp))]
pub fn derive_model(input: TokenStream) -> TokenStream {
    model::model_gen::derive(&parse_macro_input!(input as DeriveInput))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
