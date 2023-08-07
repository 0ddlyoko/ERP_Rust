use syn::{Data, DataStruct, DeriveInput, Fields};
use syn::punctuated::Punctuated;
use syn::Field as SynField;
use syn::Result;
use syn::token::Comma;
use crate::attrs::{AllowedModelAttr, parse_attributes};
use crate::field::Field;
use crate::util::generate_missing_table_name_error;

pub struct Model {
    table_name: String,
    fields: Vec<Field>,
}

impl Model {
    pub fn from_item(item: &DeriveInput) -> Result<Self> {
        let DeriveInput {
            data, ident, attrs, ..
        } = item;

        let mut table_name: String = String::new();

        for attr in parse_attributes(attrs)? {
            match attr.item {
                AllowedModelAttr::TableName(_, value) => table_name = value.value(),
            }
        }

        if table_name.is_empty() {
            return Err(generate_missing_table_name_error(ident.span()));
        }

        // Fields
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
            table_name: table_name.to_string(),
            fields: fields,
        })
    }

    pub fn table_name(&self) -> &String {
        &self.table_name
    }

    pub fn fields(&self) -> &[Field] {
        &self.fields
    }
}

fn syn_fields_from_data(fields: Option<&Punctuated<SynField, Comma>>) -> Result<Vec<Field>> {
    fields.map(|fields| {
        fields.iter()
            .map(|f| Field::from_item(f))
            .collect::<Result<Vec<_>>>()
    }).unwrap_or_else(|| Ok(Vec::new()))
}
