use crate::field::{FieldDescriptor, FieldType};

/// Field descriptor represented by a single field in a single struct model
pub(crate) struct InternalField {
    pub name: &'static str,
    pub default_value: Option<FieldType>,
    pub description: Option<String>,
    pub required: Option<bool>,
}

/// Final descriptor of a field.
///
/// Represent all combined InternalModel
pub(crate) struct FinalInternalField {
    pub name: &'static str,
    pub description: String,
    pub required: bool,
    pub default_value: FieldType,
    is_init: bool,
}

impl FinalInternalField {
    pub fn new(field_name: &'static str) -> Self {
        FinalInternalField {
            name: field_name,
            description: field_name.to_string(),
            required: false,
            default_value: FieldType::String("".to_string()),
            is_init: false,
        }
    }

    pub fn register_internal_field(&mut self, field_descriptor: &FieldDescriptor) {
        if let Some(default_value) = &field_descriptor.default_value {
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
