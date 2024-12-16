use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use syn::{DeriveInput, Result};
use crate::field::FieldGen;
use crate::model::ModelGen;

pub fn derive(item: DeriveInput) -> Result<TokenStream> {
    let DeriveInput {
        ref ident,
        ref generics,
        ..
    } = item;
    let ModelGen {
        description,
        fields,
        ..
    } = ModelGen::from_item(&item)?;

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
            ..
        } = f;

        let default_value = if *is_reference {
            quote! {
                Some(erp::field::FieldType::Ref(0))
            }
        } else {
            quote! {
                Some(#field_type_keyword::default().into())
            }
        };

        quote! {
            {
                erp::field::FieldDescriptor {
                    name: #field_name.to_string(),
                    // TODO Real default value
                    default_value: #default_value,
                    // TODO description
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
        #simplified_model_impl
    };

    Ok(result)
}