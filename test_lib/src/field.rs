
// Generated Fields

#[derive(Debug)]
pub struct GeneratedFieldDescriptor {
    pub field_name: String,
    pub is_required: Option<bool>,
    pub default_field: FieldType,
}

impl GeneratedFieldDescriptor {
    pub fn name(&self) -> &String {
        &self.field_name
    }

    pub fn required(&self) -> &Option<bool> {
        &self.is_required
    }

    pub fn default_field(&self) -> &FieldType {
        &self.default_field
    }
}

// Descriptor of a field

#[derive(Debug)]
pub struct FieldDescriptor {
    pub field_name: String,
    pub column_name: String,
    pub is_required: bool,
    pub default_value: FieldType,
}

impl FieldDescriptor {
    pub fn default(field_name: &str, default_value: FieldType) -> FieldDescriptor {
        FieldDescriptor {
            field_name: field_name.to_string(),
            column_name: field_name.to_string(),
            is_required: false,
            default_value: default_value,
        }
    }
}

// Fields used in Models

#[derive(Debug)]
pub struct Field<TYPE> {
    value: Option<TYPE>,
    pub dirty: bool,
}

impl<TYPE> Field<TYPE> {
    pub fn new(value: Option<TYPE>) -> Self {
        Field {
            value: value,
            dirty: false,
        }
    }

    pub fn is_present(&self) -> bool {
        self.value.is_some()
    }

    pub fn is_none(&self) -> bool {
        self.value.is_none()
    }

    pub fn value(&self) -> &Option<TYPE> {
        &self.value
    }

    pub fn value_as_ref(&self) -> Option<&TYPE> {
        self.value.as_ref()
    }

    pub fn set(&mut self, value: TYPE) {
        self.value = Some(value);
        self.dirty = true;
    }

    pub fn clear(&mut self) {
        self.value = None;
        self.dirty = true;
    }
}

#[derive(Debug)]
pub enum FieldType {
    String(Field<String>),
    Integer(Field<i32>),
    Boolean(Field<bool>),
}

impl FieldType {

    pub fn from(field_type: &str, value: Option<String>) -> Self {
        match field_type {
            "String" => FieldType::String(Field::new(value)),
            "Integer" | "i32" => FieldType::Integer(Field::new(value.map(|f| f.parse().unwrap()))),
            "Boolean" | "bool" => FieldType::Boolean(Field::new(value.map(|f| if f == "true" {true} else {false}))),
            _ => panic!("Unknown field type \"{}\"!", field_type),
        }
    }

    pub fn is_dirty(&self) -> bool {
        match self {
            FieldType::String(field) => field.dirty,
            FieldType::Integer(field) => field.dirty,
            FieldType::Boolean(field) => field.dirty,
        }
    }

    pub fn clear(&mut self) {
        match self {
            FieldType::String(field) => field.dirty = false,
            FieldType::Integer(field) => field.dirty = false,
            FieldType::Boolean(field) => field.dirty = false,
        }
    }

    pub fn has_entry(&self) -> bool {
        match self {
            FieldType::String(field) => field.value_as_ref().is_some(),
            FieldType::Integer(field) => field.value_as_ref().is_some(),
            FieldType::Boolean(field) => field.value_as_ref().is_some(),
        }
    }

    pub fn update_value(&mut self, field_type: &FieldType) {
        match self {
            FieldType::String(field_to_edit) => {
                if let FieldType::String(field) = field_type {
                    field_to_edit.value = field.value.clone()
                }
            }
            FieldType::Integer(field_to_edit) => {
                if let FieldType::Integer(field) = field_type {
                    field_to_edit.value = field.value.clone()
                }
            }
            FieldType::Boolean(field_to_edit) => {
                if let FieldType::Boolean(field) = field_type {
                    field_to_edit.value = field.value.clone()
                }
            }
        }
    }

    pub fn transform_to_string(field_type: FieldType) -> Field<String> {
        match field_type {
            FieldType::String(field) => field,
            FieldType::Integer(_) => panic!("Cannot convert Integer to String!"),
            FieldType::Boolean(_) => panic!("Cannot convert Boolean to String!"),
        }
    }

    pub fn transform_to_integer(field_type: FieldType) -> Field<i32> {
        match field_type {
            FieldType::String(_) => panic!("Cannot convert String to Integer!"),
            FieldType::Integer(field) => field,
            FieldType::Boolean(_) => panic!("Cannot convert Boolean to Integer!"),
        }
    }

    pub fn transform_to_boolean(field_type: FieldType) -> Field<bool> {
        match field_type {
            FieldType::String(_) => panic!("Cannot convert String to Boolean!"),
            FieldType::Integer(_) => panic!("Cannot convert Integer to Boolean!"),
            FieldType::Boolean(field) => field,
        }
    }

    pub fn clone(&self) -> Self {
        match self {
            FieldType::String(field) => {
                FieldType::String(Field {
                    value: field.value.clone().map(|value| value.as_str().to_string()),
                    dirty: field.dirty,
                })
            }
            FieldType::Integer(field) => {
                FieldType::Integer(Field {
                    value: field.value.map(|value| value),
                    dirty: field.dirty,
                })
            }
            FieldType::Boolean(field) => {
                FieldType::Boolean(Field {
                    value: field.value.map(|value| value),
                    dirty: field.dirty,
                })
            }
        }
    }
}
