extern crate proc_macro;

mod model;

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

#[proc_macro_derive(Model, attributes(erp))]
pub fn derive_model(input: TokenStream) -> TokenStream {
    model::model_gen::derive(&parse_macro_input!(input as DeriveInput))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
