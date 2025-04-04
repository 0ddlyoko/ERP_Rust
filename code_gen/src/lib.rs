extern crate proc_macro;

mod model;

use syn::{parse_macro_input, DeriveInput};

#[proc_macro_derive(Model, attributes(erp))]
pub fn derive_model(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    model::model_gen::derive(parse_macro_input!(input as DeriveInput))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
