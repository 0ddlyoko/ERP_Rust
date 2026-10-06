use crate::model::field::FieldGen;
use crate::model::model::ModelGen;
use erp_types::field::FieldType;
use erp_types::string::StringTransform;
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
        name_field,
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

        if *is_reference {
            if *is_reference_multi {
                Some(quote! {
                    pub fn #get_field_ident<M>(&self, env: &mut erp::environment::Environment) -> ::core::result::Result<M, Box<dyn std::error::Error + Send + Sync>>
                    where
                        M: erp::model::Model<erp::types::field::MultipleIds, BaseModel=#field_type_keyword>,
                    {
                        (self as &dyn erp::model::Model<erp::types::field::SingleId, BaseModel=<Self as erp::types::model::CommonModel<erp::types::field::SingleId>>::BaseModel>).get_references::<M, #field_type_keyword>(#field_name, env)
                    }
                })
            } else {
                Some(quote! {
                    pub fn #get_field_ident<M>(&self, env: &mut erp::environment::Environment) -> ::core::result::Result<M, Box<dyn std::error::Error + Send + Sync>>
                    where
                        M: erp::model::Model<erp::types::field::SingleId, BaseModel=#field_type_keyword>,
                    {
                        (self as &dyn erp::model::Model<erp::types::field::SingleId, BaseModel=<Self as erp::types::model::CommonModel<erp::types::field::SingleId>>::BaseModel>).get_reference::<M, #field_type_keyword>(#field_name, env)
                    }
                })
            }
        } else if *is_required {
            Some(quote! {
                pub fn #get_field_ident<'a>(&self, env: &'a mut erp::environment::Environment) -> ::core::result::Result<&'a #field_type_keyword, Box<dyn std::error::Error + Send + Sync>>
                {
                    (self as &dyn erp::model::Model<erp::types::field::SingleId, BaseModel=<Self as erp::types::model::CommonModel<erp::types::field::SingleId>>::BaseModel>).get(#field_name, env)
                }
            })
        } else {
            Some(quote! {
                pub fn #get_field_ident<'a>(&self, env: &'a mut erp::environment::Environment) -> ::core::result::Result<Option<&'a #field_type_keyword>, Box<dyn std::error::Error + Send + Sync>> {
                    (self as &dyn erp::model::Model<erp::types::field::SingleId, BaseModel=<Self as erp::types::model::CommonModel<erp::types::field::SingleId>>::BaseModel>).get_option(#field_name, env)
                }
            })
        }
    });
    let impl_model_fields_multi = fields.iter().filter_map(|f| {
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
        let get_field_ident = Ident::new(format!("get_{field_name}").as_str(), Span::call_site());

        // A list of the targets either way: one per record, or all of them.
        if *is_reference {
            Some(quote! {
                pub fn #get_field_ident<M>(&self, env: &mut erp::environment::Environment) -> ::core::result::Result<M, Box<dyn std::error::Error + Send + Sync>>
                where
                    M: erp::model::Model<erp::types::field::MultipleIds, BaseModel=#field_type_keyword>,
                {
                    (self as &dyn erp::model::Model<erp::types::field::MultipleIds, BaseModel=<Self as erp::types::model::CommonModel<erp::types::field::MultipleIds>>::BaseModel>).get_references::<M, #field_type_keyword>(#field_name, env)
                }
            })
        } else if *is_required {
            Some(quote! {
                pub fn #get_field_ident<'a>(&self, env: &'a mut erp::environment::Environment) -> ::core::result::Result<Vec<&'a #field_type_keyword>, Box<dyn std::error::Error + Send + Sync>>
                {
                    (self as &dyn erp::model::Model<erp::types::field::MultipleIds, BaseModel=<Self as erp::types::model::CommonModel<erp::types::field::MultipleIds>>::BaseModel>).gets(#field_name, env)
                }
            })
        } else {
            Some(quote! {
                pub fn #get_field_ident<'a>(&self, env: &'a mut erp::environment::Environment) -> ::core::result::Result<Vec<Option<&'a #field_type_keyword>>, Box<dyn std::error::Error + Send + Sync>> {
                    (self as &dyn erp::model::Model<erp::types::field::MultipleIds, BaseModel=<Self as erp::types::model::CommonModel<erp::types::field::MultipleIds>>::BaseModel>).get_options(#field_name, env)
                }
            })
        }
    });

    // Setters are the same whichever ids the record holds, so they are generated once.
    let impl_model_setters = fields.iter().filter_map(|f| {
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
        let set_field_ident = Ident::new(format!("set_{field_name}").as_str(), Span::call_site());
        let this = quote! {
            (self as &dyn erp::model::Model<Mode, BaseModel=<Self as erp::types::model::CommonModel<Mode>>::BaseModel>)
        };
        Some(if *is_reference && *is_reference_multi {
            quote! {
                pub fn #set_field_ident<V>(&self, value: V, env: &mut erp::environment::Environment) -> ::core::result::Result<(), Box<dyn std::error::Error + Send + Sync>>
                where
                    V: erp::types::field::ToRecords<#field_type_keyword>,
                {
                    let value: erp::types::field::Reference<#field_type_keyword, erp::types::field::MultipleIds> = value.record_ids().into();
                    #this.set_references(#field_name, value, env)
                }
            }
        } else if *is_reference {
            quote! {
                pub fn #set_field_ident<V>(&self, value: V, env: &mut erp::environment::Environment) -> ::core::result::Result<(), Box<dyn std::error::Error + Send + Sync>>
                where
                    V: erp::types::field::ToRecord<#field_type_keyword>,
                {
                    let id = value.record_id();
                    #this.set_option::<u32>(#field_name, (id != 0).then_some(id), env)
                }
            }
        } else if *is_required {
            quote! {
                pub fn #set_field_ident<V>(&self, value: V, env: &mut erp::environment::Environment) -> ::core::result::Result<(), Box<dyn std::error::Error + Send + Sync>>
                where
                    #field_type_keyword: erp::types::field::Accepts<V>,
                {
                    let value = <#field_type_keyword as erp::types::field::Accepts<V>>::accept(value);
                    #this.set(#field_name, value, env)
                }
            }
        } else {
            quote! {
                pub fn #set_field_ident<V>(&self, value: V, env: &mut erp::environment::Environment) -> ::core::result::Result<(), Box<dyn std::error::Error + Send + Sync>>
                where
                    #field_type_keyword: erp::types::field::AcceptsOptional<V>,
                {
                    let value = <#field_type_keyword as erp::types::field::AcceptsOptional<V>>::accept_optional(value);
                    #this.set_option(#field_name, value, env)
                }
            }
        })
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

        /// Read fields of these records, one map per record.
        pub fn read(
            &self,
            fields: &[&str],
            env: &mut erp::environment::Environment,
        ) -> ::core::result::Result<Vec<erp::types::model::MapOfFields>, #err> {
            env.read(#model_name_multi, &self.id, fields)
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

            /// Whether this is no record, as an empty many2one reads.
            pub fn is_empty(&self) -> bool {
                erp::types::field::IdMode::is_empty(&self.id)
            }

            /// The record's id; `None` for no record.
            pub fn get_optional_id(&self) -> Option<u32> {
                (!self.is_empty()).then(|| self.get_id())
            }

            #verbs_single

            #(#impl_model_fields_single)*

            /// When the record was created; empty for one created before the ORM noted it.
            pub fn get_create_date<'a>(&self, env: &'a mut erp::environment::Environment) -> ::core::result::Result<Option<&'a erp::types::field::Timestamp>, Box<dyn std::error::Error + Send + Sync>> {
                (self as &dyn erp::model::Model<erp::types::field::SingleId, BaseModel=<Self as erp::types::model::CommonModel<erp::types::field::SingleId>>::BaseModel>).get_option(erp::model::CREATE_DATE, env)
            }

            /// When the record was last changed.
            pub fn get_write_date<'a>(&self, env: &'a mut erp::environment::Environment) -> ::core::result::Result<Option<&'a erp::types::field::Timestamp>, Box<dyn std::error::Error + Send + Sync>> {
                (self as &dyn erp::model::Model<erp::types::field::SingleId, BaseModel=<Self as erp::types::model::CommonModel<erp::types::field::SingleId>>::BaseModel>).get_option(erp::model::WRITE_DATE, env)
            }

            /// The user who created the record, by id; empty without a model of users, or for
            /// work nobody in particular did.
            pub fn get_create_uid(&self, env: &mut erp::environment::Environment) -> ::core::result::Result<Option<u32>, Box<dyn std::error::Error + Send + Sync>> {
                Ok((self as &dyn erp::model::Model<erp::types::field::SingleId, BaseModel=<Self as erp::types::model::CommonModel<erp::types::field::SingleId>>::BaseModel>).get_option::<u32>(erp::model::CREATE_UID, env)?.copied())
            }

            /// The user who last changed the record, by id.
            pub fn get_write_uid(&self, env: &mut erp::environment::Environment) -> ::core::result::Result<Option<u32>, Box<dyn std::error::Error + Send + Sync>> {
                Ok((self as &dyn erp::model::Model<erp::types::field::SingleId, BaseModel=<Self as erp::types::model::CommonModel<erp::types::field::SingleId>>::BaseModel>).get_option::<u32>(erp::model::WRITE_UID, env)?.copied())
            }
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

        impl<Mode: erp::types::field::IdMode + 'static> #struct_name_ident<Mode>
        where
            #struct_name_ident<Mode>: erp::model::Model<Mode>,
        {
            #(#impl_model_setters)*
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
    let name_field = match name_field {
        Some(name_field) => quote! { Some(#name_field.to_string()) },
        None => quote! { None },
    };

    let fields_descriptor = fields.iter().map(|f| {
        let FieldGen {
            field_name,
            is_required,
            is_reference,
            is_reference_multi,
            field_type_keyword,
            default: default_value,
            label,
            description,
            compute,
            depends,
            inverse,
            relation,
            is_private,
            asks_for_storage,
            is_tracked,
            is_editable,
            is_owned,
            on_delete,
            domain,
            index,
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
                // Out of reach: a password field is refused a default while its attributes are
                // read, which is where the message belongs. Here to keep the match honest.
                FieldType::Password(_) => quote! {
                    compile_error!("A password field takes no default")
                },
                // Out of reach as well: a default is written as a literal, never as commands.
                FieldType::Commands(_) => quote! {
                    compile_error!("A default is a value, not commands")
                },
            }
        } else if *is_reference {
            // A relation starts empty; there is no "no reference" sentinel any more.
            quote! { None }
        } else if *is_required {
            // Text and dates mean nothing by default: whoever creates the record gives them.
            match field_type_keyword.to_string().as_str() {
                "String" | "NaiveDate" | "Timestamp" => quote! { None },
                _ => quote! { Some((#field_type_keyword::default()).into()) },
            }
        } else {
            // `Option<T>` starts empty — that is what declaring it optional now means.
            quote! { None }
        };

        // The kind is derived from the declared Rust type at startup, not from its name: an enum
        // declared with `#[selection]` is found as one and stored as text, anything else by
        // building its default once.
        let (kind, selection) = if *is_reference_multi {
            (quote! { erp::types::field::FieldKind::Refs }, quote! { None })
        } else if *is_reference {
            (quote! { erp::types::field::FieldKind::Ref }, quote! { None })
        } else {
            let described = quote! {
                {
                    #[allow(unused_imports)]
                    use erp::types::field::{PlainFieldProbe as _, SelectionFieldProbe as _};
                    (&&erp::types::field::FieldProbe::<#field_type_keyword>::new()).describe()
                }
            };
            (quote! { #described.0 }, quote! { #described.1 })
        };

        let label = match label {
            Some(label) => quote! { Some(#label.to_string()) },
            None => quote! { None },
        };
        let description = if let Some(description) = description {
            quote! { Some(#description.to_string()) }
        } else {
            quote! { None }
        };

        let compute = if compute.is_some() {
            let depends = if let Some(depends) = depends {
                let tokens = depends.iter().map(|dep| quote! { #dep });
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

        // A computed list with neither relation nor inverse mirrors nothing: it is only a value.
        let is_plain_list = *is_reference_multi && relation.is_none() && inverse.is_none();
        let field_reference = if *is_reference && !is_plain_list {
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
                    target_model: #field_type_keyword::_get_model_name(),
                    inverse_field: #inverse_field,
                })
            }
        } else {
            quote! { None }
        };

        let on_delete = match on_delete.as_deref() {
            Some(key) => {
                quote! { erp::types::field::OnDelete::from_key(#key) }
            }
            None => quote! { None },
        };

        let domain = match domain {
            Some(text) => quote! { Some(#text) },
            None => quote! { None },
        };

        let index = match index.as_deref() {
            Some(key) => quote! { erp::types::field::FieldIndex::from_key(#key) },
            None => quote! { None },
        };

        quote! {
            {
                // Yep, I don't know how to call _get_model_name() without this line
                use erp::types::model::BaseModel;
                erp::types::field::FieldDescriptor {
                    name: #field_name.to_string(),
                    kind: #kind,
                    default_value: #default_value,
                    label: #label,
                    description: #description,
                    required: #is_required,
                    private: #is_private,
                    asks_for_storage: #asks_for_storage,
                    tracking: #is_tracked,
                    editable: #is_editable,
                    owned: #is_owned,
                    on_delete: #on_delete,
                    domain: #domain,
                    index: #index,
                    compute: #compute,
                    field_ref: #field_reference,
                    selection: #selection,
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
                    name_field: #name_field,
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
                erp::model::ModelIntoIterator::new(self.id.get_ids_ref().clone())
            }
        }

        impl<'a, Mode: erp::types::field::IdMode> IntoIterator for &'a #ident<Mode> {
            type Item = #ident<erp::types::field::SingleId>;
            type IntoIter = erp::model::ModelIterator<'a, Self::Item>;

            fn into_iter(self) -> Self::IntoIter {
                erp::model::ModelIterator::new(self.id.get_ids_ref())
            }
        }
    };

    // Always emitted, so that registering a model is enough to register its methods too: those of
    // its `#[erp_methods]` blocks on `MultipleIds` and on `SingleId`, whichever it has. A block
    // without `methods` on the struct fails its own assertion.
    let (register_methods, declares_methods) = if has_methods {
        (
            quote! {
                #[allow(unused_imports)]
                use erp::model::{NoBlock as _, RegisterBlock as _};
                (&erp::model::MethodsProbe::<#struct_name_ident<erp::types::field::MultipleIds>>::new())
                    .register(model_manager, plugin_name);
                (&erp::model::MethodsProbe::<#struct_name_ident<erp::types::field::SingleId>>::new())
                    .register(model_manager, plugin_name);
            },
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

    // A record is its ids: the other fields only ever hold defaults, the values live in the
    // cache. So cloning one is making another instance on the same ids.
    let clone_impl = quote! {
        impl<Mode: erp::types::field::IdMode> Clone for #ident<Mode>
        where
            #ident<Mode>: erp::types::model::CommonModel<Mode>,
        {
            fn clone(&self) -> Self {
                <Self as erp::types::model::CommonModel<Mode>>::create_instance(self.id.clone())
            }
        }
    };

    // So a controller argument typed as a record is read from its id.
    let from_param = quote! {
        impl erp::http::FromParam for #ident<erp::types::field::SingleId> {
            fn from_param(
                env: &mut erp::environment::Environment,
                raw: Option<String>,
            ) -> ::core::result::Result<Self, Box<dyn std::error::Error + Send + Sync>> {
                erp::http::find_record::<Self>(env, raw)
            }
        }
    };

    let result = quote! {
        #base_model

        #impl_model

        #common_model_impl

        #has_methods_impl

        #iterator

        #clone_impl

        #from_param
    };

    Ok(result)
}
