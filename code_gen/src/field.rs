use crate::attrs::{parse_attributes, AllowedFieldAttrs};
use crate::util::{gen_field_no_field_error, gen_missing_key_error};
use proc_macro2::{Ident, Span};
use syn::spanned::Spanned;
use syn::{AngleBracketedGenericArguments, Field, GenericArgument, Path, PathArguments, PathSegment, Result, Type, TypePath};

pub struct FieldGen {
    pub field_name: String,
    pub field_span: Span,
    pub field_name_span: Span,
    pub is_required: bool,
    pub is_reference: bool,
    pub field_type_keyword: Ident,
    // TODO Handle default_value
    // default_value: FieldType,
}

impl FieldGen {
    pub fn from_item(item: &Field) -> Result<Self> {
        let Field {
            ident,
            attrs,
            ty,
            ..
        } = item;

        let ident = match ident {
            Some(name) => name,
            None => return Err(gen_missing_key_error(item.span(), "name")),
        };
        let field_name = ident.to_string();

        let mut is_required = false;
        let mut is_reference = false;
        let mut default_value = None;

        for attr in parse_attributes(attrs)? {
            match attr.item {
                AllowedFieldAttrs::Default(_, default) => default_value = Some(default.value()),
            }
        }

        let mut field_type = None;
        // Check field type
        if let Type::Path(
            TypePath {
                qself: _,
                path: Path {
                    leading_colon: _,
                    segments,
                },
            }) = ty {
            if segments.len() != 1 {
                return Err(gen_field_no_field_error(ident.span()));
            }
            let PathSegment { ident, arguments } = &segments[0];
            // PathSegment = the value after ":" in "email: Option<String>".
            // ident = "Option", arguments = "<String>"
            if ident == "Option" || ident == "Reference" {
                if ident == "Reference" {
                    is_reference = true;
                }
                // Go deeper
                if let PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                                                         args,
                                                         ..
                                                     }) = arguments {
                    // The <String> in "email: Option<String>"
                    if args.len() != 1 {
                        return Err(gen_field_no_field_error(args.span()));
                    }
                    if let GenericArgument::Type(Type::Path(TypePath {
                                                                qself: _,
                                                                path: Path {
                                                                    leading_colon: _,
                                                                    segments,
                                                                },
                                                            })) = &args[0] {
                        if segments.len() != 1 {
                            return Err(gen_field_no_field_error(segments.span()))
                        }
                        field_type = Some(segments[0].ident.clone());
                    }
                }
            } else {
                is_required = true;

                field_type = Some(ident.clone());
            }
        }

        Ok(FieldGen {
            field_name,
            field_span: item.span(),
            field_name_span: ident.span(),
            is_required,
            is_reference,
            field_type_keyword: field_type.unwrap(),
        })
    }
}
