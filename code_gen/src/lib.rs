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

/// Exports a plugin from its library, for the application to find when it loads the file.
///
/// `export_plugin!(WebPlugin)` defines `erp_create_plugin_<crate>`: named after the crate, so that
/// several plugins linked into one binary — a test's, typically — never define the same symbol.
/// The application derives that name from the library's file name, which Cargo also takes from the
/// crate. `erp_plugin_build_<crate>` beside it says which build of `erp` the library carries, which
/// the application checks before running anything else of it.
#[proc_macro]
pub fn export_plugin(input: TokenStream) -> TokenStream {
    let plugin = parse_macro_input!(input as syn::Expr);
    let crate_name = std::env::var("CARGO_CRATE_NAME").unwrap_or_default();
    let symbol = proc_macro2::Ident::new(
        &format!("erp_create_plugin_{crate_name}"),
        proc_macro2::Span::call_site(),
    );
    let build = proc_macro2::Ident::new(
        &format!("erp_plugin_build_{crate_name}"),
        proc_macro2::Span::call_site(),
    );
    quote::quote! {
        #[unsafe(no_mangle)]
        pub extern "C" fn #symbol() -> *mut Box<dyn erp::plugin::Plugin> {
            let plugin: Box<dyn erp::plugin::Plugin> = Box::new(#plugin);
            Box::into_raw(Box::new(plugin))
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn #build() -> u64 {
            erp::plugin::build_id()
        }
    }
    .into()
}
