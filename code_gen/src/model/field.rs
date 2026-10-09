use crate::model::attrs::{AllowedFieldAttrs, parse_attributes};
use crate::model::util::{
    gen_field_no_field_error, gen_inverse_not_multiple_ids, gen_missing_key_error,
    gen_multiple_ids_without_source, gen_option_not_one_generic, gen_password_has_no_default,
    gen_reference_not_two_generic, gen_wrong_default_value,
};
use erp_types::field::{FieldIndex, FieldType, OnDelete};
use proc_macro2::{Ident, Span};
use syn::spanned::Spanned;
use syn::{
    AngleBracketedGenericArguments, Field, GenericArgument, Lit, LitStr, Path, PathArguments,
    PathSegment, Result, Type, TypePath,
};

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
    pub label: Option<String>,
    pub description: Option<String>,
    pub compute: Option<String>,
    pub depends: Option<Vec<String>>,
    pub inverse: Option<String>,
    pub relation: Option<String>,
    /// The relation table's columns, when given rather than named after the two models.
    pub relation_columns: Option<(String, String)>,
    pub is_private: bool,
    /// Whether this struct asked for the field to be kept in a column.
    ///
    /// Only what this struct said. Whether the field *is* kept is settled once every struct
    /// contributing to the model has been seen: being computed belongs to the model's field, not
    /// to whichever struct happens to mention it.
    pub asks_for_storage: bool,
    pub is_tracked: bool,
    pub is_editable: bool,
    pub is_readonly: bool,
    pub is_owned: bool,
    pub on_delete: Option<String>,
    pub domain: Option<String>,
    pub index: Option<String>,
}

impl FieldGen {
    pub fn from_item(item: &Field) -> Result<Self> {
        let Field {
            ident, attrs, ty, ..
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
        let mut default_span = None;
        let mut label = None;
        let mut description = None;
        let mut compute = None;
        let mut depends = None;
        let mut inverse = None;
        let mut relation = None;
        let mut relation_columns = None;
        let mut is_private = false;
        let mut is_tracked = false;
        let mut editable = None;
        let mut is_readonly = false;
        let mut owned = None;
        let mut stored = None;
        let mut on_delete = None;
        let mut domain = None;
        let mut required = None;
        let mut index = None;

        for attr in parse_attributes(attrs)? {
            match attr.item {
                AllowedFieldAttrs::Default(ident, default_value) => {
                    default_span = Some(ident.span());
                    default = Some(match default_value {
                        Lit::Str(str) if str.value().is_empty() => {
                            return Err(syn::Error::new(
                                str.span(),
                                "an empty default is no default: a text that may be empty is an \
                                 `Option<String>`",
                            ));
                        }
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
                                    return Err(gen_wrong_default_value(
                                        i.span(),
                                        i.base10_digits(),
                                        field_name.as_str(),
                                    ));
                                }
                            }
                        }
                        Lit::Float(f) => {
                            match rust_decimal::Decimal::from_str_exact(f.base10_digits()) {
                                Ok(decimal) => FieldType::Decimal(decimal),
                                Err(_) => {
                                    return Err(gen_wrong_default_value(
                                        f.span(),
                                        f.base10_digits(),
                                        field_name.as_str(),
                                    ));
                                }
                            }
                        }
                        Lit::Bool(b) => {
                            if b.value {
                                FieldType::Bool(true)
                            } else {
                                FieldType::Bool(false)
                            }
                        }
                        Lit::ByteStr(bs) => {
                            return Err(gen_wrong_default_value(
                                bs.span(),
                                &String::from_utf8_lossy(&bs.value()),
                                field_name.as_str(),
                            ));
                        }
                        Lit::CStr(cs) => {
                            return Err(gen_wrong_default_value(
                                cs.span(),
                                &cs.value().to_string_lossy(),
                                field_name.as_str(),
                            ));
                        }
                        Lit::Byte(b) => {
                            return Err(gen_wrong_default_value(
                                b.span(),
                                &b.value().to_string(),
                                field_name.as_str(),
                            ));
                        }
                        Lit::Char(c) => {
                            return Err(gen_wrong_default_value(
                                c.span(),
                                &c.value().to_string(),
                                field_name.as_str(),
                            ));
                        }
                        Lit::Verbatim(v) => {
                            return Err(gen_wrong_default_value(
                                v.span(),
                                &v.to_string(),
                                field_name.as_str(),
                            ));
                        }
                        _ => return Err(gen_wrong_default_value(ident.span(), "???", "name")),
                    });
                    // TODO Add Enum default value
                    // default_value = Some(default.value().into());
                }
                AllowedFieldAttrs::Label(_, value) => {
                    label = Some(value.value());
                }
                AllowedFieldAttrs::Description(_, description_value) => {
                    description = Some(description_value.value());
                }
                AllowedFieldAttrs::Compute(_, compute_value) => {
                    compute = Some(compute_value.value());
                }
                AllowedFieldAttrs::Depends(_, depends_value) => {
                    depends = Some(depends_value.iter().map(|s| s.value()).collect());
                }
                AllowedFieldAttrs::Relation(ident, relation_value) => {
                    relation = Some((ident, relation_value.value()));
                }
                AllowedFieldAttrs::RelationColumns(_, value) => {
                    let text = value.value();
                    let Some((own, other)) = text.split_once(',') else {
                        return Err(syn::Error::new(
                            value.span(),
                            "relation_columns names two columns: \"own_id,other_id\"",
                        ));
                    };
                    relation_columns = Some((own.trim().to_string(), other.trim().to_string()));
                }
                AllowedFieldAttrs::Private(_) => {
                    is_private = true;
                }
                AllowedFieldAttrs::Tracking(_) => {
                    is_tracked = true;
                }
                AllowedFieldAttrs::Editable(ident) => {
                    editable = Some(ident);
                }
                AllowedFieldAttrs::Readonly(_) => {
                    is_readonly = true;
                }
                AllowedFieldAttrs::Owned(ident) => {
                    owned = Some(ident);
                }
                AllowedFieldAttrs::OnDelete(ident, value) => {
                    let key = value.value();
                    if OnDelete::from_key(&key).is_none() {
                        return Err(syn::Error::new(
                            value.span(),
                            format!(
                                "\"{key}\" is not what a many2one can do on delete: {}",
                                OnDelete::KEYS.join(", ")
                            ),
                        ));
                    }
                    on_delete = Some((ident, key));
                }
                AllowedFieldAttrs::Index(_, kind) => {
                    let key = kind.as_ref().map_or("btree".to_string(), LitStr::value);
                    if FieldIndex::from_key(&key).is_none() {
                        return Err(syn::Error::new(
                            kind.as_ref().map_or(Span::call_site(), LitStr::span),
                            format!(
                                "\"{key}\" is not a kind of index: {}",
                                FieldIndex::KEYS.join(", ")
                            ),
                        ));
                    }
                    index = Some(key);
                }
                AllowedFieldAttrs::Domain(ident, value) => {
                    // A JSON domain, or an expression of the record giving one, checked against
                    // the model's fields once every plugin has loaded.
                    let text = value.value();
                    if text.trim().is_empty() {
                        return Err(syn::Error::new(value.span(), "an empty domain"));
                    }
                    domain = Some((ident, text));
                }
                AllowedFieldAttrs::Required(ident) => {
                    required = Some(ident);
                }
                AllowedFieldAttrs::Stored(ident) => {
                    stored = Some(ident);
                }
                AllowedFieldAttrs::Inverse(ident, inverse_value) => {
                    inverse = Some((ident, inverse_value.value()));
                }
            }
        }

        let mut field_type = None;
        // Check field type
        if let Type::Path(TypePath {
            qself: _,
            path: Path {
                leading_colon: _,
                segments,
            },
        }) = ty
        {
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
                    args, ..
                }) = arguments
                {
                    // The <String> in "email: Option<String>"
                    let args_len = args.len();
                    // Check argument length
                    if args_len != 1 && !is_reference {
                        return Err(gen_option_not_one_generic(args.span()));
                    }
                    if args_len != 2 && is_reference {
                        return Err(gen_reference_not_two_generic(args.span()));
                    }
                    if let GenericArgument::Type(Type::Path(TypePath {
                        qself: _,
                        path:
                            Path {
                                leading_colon: _,
                                segments,
                            },
                    })) = &args[0]
                    {
                        if segments.len() != 1 {
                            return Err(gen_field_no_field_error(segments.span()));
                        }
                        field_type = Some(segments[0].ident.clone());
                    }
                    if is_reference
                        && let GenericArgument::Type(Type::Path(TypePath {
                            qself: _,
                            path:
                                Path {
                                    leading_colon: _,
                                    segments,
                                },
                        })) = &args[1]
                    {
                        if segments.len() != 1 {
                            return Err(gen_field_no_field_error(segments.span()));
                        }
                        is_reference_multi = segments[0].ident == "MultipleIds";
                    }
                }
            } else {
                is_required = true;

                field_type = Some(ident.clone());
            }
        }

        // A password is hashed where it is set, never declared.
        if let Some(field_type) = &field_type
            && field_type == "Password"
            && let Some(default_span) = default_span
        {
            return Err(gen_password_has_no_default(default_span));
        }

        // "inverse" should only work on MultipleIds
        if !is_reference_multi && let Some((inverse_ident, _)) = inverse {
            return Err(gen_inverse_not_multiple_ids(inverse_ident.span()));
        }
        // So should "relation": a many2many is a list on both sides.
        if !is_reference_multi && let Some((relation_ident, _)) = relation {
            return Err(gen_inverse_not_multiple_ids(relation_ident.span()));
        }
        // Only the records a one2many holds can belong to the record holding them.
        if let Some(owned) = &owned
            && inverse.is_none()
        {
            return Err(syn::Error::new(
                owned.span(),
                "only a one2many — a list with an inverse — owns its records",
            ));
        }
        if let Some((ident, _)) = &on_delete
            && (!is_reference || is_reference_multi)
        {
            return Err(syn::Error::new(
                ident.span(),
                "only a many2one — a `Reference<_, SingleId>` — takes this",
            ));
        }
        if let Some((ident, _)) = &domain
            && !is_reference
        {
            return Err(syn::Error::new(
                ident.span(),
                "only a relation — a `Reference` — takes this",
            ));
        }
        if let Some(ident) = &required
            && !is_reference
        {
            return Err(syn::Error::new(
                ident.span(),
                "this field is required by not being an `Option`",
            ));
        }
        if let Some(ident) = &editable
            && (compute.is_none() || stored.is_none())
        {
            return Err(syn::Error::new(
                ident.span(),
                "only a stored computed field can also be set by hand",
            ));
        }
        // A list of references is filled by one of three things; with none it holds nothing.
        if is_reference_multi && inverse.is_none() && relation.is_none() && compute.is_none() {
            return Err(gen_multiple_ids_without_source(ident.span()));
        }

        Ok(FieldGen {
            field_name,
            field_span: item.span(),
            field_name_span: ident.span(),
            is_required: is_required || required.is_some(),
            is_reference,
            is_reference_multi,
            field_type_keyword: field_type.unwrap(),
            default,
            label,
            description,
            compute,
            depends,
            inverse: inverse.map(|inv| inv.1),
            relation: relation.map(|rel| rel.1),
            relation_columns,
            is_private,
            asks_for_storage: stored.is_some(),
            is_tracked,
            is_editable: editable.is_some(),
            is_readonly,
            is_owned: owned.is_some(),
            on_delete: on_delete.map(|(_, key)| key),
            domain: domain.map(|(_, text)| text),
            index,
        })
    }
}
