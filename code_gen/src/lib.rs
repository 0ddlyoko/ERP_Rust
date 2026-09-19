extern crate proc_macro;

mod methods;
mod model;

use proc_macro::TokenStream;
use syn::{DeriveInput, ItemImpl, parse_macro_input};

#[proc_macro_derive(Model, attributes(erp))]
pub fn derive_model(input: TokenStream) -> TokenStream {
    model::model_gen::derive(&parse_macro_input!(input as DeriveInput))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Declares the overridable methods an `impl` block contributes.
///
/// Needed because a derive macro sees only the struct: the methods live in an `impl` block it
/// never gets to look at. Methods without an `#[erp(...)]` attribute pass through untouched.
#[proc_macro_attribute]
pub fn erp_methods(_attr: TokenStream, item: TokenStream) -> TokenStream {
    methods::expand(parse_macro_input!(item as ItemImpl))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
