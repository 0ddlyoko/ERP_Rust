use crate::field::{IdMode, MultipleIds, Reference, SingleId};
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
    Integer(i32),
    Float(f64),
    Bool(bool),
    Enum(String),
    Ref(u32),
    Refs(Vec<u32>),
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
            FieldType::Refs(ids) => write!(f, "{:?}", ids),
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

// f64

impl<'a> From<&'a FieldType> for Option<&'a f64> {
    fn from(t: &'a FieldType) -> Self {
        match t {
            FieldType::Float(f) => Some(f),
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

pub trait EnumType: Debug + PartialEq + Eq + Copy + Clone {
}

impl<'a, E> From<&'a FieldType> for Option<&'a E> where E: EnumType, &'a E: From<&'a str> {
    fn from(t: &'a FieldType) -> Self {
        match t {
            FieldType::Enum(s) => Some(s.as_str().into()),
            _ => None,
        }
    }
}

impl<'a, E> From<E> for FieldType where E: EnumType, &'a str: From<E> {
    fn from(t: E) -> Self {
        let result: &str = t.into();
        FieldType::Enum(result.to_string())
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

impl<E> From<&FieldType> for Option<Reference<E, SingleId>> where E: BaseModel {
    fn from(t: &FieldType) -> Self {
        match t {
            FieldType::Ref(id) => Some(id.into()),
            _ => None,
        }
    }
}

impl<E> From<&Reference<E, SingleId>> for FieldType where E: BaseModel {
    fn from(t: &Reference<E, SingleId>) -> Self {
        FieldType::Ref(t.id_mode.id)
    }
}

impl<E> From<Reference<E, SingleId>> for FieldType where E: BaseModel {
    fn from(t: Reference<E, SingleId>) -> Self {
        FieldType::Ref(t.id_mode.id)
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

impl<E> From<&FieldType> for Option<Reference<E, MultipleIds>> where E: BaseModel {
    fn from(t: &FieldType) -> Self {
        match t {
            FieldType::Refs(ids) => Some(ids.clone().into()),
            _ => None,
        }
    }
}

impl<E> From<&Reference<E, MultipleIds>> for FieldType where E: BaseModel {
    fn from(t: &Reference<E, MultipleIds>) -> Self {
        FieldType::Refs(t.id_mode.get_ids())
    }
}

impl<E> From<Reference<E, MultipleIds>> for FieldType where E: BaseModel {
    fn from(t: Reference<E, MultipleIds>) -> Self {
        FieldType::Refs(t.id_mode.ids)
    }
}
