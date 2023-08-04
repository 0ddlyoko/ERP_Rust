use proc_macro::*;
use syn::Fields;

mod shared_data;

use shared_data::SharedData;

pub fn handle_main(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = syn::parse_macro_input!(item as syn::ItemFn);

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