use std::fmt::{Display, Formatter};
use crate::field::FromType;

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

#[derive(Debug)]
pub enum FieldType {
    String(String),
    Integer(i64),
    Float(f64),
    Bool(bool),
}

impl FromType<&FieldType> for String {
    fn from_type(t: &FieldType) -> Option<Self> {
        match t {
            FieldType::String(s) => Some(s.clone()),
            _ => None,
        }
    }
}

impl FromType<&FieldType> for i64 {
    fn from_type(t: &FieldType) -> Option<Self> {
        match t {
            FieldType::Integer(s) => Some(*s),
            _ => None,
        }
    }
}

impl FromType<&FieldType> for f64 {
    fn from_type(t: &FieldType) -> Option<Self> {
        match t {
            FieldType::Float(f) => Some(*f),
            _ => None,
        }
    }
}

impl FromType<&FieldType> for bool {
    fn from_type(t: &FieldType) -> Option<Self> {
        match t {
            FieldType::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

impl FromType<&String> for FieldType {
    fn from_type(t: &String) -> Option<Self> {
        Some(FieldType::String(t.clone()))
    }
}

impl FromType<i64> for FieldType {
    fn from_type(t: i64) -> Option<Self> {
        Some(FieldType::Integer(t))
    }
}

impl FromType<f64> for FieldType {
    fn from_type(t: f64) -> Option<Self> {
        Some(FieldType::Float(t))
    }
}

impl FromType<bool> for FieldType {
    fn from_type(t: bool) -> Option<Self> {
        Some(FieldType::Bool(t))
    }
}

impl Display for FieldType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldType::String(s) => write!(f, "{}", s),
            FieldType::Integer(i) => write!(f, "{}", i),
            FieldType::Float(fl) => write!(f, "{}", fl),
            FieldType::Bool(b) => write!(f, "{}", b),
        }
    }
}

impl Clone for FieldType {
    fn clone(&self) -> Self {
        match self {
            FieldType::String(str) => { FieldType::String(str.clone()) }
            FieldType::Integer(int) => { FieldType::Integer(*int) }
            FieldType::Float(float) => { FieldType::Float(*float) }
            FieldType::Bool(bool) => { FieldType::Bool(*bool) }
        }
    }
}

impl PartialEq for FieldType {
    fn eq(&self, other: &Self) -> bool {
        make_eq!(
            self, other,
            FieldType::String,
            FieldType::Integer,
            FieldType::Float,
            FieldType::Bool
        )
    }
}
