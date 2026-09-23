use crate::model::field::FieldGen;
use crate::model::model::ModelGen;
use erp::types::field::FieldType;
use erp::util::string::StringTransform;
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use syn::{DeriveInput, Path, Result, parse_str};

pub fn derive(item: &DeriveInput) -> Result<TokenStream> {
    let DeriveInput { ident, .. } = item;
    let ModelGen {
        struct_name,
        id,
        table_name,
        description,
        derived_model,
        has_methods,
        fields,
        ..
    } = ModelGen::from_item(item)?;

    let struct_name_ident = Ident::new(struct_name.as_str(), Span::call_site());
    // Derived from the identity, not the table: renaming the physical table must not rename
    // the generated Rust type.
    let camel_case_id = id.replace("_", " ").to_camel_case();
    let base_model_name = format!("Base{camel_case_id}");

    let (base_model, base_model_ref): (TokenStream, Path) = if let Some(derived_model) =
        derived_model
    {
        let full_base_model = if derived_model.is_empty() {
            base_model_name
        } else {
            format!("{derived_model}::{base_model_name}")
        };
        let full_base_model_path: Path = parse_str(&full_base_model)?;
        (
            quote! {
                impl erp::model::Model<erp::types::field::SingleId> for #struct_name_ident<erp::types::field::SingleId> {

                }
                impl erp::model::Model<erp::types::field::MultipleIds> for #struct_name_ident<erp::types::field::MultipleIds> {

                }
            },
            full_base_model_path,
        )
    } else {
        let base_model_name_ident = Ident::new(base_model_name.as_str(), Span::call_site());
        let base_model_path: Path = parse_str(&base_model_name_ident.to_string())?;
        (
            quote! {
                #[derive(Default, Debug)]
                pub struct #base_model_name_ident;

                impl erp::types::model::BaseModel for #base_model_name_ident {
                    fn _get_model_name() -> &'static str {
                        #id
                    }
                }

                impl erp::model::Model<erp::types::field::SingleId> for #struct_name_ident<erp::types::field::SingleId> {

                }
                impl erp::model::Model<erp::types::field::MultipleIds> for #struct_name_ident<erp::types::field::MultipleIds> {

                }
            },
            base_model_path,
        )
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
        let get_field_ident = Ident::new(format!("get_{field_name}").as_str(), Span::call_site());
        // TODO Move the set to another place, as it's not needed to be different between SingleId & MultipleIds
        let set_field_ident = Ident::new(format!("set_{field_name}").as_str(), Span::call_site());

        if *is_reference {
            if *is_reference_multi {
                Some(quote! {
                    pub fn #get_field_ident<M>(&self, env: &mut erp::environment::Environment) -> ::core::result::Result<M, Box<dyn std::error::Error + Send + Sync>>
                    where
                        M: erp::model::Model<erp::types::field::MultipleIds, BaseModel=#field_type_keyword>,
                    {
                        (self as &dyn erp::model::Model<erp::types::field::SingleId, BaseModel=<Self as erp::types::model::CommonModel<erp::types::field::SingleId>>::BaseModel>).get_references::<M, #field_type_keyword>(#field_name, env)
                    }
                    pub fn #set_field_ident(&self, value: erp::types::field::Reference<#field_type_keyword, erp::types::field::MultipleIds>, env: &mut erp::environment::Environment) -> ::core::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
                        (self as &dyn erp::model::Model<erp::types::field::SingleId, BaseModel=<Self as erp::types::model::CommonModel<erp::types::field::SingleId>>::BaseModel>).set_references(#field_name, value, env)
                    }
                })
            } else {
                Some(quote! {
                    pub fn #get_field_ident<M>(&self, env: &mut erp::environment::Environment) -> ::core::result::Result<Option<M>, Box<dyn std::error::Error + Send + Sync>>
                    where
                        M: erp::model::Model<erp::types::field::SingleId, BaseModel=#field_type_keyword>,
                    {
                        (self as &dyn erp::model::Model<erp::types::field::SingleId, BaseModel=<Self as erp::types::model::CommonModel<erp::types::field::SingleId>>::BaseModel>).get_reference::<M, #field_type_keyword>(#field_name, env)
                    }
                    pub fn #set_field_ident(&self, value: Option<erp::types::field::Reference<#field_type_keyword, erp::types::field::SingleId>>, env: &mut erp::environment::Environment) -> ::core::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
                        if let Some(value) = value {
                            (self as &dyn erp::model::Model<erp::types::field::SingleId, BaseModel=<Self as erp::types::model::CommonModel<erp::types::field::SingleId>>::BaseModel>).set_reference(#field_name, value, env)
                        } else {
                            (self as &dyn erp::model::Model<erp::types::field::SingleId, BaseModel=<Self as erp::types::model::CommonModel<erp::types::field::SingleId>>::BaseModel>).set_option::<u32>(#field_name, None, env)
                        }
                    }
                })
            }
        } else if *is_required {
            Some(quote! {
                pub fn #get_field_ident<'a>(&self, env: &'a mut erp::environment::Environment) -> ::core::result::Result<&'a #field_type_keyword, Box<dyn std::error::Error + Send + Sync>>
                {
                    (self as &dyn erp::model::Model<erp::types::field::SingleId, BaseModel=<Self as erp::types::model::CommonModel<erp::types::field::SingleId>>::BaseModel>).get(#field_name, env)
                }
                pub fn #set_field_ident(&self, value: #field_type_keyword, env: &mut erp::environment::Environment) -> ::core::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
                    (self as &dyn erp::model::Model<erp::types::field::SingleId, BaseModel=<Self as erp::types::model::CommonModel<erp::types::field::SingleId>>::BaseModel>).set(#field_name, value, env)
                }
            })
        } else {
            Some(quote! {
                pub fn #get_field_ident<'a>(&self, env: &'a mut erp::environment::Environment) -> ::core::result::Result<Option<&'a #field_type_keyword>, Box<dyn std::error::Error + Send + Sync>> {
                    (self as &dyn erp::model::Model<erp::types::field::SingleId, BaseModel=<Self as erp::types::model::CommonModel<erp::types::field::SingleId>>::BaseModel>).get_option(#field_name, env)
                }
                pub fn #set_field_ident(&self, value: Option<#field_type_keyword>, env: &mut erp::environment::Environment) -> ::core::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
                    (self as &dyn erp::model::Model<erp::types::field::SingleId, BaseModel=<Self as erp::types::model::CommonModel<erp::types::field::SingleId>>::BaseModel>).set_option(#field_name, value, env)
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
        let get_field_ident = Ident::new(format!("get_{field_name}").as_str(), Span::call_site());
        // TODO Move the set to another place, as it's not needed to be different between SingleId & MultipleIds
        let set_field_ident = Ident::new(format!("set_{field_name}").as_str(), Span::call_site());

        if *is_reference {
            if *is_reference_multi {
                Some(quote! {
                    pub fn #get_field_ident<M>(&self, env: &mut erp::environment::Environment) -> ::core::result::Result<M, Box<dyn std::error::Error + Send + Sync>>
                    where
                        M: erp::model::Model<erp::types::field::MultipleIds, BaseModel=#field_type_keyword>,
                    {
                        (self as &dyn erp::model::Model<erp::types::field::MultipleIds, BaseModel=<Self as erp::types::model::CommonModel<erp::types::field::MultipleIds>>::BaseModel>).get_references::<M, #field_type_keyword>(#field_name, env)
                    }
                    pub fn #set_field_ident(&self, value: erp::types::field::Reference<#field_type_keyword, erp::types::field::MultipleIds>, env: &mut erp::environment::Environment) -> ::core::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
                        (self as &dyn erp::model::Model<erp::types::field::MultipleIds, BaseModel=<Self as erp::types::model::CommonModel<erp::types::field::MultipleIds>>::BaseModel>).set_references(#field_name, value, env)
                    }
                })
            } else {
                Some(quote! {
                    pub fn #get_field_ident<M>(&self, env: &mut erp::environment::Environment) -> ::core::result::Result<M, Box<dyn std::error::Error + Send + Sync>>
                    where
                        M: erp::model::Model<erp::types::field::MultipleIds, BaseModel=#field_type_keyword>,
                    {
                        (self as &dyn erp::model::Model<erp::types::field::MultipleIds, BaseModel=<Self as erp::types::model::CommonModel<erp::types::field::MultipleIds>>::BaseModel>).get_references::<M, #field_type_keyword>(#field_name, env)
                    }
                    pub fn #set_field_ident(&self, value: Option<erp::types::field::Reference<#field_type_keyword, erp::types::field::SingleId>>, env: &mut erp::environment::Environment) -> ::core::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
                        if let Some(value) = value {
                            (self as &dyn erp::model::Model<erp::types::field::MultipleIds, BaseModel=<Self as erp::types::model::CommonModel<erp::types::field::MultipleIds>>::BaseModel>).set_reference(#field_name, value, env)
                        } else {
                            (self as &dyn erp::model::Model<erp::types::field::MultipleIds, BaseModel=<Self as erp::types::model::CommonModel<erp::types::field::MultipleIds>>::BaseModel>).set_option::<u32>(#field_name, None, env)
                        }
                    }
                })
            }
        } else if *is_required {
            Some(quote! {
                pub fn #get_field_ident<'a>(&self, env: &'a mut erp::environment::Environment) -> ::core::result::Result<Vec<&'a #field_type_keyword>, Box<dyn std::error::Error + Send + Sync>>
                {
                    (self as &dyn erp::model::Model<erp::types::field::MultipleIds, BaseModel=<Self as erp::types::model::CommonModel<erp::types::field::MultipleIds>>::BaseModel>).gets(#field_name, env)
                }
                pub fn #set_field_ident(&self, value: #field_type_keyword, env: &mut erp::environment::Environment) -> ::core::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
                    (self as &dyn erp::model::Model<erp::types::field::MultipleIds, BaseModel=<Self as erp::types::model::CommonModel<erp::types::field::MultipleIds>>::BaseModel>).set(#field_name, value, env)
                }
            })
        } else {
            Some(quote! {
                pub fn #get_field_ident<'a>(&self, env: &'a mut erp::environment::Environment) -> ::core::result::Result<Vec<Option<&'a #field_type_keyword>>, Box<dyn std::error::Error + Send + Sync>> {
                    (self as &dyn erp::model::Model<erp::types::field::MultipleIds, BaseModel=<Self as erp::types::model::CommonModel<erp::types::field::MultipleIds>>::BaseModel>).get_options(#field_name, env)
                }
                pub fn #set_field_ident(&self, value: Option<#field_type_keyword>, env: &mut erp::environment::Environment) -> ::core::result::Result<(), Box<dyn std::error::Error + Send + Sync>> {
                    (self as &dyn erp::model::Model<erp::types::field::MultipleIds, BaseModel=<Self as erp::types::model::CommonModel<erp::types::field::MultipleIds>>::BaseModel>).set_option(#field_name, value, env)
                }
            })
        }
    });

    // The verbs of the ORM, on the model itself: `SaleOrder::search(...)` rather than
    // `env.search::<SaleOrder<_>>(...)`. Generated as inherent methods rather than put on a
    // trait, so that reaching them never asks for an import.
    let err = quote! { ::std::boxed::Box<dyn ::std::error::Error + Send + Sync> };
    let model_name_multi = quote! {
        <Self as erp::types::model::CommonModel<erp::types::field::MultipleIds>>::_get_model_name()
    };
    let model_name_single = quote! {
        <Self as erp::types::model::CommonModel<erp::types::field::SingleId>>::_get_model_name()
    };

    let verbs_multi = quote! {
        /// Records matching a domain.
        pub fn search(
            domain: &erp::search::SearchType,
            env: &mut erp::environment::Environment,
        ) -> ::core::result::Result<Self, #err> {
            env.search::<Self>(domain)
        }

        /// Same, limited, offset or ordered.
        pub fn search_with(
            domain: &erp::search::SearchType,
            options: &erp::search::SearchOptions,
            env: &mut erp::environment::Environment,
        ) -> ::core::result::Result<Self, #err> {
            env.search_with::<Self>(domain, options)
        }

        /// How many records match, before any limit would apply.
        pub fn count(
            domain: &erp::search::SearchType,
            env: &mut erp::environment::Environment,
        ) -> ::core::result::Result<u32, #err> {
            env.count(#model_name_multi, domain)
        }

        /// A recordset over ids already known, without asking the database.
        pub fn from_ids(
            ids: impl Into<erp::types::field::MultipleIds>,
            env: &erp::environment::Environment,
        ) -> Self {
            env.get_record::<Self, erp::types::field::MultipleIds>(ids.into())
        }

        /// Create one record per set of values.
        pub fn create(
            values: Vec<erp::types::model::MapOfFields>,
            env: &mut erp::environment::Environment,
        ) -> ::core::result::Result<Self, #err> {
            env.create_new_records_from_maps::<Self>(values)
        }

        /// Read fields of these records, one map per record.
        pub fn read(
            &self,
            fields: &[&str],
            env: &mut erp::environment::Environment,
        ) -> ::core::result::Result<Vec<erp::types::model::MapOfFields>, #err> {
            env.read(#model_name_multi, &self.id, fields)
        }

        /// Write the same values to every record of the set.
        pub fn write(
            &self,
            values: erp::types::model::MapOfFields,
            env: &mut erp::environment::Environment,
        ) -> ::core::result::Result<(), #err> {
            env.write(#model_name_multi, &self.id, values)
        }

        /// Delete these records, and report how many went.
        pub fn delete(
            &self,
            env: &mut erp::environment::Environment,
        ) -> ::core::result::Result<u32, #err> {
            env.delete(#model_name_multi, &self.id)
        }
    };

    let verbs_single = quote! {
        /// A record over an id already known, without asking the database.
        pub fn from_id(id: u32, env: &erp::environment::Environment) -> Self {
            env.get_record::<Self, erp::types::field::SingleId>(id.into())
        }

        /// Create one record.
        pub fn create(
            values: erp::types::model::MapOfFields,
            env: &mut erp::environment::Environment,
        ) -> ::core::result::Result<Self, #err> {
            env.create_new_record_from_map::<Self>(values)
        }

        /// Read fields of this record.
        pub fn read(
            &self,
            fields: &[&str],
            env: &mut erp::environment::Environment,
        ) -> ::core::result::Result<Vec<erp::types::model::MapOfFields>, #err> {
            env.read(#model_name_single, &self.id, fields)
        }

        /// Write values to this record.
        pub fn write(
            &self,
            values: erp::types::model::MapOfFields,
            env: &mut erp::environment::Environment,
        ) -> ::core::result::Result<(), #err> {
            env.write(#model_name_single, &self.id, values)
        }

        /// Delete this record.
        pub fn delete(
            &self,
            env: &mut erp::environment::Environment,
        ) -> ::core::result::Result<u32, #err> {
            env.delete(#model_name_single, &self.id)
        }
    };

    let impl_model = quote! {

        impl #struct_name_ident<erp::types::field::SingleId> {
            pub fn get_id(&self) -> u32 {
                self.id.get_id()
            }
            pub fn get_id_ref(&self) -> &u32 {
                self.id.get_id_ref()
            }

            #verbs_single

            #(#impl_model_fields_single)*
        }

        impl #struct_name_ident<erp::types::field::MultipleIds> {
            pub fn get_ids(&self) -> Vec<u32> {
                self.id.get_ids_ref().clone()
            }

            pub fn get_ids_ref(&self) -> &Vec<u32> {
                &self.id.get_ids_ref()
            }

            #verbs_multi

            #(#impl_model_fields_multi)*
        }

        impl<Mode: erp::types::field::IdMode + PartialEq> PartialEq for #struct_name_ident<Mode> {
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
            is_reference_multi,
            field_type_keyword,
            default: default_value,
            description,
            compute,
            depends,
            inverse,
            relation,
            is_private,
            ..
        } = f;

        let default_value = if let Some(default_value) = default_value {
            match default_value {
                FieldType::String(s) => quote! {
                    Some(erp::types::field::FieldType::String(#s.to_string()))
                },
                FieldType::Integer(i) => quote! {
                    Some(erp::types::field::FieldType::Integer(#i))
                },
                FieldType::Decimal(d) => {
                    // Rebuilt from its textual form: `Decimal` has no literal representation the
                    // macro could emit directly, and the value was already validated above.
                    let repr = d.to_string();
                    quote! {
                        Some(erp::types::field::FieldType::Decimal(
                            <erp::types::field::Decimal as ::core::str::FromStr>::from_str(#repr)
                                .expect("decimal default validated at compile time")
                        ))
                    }
                }
                FieldType::Bool(b) => quote! {
                    Some(erp::types::field::FieldType::Bool(#b))
                },
                FieldType::Date(d) => {
                    let repr = d.to_string();
                    quote! {
                        Some(erp::types::field::FieldType::Date(
                            <erp::types::field::NaiveDate as ::core::str::FromStr>::from_str(#repr)
                                .expect("date default validated at compile time")
                        ))
                    }
                }
                FieldType::DateTime(dt) => {
                    let repr = dt.to_rfc3339();
                    quote! {
                        Some(erp::types::field::FieldType::DateTime(
                            <erp::types::field::Timestamp as ::core::str::FromStr>::from_str(#repr)
                                .expect("datetime default validated at compile time")
                        ))
                    }
                }
                FieldType::Ref(r) => quote! {
                    Some(erp::types::field::FieldType::Ref(#r))
                },
                FieldType::Refs(r) => {
                    let tokens = r.iter().map(|dep| quote! { #dep });
                    quote! {
                        {
                            let refs = vec![#(#tokens),*];
                            Some(erp::types::field::FieldType::Refs(refs))
                        }
                    }
                }
            }
        } else if *is_reference {
            // A relation starts empty; there is no "no reference" sentinel any more.
            quote! { None }
        } else if *is_required {
            // A bare `T` takes its type's default.
            quote! { Some((#field_type_keyword::default()).into()) }
        } else {
            // `Option<T>` starts empty — that is what declaring it optional now means.
            quote! { None }
        };

        // The kind is derived from the declared Rust type by building its default once at
        // startup, which reuses the existing `From<T> for FieldType` impls — including the
        // blanket one for enums — instead of matching on type names in the macro.
        let kind = if *is_reference_multi {
            quote! { erp::types::field::FieldKind::Refs }
        } else if *is_reference {
            quote! { erp::types::field::FieldKind::Ref }
        } else {
            quote! {
                erp::types::field::FieldType::from(#field_type_keyword::default()).kind()
            }
        };

        let description = if let Some(description) = description {
            quote! { Some(#description.to_string()) }
        } else {
            quote! { None }
        };

        let compute = if compute.is_some() {
            let depends = if let Some(depends) = depends {
                let tokens = depends.iter().map(|dep| quote! { #dep.to_string() });
                quote! {
                    {
                        vec![#(#tokens),*]
                    }
                }
            } else {
                quote! { vec![] }
            };

            let method = compute.as_ref().map(ToString::to_string).unwrap_or_default();
            quote! {
                Some(erp::types::field::FieldCompute {
                    method: #method.to_string(),
                    depends: #depends,
                })
            }
        } else {
            quote! { None }
        };

        let field_reference = if *is_reference {
            let inverse_field = if let Some(relation) = relation {
                // Column names come from the two model ids, so declaring the relation table is
                // enough; the other side names the same table and sees the columns swapped.
                quote! {
                    erp::types::field::FieldReferenceType::M2M {
                        relation: #relation.to_string(),
                        column: erp::types::field::FieldReferenceType::relation_column(
                            <Self as erp::types::model::CommonModel<Mode>>::_get_model_name(),
                        ),
                        target_column: erp::types::field::FieldReferenceType::relation_column(
                            #field_type_keyword::_get_model_name(),
                        ),
                    }
                }
            } else if let Some(inverse) = inverse {
                quote! { erp::types::field::FieldReferenceType::O2M { inverse_field: #inverse.to_string() } }
            } else {
                quote! { erp::types::field::FieldReferenceType::M2O { inverse_fields: Vec::new() } }
            };

            quote! {
                Some(erp::types::field::FieldReference {
                    target_model: #field_type_keyword::_get_model_name().to_string(),
                    inverse_field: #inverse_field,
                })
            }
        } else {
            quote! { None }
        };

        quote! {
            {
                // Yep, I don't know how to call _get_model_name() without this line
                use erp::types::model::BaseModel;
                erp::types::field::FieldDescriptor {
                    name: #field_name.to_string(),
                    kind: #kind,
                    default_value: #default_value,
                    description: #description,
                    required: #is_required,
                    private: #is_private,
                    compute: #compute,
                    field_ref: #field_reference,
                }
            }
        }
    });

    let create_model = fields.iter().map(|f| {
        let FieldGen { field_name, .. } = f;
        let field_ident = Ident::new(field_name, Span::call_site());

        quote! {
            #field_ident: Default::default()
        }
    });

    let common_model_impl = quote! {
        impl<Mode: erp::types::field::IdMode> erp::types::model::CommonModel<Mode> for #ident<Mode> where #ident<Mode>: erp::model::Model<Mode> {
            type BaseModel = #base_model_ref;

            fn get_id_mode(&self) -> &Mode {
                &self.id
            }

            fn get_model_descriptor() -> erp::types::model::ModelDescriptor {
                let name = Self::_get_model_name().to_string();
                let description = #description;
                let fields = vec![
                    #(#fields_descriptor,)*
                ];
                erp::types::model::ModelDescriptor {
                    name,
                    table_name: #table_name.to_string(),
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

        }
    };

    let iterator = quote! {
        impl<Mode: erp::types::field::IdMode> IntoIterator for #ident<Mode> {
            type Item = #ident<erp::types::field::SingleId>;
            type IntoIter = erp::model::ModelIntoIterator<Self::Item>;

            fn into_iter(self) -> Self::IntoIter {
                erp::model::ModelIntoIterator {
                    ids: self.id.get_ids_ref().clone().into_iter(),
                    _phantom_data: Default::default(),
                }
            }
        }

        impl<'a, Mode: erp::types::field::IdMode> IntoIterator for &'a #ident<Mode> {
            type Item = #ident<erp::types::field::SingleId>;
            type IntoIter = erp::model::ModelIterator<'a, Self::Item>;

            fn into_iter(self) -> Self::IntoIter {
                erp::model::ModelIterator {
                    ids: self.id.get_ids_ref().iter(),
                    _phantom_data: Default::default(),
                }
            }
        }
    };

    // Always emitted, so that registering a model is enough to register its methods too. The
    // body delegates to `#[erp_methods]` when the struct declares any, which is also what makes
    // the two attributes have to agree: `methods` without the block fails to resolve the
    // function, and the block without `methods` fails its own assertion.
    let (register_methods, declares_methods) = if has_methods {
        (
            quote! { Self::__erp_register_methods(model_manager, plugin_name) },
            quote! {
                impl erp::model::DeclaresMethods
                    for #struct_name_ident<erp::types::field::MultipleIds> {}
            },
        )
    } else {
        (quote! { let _ = (model_manager, plugin_name); }, quote! {})
    };
    let has_methods_impl = quote! {
        #declares_methods

        impl erp::model::HasMethods for #struct_name_ident<erp::types::field::MultipleIds> {
            fn register_methods(
                model_manager: &mut erp::model::ModelManager,
                plugin_name: &str,
            ) {
                #register_methods
            }
        }
    };

    let result = quote! {
        #base_model

        #impl_model

        #common_model_impl

        #has_methods_impl

        #iterator
    };

    Ok(result)
}
