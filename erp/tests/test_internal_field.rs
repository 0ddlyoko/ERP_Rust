use erp::field::FieldType;
use erp::internal::internal_field::FinalInternalField;
use erp::internal::internal_field::InternalField;
use std::any::TypeId;

#[test]
fn test_register_field() {
    let type_id = TypeId::of::<InternalField>();
    let mut field_name = FinalInternalField::new("name");
    let mut field_age = FinalInternalField::new("age");

    field_name.register_internal_field(
        &InternalField {
            name: "name".to_string(),
            default_value: Some(FieldType::String("0ddlyoko".to_string())),
            description: Some("This is the name".to_string()),
            required: false,
            compute: None,
        },
        &type_id,
    );

    field_age.register_internal_field(
        &InternalField {
            name: "age".to_string(),
            default_value: Some(FieldType::Integer(42)),
            description: Some("This is the age of the person".to_string()),
            required: false,
            compute: None,
        },
        &type_id,
    );

    assert_eq!(field_name.name, "name");
    assert_eq!(field_name.description, "This is the name".to_string());
    assert!(!field_name.required);
    assert_eq!(
        field_name.default_value,
        FieldType::String("0ddlyoko".to_string())
    );
    assert!(field_name.compute.is_none());

    assert_eq!(field_age.name, "age");
    assert_eq!(
        field_age.description,
        "This is the age of the person".to_string()
    );
    assert!(!field_age.required);
    assert_eq!(field_age.default_value, FieldType::Integer(42));
    assert!(field_name.compute.is_none());

    // Register a new existing field ("name") should override data
    field_name.register_internal_field(
        &InternalField {
            name: "name".to_string(),
            default_value: Some(FieldType::String("1ddlyoko".to_string())),
            description: None,
            required: true,
            compute: None,
        },
        &type_id,
    );

    assert_eq!(field_name.name, "name");
    assert_eq!(field_name.description, "This is the name".to_string());
    assert!(field_name.required);
    assert_eq!(
        field_name.default_value,
        FieldType::String("1ddlyoko".to_string())
    );
    assert!(field_name.compute.is_none());

    // Again
    field_name.register_internal_field(
        &InternalField {
            name: "name".to_string(),
            default_value: None,
            description: Some("This is another description".to_string()),
            required: true,
            compute: None,
        },
        &type_id,
    );

    assert_eq!(field_name.name, "name");
    assert_eq!(
        field_name.description,
        "This is another description".to_string()
    );
    assert!(field_name.required);
    assert_eq!(
        field_name.default_value,
        FieldType::String("1ddlyoko".to_string())
    );
    assert!(field_name.compute.is_none());

    // Again
    field_name.register_internal_field(
        &InternalField {
            name: "name".to_string(),
            default_value: None,
            description: Some("This is another description".to_string()),
            required: true,
            compute: Some(true),
        },
        &type_id,
    );

    assert_eq!(field_name.name, "name");
    assert_eq!(
        field_name.description,
        "This is another description".to_string()
    );
    assert!(field_name.required);
    assert_eq!(
        field_name.default_value,
        FieldType::String("1ddlyoko".to_string())
    );
    assert!(field_name.compute.is_some());
}

#[test]
#[should_panic]
fn test_register_field_without_default_value_should_fail() {
    let type_id = TypeId::of::<InternalField>();
    let mut field_name = FinalInternalField::new("field_name");

    field_name.register_internal_field(
        &InternalField {
            name: "name".to_string(),
            default_value: None,
            description: Some("This is the name".to_string()),
            required: true,
            compute: None,
        },
        &type_id,
    );
}

#[test]
#[should_panic]
fn test_register_field_with_another_default_type_should_fail() {
    let type_id = TypeId::of::<InternalField>();
    let mut field_name = FinalInternalField::new("field_name");

    field_name.register_internal_field(
        &InternalField {
            name: "name".to_string(),
            default_value: Some(FieldType::String("0ddlyoko".to_string())),
            description: Some("This is the name".to_string()),
            required: true,
            compute: None,
        },
        &type_id,
    );

    field_name.register_internal_field(
        &InternalField {
            name: "name".to_string(),
            default_value: Some(FieldType::Integer(42)),
            description: None,
            required: true,
            compute: None,
        },
        &type_id,
    );
}
