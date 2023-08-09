use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;
use syn::Result;

use crate::model::Model;

pub fn derive(item: DeriveInput) -> Result<TokenStream> {
    let struct_name = &item.ident;
    let model = Model::from_item(&item)?;

    let table_name = model.table_name();
    let fields_str: Vec<&str> = model.fields().iter().map(|f| f.name().as_str()).collect::<Vec<_>>();
    let fields_required: Vec<bool> = model.fields().iter().map(|f| f.required()).collect::<Vec<_>>();

    let fields = model.fields().iter().map(|f| {
        let field_name = f.name();
        let is_required = f.required();
        quote! {
            FieldDescriptor {
                field_name: (#field_name).to_string(),
                is_required: #is_required,
            }
        }
    });

    let model_descriptor = quote! {
        let mut fields = HashMap::new();
        #(
            let fieldDescriptor = #fields;
            fields.insert(fieldDescriptor.name().to_string(), fieldDescriptor);
        )*
        ModelDescriptor {
            table_name: (#table_name).to_string(),
            fields: fields,
        }
    };

    Ok(quote! {
        impl #struct_name {
            pub fn _name(&self) -> &'static str {
                #table_name
            }

            pub fn _get_fields(&self) -> Vec<&'static str> {
                vec![#(#fields_str,)*]
            }

            pub fn _get_fields_required(&self) -> Vec<bool> {
                vec![#(#fields_required,)*]
            }
        }

        impl InternalModelGetterDescriptor for #struct_name {

            fn _get_model_descriptor() -> ModelDescriptor {
                #model_descriptor
            }
        }
    })
}
