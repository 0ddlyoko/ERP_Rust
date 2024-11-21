
use core::internal::internal_field::InternalField;
use core::internal::internal_field::FinalInternalField;
use core::field::FieldType;
use core::field::FieldDescriptor;
use test_utilities::models::test_model::TestEmptyModel;

#[test]
fn test_register_field() {
    let mut field_name = FinalInternalField::new("name");
    let mut field_age = FinalInternalField::new("age");

    field_name.register_internal_field(&InternalField {
        name: "name".to_string(),
        default_value: Some(FieldType::String("0ddlyoko".to_string())),
        description: Some("This is the name".to_string()),
        required: None,
        compute: None,
    });

    field_age.register_internal_field(&InternalField {
        name: "age".to_string(),
        default_value: Some(FieldType::Integer(42)),
        description: Some("This is the age of the person".to_string()),
        required: None,
        compute: None,
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
    field_name.register_internal_field(&InternalField {
        name: "name".to_string(),
        default_value: Some(FieldType::String("1ddlyoko".to_string())),
        description: None,
        required: Some(true),
        compute: None,
    });

    assert_eq!(field_name.name, "name");
    assert_eq!(field_name.description, "This is the name".to_string());
    assert!(field_name.required);
    assert_eq!(field_name.default_value, FieldType::String("1ddlyoko".to_string()));

    // Again
    field_name.register_internal_field(&InternalField {
        name: "name".to_string(),
        default_value: None,
        description: Some("This is another description".to_string()),
        required: None,
        compute: None,
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

    field_name.register_internal_field(&InternalField {
        name: "name".to_string(),
        default_value: None,
        description: Some("This is the name".to_string()),
        required: None,
        compute: None,
    });
}

#[test]
#[should_panic]
fn test_register_field_with_another_default_type_should_fail() {
    let mut field_name = FinalInternalField::new("field_name");

    field_name.register_internal_field(&InternalField {
        name: "name".to_string(),
        default_value: Some(FieldType::String("0ddlyoko".to_string())),
        description: Some("This is the name".to_string()),
        required: None,
        compute: None,
    });

    field_name.register_internal_field(&InternalField {
        name: "name".to_string(),
        default_value: Some(FieldType::Integer(42)),
        description: None,
        required: None,
        compute: None,
    });
}