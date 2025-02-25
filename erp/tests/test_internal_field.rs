use test_plugin::TestPlugin;
use erp::field::FieldType;
use erp::internal::internal_field::FinalInternalField;
use erp::internal::internal_field::InternalField;
use std::any::TypeId;
use std::error::Error;
use erp::app::Application;

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
            depends: Some(vec!("age".to_string(), "test".to_string())),
            target_model: None,
            inverse: None,
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
            depends: None,
            target_model: None,
            inverse: None,
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
    assert!(field_name.depends.is_some());
    assert_eq!(field_name.depends, Some(vec!("age".to_string(), "test".to_string())));

    assert_eq!(field_age.name, "age");
    assert_eq!(
        field_age.description,
        "This is the age of the person".to_string()
    );
    assert!(!field_age.required);
    assert_eq!(field_age.default_value, FieldType::Integer(42));
    assert!(field_age.compute.is_none());
    assert!(field_age.depends.is_none());

    // Register a new existing field ("name") should override data
    field_name.register_internal_field(
        &InternalField {
            name: "name".to_string(),
            default_value: Some(FieldType::String("1ddlyoko".to_string())),
            description: None,
            required: true,
            compute: None,
            depends: None,
            target_model: None,
            inverse: None,
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
    assert!(field_name.depends.is_some());
    assert_eq!(field_name.depends, Some(vec!("age".to_string(), "test".to_string())));

    // Again
    field_name.register_internal_field(
        &InternalField {
            name: "name".to_string(),
            default_value: None,
            description: Some("This is another description".to_string()),
            required: true,
            compute: None,
            depends: Some(vec!("age".to_string(), "test2".to_string())),
            target_model: None,
            inverse: None,
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
    assert!(field_name.depends.is_some());
    assert_eq!(field_name.depends, Some(vec!("age".to_string(), "test".to_string(), "test2".to_string())));

    // Again
    field_name.register_internal_field(
        &InternalField {
            name: "name".to_string(),
            default_value: None,
            description: Some("This is another description".to_string()),
            required: true,
            compute: Some(true),
            depends: Some(vec!("age".to_string())),
            target_model: None,
            inverse: None,
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
    assert!(field_name.depends.is_some());
    assert_eq!(field_name.depends, Some(vec!("age".to_string(), "test".to_string(), "test2".to_string())));
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
            depends: None,
            target_model: None,
            inverse: None,
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
            depends: None,
            target_model: None,
            inverse: None,
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
            depends: None,
            target_model: None,
            inverse: None,
        },
        &type_id,
    );
}

#[test]
fn test_register_fields_with_real_model() -> Result<(), Box<dyn Error>> {
    let mut app = Application::new(test_utilities::default_config()?);
    app.register_plugin(Box::new(TestPlugin {}))
        .expect("Plugin should load");

    let model = app.model_manager.get_model("sale_order_test");
    // Should be none as the plugin is registered but not loaded
    assert!(model.is_none());
    app.load_plugin("test_plugin")?;

    let model = app.model_manager.get_model("sale_order_test");
    // Should exist as the plugin is registered and loaded
    assert!(model.is_some());
    let model = model.unwrap();
    let field = model.get_internal_field("name");
    assert_eq!(field.name, "name");
    // Description should be overridden
    assert_eq!(field.description, "New name of the SO");
    assert!(field.compute.is_none());
    assert!(field.required);
    assert_eq!(field.default_value, FieldType::String("".to_string()));

    Ok(())
}
