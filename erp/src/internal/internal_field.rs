use crate::field::FieldType;
use std::any::TypeId;
use std::collections::HashSet;

/// Field descriptor represented by a single field in a single struct model
pub struct InternalField {
    pub name: String,
    pub default_value: Option<FieldType>,
    pub description: Option<String>,
    pub required: bool,
    pub compute: Option<bool>,
    // TODO change String to &'static
    pub depends: Option<Vec<String>>,
    pub inverse: Option<String>,
}

/// Final descriptor of a field.
///
/// Represent all combined InternalModel
pub struct FinalInternalField {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub default_value: FieldType,
    pub compute: Option<TypeId>,
    pub depends: Option<Vec<String>>,
    pub inverse: Option<String>,
    is_init: bool,
}

impl FinalInternalField {
    pub fn new(field_name: &str) -> Self {
        FinalInternalField {
            name: field_name.to_string(),
            description: field_name.to_string(),
            required: false,
            default_value: FieldType::String("".to_string()),
            compute: None,
            depends: None,
            inverse: None,
            is_init: false,
        }
    }

    pub fn register_internal_field(&mut self, field_descriptor: &InternalField, type_id: &TypeId) {
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
        self.required = field_descriptor.required;
        if let Some(compute) = &field_descriptor.compute {
            if *compute {
                self.compute = Some(*type_id);
            }
        }
        if let Some(depends) = &field_descriptor.depends {
            if let Some(current_depends) = &mut self.depends {
                current_depends.append(&mut depends.clone());
                // Remove duplicates
                let mut seen = HashSet::new();
                current_depends.retain(|dep| seen.insert(dep.clone()));
            } else {
                self.depends = Some(depends.clone());
            }
        }
        if let Some(inverse) = &field_descriptor.inverse {
            self.inverse = Some(inverse.clone());
        }
        self.is_init = true;
    }
}
