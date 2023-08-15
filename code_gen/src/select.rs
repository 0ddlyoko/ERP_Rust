use std::fs::File;
use std::io::Write;
use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;
use syn::Result;
use test_lib::FieldType;

use crate::model::Model;
use crate::util::option_to_tuple;

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
        let (field_type, (is_default_present, default_value)) = match f.default_value() {
            FieldType::String(default_value) => {
                let option_string = default_value.value_as_ref().map(|f| f.clone());
                ("String", option_to_tuple(option_string, "".to_string()))
            },
            FieldType::Integer(default_value) => {
                let option_string = default_value.value_as_ref().map(|f| f.to_string());
                ("Integer", option_to_tuple(option_string, "0".to_string()))
            },
            FieldType::Boolean(default_value) => {
                let option_string = default_value.value_as_ref().map(|f| f.to_string());
                ("Boolean", option_to_tuple(option_string, "false".to_string()))
            },
        };
        quote! {
            GeneratedFieldDescriptor {
                field_name: (#field_name).to_string(),
                is_required: if #is_required_present { Some(#is_required) } else { None },
                default_field: FieldType::from(#field_type, if #is_default_present {Option::from(#default_value.to_string())} else {Option::None})
            }
        }
    });

    let model_descriptor = quote! {
        let mut fields = HashMap::new();
        #(
            let field_descriptor = #fields;
            fields.insert(field_descriptor.name().to_string(), field_descriptor);
        )*
        GeneratedModelDescriptor {
            table_name: (#table_name).to_string(),
            fields: fields,
        }
    };

    let impl_struct = quote! {
        impl #generics #struct_name #generics {
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
            fn _name() -> &'static str {
                #table_name
            }

            fn _get_generated_model_descriptor() -> GeneratedModelDescriptor {
                #model_descriptor
            }
        }
    };

    let model_environment_impl = quote! {
        impl #generics ModelEnvironment<'env> for #struct_name #generics {
            fn env(&self) -> &Environment<'env> {
                self._env
            }

            fn env_mut(&mut self) -> &mut Environment<'env> {
                self._env
            }

            fn restore_env(&mut self, env: &'env mut Environment<'env>) {
                self._env = env;
            }
        }
    };

    // let from_impl = quote! {
    //     impl #generics From<CachedRecord> for #struct_name #generics {
    //         fn from(value: CachedRecord) -> Self {
    //
    //         }
    //     }
    // };

    let result = quote! {
        #impl_struct

        #internal_model_getter_descriptor_impl

        #model_environment_impl
    };

    let mut file = File::create(format!("generated/generated_code_{}.rs", table_name));
    match file {
        Ok(mut file) => {
            file.write_all(result.to_string().as_bytes());
        }
        Err(_) => {}
    };

    Ok(result)
}
