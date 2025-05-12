use erp_search::RightTuple;

#[derive(Clone)]
pub enum FieldType {
    String(String),
    Integer(i32),
    UInteger(u32),
    Float(f32),
    Boolean(bool),
}

impl FieldType {
    // TODO Remove this method once correctly implemented
    pub fn is_same(&self, other: &str) -> bool {
        match self {
            FieldType::String(value) => {
                value == other
            },
            FieldType::Integer(value) => {
                let other = other.parse::<i32>();
                if let Ok(other) = other {
                    value == &other
                } else {
                    false
                }
            },
            FieldType::UInteger(value) => {
                let other = other.parse::<u32>();
                if let Ok(other) = other {
                    value == &other
                } else {
                    false
                }
            },
            FieldType::Float(value) => {
                let other = other.parse::<f32>();
                if let Ok(other) = other {
                    value == &other
                } else {
                    false
                }
            },
            FieldType::Boolean(value) => {
                let other = other.parse::<bool>();
                if let Ok(other) = other {
                    value == &other
                } else {
                    false
                }
            },
        }
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
