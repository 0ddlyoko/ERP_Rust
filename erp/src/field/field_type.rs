use std::fmt::{Debug, Display, Formatter};
use crate::field::Reference;
use crate::model::Model;

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
    Ref((String, u32)),
}

impl Display for FieldType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldType::String(s) => write!(f, "{}", s),
            FieldType::Integer(i) => write!(f, "{}", i),
            FieldType::Float(fl) => write!(f, "{}", fl),
            FieldType::Bool(b) => write!(f, "{}", b),
            FieldType::Enum(e) => write!(f, "{}", e),
            FieldType::Ref((model_name, id)) => write!(f, "{}:{}", model_name, id),
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

// Enums

pub trait EnumType: Debug + PartialEq + Eq {
    fn to_string(&self) -> String;
    fn from_string(t: String) -> Self;
}

impl<E: EnumType> From<&FieldType> for Option<E> {
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

impl From<&FieldType> for Option<(String, u32)> {
    fn from(t: &FieldType) -> Self {
        match t {
            FieldType::Ref(s) => Some(s.clone()),
            _ => None,
        }
    }
}

impl From<&(String, u32)> for FieldType {
    fn from(t: &(String, u32)) -> Self {
        FieldType::Ref(t.clone())
    }
}

impl<M: Model> From<&FieldType> for Option<Reference<M>> {
    fn from(t: &FieldType) -> Self {
        match t {
            FieldType::Ref((_, id)) => Some(id.into()),
            _ => None,
        }
    }
}

impl<M: Model> From<&Reference<M>> for FieldType {
    fn from(t: &Reference<M>) -> Self {
        FieldType::Ref((M::get_model_name().clone(), t.id))
    }
}
