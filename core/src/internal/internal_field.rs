use crate::field::{FieldDescriptor, FieldType};

/// Field descriptor represented by a single field in a single struct model
pub struct InternalField {
    pub name: &'static str,
    pub default_value: Option<FieldType>,
    pub description: Option<String>,
    pub required: Option<bool>,
}

/// Final descriptor of a field.
///
/// Represent all combined InternalModel
pub struct FinalInternalField {
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

#[cfg(test)]
mod tests {
    use crate::field::{FieldDescriptor, FieldType};
    use crate::internal::internal_field::FinalInternalField;

    #[test]
    fn test_register_field() {
        let mut field_name = FinalInternalField::new("name");
        let mut field_age = FinalInternalField::new("age");

        field_name.register_internal_field(&FieldDescriptor {
            name: "name",
            default_value: Some(FieldType::String("0ddlyoko".to_string())),
            description: Some("This is the name".to_string()),
            required: None,
        });

        field_age.register_internal_field(&FieldDescriptor {
            name: "age",
            default_value: Some(FieldType::Integer(42)),
            description: Some("This is the age of the person".to_string()),
            required: None,
        });

        assert_eq!(field_name.name, "name");
        assert_eq!(field_name.description, "This is the name".to_string());
        assert!(!field_name.required);
        assert_eq!(field_name.default_value, FieldType::String("0ddlyoko".to_string()));

        assert_eq!(field_age.name, "age");
        assert_eq!(field_age.description, "This is the age of the person".to_string());
        assert!(!field_age.required);
        assert_eq!(field_age.default_value, FieldType::Integer(42));

        // Register a new existing field ("name") should override data
        field_name.register_internal_field(&FieldDescriptor {
            name: "name",
            default_value: Some(FieldType::String("1ddlyoko".to_string())),
            description: None,
            required: Some(true),
        });

        assert_eq!(field_name.name, "name");
        assert_eq!(field_name.description, "This is the name".to_string());
        assert!(field_name.required);
        assert_eq!(field_name.default_value, FieldType::String("1ddlyoko".to_string()));

        // Again
        field_name.register_internal_field(&FieldDescriptor {
            name: "name",
            default_value: None,
            description: Some("This is another description".to_string()),
            required: None,
        });

        assert_eq!(field_name.name, "name");
        assert_eq!(field_name.description, "This is another description".to_string());
        assert!(field_name.required);
        assert_eq!(field_name.default_value, FieldType::String("1ddlyoko".to_string()));
    }

    #[test]
    #[should_panic]
    fn test_register_field_without_default_value_should_fail() {
        let mut field_name = FinalInternalField::new("field_name");

        field_name.register_internal_field(&FieldDescriptor {
            name: "name",
            default_value: None,
            description: Some("This is the name".to_string()),
            required: None,
        });
    }

    #[test]
    #[should_panic]
    fn test_register_field_with_another_default_type_should_fail() {
        let mut field_name = FinalInternalField::new("field_name");

        field_name.register_internal_field(&FieldDescriptor {
            name: "name",
            default_value: Some(FieldType::String("0ddlyoko".to_string())),
            description: Some("This is the name".to_string()),
            required: None,
        });

        field_name.register_internal_field(&FieldDescriptor {
            name: "name",
            default_value: Some(FieldType::Integer(42)),
            description: None,
            required: None,
        });
    }
}

