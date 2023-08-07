use proc_macro2::Span;
use syn::Field as SynField;
use syn::Result;
use syn::spanned::Spanned;
use crate::attrs::{AllowedFieldAttr, parse_attributes};
use crate::util::generate_field_no_name_error;


pub struct Field {
    field_name: String,
    field_span: Span,
    field_name_span: Span,
    is_required: bool,
}

impl Field {
    pub fn from_item(item: &SynField) -> Result<Self> {
        let SynField {
            ident,  attrs, ..
        } = item;

        let ident = match ident {
            Some(name) => name,
            None => return Err(generate_field_no_name_error(item.span())),
        };

        let field_name = ident.to_string();
        let mut is_required = false;

        for attr in parse_attributes(attrs)? {
            match attr.item {
                AllowedFieldAttr::Required(_) => is_required = true,
            }
        }

        Ok(Field {
            field_name: field_name,
            field_span: item.span(),
            field_name_span: ident.span(),
            is_required: is_required,
        })
    }

    pub fn name(&self) -> &String {
        &self.field_name
    }

    pub fn span(&self) -> Span {
        self.field_span
    }

    pub fn field_span(&self) -> Span {
        self.field_name_span
    }

    pub fn required(&self) -> bool {
        self.is_required
    }
}
