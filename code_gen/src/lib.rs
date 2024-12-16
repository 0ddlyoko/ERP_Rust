extern crate proc_macro;

mod attrs;
mod field;
mod model;
mod model_gen;
mod util;

use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Model, attributes(erp))]
pub fn derive_model(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    model_gen::derive(parse_macro_input!(input as DeriveInput))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
