use crate::field::Password;
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use std::fmt::{Debug, Display, Formatter};

#[macro_export]
macro_rules! field_type_make_eq {
    ( $self:expr, $other:expr, $( $path:path ),* ) => {
        match $self {
            $($path(self_value) => {
                if let $path(other_value) = $other {
                    self_value == other_value
                } else {
                    false
                }
            })*
        }
    };
}

#[derive(Debug, Clone)]
pub enum FieldType {
    String(String),
    Integer(i32),
    /// Fixed-point decimal. Money and any other exact quantity belongs here, never in a float.
    Decimal(Decimal),
    Bool(bool),
    Date(NaiveDate),
    DateTime(DateTime<Utc>),
    Ref(u32),
    Refs(Vec<u32>),
    /// A secret, held as its hash. See [`Password`].
    Password(Password),
}

impl Display for FieldType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldType::String(s) => write!(f, "{s}"),
            FieldType::Integer(i) => write!(f, "{i}"),
            FieldType::Decimal(d) => write!(f, "{d}"),
            FieldType::Bool(b) => write!(f, "{b}"),
            FieldType::Date(d) => write!(f, "{d}"),
            FieldType::DateTime(dt) => write!(f, "{dt}"),
            FieldType::Ref(id) => write!(f, "{id}"),
            FieldType::Refs(ids) => write!(f, "{ids:?}"),
            FieldType::Password(password) => write!(f, "{password}"),
        }
    }
}

impl PartialEq for FieldType {
    fn eq(&self, other: &Self) -> bool {
        field_type_make_eq!(
            self,
            other,
            FieldType::String,
            FieldType::Integer,
            FieldType::Decimal,
            FieldType::Bool,
            FieldType::Date,
            FieldType::DateTime,
            FieldType::Ref,
            FieldType::Refs,
            FieldType::Password
        )
    }
}

// String
impl<'a> From<&'a FieldType> for Option<&'a String> {
    fn from(t: &'a FieldType) -> Self {
        match t {
            FieldType::String(s) => Some(s),
            _ => None,
        }
    }
}

impl From<&String> for FieldType {
    fn from(t: &String) -> Self {
        FieldType::String(t.clone())
    }
}

impl From<String> for FieldType {
    fn from(t: String) -> Self {
        FieldType::String(t)
    }
}

impl From<&str> for FieldType {
    fn from(t: &str) -> Self {
        FieldType::String(t.to_string())
    }
}

// i32
impl<'a> From<&'a FieldType> for Option<&'a i32> {
    fn from(t: &'a FieldType) -> Self {
        match t {
            FieldType::Integer(i) => Some(i),
            _ => None,
        }
    }
}

impl From<i32> for FieldType {
    fn from(t: i32) -> Self {
        FieldType::Integer(t)
    }
}

impl From<&i32> for FieldType {
    fn from(t: &i32) -> Self {
        FieldType::Integer(*t)
    }
}

// Decimal
impl<'a> From<&'a FieldType> for Option<&'a Decimal> {
    fn from(t: &'a FieldType) -> Self {
        match t {
            FieldType::Decimal(d) => Some(d),
            _ => None,
        }
    }
}

impl From<Decimal> for FieldType {
    fn from(t: Decimal) -> Self {
        FieldType::Decimal(t)
    }
}

impl From<&Decimal> for FieldType {
    fn from(t: &Decimal) -> Self {
        FieldType::Decimal(*t)
    }
}

// NaiveDate
impl<'a> From<&'a FieldType> for Option<&'a NaiveDate> {
    fn from(t: &'a FieldType) -> Self {
        match t {
            FieldType::Date(d) => Some(d),
            _ => None,
        }
    }
}

impl From<NaiveDate> for FieldType {
    fn from(t: NaiveDate) -> Self {
        FieldType::Date(t)
    }
}

impl From<&NaiveDate> for FieldType {
    fn from(t: &NaiveDate) -> Self {
        FieldType::Date(*t)
    }
}

// DateTime<Utc>
impl<'a> From<&'a FieldType> for Option<&'a DateTime<Utc>> {
    fn from(t: &'a FieldType) -> Self {
        match t {
            FieldType::DateTime(dt) => Some(dt),
            _ => None,
        }
    }
}

impl From<DateTime<Utc>> for FieldType {
    fn from(t: DateTime<Utc>) -> Self {
        FieldType::DateTime(t)
    }
}

impl From<&DateTime<Utc>> for FieldType {
    fn from(t: &DateTime<Utc>) -> Self {
        FieldType::DateTime(*t)
    }
}

// bool
impl<'a> From<&'a FieldType> for Option<&'a bool> {
    fn from(t: &'a FieldType) -> Self {
        match t {
            FieldType::Bool(b) => Some(b),
            _ => None,
        }
    }
}

impl From<bool> for FieldType {
    fn from(t: bool) -> Self {
        FieldType::Bool(t)
    }
}

impl From<&bool> for FieldType {
    fn from(t: &bool) -> Self {
        FieldType::Bool(*t)
    }
}

// Enums
pub trait EnumType: Debug + PartialEq + Eq + Copy + Clone {}

impl<'a, E> From<&'a FieldType> for Option<&'a E>
where
    E: EnumType,
    &'a str: Into<&'a E>,
{
    fn from(t: &'a FieldType) -> Self {
        match t {
            FieldType::String(s) => Some(s.as_str().into()),
            _ => None,
        }
    }
}

impl<'a, E> From<E> for FieldType
where
    E: EnumType + Into<&'a str>,
{
    fn from(t: E) -> Self {
        let result: &str = t.into();
        FieldType::String(result.to_string())
    }
}

// Ref
impl<'a> From<&'a FieldType> for Option<&'a u32> {
    fn from(t: &'a FieldType) -> Self {
        match t {
            FieldType::Ref(r) => Some(r),
            _ => None,
        }
    }
}

impl From<u32> for FieldType {
    fn from(t: u32) -> Self {
        FieldType::Ref(t)
    }
}

impl From<&u32> for FieldType {
    fn from(t: &u32) -> Self {
        FieldType::Ref(*t)
    }
}

// Refs
impl<'a> From<&'a FieldType> for Option<&'a Vec<u32>> {
    fn from(t: &'a FieldType) -> Self {
        match t {
            FieldType::Refs(vec) => Some(vec),
            _ => None,
        }
    }
}

impl From<Vec<u32>> for FieldType {
    fn from(t: Vec<u32>) -> Self {
        FieldType::Refs(t)
    }
}

impl From<&Vec<u32>> for FieldType {
    fn from(t: &Vec<u32>) -> Self {
        FieldType::Refs(t.clone())
    }
}

// Password
impl<'a> From<&'a FieldType> for Option<&'a Password> {
    fn from(t: &'a FieldType) -> Self {
        match t {
            FieldType::Password(password) => Some(password),
            _ => None,
        }
    }
}

impl From<Password> for FieldType {
    fn from(t: Password) -> Self {
        FieldType::Password(t)
    }
}

impl From<&Password> for FieldType {
    fn from(t: &Password) -> Self {
        FieldType::Password(t.clone())
    }
}

/// Raised when text cannot be read as the field type it is destined for.
#[derive(Debug, Clone, thiserror::Error)]
#[error("Cannot read {raw:?} as a {expected}")]
pub struct ParseFieldTypeError {
    pub raw: String,
    pub expected: FieldKind,
}

impl FieldType {
    /// Read `raw` as a value of the same kind as `self`.
    ///
    /// Serialization writes bare values, so nothing in `"4"` says whether it is an integer or a
    /// reference; the field's declared kind carries that answer.
    pub fn parse_like(&self, raw: &str) -> Result<FieldType, ParseFieldTypeError> {
        self.kind().parse(raw)
    }
}

impl FieldKind {
    /// Read `raw` as a value of this kind.
    pub fn parse(&self, raw: &str) -> Result<FieldType, ParseFieldTypeError> {
        let expected = *self;
        let fail = || ParseFieldTypeError {
            raw: raw.to_string(),
            expected,
        };
        Ok(match self {
            FieldKind::String => FieldType::String(raw.to_string()),
            FieldKind::Integer => FieldType::Integer(raw.parse().map_err(|_| fail())?),
            FieldKind::Decimal => {
                FieldType::Decimal(Decimal::from_str_exact(raw).map_err(|_| fail())?)
            }
            FieldKind::Bool => match raw {
                "true" | "True" | "1" => FieldType::Bool(true),
                "false" | "False" | "0" => FieldType::Bool(false),
                _ => return Err(fail()),
            },
            FieldKind::Date => FieldType::Date(raw.parse().map_err(|_| fail())?),
            FieldKind::DateTime => {
                FieldType::DateTime(raw.parse::<DateTime<Utc>>().map_err(|_| fail())?)
            }
            FieldKind::Ref => FieldType::Ref(raw.parse().map_err(|_| fail())?),
            FieldKind::Refs => {
                let ids = raw
                    .split(',')
                    .map(str::trim)
                    .filter(|part| !part.is_empty())
                    .map(|part| part.parse::<u32>().map_err(|_| fail()))
                    .collect::<Result<Vec<u32>, _>>()?;
                FieldType::Refs(ids)
            }
            // The text is the clear password, hashed on the spot: a data file or a migration
            // names a password the way a person would, and never holds a hash it could not have
            // produced anyway, since the salt is drawn per account.
            FieldKind::Password => FieldType::Password(Password::new(raw).map_err(|_| fail())?),
        })
    }
}

impl serde::Serialize for FieldType {
    /// Written as a bare value, never as a tagged enum: the type belongs to the field's
    /// metadata, not to each value.
    ///
    /// `Decimal` goes out as a string on purpose — a JSON number would route it through a float
    /// and lose the exactness the type exists for.
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            FieldType::String(value) => serializer.serialize_str(value),
            FieldType::Integer(value) => serializer.serialize_i32(*value),
            FieldType::Decimal(value) => serializer.serialize_str(&value.to_string()),
            FieldType::Bool(value) => serializer.serialize_bool(*value),
            FieldType::Date(value) => serializer.serialize_str(&value.to_string()),
            FieldType::DateTime(value) => serializer.serialize_str(&value.to_rfc3339()),
            FieldType::Ref(value) => serializer.serialize_u32(*value),
            FieldType::Refs(value) => serde::Serialize::serialize(value, serializer),
            // A password goes out as nothing at all. Every route to here is already closed —
            // the kind is private, so a read blanks it and a domain on it matches nothing — and
            // this is the one that would not depend on a caller having got something right.
            FieldType::Password(_) => serializer.serialize_none(),
        }
    }
}

/// Type of a field, independent of any value it may hold.
///
/// Until now the type was read off the discriminant of the declared default, which forced every
/// field to carry one and left no way to say "no value". Carrying it explicitly is what lets a
/// field be genuinely empty, and what a DDL generator or a UI needs to know about a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    #[default]
    String,
    Integer,
    Decimal,
    Bool,
    Date,
    DateTime,
    /// many2one
    Ref,
    /// one2many, and later many2many
    Refs,
    /// A secret, stored hashed and never readable. Always private, whatever the field declares.
    Password,
}

impl FieldKind {
    /// Whether values of this kind live in a column of their own.
    ///
    /// Only the "many" side of a relation does not: it is derived from the other side's foreign
    /// key.
    pub fn is_stored(&self) -> bool {
        !matches!(self, FieldKind::Refs)
    }

    /// Whether this kind holds a reference to another model.
    pub fn is_relational(&self) -> bool {
        matches!(self, FieldKind::Ref | FieldKind::Refs)
    }
}

impl std::fmt::Display for FieldKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            FieldKind::String => "string",
            FieldKind::Integer => "integer",
            FieldKind::Decimal => "decimal",
            FieldKind::Bool => "boolean",
            FieldKind::Date => "date",
            FieldKind::DateTime => "datetime",
            FieldKind::Ref => "reference",
            FieldKind::Refs => "references",
            FieldKind::Password => "password",
        };
        write!(f, "{name}")
    }
}

impl FieldType {
    /// Kind of this value.
    ///
    /// Lets the derive macro name a field's type by building its default once at startup, rather
    /// than by matching on the Rust type name.
    pub fn kind(&self) -> FieldKind {
        match self {
            FieldType::String(_) => FieldKind::String,
            FieldType::Integer(_) => FieldKind::Integer,
            FieldType::Decimal(_) => FieldKind::Decimal,
            FieldType::Bool(_) => FieldKind::Bool,
            FieldType::Date(_) => FieldKind::Date,
            FieldType::DateTime(_) => FieldKind::DateTime,
            FieldType::Ref(_) => FieldKind::Ref,
            FieldType::Refs(_) => FieldKind::Refs,
            FieldType::Password(_) => FieldKind::Password,
        }
    }
}
