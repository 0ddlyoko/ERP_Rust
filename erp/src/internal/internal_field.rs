use crate::field::FieldType;

/// Field descriptor represented by a single field in a single struct model
pub struct InternalField {
    pub name: String,
    pub default_value: Option<FieldType>,
    pub description: Option<String>,
    pub required: Option<bool>,
    // TODO
    pub compute: Option<bool>,
}

/// Final descriptor of a field.
///
/// Represent all combined InternalModel
pub struct FinalInternalField {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub default_value: FieldType,
    is_init: bool,
}

impl FinalInternalField {
    pub fn new(field_name: &str) -> Self {
        FinalInternalField {
            name: field_name.to_string(),
            description: field_name.to_string(),
            required: false,
            default_value: FieldType::String("".to_string()),
            is_init: false,
        }
    }

    pub fn register_internal_field(&mut self, field_descriptor: &InternalField) {
        if let Some(default_value) = &field_descriptor.default_value {
            if self.is_init {
                // If different type, panic
                let a = std::mem::discriminant(&self.default_value);
                let b = std::mem::discriminant(default_value);
                if a != b {
                    panic!("Default values are of different type (name: {}, first default value: {}, second default value: {}", self.name, &self.default_value, &default_value);
                }
            }
            self.default_value = default_value.clone();
        } else if !self.is_init {
            panic!("First register should have a default value. This is needed to identify the type of the field (name: {}).", field_descriptor.name);
        }
        if let Some(description) = &field_descriptor.description {
            self.description = description.clone();
        }
        if let Some(required) = &field_descriptor.required {
            self.required = *required;
        }
        self.is_init = true;
    }
}
