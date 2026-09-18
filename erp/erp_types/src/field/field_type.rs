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
            FieldType::Refs
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
