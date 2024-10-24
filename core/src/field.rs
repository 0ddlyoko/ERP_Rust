
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

pub struct FieldDescriptor {
    pub name: String,
    pub field_type: FieldType,
    pub description: Option<String>,
    pub required: Option<bool>,
}

impl Default for FieldDescriptor {
    fn default() -> FieldDescriptor {
        FieldDescriptor {
            name: "".to_string(),
            field_type: FieldType::String("".to_string()),
            description: None,
            required: None,
        }
    }
}
