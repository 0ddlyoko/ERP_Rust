use crate::model::attrs::{AllowedModelAttrs, parse_attributes};
use crate::model::field::FieldGen;
use crate::model::util::gen_missing_key_error;
use syn::punctuated::Punctuated;
use syn::token::Comma;
use syn::{Data, DataStruct, DeriveInput, Field, Fields, Result};

pub struct ModelGen {
    pub struct_name: String,
    /// Identity of the model: what the API, `derived_model` and the registry key on.
    pub id: String,
    /// Physical table backing the model. Defaults to [`ModelGen::id`].
    pub table_name: String,
    pub description: Option<String>,
    pub derived_model: Option<String>,
    pub fields: Vec<FieldGen>,
}

impl ModelGen {
    pub fn from_item(item: &DeriveInput) -> Result<Self> {
        let DeriveInput {
            data, ident, attrs, ..
        } = item;
        let struct_name = ident.to_string();
        let mut id = String::new();
        let mut table_name = None;
        let mut description = None;
        let mut derived_model = None;

        for attr in parse_attributes(attrs)? {
            match attr.item {
                AllowedModelAttrs::Id(_, value) => id = value.value(),
                AllowedModelAttrs::TableName(_, value) => table_name = Some(value.value()),
                AllowedModelAttrs::Description(_, value) => description = Some(value.value()),
                AllowedModelAttrs::DerivedModel(_, value) => derived_model = Some(value.value()),
            }
        }
        if id.is_empty() {
            return Err(gen_missing_key_error(ident.span(), "id"));
        }
        let table_name = table_name.unwrap_or_else(|| id.clone());
        if description.is_none() {
            description = Some(id.clone());
        }

        let fields = match data {
            Data::Struct(DataStruct {
                fields: Fields::Named(named_fields),
                ..
            }) => Some(&named_fields.named),
            Data::Struct(DataStruct {
                fields: Fields::Unnamed(unnamed_fields),
                ..
            }) => Some(&unnamed_fields.unnamed),
            _ => None,
        };
        let fields = syn_fields_from_data(fields)?;

        Ok(Self {
            struct_name,
            id,
            table_name,
            description,
            derived_model,
            fields,
        })
    }
}

fn syn_fields_from_data(fields: Option<&Punctuated<Field, Comma>>) -> Result<Vec<FieldGen>> {
    fields
        .map(|fields| {
            fields
                .iter()
                .filter(|field| {
                    field
                        .ident
                        .as_ref()
                        .map(|ident| ident != "id")
                        .unwrap_or(true)
                })
                .map(FieldGen::from_item)
                .collect()
        })
        .unwrap_or_else(|| Ok(Vec::new()))
}
