use crate::attrs::AllowedFieldAttrs::{Default};
use crate::attrs::AllowedModelAttrs::{Description, TableName};
use crate::util::{gen_unknown_key_error, parse_eq};
use proc_macro2::{Ident, Span};
use syn::parse::{Parse, ParseStream, Result};
use syn::{Attribute, LitStr};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::token::Comma;

pub trait MySpanned {
    fn span(&self) -> Span;
}

pub struct AttributeWrapper<T> {
    pub item: T,
    pub attribute_span: Span,
}

// Models

pub enum AllowedModelAttrs {
    TableName(Ident, LitStr),
    Description(Ident, LitStr),
}

static VALID_MODEL_STRINGS: &[&str] = &[
    "table_name",
    "description",
];

impl Parse for AllowedModelAttrs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        let name_str = name.to_string();

        match name_str.as_str() {
            "table_name" => Ok(TableName(name, parse_eq(input, "table_name = \"my_table_name\"")?)),
            "description" => Ok(Description(name, parse_eq(input, "description = \"Description of the struct\"")?)),
            _ => Err(gen_unknown_key_error(name.span(), &name_str, VALID_MODEL_STRINGS))
        }
    }
}

impl MySpanned for AllowedModelAttrs {
    fn span(&self) -> Span {
        match self {
            TableName(ident, _) => ident.span(),
            Description(ident, _) => ident.span(),
        }
    }
}

// Fields

pub enum AllowedFieldAttrs {
    Default(Ident, LitStr),
}

static VALID_FIELD_STRINGS: &[&str] = &[
    "default",
];

impl Parse for AllowedFieldAttrs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        let name_str = name.to_string();

        match name_str.as_str() {
            "default" => Ok(Default(name, parse_eq(input, "default = \"default_value\"")?)),
            _ => Err(gen_unknown_key_error(name.span(), &name_str, VALID_FIELD_STRINGS))
        }
    }
}

impl MySpanned for AllowedFieldAttrs {
    fn span(&self) -> Span {
        match self {
            Default(ident, _) => ident.span(),
        }
    }
}

// Attributes

pub fn parse_attributes<T>(attrs: &[Attribute]) -> Result<Vec<AttributeWrapper<T>>>
where
    T: Parse + MySpanned,
{
    let mut result = Vec::new();

    for attr in attrs.iter().filter(|attr| {
        attr.meta.path().is_ident("erp")
    }) {
        let map = attr
            .parse_args_with(Punctuated::<T, Comma>::parse_terminated)?
            .into_iter()
            .map(|item| AttributeWrapper {
                item,
                attribute_span: attr.meta.span(),
            });
        result.extend(map);
    }


    Ok(result)
}
