use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use syn::{parse_str, DeriveInput, Path, Result};
use erp::field::FieldType;
use erp::util::string::StringTransform;
use crate::field::FieldGen;
use crate::model::ModelGen;

pub fn derive(item: DeriveInput) -> Result<TokenStream> {
    let DeriveInput {
        ref ident,
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
    let camel_case_table_name = table_name.replace("_", " ").to_camel_case();
    let base_model_name = format!("Base{}", camel_case_table_name);
    let base_model = if let Some(derived_model) = derived_model {
        let full_base_model = if derived_model.is_empty() {
            base_model_name
        } else {
            format!("{}::{}", derived_model, base_model_name)
        };
        let full_base_model_path: Path = parse_str(&full_base_model)?;
        quote! {
            impl erp::model::Model<erp::field::SingleId> for #struct_name_ident<erp::field::SingleId> {
                type BaseModel = #full_base_model_path;
            }
            impl erp::model::Model<erp::field::MultipleIds> for #struct_name_ident<erp::field::MultipleIds> {
                type BaseModel = #full_base_model_path;
            }
        }
    } else {
        let base_model_name_ident = Ident::new(base_model_name.as_str(), Span::call_site());
        quote! {
            #[derive(Default, Debug)]
            pub struct #base_model_name_ident;

            impl erp::model::BaseModel for #base_model_name_ident {
                fn get_model_name() -> &'static str {
                    #table_name
                }
            }

            impl erp::model::Model<erp::field::SingleId> for #struct_name_ident<erp::field::SingleId> {
                type BaseModel = #base_model_name_ident;
            }
            impl erp::model::Model<erp::field::MultipleIds> for #struct_name_ident<erp::field::MultipleIds> {
                type BaseModel = #base_model_name_ident;
            }
        }
    };

    let impl_model_fields_single = fields.iter().filter_map(|f| {
        let FieldGen {
            field_name,
            is_required,
            is_reference,
            is_reference_multi,
            field_type_keyword,
            ..
        } = f;
        if field_name == "id" {
            return None;
        }
        let get_field_ident = Ident::new(format!("get_{}", field_name).as_str(), Span::call_site());
        let set_field_ident = Ident::new(format!("set_{}", field_name).as_str(), Span::call_site());

        if *is_reference {
            if *is_reference_multi {
                Some(quote! {
                    pub fn #get_field_ident<M>(&self, env: &mut erp::environment::Environment) -> Result<M, Box<dyn std::error::Error>>
                    where
                        M: erp::model::Model<erp::field::MultipleIds, BaseModel=#field_type_keyword>,
                    {
                        <Self as erp::model::Model<erp::field::SingleId>>::get_references::<M, #field_type_keyword>(self, #field_name, env)
                    }
                    pub fn #set_field_ident(&self, value: erp::field::Reference<#field_type_keyword, erp::field::MultipleIds>, env: &mut erp::environment::Environment) -> Result<(), Box<dyn std::error::Error>> {
                        <Self as erp::model::Model<erp::field::SingleId>>::set_references(self, #field_name, value, env)
                    }
                })
            } else {
                Some(quote! {
                    pub fn #get_field_ident<M>(&self, env: &mut erp::environment::Environment) -> Result<Option<M>, Box<dyn std::error::Error>>
                    where
                        M: erp::model::Model<erp::field::SingleId, BaseModel=#field_type_keyword>,
                    {
                        <Self as erp::model::Model<erp::field::SingleId>>::get_reference::<M, #field_type_keyword>(self, #field_name, env)
                    }
                    pub fn #set_field_ident(&self, value: Option<erp::field::Reference<#field_type_keyword, erp::field::SingleId>>, env: &mut erp::environment::Environment) -> Result<(), Box<dyn std::error::Error>> {
                        if let Some(value) = value {
                            <Self as erp::model::Model<erp::field::SingleId>>::set_reference(self, #field_name, value, env)
                        } else {
                            <Self as erp::model::Model<erp::field::SingleId>>::set_option::<u32>(self, #field_name, None, env)
                        }
                    }
                })
            }
        } else if *is_required {
            Some(quote! {
                pub fn #get_field_ident<'a>(&self, env: &'a mut erp::environment::Environment) -> Result<&'a #field_type_keyword, Box<dyn std::error::Error>>
                {
                    <Self as erp::model::Model<erp::field::SingleId>>::get(self, #field_name, env)
                }
                pub fn #set_field_ident(&self, value: #field_type_keyword, env: &mut erp::environment::Environment) -> Result<(), Box<dyn std::error::Error>> {
                    <Self as erp::model::Model<erp::field::SingleId>>::set(self, #field_name, value, env)
                }
            })
        } else {
            Some(quote! {
                pub fn #get_field_ident<'a>(&self, env: &'a mut erp::environment::Environment) -> Result<Option<&'a #field_type_keyword>, Box<dyn std::error::Error>> {
                    <Self as erp::model::Model<erp::field::SingleId>>::get_option(self, #field_name, env)
                }
                pub fn #set_field_ident(&self, value: Option<#field_type_keyword>, env: &mut erp::environment::Environment) -> Result<(), Box<dyn std::error::Error>> {
                    <Self as erp::model::Model<erp::field::SingleId>>::set_option(self, #field_name, value, env)
                }
            })
        }
    });
    let impl_model_fields_multi = fields.iter().filter_map(|f| {
        let FieldGen {
            field_name,
            is_required,
            is_reference,
            is_reference_multi,
            field_type_keyword,
            ..
        } = f;
        if field_name == "id" {
            return None;
        }
        let get_field_ident = Ident::new(format!("get_{}", field_name).as_str(), Span::call_site());
        // TODO Move the set to another place, as it's not needed to be different between SingleId & MultipleIds
        let set_field_ident = Ident::new(format!("set_{}", field_name).as_str(), Span::call_site());

        if *is_reference {
            if *is_reference_multi {
                Some(quote! {
                    pub fn #get_field_ident<M>(&self, env: &mut erp::environment::Environment) -> Result<M, Box<dyn std::error::Error>>
                    where
                        M: erp::model::Model<erp::field::MultipleIds, BaseModel=#field_type_keyword>,
                    {
                        <Self as erp::model::Model<erp::field::MultipleIds>>::get_references::<M, #field_type_keyword>(self, #field_name, env)
                    }
                    pub fn #set_field_ident(&self, value: erp::field::Reference<#field_type_keyword, erp::field::MultipleIds>, env: &mut erp::environment::Environment) -> Result<(), Box<dyn std::error::Error>> {
                        <Self as erp::model::Model<erp::field::MultipleIds>>::set_references(self, #field_name, value, env)
                    }
                })
            } else {
                Some(quote! {
                    pub fn #get_field_ident<M>(&self, env: &mut erp::environment::Environment) -> Result<M, Box<dyn std::error::Error>>
                    where
                        M: erp::model::Model<erp::field::MultipleIds, BaseModel=#field_type_keyword>,
                    {
                        <Self as erp::model::Model<erp::field::MultipleIds>>::get_references::<M, #field_type_keyword>(self, #field_name, env)
                    }
                    pub fn #set_field_ident(&self, value: Option<erp::field::Reference<#field_type_keyword, erp::field::SingleId>>, env: &mut erp::environment::Environment) -> Result<(), Box<dyn std::error::Error>> {
                        if let Some(value) = value {
                            <Self as erp::model::Model<erp::field::MultipleIds>>::set_reference(self, #field_name, value, env)
                        } else {
                            <Self as erp::model::Model<erp::field::MultipleIds>>::set_option::<u32>(self, #field_name, None, env)
                        }
                    }
                })
            }
        } else if *is_required {
            Some(quote! {
                pub fn #get_field_ident<'a>(&self, env: &'a mut erp::environment::Environment) -> Result<Vec<&'a #field_type_keyword>, Box<dyn std::error::Error>>
                {
                    <Self as erp::model::Model<erp::field::MultipleIds>>::gets(self, #field_name, env)
                }
                pub fn #set_field_ident(&self, value: #field_type_keyword, env: &mut erp::environment::Environment) -> Result<(), Box<dyn std::error::Error>> {
                    <Self as erp::model::Model<erp::field::MultipleIds>>::set(self, #field_name, value, env)
                }
            })
        } else {
            Some(quote! {
                pub fn #get_field_ident<'a>(&self, env: &'a mut erp::environment::Environment) -> Result<Vec<Option<&'a #field_type_keyword>>, Box<dyn std::error::Error>> {
                    <Self as erp::model::Model<erp::field::MultipleIds>>::get_options(self, #field_name, env)
                }
                pub fn #set_field_ident(&self, value: Option<#field_type_keyword>, env: &mut erp::environment::Environment) -> Result<(), Box<dyn std::error::Error>> {
                    <Self as erp::model::Model<erp::field::MultipleIds>>::set_option(self, #field_name, value, env)
                }
            })
        }
    });
    let impl_model = quote! {

        impl #struct_name_ident<erp::field::SingleId> {
            pub fn get_id(&self) -> u32 {
                self.id.get_id()
            }
            pub fn get_id_ref(&self) -> &u32 {
                self.id.get_id_ref()
            }

            #(#impl_model_fields_single)*
        }

        impl #struct_name_ident<erp::field::MultipleIds> {
            pub fn get_ids(&self) -> Vec<u32> {
                self.id.get_ids_ref().clone()
            }

            pub fn get_ids_ref(&self) -> &Vec<u32> {
                &self.id.get_ids_ref()
            }

            #(#impl_model_fields_multi)*
        }

        impl<Mode: erp::field::IdMode + PartialEq> PartialEq for #struct_name_ident<Mode> {
            fn eq(&self, other: &Self) -> bool {
                self.id == other.id
            }
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
            default: default_value,
            description,
            compute,
            depends,
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
                FieldType::Refs(r) => {
                    let tokens = r.iter().map(|dep| quote! { #dep });
                    quote! {
                        {
                            let refs = vec![#(#tokens),*];
                            Some(erp::field::FieldType::Refs(refs))
                        }
                    }
                },
            }
        } else if *is_reference {
            quote! {
                Some(erp::field::FieldType::Ref(0))
            }
        } else {
            quote! {
                Some((#field_type_keyword::default()).into())
            }
        };

        let description = if let Some(description) = description {
            quote! { Some(#description.to_string()) }
        } else {
            quote! { None }
        };

        let compute = if compute.is_some() {
            quote! { Some(true) }
        } else {
            quote! { None }
        };

        let depends = if let Some(depends) = depends {
            let tokens = depends.iter().map(|dep| quote! { #dep.to_string() });
            quote! {
                {
                    let depends = vec![#(#tokens),*];
                    Some(depends)
                }
            }
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
                    compute: #compute,
                    depends: #depends,
                }
            }
        }
    });

    let create_model = fields.iter().map(|f| {
        let FieldGen {
            field_name,
            ..
        } = f;
        let field_ident = Ident::new(field_name, Span::call_site());

        quote! {
            #field_ident: Default::default()
        }
    });
    let create_model_2 = create_model.clone();
    let create_model_3 = create_model.clone();

    let compute_fields = fields.iter().filter_map(|f| {
        let FieldGen {
            field_name,
            compute,
            ..
        } = f;
        let compute = compute.as_ref()?.to_string();
        let compute_method_ident = Ident::new(&compute, Span::call_site());
        Some(quote! {
            if field_name == #field_name {
                return self.#compute_method_ident(env);
            }
        })
    });

    let common_model_impl = quote! {
        impl<Mode: erp::field::IdMode> erp::model::CommonModel<Mode> for #ident<Mode> where #ident<Mode>: erp::model::Model<Mode> {
            fn get_id_mode(&self) -> &Mode {
                &self.id
            }

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

            fn create_instance(id: Mode) -> Self {
                Self {
                    id,
                    #(#create_model,)*
                }
            }

            fn create_single_id_instance(id: erp::field::SingleId) -> Box<dyn erp::model::CommonModel<erp::field::SingleId>>
            where
                Self: Sized
            {
                Box::new(#ident {
                    id,
                    #(#create_model_2,)*
                })
            }

            fn create_multiple_ids_instance(id: erp::field::MultipleIds) -> Box<dyn erp::model::CommonModel<erp::field::MultipleIds>>
            where
                Self: Sized
            {
                Box::new(#ident {
                    id,
                    #(#create_model_3,)*
                })
            }

            // fn call_compute_method(
            //     &mut self,
            //     field_name: &str,
            //     env: &mut erp::environment::Environment,
            // ) -> Result<(), Box<dyn std::error::Error>> {
            //     #(#compute_fields)*
            //     Ok(())
            // }
        }
    };

    let result = quote! {
        #base_model

        #impl_model

        #common_model_impl
    };

    Ok(result)
}