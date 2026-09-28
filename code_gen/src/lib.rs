extern crate proc_macro;

mod controller;
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

/// Declares a controller: the id every struct extending it names.
#[proc_macro_derive(Controller, attributes(erp))]
pub fn derive_controller(input: TokenStream) -> TokenStream {
    controller::derive::derive(&parse_macro_input!(input as DeriveInput))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Declares the overridable methods of a controller, and the URLs some of them answer.
///
/// Every method of the block is overridable by name, like in `#[erp_methods]`. One marked
/// `#[erp(route = "/path/<param>", methods = ["GET"])]` also answers that URL; its arguments are
/// read from the route's parameters and the query string, by name.
#[proc_macro_attribute]
pub fn erp_routes(_attr: TokenStream, item: TokenStream) -> TokenStream {
    controller::routes::expand(parse_macro_input!(item as ItemImpl))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
