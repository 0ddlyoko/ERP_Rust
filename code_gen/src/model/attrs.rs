use crate::model::util::{gen_unknown_key_error, parse_eq};
use proc_macro2::{Ident, Span};
use syn::parse::{Parse, ParseStream, Result};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::token::{Comma, Eq};
use syn::{Attribute, Lit, LitStr, bracketed};

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
    Id(Ident, LitStr),
    TableName(Ident, LitStr),
    Description(Ident, LitStr),
    DerivedModel(Ident, LitStr),
    /// The field naming a record, when it is not `name`.
    NameField(Ident, LitStr),
    /// How the records come when nobody asks for an order: `date_order desc, id desc`.
    Order(Ident, LitStr),
    /// The many2one to the contact a record is about — an order's customer — whom its messages go
    /// to.
    ContactField(Ident, LitStr),
    /// Says an `#[erp_methods]` block declares overridable methods for this struct.
    Methods(Ident),
}

static VALID_MODEL_STRINGS: &[&str] = &[
    "id",
    "table_name",
    "description",
    "derived_model",
    "name_field",
    "order",
    "contact_field",
    "methods",
];

impl Parse for AllowedModelAttrs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        let name_str = name.to_string();

        match name_str.as_str() {
            "id" => Ok(AllowedModelAttrs::Id(
                name,
                parse_eq(input, "id = \"my_model_id\"")?,
            )),
            "table_name" => Ok(AllowedModelAttrs::TableName(
                name,
                parse_eq(input, "table_name = \"my_table_name\"")?,
            )),
            "description" => Ok(AllowedModelAttrs::Description(
                name,
                parse_eq(input, "description = \"Description of the struct\"")?,
            )),
            "derived_model" => Ok(AllowedModelAttrs::DerivedModel(
                name,
                parse_eq(input, "derived_model = \"base::models::company\"")?,
            )),
            "name_field" => Ok(AllowedModelAttrs::NameField(
                name,
                parse_eq(input, "name_field = \"login\"")?,
            )),
            "order" => Ok(AllowedModelAttrs::Order(
                name,
                parse_eq(input, "order = \"sequence, id\"")?,
            )),
            "contact_field" => Ok(AllowedModelAttrs::ContactField(
                name,
                parse_eq(input, "contact_field = \"partner\"")?,
            )),
            "methods" => Ok(AllowedModelAttrs::Methods(name)),
            _ => Err(gen_unknown_key_error(
                name.span(),
                &name_str,
                VALID_MODEL_STRINGS,
            )),
        }
    }
}

impl MySpanned for AllowedModelAttrs {
    fn span(&self) -> Span {
        match self {
            AllowedModelAttrs::Id(ident, _) => ident.span(),
            AllowedModelAttrs::TableName(ident, _) => ident.span(),
            AllowedModelAttrs::Description(ident, _) => ident.span(),
            AllowedModelAttrs::DerivedModel(ident, _) => ident.span(),
            AllowedModelAttrs::NameField(ident, _) => ident.span(),
            AllowedModelAttrs::Order(ident, _) => ident.span(),
            AllowedModelAttrs::ContactField(ident, _) => ident.span(),
            AllowedModelAttrs::Methods(ident) => ident.span(),
        }
    }
}

// Fields

pub enum AllowedFieldAttrs {
    Default(Ident, Lit),
    /// What the field is shown as: a column's header, a form's label.
    Label(Ident, LitStr),
    /// What the field is for, at more length than its label: help shown beside it.
    Description(Ident, LitStr),
    Compute(Ident, LitStr),
    Depends(Ident, Vec<LitStr>),
    Inverse(Ident, LitStr),
    Relation(Ident, LitStr),
    /// The two columns of the relation table, this side's then the other's: needed when a model
    /// is related to itself, both of them naming it.
    RelationColumns(Ident, LitStr),
    /// Says the field never leaves the process.
    Private(Ident),
    /// Says a computed field is kept in a column rather than worked out on every read.
    Stored(Ident),
    /// Says changes of the field are noted on the record.
    Tracking(Ident),
    /// Says a computed field may also be set by hand: it is worked out when what it depends on
    /// changes, and a value written to it is kept.
    Editable(Ident),
    /// Says the user does not set the field by hand — a state moved by the record's buttons —
    /// though the code may.
    Readonly(Ident),
    /// Says the records of a one2many belong to the record: removed from it, they are deleted.
    Owned(Ident),
    /// Says what a many2one does when the record it points to is deleted.
    OnDelete(Ident, LitStr),
    Domain(Ident, LitStr),
    /// Says a many2one always points to a record.
    Required(Ident),
    /// Says the column is indexed: `index`, or `index = "trigram"` for a text searched anywhere.
    Index(Ident, Option<LitStr>),
}

static VALID_FIELD_STRINGS: &[&str] = &[
    "default",
    "label",
    "description",
    "compute",
    "depends",
    "inverse",
    "relation",
    "relation_columns",
    "private",
    "stored",
    "tracking",
    "editable",
    "readonly",
    "owned",
    "ondelete",
    "domain",
    "required",
    "index",
];

impl Parse for AllowedFieldAttrs {
    fn parse(input: ParseStream) -> Result<Self> {
        let name: Ident = input.parse()?;
        let name_str = name.to_string();

        match name_str.as_str() {
            "default" => Ok(AllowedFieldAttrs::Default(
                name,
                parse_eq(input, "default = \"default_value\"")?,
            )),
            "label" => Ok(AllowedFieldAttrs::Label(
                name,
                parse_eq(input, "label = \"Customer\"")?,
            )),
            "description" => Ok(AllowedFieldAttrs::Description(
                name,
                parse_eq(input, "description = \"The company the order is sent to\"")?,
            )),
            "compute" => Ok(AllowedFieldAttrs::Compute(
                name,
                parse_eq(input, "compute = \"compute_method\"")?,
            )),
            "depends" => {
                input.parse::<Eq>()?;

                let content;
                bracketed!(content in input);
                let dependencies: Punctuated<LitStr, Comma> =
                    content.parse_terminated(<LitStr as Parse>::parse, Comma)?;

                Ok(AllowedFieldAttrs::Depends(
                    name,
                    dependencies.into_iter().collect(),
                ))
            }
            "relation" => Ok(AllowedFieldAttrs::Relation(
                name,
                parse_eq(input, "relation = \"my_relation_table\"")?,
            )),
            "relation_columns" => Ok(AllowedFieldAttrs::RelationColumns(
                name,
                parse_eq(input, "relation_columns = \"task_id,depends_on_id\"")?,
            )),
            "inverse" => Ok(AllowedFieldAttrs::Inverse(
                name,
                parse_eq(input, "inverse = \"inverse\"")?,
            )),
            "private" => Ok(AllowedFieldAttrs::Private(name)),
            "stored" => Ok(AllowedFieldAttrs::Stored(name)),
            "tracking" => Ok(AllowedFieldAttrs::Tracking(name)),
            "editable" => Ok(AllowedFieldAttrs::Editable(name)),
            "readonly" => Ok(AllowedFieldAttrs::Readonly(name)),
            "owned" => Ok(AllowedFieldAttrs::Owned(name)),
            "ondelete" => Ok(AllowedFieldAttrs::OnDelete(
                name,
                parse_eq(input, "ondelete = \"cascade\"")?,
            )),
            "domain" => Ok(AllowedFieldAttrs::Domain(
                name,
                parse_eq(input, r#"domain = "[[\"active\", \"=\", true]]""#)?,
            )),
            "required" => Ok(AllowedFieldAttrs::Required(name)),
            "index" => {
                let kind = if input.peek(Eq) {
                    Some(parse_eq(input, "index = \"trigram\"")?)
                } else {
                    None
                };
                Ok(AllowedFieldAttrs::Index(name, kind))
            }
            _ => Err(gen_unknown_key_error(
                name.span(),
                &name_str,
                VALID_FIELD_STRINGS,
            )),
        }
    }
}

impl MySpanned for AllowedFieldAttrs {
    fn span(&self) -> Span {
        match self {
            AllowedFieldAttrs::Default(ident, _) => ident.span(),
            AllowedFieldAttrs::Label(ident, _) => ident.span(),
            AllowedFieldAttrs::Description(ident, _) => ident.span(),
            AllowedFieldAttrs::Compute(ident, _) => ident.span(),
            AllowedFieldAttrs::Depends(ident, _) => ident.span(),
            AllowedFieldAttrs::Inverse(ident, _) => ident.span(),
            AllowedFieldAttrs::Relation(ident, _) => ident.span(),
            AllowedFieldAttrs::RelationColumns(ident, _) => ident.span(),
            AllowedFieldAttrs::Private(ident) => ident.span(),
            AllowedFieldAttrs::Stored(ident) => ident.span(),
            AllowedFieldAttrs::Tracking(ident) => ident.span(),
            AllowedFieldAttrs::Editable(ident) => ident.span(),
            AllowedFieldAttrs::Readonly(ident) => ident.span(),
            AllowedFieldAttrs::Owned(ident) => ident.span(),
            AllowedFieldAttrs::OnDelete(ident, _) => ident.span(),
            AllowedFieldAttrs::Domain(ident, _) => ident.span(),
            AllowedFieldAttrs::Required(ident) => ident.span(),
            AllowedFieldAttrs::Index(ident, _) => ident.span(),
        }
    }
}

// Attributes

pub fn parse_attributes<T>(attrs: &[Attribute]) -> Result<Vec<AttributeWrapper<T>>>
where
    T: Parse + MySpanned,
{
    let mut result = Vec::new();

    for attr in attrs.iter().filter(|attr| attr.meta.path().is_ident("erp")) {
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
