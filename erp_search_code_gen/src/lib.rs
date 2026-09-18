extern crate proc_macro;

use proc_macro::TokenStream;
use syn::{ExprArray, parse_macro_input};

mod domain;

#[proc_macro]
pub fn make_domain(input: TokenStream) -> TokenStream {
    domain::domain_gen::derive(&parse_macro_input!(input as ExprArray).elems)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
