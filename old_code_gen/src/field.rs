use proc_macro2::Span;
use syn::{Field as SynField, GenericArgument, PathArguments, PathSegment, Type};
use syn::Result;
use syn::spanned::Spanned;
use test_lib::FieldType;
use crate::attrs::{AllowedFieldAttr, parse_attributes};
use crate::util::{generate_field_no_field_error, generate_field_no_name_error};


pub struct FieldGen {
    field_name: String,
    field_span: Span,
    field_name_span: Span,
    is_required: Option<bool>,
    default_value: FieldType,
}

impl FieldGen {
    pub fn from_item(item: &SynField) -> Result<Self> {
        let SynField {
            ident,  attrs, ty, ..
        } = item;

        let ident = match ident {
            Some(name) => name,
            None => return Err(generate_field_no_name_error(item.span())),
        };
        let field_name = ident.to_string();

        let mut is_required = None;
        let mut default_value = None;

        for attr in parse_attributes(attrs)? {
            match attr.item {
                AllowedFieldAttr::Required(_) => is_required = Some(true),
                AllowedFieldAttr::Default(_, default) => {
                    default_value = Some(default.value())
                },
            }
        }


        let mut field_type = FieldType::String(test_lib::Field::new(None));
        // Check field type
        if field_name != "id" && field_name != "_env" {
            if let Type::Path(type_path) = ty {
                let segments = &type_path.path.segments;
                if segments.len() != 1 {
                    return Err(generate_field_no_field_error(ident.span()));
                }
                let PathSegment { ident, arguments } = &segments[0];
                if ident != "Field" {
                    return Err(generate_field_no_field_error(ident.span()));
                }
                if let PathArguments::AngleBracketed(angle_bracketed_generic_arguments) = arguments {
                    let args = &angle_bracketed_generic_arguments.args;
                    if args.len() != 1 {
                        return Err(generate_field_no_field_error(args.span()));
                    }
                    if let GenericArgument::Type(generic_type) = &args[0] {
                        if let Type::Path(type_path) = generic_type {
                            let segments = &type_path.path.segments;
                            if segments.len() != 1 {
                                return Err(generate_field_no_field_error(segments.span()))
                            }
                            let field_type_keyword = &segments[0].ident;
                            field_type = FieldType::from(field_type_keyword.to_string().as_str(), default_value);
                        } else {
                            return Err(generate_field_no_field_error(generic_type.span()))
                        }
                    } else {
                        return Err(generate_field_no_field_error(args[0].span()))
                    }
                } else {
                    return Err(generate_field_no_field_error(arguments.span()))
                }
            } else {
                return Err(generate_field_no_field_error(ty.span()))
            }
        }

        Ok(FieldGen {
            field_name: field_name,
            field_span: item.span(),
            field_name_span: ident.span(),
            is_required: is_required,
            default_value: field_type,
        })
    }

    pub fn name(&self) -> &String {
        &self.field_name
    }

    pub fn span(&self) -> &Span {
        &self.field_span
    }

    pub fn field_span(&self) -> &Span {
        &self.field_name_span
    }

    pub fn required(&self) -> Option<bool> {
        self.is_required
    }

    pub fn default_value(&self) -> &FieldType {
        &self.default_value
    }
}
