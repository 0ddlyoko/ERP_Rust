use crate::field::{FieldType, FieldReference, FieldCompute};
use std::any::TypeId;
use std::collections::HashSet;

/// Field descriptor represented by a single field in a single struct model
pub struct InternalField {
    pub name: String,
    pub default_value: Option<FieldType>,
    pub description: Option<String>,
    pub required: bool,
    pub compute: Option<FieldCompute>,
    pub field_ref: Option<FieldReference>,
}

/// Final descriptor of a field.
///
/// Represent all combined InternalModel
pub struct FinalInternalField {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub default_value: FieldType,
    pub compute: Option<FieldCompute>,
    pub inverse: Option<FieldReference>,
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
        if let Some(new_compute) = &field_descriptor.compute {
            if let Some(existing_compute) = &mut self.compute {
                existing_compute.type_id = *type_id;
                existing_compute.depends.append(&mut new_compute.depends.clone());
                // Remove duplicates
                let mut seen = HashSet::new();
                existing_compute.depends.retain(|dep| seen.insert(dep.clone()));
            } else {
                self.compute = Some(FieldCompute {
                    type_id: *type_id,
                    depends: new_compute.depends.clone(),
                });
            }
        }
        if let Some(inverse) = &field_descriptor.field_ref {
            self.inverse = Some(inverse.clone());
        }
        self.is_init = true;
    }
}
