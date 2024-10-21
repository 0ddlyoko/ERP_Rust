use std::fs::File;
use std::io::Write;
use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;
use syn::Result;
use test_lib::FieldType;

use crate::model::ModelGen;
use crate::util::option_to_tuple;

pub fn derive(item: DeriveInput) -> Result<TokenStream> {
    let DeriveInput {
        ident: ref struct_name, ref generics, ..
    } = item;
    let model = ModelGen::from_item(&item)?;

    let table_name = model.table_name();
    let fields_str: Vec<&str> = model.fields().iter().map(|f| f.name().as_str()).collect::<Vec<_>>();
    let fields_required: Vec<bool> = model.fields().iter().map(|f| {
        match f.required() {
            None => false,
            Some(required) => required,
        }
    }).collect::<Vec<_>>();

    let fields_descriptor = model.fields().iter().map(|f| {
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
            let field_descriptor = #fields_descriptor;
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

    let fields_filtered = model.fields().iter().filter(|f| f.name() != "id");

    let fields_from_map = fields_filtered.clone().map(|f| {
        let field_name = f.name();
        let field_ident = syn::Ident::new(field_name, f.span().clone());
        let field_type = match f.default_value() {
            FieldType::String(_) => quote! { FieldType::transform_to_string(map.remove(#field_name).unwrap()) },
            FieldType::Integer(_) => quote! { FieldType::transform_to_integer(map.remove(#field_name).unwrap()) },
            FieldType::Boolean(_) => quote! { FieldType::transform_to_boolean(map.remove(#field_name).unwrap()) },
        };
        quote! {
            #field_ident: #field_type,
        }
    });

    let fields_to_map = fields_filtered.clone().map(|f| {
        let field_name = f.name();
        let field_ident = syn::Ident::new(field_name, f.span().clone());
        match f.default_value() {
            FieldType::String(_) => quote! { map.insert(#field_name.to_string(), FieldType::String(Field::new(self.#field_ident.value().clone()))); },
            FieldType::Integer(_) => quote! { map.insert(#field_name.to_string(), FieldType::Integer(Field::new(self.#field_ident.value().clone()))); },
            FieldType::Boolean(_) => quote! { map.insert(#field_name.to_string(), FieldType::Boolean(Field::new(self.#field_ident.value().clone()))); },
        }
    });
    // TODO Find a way to remove all those generated code
    let fields_to_map_dirty = fields_filtered.clone().map(|f| {
        let field_name = f.name();
        let field_ident = syn::Ident::new(field_name, f.span().clone());
        match f.default_value() {
            FieldType::String(_) => quote! {
                if (self.#field_ident.is_dirty()) {
                    map.insert(#field_name.to_string(), FieldType::String(Field::new(self.#field_ident.value().clone())));
                }
            },
            FieldType::Integer(_) => quote! {
                if (self.#field_ident.is_dirty()) {
                   map.insert(#field_name.to_string(), FieldType::Integer(Field::new(self.#field_ident.value().clone())));
                }
            },
            FieldType::Boolean(_) => quote! {
                if (self.#field_ident.is_dirty()) {
                   map.insert(#field_name.to_string(), FieldType::Boolean(Field::new(self.#field_ident.value().clone())));
                }
            },
        }
    });

    let fields_reset_dirty = fields_filtered.clone().map(|f| {
        let field_name = f.name();
        let field_ident = syn::Ident::new(field_name, f.span().clone());
        quote! { self.#field_ident.reset_dirty(); }
    });

    let fields_get_field_mut = fields_filtered.clone().map(|f| {
        let field_name = f.name();
        let field_ident = syn::Ident::new(field_name, f.span().clone());
        quote! {
            if (field_name.eq(#field_name)) {
                return Option::Some(&self.#field_ident);
            }
        }
    });

    let fields_update = fields_filtered.clone().map(|f| {
        let field_name = f.name();
        let field_ident = syn::Ident::new(field_name, f.span().clone());
        quote! {
            if (field_name.eq(#field_name)) {
                self.#field_ident.set_option_from_string(option);
            }
        }
    });

    let model_impl = quote! {
        impl #generics Model for #struct_name #generics {
            fn _name() -> &'static str {
                #table_name
            }

            fn _get_generated_model_descriptor() -> GeneratedModelDescriptor {
                #model_descriptor
            }

            fn _from_map(id: u32, mut map: HashMap<String, FieldType>, env: &'env Environment<'env>) -> Self {
                Self {
                    id: id,
                    #(#fields_from_map)*
                    _env: env,
                }
            }

            fn id(&self) -> u32 {
                self.id
            }

            fn _to_map(&self) -> HashMap<String, FieldType> {
                let mut map = HashMap::new();
                #(#fields_to_map)*
                map
            }

            fn _to_map_dirty(&self) -> HashMap<String, FieldType> {
                let mut map = HashMap::new();
                #(#fields_to_map_dirty)*
                map
            }

            fn _remove_dirty(&mut self) {
                #(#fields_reset_dirty)*
            }

            fn update(&mut self, map: HashMap<&str, Option<&str>>) {
                for (field_name, option) in map {
                    #(#fields_update)*
                }
            }
        }
    };

    let result = quote! {
        #impl_struct

        #model_impl
    };

    let file = File::create(format!("generated/generated_code_{}.rs", table_name));
    match file {
        Ok(mut file) => {
            file.write_all(result.to_string().as_bytes()).expect("TODO: panic message");
        }
        Err(_) => {}
    };

    Ok(result)
}
