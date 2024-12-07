use std::fmt::{Display, Formatter};

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
    // TODO Add Ref
    // Ref(u32),
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

impl PartialEq for FieldType {
    fn eq(&self, other: &Self) -> bool {
        make_eq!(
            self,
            other,
            FieldType::String,
            FieldType::Integer,
            FieldType::Float,
            FieldType::Bool
        )
    }
}

// FromType

pub trait FromType<T>
where
    Self: Sized,
{
    fn from_type(t: T) -> Self;
}

// String

impl FromType<&FieldType> for Option<String> {
    fn from_type(t: &FieldType) -> Self {
        match t {
            FieldType::String(s) => Some(s.clone()),
            _ => None,
        }
    }
}

impl FromType<&String> for FieldType {
    fn from_type(t: &String) -> Self {
        FieldType::String(t.clone())
    }
}

// i64

impl FromType<&FieldType> for Option<i64> {
    fn from_type(t: &FieldType) -> Self {
        match t {
            FieldType::Integer(s) => Some(*s),
            _ => None,
        }
    }
}

impl FromType<i64> for FieldType {
    fn from_type(t: i64) -> Self {
        FieldType::Integer(t)
    }
}

// f64

impl FromType<&FieldType> for Option<f64> {
    fn from_type(t: &FieldType) -> Self {
        match t {
            FieldType::Float(f) => Some(*f),
            _ => None,
        }
    }
}

impl FromType<f64> for FieldType {
    fn from_type(t: f64) -> Self {
        FieldType::Float(t)
    }
}

// bool

impl FromType<&FieldType> for Option<bool> {
    fn from_type(t: &FieldType) -> Self {
        match t {
            FieldType::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

impl FromType<bool> for FieldType {
    fn from_type(t: bool) -> Self {
        FieldType::Bool(t)
    }
}
