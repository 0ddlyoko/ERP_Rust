use std::fs::File;
use std::io::Write;
use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;
use syn::Result;

use crate::model::Model;
use crate::util::{generate_missing_table_name_error, option_to_tuple};

pub fn derive(item: DeriveInput) -> Result<TokenStream> {
    let DeriveInput {
        ident: ref struct_name, ref generics, ..
    } = item;
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

    let impl_struct = quote! {
        impl #generics #struct_name #generics {
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
    };

    let internal_model_getter_descriptor_impl = quote! {
        impl #generics InternalModelGetterDescriptor for #struct_name #generics {

            fn _get_generated_model_descriptor() -> GeneratedModelDescriptor {
                #model_descriptor
            }
        }
    };

    let result = quote! {
        #impl_struct

        #internal_model_getter_descriptor_impl
    };

    Ok(result)
}
