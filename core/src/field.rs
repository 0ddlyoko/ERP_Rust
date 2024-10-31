use std::fmt::{Display, Formatter};

#[derive(Debug)]
pub enum FieldType {
    String(String),
    Integer(i64),
    Float(f64),
    Bool(bool),
}

impl FieldType {
    pub fn string(&self) -> String {
        match self {
            FieldType::String(s) => s.clone(),
            _ => panic!("Not a string field type!"),
        }
    }

    pub fn integer(&self) -> i64 {
        match self {
            FieldType::Integer(i) => *i,
            _ => panic!("Not an integer field type!"),
        }
    }

    pub fn float(&self) -> f64 {
        match self {
            FieldType::Float(f) => *f,
            _ => panic!("Not a float field type!"),
        }
    }

    pub fn bool(&self) -> bool {
        match self {
            FieldType::Bool(b) => *b,
            _ => panic!("Not a boolean field type!"),
        }
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
        match self {
            FieldType::String(str) => {
                if let FieldType::String(str2) = other {
                    str == str2
                } else {
                    false
                }
            }
            FieldType::Integer(int) => {
                if let FieldType::Integer(int2) = other {
                    int == int2
                } else {
                    false
                }
            }
            FieldType::Float(float) => {
                if let FieldType::Float(float2) = other {
                    float == float2
                } else {
                    false
                }
            }
            FieldType::Bool(bool) => {
                if let FieldType::Bool(bool2) = other {
                    bool == bool2
                } else {
                    false
                }
            }
        }
    }
}

pub struct FieldDescriptor {
    pub name: &'static str,
    pub default_value: Option<FieldType>,
    pub description: Option<String>,
    pub required: Option<bool>,
}

impl Default for FieldDescriptor {
    fn default() -> FieldDescriptor {
        FieldDescriptor {
            name: "",
            default_value: None,
            description: None,
            required: None,
        }
    }
}
