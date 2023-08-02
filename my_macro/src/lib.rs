extern crate proc_macro;

use proc_macro::*;

// pub enum Method {
//     Get,
//     Put,
//     Post,
//     Delete,
//     Patch,
//     Options,
// }

trait Show {
    fn show(&self) -> String;
}

impl Show for i32 {
    fn show(&self) -> String {
        format!("four-byte signed {}", self)
    }
}

macro_rules! create_function {
    ($name:ident, $fnc:expr) => {
        #[proc_macro_attribute]
        pub fn $name(args: TokenStream, input: TokenStream) -> TokenStream {
            let args: proc_macro2::TokenStream = args.into();
            let input: proc_macro2::TokenStream = input.into();

            let expanded: proc_macro2::TokenStream = quote::quote! {
                #input
            };

            expanded.into()
        }
    }
}

create_function!(lol, || {
    println!("LOL");
});

#[proc_macro_attribute]
pub fn log_entry_and_exit(args: TokenStream, input: TokenStream) -> TokenStream {
    let args: proc_macro2::TokenStream = args.into();
    let input: proc_macro2::TokenStream = input.into();

    let args_string = args.to_string();
    let input_string = input.to_string();

    let expanded: proc_macro2::TokenStream = quote::quote! {
        fn test() {
            println!("entering");
            println!("args tokens: {:?}", #args_string);
            println!("input tokens: {:?}", #input_string);
            println!("exiting");
        }

        #input
    };

    expanded.into()
}

#[proc_macro_attribute]
pub fn log_entry_test(args: TokenStream, input: TokenStream) -> TokenStream {
    println!("attr: \"{}\"", args.to_string());
    println!("input: \"{}\"", input.to_string());
    input
}

// fn test() {
//     let a = quote::quote!(fn main() {});
// }

// #[macro_use]
// mod attribute;

//
// macro_rules! teeeeeeeeest {
//     ($tokens:expr) => ({
//         let mut tokens = $tokens;
//         tokens.into()
//     })
// }
//
//
// macro_rules! teeeeeest {
//     ($name:ident => $method:expr) => (
//         #[proc_macro_attribute]
//         pub fn $name(args: TokenStream, input: TokenStream) -> TokenStream {
//             teeeeeeeeest!(attribute::route::teeeeeest($method, args, input))
//         }
//     )
// }
//
//
// teeeeeest!(log_entry_and_exit => None);


// #[proc_macro_attribute]
// pub fn log_entry_and_exit(args: TokenStream, input: TokenStream) -> TokenStream {
//     let input: proc_macro2::TokenStream = input.into();
//     println!("args tokens: {:?}", args.to_string());
//     println!("input tokens: {:?}", input.to_string());
//     let expanded = quote::quote! {
//         fn teeeest() {
//
//         }
//         let looool = || {
//             println!("entering");
//             println!("args tokens: {:?}", args.to_string());
//             println!("input tokens: {:?}", input.to_string());
//             println!("exiting");
//         }
//     };
//
//     expanded.into()
// }
