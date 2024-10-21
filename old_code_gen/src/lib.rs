extern crate proc_macro;

mod attrs;
mod field;
mod model;
mod select;
mod util;


use proc_macro::*;
use syn::parse_macro_input;

#[proc_macro_derive(Model, attributes(odd))]
pub fn derive_model(input: TokenStream) -> TokenStream {
    select::derive(parse_macro_input!(input))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
