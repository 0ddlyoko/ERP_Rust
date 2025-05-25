use std::fmt::{Display, Formatter};
use erp_search::RightTuple;
use crate::field;

#[macro_export]
macro_rules! database_field_type_make_eq {
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
    UInteger(u32),
    Float(f32),
    Boolean(bool),
}

impl Display for FieldType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FieldType::String(s) => write!(f, "{}", s),
            FieldType::Integer(i) => write!(f, "{}", i),
            FieldType::UInteger(b) => write!(f, "{}", b),
            FieldType::Float(fl) => write!(f, "{}", fl),
            FieldType::Boolean(e) => write!(f, "{}", e),
        }
    }
}

impl PartialEq for FieldType {
    fn eq(&self, other: &Self) -> bool {
        database_field_type_make_eq!(
            self,
            other,
            FieldType::String,
            FieldType::Integer,
            FieldType::UInteger,
            FieldType::Float,
            FieldType::Boolean
        )
    }
}

impl From<FieldType> for RightTuple {
    fn from(other: FieldType) -> Self {
        match other {
            FieldType::String(value) => RightTuple::String(value),
            FieldType::Integer(value) => RightTuple::Integer(value),
            FieldType::UInteger(value) => RightTuple::UInteger(value),
            FieldType::Float(value) => RightTuple::Float(value),
            FieldType::Boolean(value) => RightTuple::Boolean(value),
        }
    }
}

impl PartialEq<RightTuple> for FieldType {
    fn eq(&self, other: &RightTuple) -> bool {
        match (self, other) {
            (FieldType::String(value), RightTuple::String(other_value)) => value == other_value,
            (FieldType::Integer(value), RightTuple::Integer(other_value)) => value == other_value,
            (FieldType::UInteger(value), RightTuple::UInteger(other_value)) => value == other_value,
            (FieldType::Float(value), RightTuple::Float(other_value)) => value == other_value,
            (FieldType::Boolean(value), RightTuple::Boolean(other_value)) => value == other_value,
            (value, RightTuple::Array(other_value)) => {
                other_value.contains(&value.clone().into())
            },
            _ => false,
        }
    }
}

impl PartialEq<FieldType> for RightTuple {
    fn eq(&self, other: &FieldType) -> bool {
        // Call method above
        other.eq(self)
    }
}

impl From<field::FieldType> for FieldType {
    fn from(value: field::FieldType) -> Self {
        match value {
            field::FieldType::String(v) => FieldType::String(v),
            field::FieldType::Integer(v) => FieldType::Integer(v),
            field::FieldType::Float(v) => FieldType::Float(v),
            field::FieldType::Bool(v) => FieldType::Boolean(v),
            field::FieldType::Enum(v) => FieldType::String(v),
            field::FieldType::Ref(v) => FieldType::UInteger(v),
            // This should not occur
            field::FieldType::Refs(_v) => panic!("Cannot convert Refs fields to database objet"),
        }
    }
}

impl From<FieldType> for field::FieldType {
    fn from(value: FieldType) -> Self {
        match value {
            FieldType::String(v) => field::FieldType::String(v),
            FieldType::Integer(v) => field::FieldType::Integer(v),
            FieldType::UInteger(v) => field::FieldType::Ref(v),
            FieldType::Float(v) => field::FieldType::Float(v),
            FieldType::Boolean(v) => field::FieldType::Bool(v),
        }
    }
}
