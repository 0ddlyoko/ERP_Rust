use proc_macro2::{Ident, Span};
use syn::{Attribute, LitStr};
use syn::parse::{Parse, ParseStream, Result};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::token::Comma;
use crate::attrs::AllowedFieldAttr::{Default, Required};
use crate::attrs::AllowedModelAttr::TableName;
use crate::util::{parse_eq, generate_unknown_key_error};

pub trait MySpanned {
    fn span(&self) -> Span;
}

pub struct AttributeWrapper<T> {
    pub item: T,
    pub attribute_span: Span,
}

// Models

pub enum AllowedModelAttr {
    TableName(Ident, LitStr),
}

static VALID_MODEL_STRINGS: &'static [&str] = &["table_name"];

impl Parse for AllowedModelAttr {
    fn parse(input: ParseStream) -> Result<Self> {
        let name: Ident = input.parse()?;
        let name_str = name.to_string();

        match name_str.as_str() {
            "table_name" => Ok(TableName(name, parse_eq(input, "table_name = \"my_table_name\"")?)),
            _ => {
                Err(generate_unknown_key_error(name.span(), &name_str, VALID_MODEL_STRINGS))
            },
        }
    }
}

impl MySpanned for AllowedModelAttr {
    fn span(&self) -> Span {
        match self {
            TableName(ident, _) => ident.span(),
        }
    }
}

// Fields

pub enum AllowedFieldAttr {
    Required(Ident),
    Default(Ident, LitStr),
}

static VALID_FIELD_STRINGS: &'static [&str] = &["required", "default"];

impl Parse for AllowedFieldAttr {
    fn parse(input: ParseStream) -> Result<Self> {
        let name: Ident = input.parse()?;
        let name_str = name.to_string();

        match name_str.as_str() {
            "required" => Ok(Required(name)),
            "default" => Ok(Default(name, parse_eq(input, "default = \"default_value\"")?)),
            _ => {
                Err(generate_unknown_key_error(name.span(), &name_str, VALID_FIELD_STRINGS))
            },
        }
    }
}

impl MySpanned for AllowedFieldAttr {
    fn span(&self) -> Span {
        match self {
            Required(ident) => ident.span(),
            Default(ident, _) => ident.span(),
        }
    }
}

// Attributes

pub fn parse_attributes<T>(attrs: &[Attribute]) -> Result<Vec<AttributeWrapper<T>>>
    where
    T: Parse + MySpanned {
    let mut result = Vec::new();

    for attr in attrs.iter().filter(|attr| {
        attr.meta.path().is_ident("odd")
    }) {
        let map = attr
            .parse_args_with(Punctuated::<T, Comma>::parse_terminated)?
            .into_iter()
            .map(|a| AttributeWrapper {
                item: a,
                attribute_span: attr.meta.span(),
            });
        result.extend(map);
    }

    Ok(result)
}
