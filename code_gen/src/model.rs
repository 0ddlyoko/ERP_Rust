use proc_macro::*;
use syn::Fields;

mod shared_data;

use shared_data::SharedData;

pub fn handle_model(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = syn::parse_macro_input!(item as syn::ItemStruct);

    // let fn_name = &input_fn.ident;

    let attribute_value = if !attr.is_empty() {
        attr.to_string().replace("\"", "")
    } else {
        "Undefined".to_string()
    };


    if let Fields::Named(fields_named) = &input_fn.fields {
        let fields = fields_named.named.iter();

        let generated_code = quote::quote! {
            #(let #fields;)*
        };
        save_generated_code(&attribute_value, &generated_code.into());
    }


    return TokenStream::from(quote::quote! {

    });

    // if let Fields::Named(fields_named) = &input_fn.fields {
    //     let fields = fields_named.named.iter().map(|field| &field.ident);
    //
    //
    //     let generated_code: proc_macro2::TokenStream = quote::quote! {
    //         #input_fn
    //
    //         impl #fn_name<'_> {
    //             pub fn the_name(&self) -> &'static str {
    //                 #attribute_value
    //             }
    //         }
    //         //
    //         // impl Display for #fn_name {
    //         //     fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
    //         //         write!(f, "Model {}", #fn_name)
    //         //     }
    //         // }
    //     };
    //     return generated_code.into();
    // }
}

fn save_generated_code(name: &str, code: &TokenStream) {
    let mut shared_data = SharedData::default();
    let code_str = code.to_string();
    {
        let mut data_map = shared_data.data_map.write().unwrap();
        data_map.insert(name.to_lowercase(), code_str);
    }
    // let file_name = format!("build/{}_generated.rs", name.to_lowercase());
    // std::fs::write(file_name, code_str).expect("Failed to write the generated code to file");
}
