extern crate proc_macro;

mod model;

use proc_macro::*;
use std::fmt::format;
use syn::{Data, DeriveInput, Fields, parse_quote};
use crate::model::handle_model;

#[proc_macro_attribute]
pub fn model(attr: TokenStream, item: TokenStream) -> TokenStream {
    handle_model(attr, item)
}

#[proc_macro_attribute]
pub fn test_main(args: TokenStream, input: TokenStream) -> TokenStream {
    let input_fn = syn::parse_macro_input!(input as syn::ItemFn);

    let fn_sig = &input_fn.sig;
    let fn_name = &fn_sig.ident;
    let fn_block = &input_fn.block;

    let expanded: proc_macro2::TokenStream = quote::quote! {
        #input_fn

        fn main() {
            println!("entering");

            #fn_name();

            // #fn_name();
            println!("exiting");
        }
    };

    expanded.into()
}
