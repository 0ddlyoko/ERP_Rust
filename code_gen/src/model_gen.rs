use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use syn::{parse_str, DeriveInput, Path, Result};
use erp::field::FieldType;
use crate::field::FieldGen;
use crate::model::ModelGen;

pub fn derive(item: DeriveInput) -> Result<TokenStream> {
    let DeriveInput {
        ref ident,
        ref generics,
        ..
    } = item;
    let ModelGen {
        struct_name,
        table_name,
        description,
        derived_model,
        fields,
        ..
    } = ModelGen::from_item(&item)?;

    let struct_name_ident = Ident::new(struct_name.as_str(), Span::call_site());
    let base_model_name = format!("Base{}", table_name);
    let base_model = if let Some(derived_model) = derived_model {
        let full_base_model = if derived_model.is_empty() {
            base_model_name
        } else {
            format!("{}::{}", derived_model, base_model_name)
        };
        let full_base_model_path: Path = parse_str(&full_base_model)?;
        quote! {
            impl erp::model::Model for #struct_name_ident {
                type BaseModel = #full_base_model_path;
            }
        }
    } else {
        let base_model_name_ident = Ident::new(base_model_name.as_str(), Span::call_site());
        quote! {
            pub struct #base_model_name_ident;

            impl erp::model::BaseModel for #base_model_name_ident {
                fn get_model_name() -> &'static str {
                    #table_name
                }
            }

            impl erp::model::Model for #struct_name_ident {
                type BaseModel = #base_model_name_ident;
            }
        }
    };

    let impl_model_fields = fields.iter().filter_map(|f| {
        let FieldGen {
            field_name,
            is_required,
            is_reference,
            field_type_keyword,
            ..
        } = f;
        if field_name == "id" {
            return None;
        }
        let field_ident = Ident::new(field_name, Span::call_site());
        let get_field_ident = Ident::new(format!("get_{}", field_name).as_str(), Span::call_site());
        if *is_required {
            Some(quote! {
                pub fn #get_field_ident(&self) -> &#field_type_keyword {
                    &self.#field_ident
                }
            })
        } else if *is_reference {
            Some(quote! {
                pub fn #get_field_ident<E>(&mut self, env: &mut erp::environment::Environment) -> Result<Option<E>, Box<dyn std::error::Error>>
                where
                    E: erp::model::Model<BaseModel=#field_type_keyword> {
                    self.#field_ident.get(env)
                }
            })
        } else {
            Some(quote! {
                pub fn #get_field_ident(&self) -> Option<&#field_type_keyword> {
                    self.#field_ident.as_ref()
                }
            })
        }
    });
    let impl_model = quote! {
        impl #struct_name_ident {
            pub fn get_id(&self) -> u32 {
                self.id
            }

            #(#impl_model_fields)*
        }
    };

    let description = if let Some(description) = description {
        quote! { Some(#description.to_string()) }
    } else {
        quote! { None }
    };

    let fields_descriptor = fields.iter().map(|f| {
        let FieldGen {
            field_name,
            is_required,
            is_reference,
            field_type_keyword,
            default_value,
            description,
            ..
        } = f;

        let default_value = if let Some(default_value) = default_value {
            match default_value {
                FieldType::String(s) => quote! {
                    Some(erp::field::FieldType::String(#s.to_string()))
                },
                FieldType::Integer(i) => quote! {
                    Some(erp::field::FieldType::Integer(#i))
                },
                FieldType::Float(f) => quote! {
                    Some(erp::field::FieldType::Float(#f))
                },
                FieldType::Bool(b) => quote! {
                    Some(erp::field::FieldType::Bool(#b))
                },
                FieldType::Enum(e) => quote! {
                    Some(erp::field::FieldType::Enum(#e))
                },
                FieldType::Ref(r) => quote! {
                    Some(erp::field::FieldType::Ref(#r))
                },
            }
        } else if *is_reference {
            quote! {
                Some(erp::field::FieldType::Ref(0))
            }
        } else {
            quote! {
                Some((&#field_type_keyword::default()).into())
            }
        };

        let description = if let Some(description) = description {
            quote! { Some(#description.to_string()) }
        } else {
            quote! { None }
        };

        quote! {
            {
                erp::field::FieldDescriptor {
                    name: #field_name.to_string(),
                    default_value: #default_value,
                    description: #description,
                    required: #is_required,
                    ..erp::field::FieldDescriptor::default()
                }
            }
        }
    });

    let get_data = fields.iter().map(|f| {
        let FieldGen {
            field_name,
            is_required,
            is_reference,
            ..
        } = f;
        let field_ident = Ident::new(field_name, Span::call_site());

        if *is_required || *is_reference {
            quote! {
                result.insert(#field_name, &self.#field_ident);
            }
        } else {
            quote! {
                result.insert_option(#field_name, self.#field_ident.as_ref());
            }
        }
    });

    let create_model = fields.iter().map(|f| {
        let FieldGen {
            field_name,
            is_required,
            is_reference,
            ..
        } = f;
        let field_ident = Ident::new(field_name, Span::call_site());

        if *is_required || *is_reference {
            quote! {
                #field_ident: data.get(#field_name)
            }
        } else {
            quote! {
                #field_ident: data.get_option(#field_name)
            }
        }
    });

    let simplified_model_impl = quote! {
        impl #generics erp::model::SimplifiedModel for #ident #generics {
            fn get_model_descriptor() -> erp::model::ModelDescriptor {
                let name = Self::get_model_name().to_string();
                let description = #description;
                let fields = vec![
                    #(#fields_descriptor,)*
                ];
                erp::model::ModelDescriptor {
                    name,
                    description,
                    fields,
                }
            }

            fn get_id(&self) -> u32 {
                self.id
            }

            fn get_data(&self) -> erp::model::MapOfFields {
                let mut result = erp::model::MapOfFields::default();
                #(#get_data;)*
                result
            }

            fn create_model(id: u32, data: erp::model::MapOfFields) -> Self {
                Self {
                    id,
                    #(#create_model,)*
                }
            }

            fn call_compute_method(
                &mut self,
                field_name: &str,
                env: &mut erp::environment::Environment,
            ) -> Result<(), Box<dyn std::error::Error>> {
                // TODO Computed methods
                Ok(())
            }
        }
    };

    let result = quote! {
        #base_model

        #impl_model

        #simplified_model_impl
    };

    Ok(result)
}