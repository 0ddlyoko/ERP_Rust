extern crate proc_macro;

// mod model;
// mod the_main;

mod attrs;
mod field;
mod model;
mod select;
mod util;


use proc_macro::*;
use std::fmt::format;
use syn::{Data, DeriveInput, Fields, parse_macro_input, parse_quote};

#[proc_macro_derive(Model, attributes(odd))]
pub fn derive_model(input: TokenStream) -> TokenStream {
    select::derive(parse_macro_input!(input))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
// use model::handle_model;
// use the_main::handle_main;

// #[proc_macro_attribute]
// pub fn model(attr: TokenStream, item: TokenStream) -> TokenStream {
//     let input_fn = syn::parse_macro_input!(item as syn::ItemStruct);
//
//     // let fn_name = &input_fn.ident;
//
//     let attribute_value = if !attr.is_empty() {
//         attr.to_string().replace("\"", "")
//     } else {
//         "Undefined".to_string()
//     };
//
//     if let Fields::Named(fields_named) = &input_fn.fields {
//         let fields = fields_named.named.iter();
//
//         let generated_code = quote::quote! {
//             #(let #fields;)*
//         };
//         save_generated_code(&attribute_value, &generated_code.into());
//     }
//
//     return TokenStream::from(quote::quote! {
//     });
// }
//
// fn save_generated_code(name: &str, code: &TokenStream) {
//     let mut shared_data = SharedData::default();
//     let code_str = code.to_string();
//     {
//         let mut data_map = shared_data.data_map.write().unwrap();
//         data_map.insert(name.to_lowercase(), code_str);
//         println!("INSERTING {}", name.to_lowercase());
//         println!("{:?}", &data_map);
//     }
//     // let file_name = format!("build/{}_generated.rs", name.to_lowercase());
//     // std::fs::write(file_name, code_str).expect("Failed to write the generated code to file");
// }
//
// #[proc_macro_attribute]
// pub fn test_main(attr: TokenStream, item: TokenStream) -> TokenStream {
//     let input_fn = syn::parse_macro_input!(item as syn::ItemFn);
//
//     let fn_sig = &input_fn.sig;
//     let fn_name = &fn_sig.ident;
//     let fn_block = &input_fn.block;
//
//     let expanded: proc_macro2::TokenStream = quote::quote! {
//         #input_fn
//
//         fn main() {
//             println!("enteringgg");
//
//             #fn_name();
//
//             // #fn_name();
//             println!("exiting");
//         }
//     };
//
//     save_generated_code_2();
//
//     expanded.into()
// }
//
// fn save_generated_code_2() {
//     let mut shared_data = SharedData::default();
//     let mut data_map = shared_data.data_map.read().unwrap();
//     println!("SALUT :D");
//     println!("{:?}", &data_map);
//     data_map.iter().for_each(|(name, code)| save_in_file(name, code));
//     // let file_name = format!("build/{}_generated.rs", name.to_lowercase());
//     // std::fs::write(file_name, code_str).expect("Failed to write the generated code to file");
// }
//
// fn save_in_file(name: &String, code: &String) {
//     let file_name = format!("build/{}_generated.rs", name.to_lowercase());
//     std::fs::write(file_name, code).expect("Failed to write the generated code to file");
// }

// pub fn derive_test()
