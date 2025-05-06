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
