use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;
use syn::Result;

use crate::model::Model;

pub fn derive(item: DeriveInput) -> Result<TokenStream> {
    let struct_name = &item.ident;
    let model = Model::from_item(&item)?;

    let table_name = model.table_name();
    let fields: Vec<&str> = model.fields().iter().map(|f| f.name().as_str()).collect::<Vec<_>>();
    let fields_required: Vec<bool> = model.fields().iter().map(|f| f.required()).collect::<Vec<_>>();

    Ok(quote! {
        impl #struct_name {
            // pub const model_data: String = "4".to_string();

            pub fn _name(&self) -> &'static str {
                #table_name
            }

            pub fn _get_fields(&self) -> Vec<&'static str> {
                vec![#(#fields,)*]
            }

            pub fn _get_fields_required(&self) -> Vec<bool> {
                vec![#(#fields_required,)*]
            }
        }

        impl InternalModel for #struct_name {
            fn _get_model() -> &'static str {
                "salut"
            }
        }
    })
}
