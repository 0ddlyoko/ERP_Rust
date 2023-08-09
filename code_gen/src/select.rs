use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;
use syn::Result;

use crate::model::Model;

fn option_to_tuple<E>(the_option: Option<E>, default_value: E) -> (bool, E) {
    let is_present = the_option.is_some();
    let value = match the_option {
        Some(value) => value,
        None => default_value,
    };
    return (is_present, value);
}

pub fn derive(item: DeriveInput) -> Result<TokenStream> {
    let struct_name = &item.ident;
    let model = Model::from_item(&item)?;

    let table_name = model.table_name();
    let fields_str: Vec<&str> = model.fields().iter().map(|f| f.name().as_str()).collect::<Vec<_>>();
    let fields_required: Vec<bool> = model.fields().iter().map(|f| {
        match f.required() {
            None => false,
            Some(required) => required,
        }
    }).collect::<Vec<_>>();

    let fields = model.fields().iter().map(|f| {
        let field_name = f.name();
        let (is_required_present, is_required) = option_to_tuple(f.required(), false);
        quote! {
            GeneratedFieldDescriptor {
                field_name: (#field_name).to_string(),
                is_required: if #is_required_present { Some(#is_required) } else { None },
            }
        }
    });

    let model_descriptor = quote! {
        let mut fields = HashMap::new();
        #(
            let fieldDescriptor = #fields;
            fields.insert(fieldDescriptor.name().to_string(), fieldDescriptor);
        )*
        GeneratedModelDescriptor {
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

            fn _get_generated_model_descriptor() -> GeneratedModelDescriptor {
                #model_descriptor
            }
        }
    })
}
