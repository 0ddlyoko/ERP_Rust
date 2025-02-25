use crate::util::{gen_unknown_key_error, parse_eq};
use proc_macro2::{Ident, Span};
use syn::parse::{Parse, ParseStream, Result};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::token::{Comma, Eq};
use syn::{bracketed, Attribute, Lit, LitStr};

#[allow(dead_code)]
pub trait MySpanned {
    fn span(&self) -> Span;
}

#[allow(dead_code)]
pub struct AttributeWrapper<T> {
    pub item: T,
    pub attribute_span: Span,
}

// Models

pub enum AllowedModelAttrs {
    TableName(Ident, LitStr),
    Description(Ident, LitStr),
    DerivedModel(Ident, LitStr),
}

static VALID_MODEL_STRINGS: &[&str] = &[
    "table_name",
    "description",
    "derived_model",
];

impl Parse for AllowedModelAttrs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        let name_str = name.to_string();

        match name_str.as_str() {
            "table_name" => Ok(AllowedModelAttrs::TableName(name, parse_eq(input, "table_name = \"my_table_name\"")?)),
            "description" => Ok(AllowedModelAttrs::Description(name, parse_eq(input, "description = \"Description of the struct\"")?)),
            "derived_model" => Ok(AllowedModelAttrs::DerivedModel(name, parse_eq(input, "derived_model = \"base::models::company\"")?)),
            _ => Err(gen_unknown_key_error(name.span(), &name_str, VALID_MODEL_STRINGS))
        }
    }
}

impl MySpanned for AllowedModelAttrs {
    fn span(&self) -> Span {
        match self {
            AllowedModelAttrs::TableName(ident, _) => ident.span(),
            AllowedModelAttrs::Description(ident, _) => ident.span(),
            AllowedModelAttrs::DerivedModel(ident, _) => ident.span(),
        }
    }
}

// Fields

pub enum AllowedFieldAttrs {
    Default(Ident, Lit),
    Description(Ident, LitStr),
    Compute(Ident, LitStr),
    Depends(Ident, Vec<LitStr>),
    Inverse(Ident, LitStr),
}

static VALID_FIELD_STRINGS: &[&str] = &[
    "default",
    "description",
    "compute",
    "depends",
    "inverse",
];

impl Parse for AllowedFieldAttrs {
    fn parse(input: ParseStream) -> Result<Self> {
        let name: Ident = input.parse()?;
        let name_str = name.to_string();

        match name_str.as_str() {
            "default" => Ok(AllowedFieldAttrs::Default(name, parse_eq(input, "default = \"default_value\"")?)),
            "description" => Ok(AllowedFieldAttrs::Description(name, parse_eq(input, "description = \"Description of the field\"")?)),
            "compute" => Ok(AllowedFieldAttrs::Compute(name, parse_eq(input, "compute = \"compute_method\"")?)),
            "depends" => {
                input.parse::<Eq>()?;

                let content;
                bracketed!(content in input);
                let dependencies: Punctuated<LitStr, Comma> = content.parse_terminated(<LitStr as Parse>::parse, Comma)?;
                
                Ok(AllowedFieldAttrs::Depends(name, dependencies.into_iter().collect()))
            },
            "inverse" => Ok(AllowedFieldAttrs::Inverse(name, parse_eq(input, "inverse = \"inverse\"")?)),
            _ => Err(gen_unknown_key_error(name.span(), &name_str, VALID_FIELD_STRINGS))
        }
    }
}

impl MySpanned for AllowedFieldAttrs {
    fn span(&self) -> Span {
        match self {
            AllowedFieldAttrs::Default(ident, _) => ident.span(),
            AllowedFieldAttrs::Description(ident, _) => ident.span(),
            AllowedFieldAttrs::Compute(ident, _) => ident.span(),
            AllowedFieldAttrs::Depends(ident, _) => ident.span(),
            AllowedFieldAttrs::Inverse(ident, _) => ident.span(),
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
