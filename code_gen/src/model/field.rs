use crate::model::attrs::{parse_attributes, AllowedFieldAttrs};
use crate::model::util::{gen_field_no_field_error, gen_inverse_not_multiple_ids, gen_missing_key_error, gen_option_not_one_generic, gen_reference_not_two_generic, gen_wrong_default_value};
use proc_macro2::{Ident, Span};
use syn::spanned::Spanned;
use syn::{AngleBracketedGenericArguments, Field, GenericArgument, Lit, Path, PathArguments, PathSegment, Result, Type, TypePath};
use erp::field::FieldType;

#[allow(dead_code)]
pub struct FieldGen {
    pub field_name: String,
    pub field_span: Span,
    pub field_name_span: Span,
    pub is_required: bool,
    pub is_reference: bool,
    pub is_reference_multi: bool,
    pub field_type_keyword: Ident,
    pub default: Option<FieldType>,
    pub description: Option<String>,
    pub compute: Option<String>,
    pub depends: Option<Vec<String>>,
    pub inverse: Option<String>,
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
        let mut is_reference_multi = false;
        let mut default = None;
        let mut description = None;
        let mut compute = None;
        let mut depends = None;
        let mut inverse = None;

        for attr in parse_attributes(attrs)? {
            match attr.item {
                AllowedFieldAttrs::Default(ident, default_value) => {
                    default = Some(match default_value {
                        Lit::Str(str) => FieldType::String(str.value()),
                        Lit::Int(i) => {
                            let int = i.base10_parse::<i32>();
                            if int.is_ok() {
                                FieldType::Integer(int?)
                            } else {
                                let int = i.base10_parse::<u32>();
                                if int.is_ok() {
                                    FieldType::Ref(int?)
                                } else {
                                    return Err(gen_wrong_default_value(i.span(), i.base10_digits(), field_name.as_str()))
                                }
                            }
                        },
                        Lit::Float(f) => {
                            let float = f.base10_parse::<f64>();
                            if float.is_ok() {
                                FieldType::Float(float?)
                            } else {
                                return Err(gen_wrong_default_value(f.span(), f.base10_digits(), field_name.as_str()))
                            }
                        }
                        Lit::Bool(b) => {
                            if b.value {
                                FieldType::Bool(true)
                            } else {
                                FieldType::Bool(false)
                            }
                        }
                        // TODO Remove the 2 unwrap
                        Lit::ByteStr(bs) => return Err(gen_wrong_default_value(bs.span(), &String::from_utf8(bs.value()).unwrap(), field_name.as_str())),
                        Lit::CStr(cs) => return Err(gen_wrong_default_value(cs.span(), &cs.value().into_string().unwrap(), field_name.as_str())),
                        Lit::Byte(b) => return Err(gen_wrong_default_value(b.span(), &b.value().to_string(), field_name.as_str())),
                        Lit::Char(c) => return Err(gen_wrong_default_value(c.span(), &c.value().to_string(), field_name.as_str())),
                        Lit::Verbatim(v) => return Err(gen_wrong_default_value(v.span(), &v.to_string(), field_name.as_str())),
                        _ => return Err(gen_wrong_default_value(ident.span(), "???", "name")),
                    });
                    // TODO Add Enum default value
                    // default_value = Some(default.value().into());
                },
                AllowedFieldAttrs::Description(_, description_value) => {
                    description = Some(description_value.value());
                }
                AllowedFieldAttrs::Compute(_, compute_value) => {
                    compute = Some(compute_value.value());
                }
                AllowedFieldAttrs::Depends(_, depends_value) => {
                    depends = Some(depends_value.iter().map(|s| s.value()).collect());
                }
                AllowedFieldAttrs::Inverse(ident, inverse_value) => {
                    inverse = Some((ident, inverse_value.value()));
                }
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
                    let args_len = args.len();
                    // Check argument length
                    if args_len != 1 && !is_reference {
                        return Err(gen_option_not_one_generic(args.span()))
                    }
                    if args_len != 2 && is_reference {
                        return Err(gen_reference_not_two_generic(args.span()));
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
                    if is_reference {
                        if let GenericArgument::Type(Type::Path(TypePath {
                                                                    qself: _,
                                                                    path: Path {
                                                                        leading_colon: _,
                                                                        segments,
                                                                    },
                                                                })) = &args[1] {
                            if segments.len() != 1 {
                                return Err(gen_field_no_field_error(segments.span()))
                            }
                            is_reference_multi = segments[0].ident == "MultipleIds";
                        }
                    }
                }
            } else {
                is_required = true;

                field_type = Some(ident.clone());
            }
        }

        // "inverse" should only work on MultipleIds
        if !is_reference_multi {
            if let Some((inverse_ident, _)) = inverse {
                return Err(gen_inverse_not_multiple_ids(inverse_ident.span()));
            }
        }

        Ok(FieldGen {
            field_name,
            field_span: item.span(),
            field_name_span: ident.span(),
            is_required,
            is_reference,
            is_reference_multi,
            field_type_keyword: field_type.unwrap(),
            default,
            description,
            compute,
            depends,
            inverse: inverse.map(|inv| inv.1),
        })
    }
}
