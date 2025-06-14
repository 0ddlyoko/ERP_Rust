use test_utilities::TestLibPlugin;
use test_plugin::TestPlugin;
use erp::field::{FieldCompute, FieldReferenceType, FieldType};
use erp::internal::internal_field::FinalInternalField;
use erp::internal::internal_field::InternalField;
use std::any::TypeId;
use std::error::Error;
use erp::app::Application;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

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
            compute: Some(FieldCompute {
                type_id,
                depends: vec!("age".to_string(), "test".to_string()),
            }),
            field_ref: None,
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
            field_ref: None,
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
    assert!(field_name.compute.is_some());
    let field_name_compute = field_name.compute.as_ref().unwrap();
    assert_eq!(field_name_compute.depends, vec!("age".to_string(), "test".to_string()));

    assert_eq!(field_age.name, "age");
    assert_eq!(
        field_age.description,
        "This is the age of the person".to_string()
    );
    assert!(!field_age.required);
    assert_eq!(field_age.default_value, FieldType::Integer(42));
    assert!(field_age.compute.is_none());

    // Register a new existing field ("name") should override data
    field_name.register_internal_field(
        &InternalField {
            name: "name".to_string(),
            default_value: Some(FieldType::String("1ddlyoko".to_string())),
            description: None,
            required: true,
            compute: None,
            field_ref: None,
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
    assert!(field_name.compute.is_some());
    let field_name_compute = field_name.compute.as_ref().unwrap();
    assert_eq!(field_name_compute.depends, vec!("age".to_string(), "test".to_string()));

    // Again
    field_name.register_internal_field(
        &InternalField {
            name: "name".to_string(),
            default_value: None,
            description: Some("This is another description".to_string()),
            required: true,
            compute: Some(FieldCompute {
                type_id,
                depends: vec!("age".to_string(), "test2".to_string()),
            }),
            field_ref: None,
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
    let field_name_compute = field_name.compute.as_ref().unwrap();
    assert_eq!(field_name_compute.depends, vec!("age".to_string(), "test".to_string(), "test2".to_string()));

    // Again
    field_name.register_internal_field(
        &InternalField {
            name: "name".to_string(),
            default_value: None,
            description: Some("This is another description".to_string()),
            required: true,
            compute: Some(FieldCompute {
                type_id,
                depends: vec!("age".to_string()),
            }),
            field_ref: None,
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
    let field_name_compute = field_name.compute.as_ref().unwrap();
    assert_eq!(field_name_compute.depends, vec!("age".to_string(), "test".to_string(), "test2".to_string()));
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
            field_ref: None,
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
            field_ref: None,
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
            field_ref: None,
        },
        &type_id,
    );
}

#[test]
fn test_register_fields_with_real_model() -> Result<()> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(TestPlugin {}))
        .expect("Plugin should load");

    app.load_plugin("test_plugin")?;

    // Should exist as the plugin is registered and loaded
    let model = app.model_manager.get_model("sale_order_test");
    let field = model.get_internal_field("name");
    assert_eq!(field.name, "name");
    // Description should be overridden
    assert_eq!(field.description, "New name of the SO");
    assert!(field.compute.is_none());
    assert!(field.required);
    assert_eq!(field.default_value, FieldType::String("".to_string()));

    Ok(())
}

#[test]
fn test_reference_inverse_link() -> Result<()> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    app.load_plugin("test_lib_plugin")?;
    let so_model = app.model_manager.get_model("sale_order");
    let so_line_model = app.model_manager.get_model("sale_order_line");

    let so_field_inverse = &so_model.get_internal_field("lines").inverse;
    let so_line_field_inverse = &so_line_model.get_internal_field("order").inverse;
    assert!(so_field_inverse.is_some());
    assert!(so_line_field_inverse.is_some());
    let so_field_inverse = so_field_inverse.as_ref().unwrap();
    let so_line_field_inverse = so_line_field_inverse.as_ref().unwrap();

    assert_eq!(so_field_inverse.target_model, "sale_order_line");
    assert_eq!(so_line_field_inverse.target_model, "sale_order");

    assert!(matches!(so_field_inverse.inverse_field.clone(), FieldReferenceType::O2M { inverse_field } if inverse_field == "order"));
    assert!(matches!(so_line_field_inverse.inverse_field.clone(), FieldReferenceType::M2O { inverse_fields } if inverse_fields.contains(&"lines".to_string())));

    Ok(())
}
