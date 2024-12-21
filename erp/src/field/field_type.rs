use crate::field::Reference;
use std::fmt::{Debug, Display, Formatter};
use crate::model::BaseModel;

#[macro_export]
macro_rules! make_eq {
    ( $self:expr, $other:expr, $( $path:path ),* ) => {
        match $self {
            $($path(ref self_value) => {
                if let $path(ref other_value) = $other {
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
    Integer(i64),
    Float(f64),
    Bool(bool),
    Enum(String),
    Ref(u32),
}

impl Display for FieldType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldType::String(s) => write!(f, "{}", s),
            FieldType::Integer(i) => write!(f, "{}", i),
            FieldType::Float(fl) => write!(f, "{}", fl),
            FieldType::Bool(b) => write!(f, "{}", b),
            FieldType::Enum(e) => write!(f, "{}", e),
            FieldType::Ref(id) => write!(f, "{}", id),
        }
    }
}

impl PartialEq for FieldType {
    fn eq(&self, other: &Self) -> bool {
        make_eq!(
            self,
            other,
            FieldType::String,
            FieldType::Integer,
            FieldType::Float,
            FieldType::Bool,
            FieldType::Enum,
            FieldType::Ref
        )
    }
}

// String

impl From<&FieldType> for Option<String> {
    fn from(t: &FieldType) -> Self {
        match t {
            FieldType::String(s) => Some(s.clone()),
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

// i64

impl From<&FieldType> for Option<i64> {
    fn from(t: &FieldType) -> Self {
        match t {
            FieldType::Integer(s) => Some(*s),
            _ => None,
        }
    }
}

impl From<i64> for FieldType {
    fn from(t: i64) -> Self {
        FieldType::Integer(t)
    }
}

impl From<&i64> for FieldType {
    fn from(t: &i64) -> Self {
        FieldType::Integer(*t)
    }
}

// f64

impl From<&FieldType> for Option<f64> {
    fn from(t: &FieldType) -> Self {
        match t {
            FieldType::Float(f) => Some(*f),
            _ => None,
        }
    }
}

impl From<f64> for FieldType {
    fn from(t: f64) -> Self {
        FieldType::Float(t)
    }
}

impl From<&f64> for FieldType {
    fn from(t: &f64) -> Self {
        FieldType::Float(*t)
    }
}

// bool

impl From<&FieldType> for Option<bool> {
    fn from(t: &FieldType) -> Self {
        match t {
            FieldType::Bool(b) => Some(*b),
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

pub trait EnumType: Debug + PartialEq + Eq + Copy + Clone {
    fn to_string(&self) -> String;
    fn from_string(t: String) -> Self;
}

impl<E> From<&FieldType> for Option<E> where E: EnumType {
    fn from(t: &FieldType) -> Self {
        match t {
            FieldType::Enum(s) => Some(E::from_string(s.clone())),
            _ => None,
        }
    }
}

impl<E> From<&E> for FieldType where E: EnumType {
    fn from(t: &E) -> Self {
        FieldType::Enum(t.to_string())
    }
}

// Ref

impl From<&FieldType> for Option<u32> {
    fn from(t: &FieldType) -> Self {
        match t {
            FieldType::Ref(r) => Some(*r),
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

impl<E> From<&FieldType> for Option<Reference<E>> where E: BaseModel {
    fn from(t: &FieldType) -> Self {
        match t {
            FieldType::Ref(id) => Some(id.into()),
            _ => None,
        }
    }
}

impl<E> From<&Reference<E>> for FieldType where E: BaseModel {
    fn from(t: &Reference<E>) -> Self {
        FieldType::Ref(t.id)
    }
}
