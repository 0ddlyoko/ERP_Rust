use erp::app::Application;
use erp_internal_types::{FinalInternalField, InternalField};
use erp_types::field::{FieldCompute, FieldReferenceType};
use erp_types::field::{FieldKind, FieldType};
use std::error::Error;
use test_plugin::TestPlugin;
use test_utilities::TestLibPlugin;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

#[test]
fn test_register_field() {
    let mut field_name = FinalInternalField::new("name");
    let mut field_age = FinalInternalField::new("age");

    field_name.register_internal_field(&InternalField {
        name: "name".to_string(),
        kind: FieldKind::String,
        default_value: Some(FieldType::String("0ddlyoko".to_string())),
        label: None,
        description: Some("This is the name".to_string()),
        required: false,
        private: false,
        asks_for_storage: false,
        compute: Some(FieldCompute {
            method: "compute_it".to_string(),
            depends: vec!["age", "test"],
        }),
        field_ref: None,
        selection: None,
        tracking: false,
        owned: false,
        on_delete: None,
    });

    field_age.register_internal_field(&InternalField {
        name: "age".to_string(),
        kind: FieldKind::Integer,
        default_value: Some(FieldType::Integer(42)),
        label: None,
        description: Some("This is the age of the person".to_string()),
        required: false,
        private: false,
        asks_for_storage: false,
        compute: None,
        field_ref: None,
        selection: None,
        tracking: false,
        owned: false,
        on_delete: None,
    });

    assert_eq!(field_name.name, "name");
    assert_eq!(field_name.description.as_deref(), Some("This is the name"));
    assert!(!field_name.required);
    assert_eq!(
        field_name.default_value,
        Some(FieldType::String("0ddlyoko".to_string()))
    );
    assert!(field_name.compute.is_some());
    let field_name_compute = field_name.compute.as_ref().unwrap();
    assert_eq!(
        field_name_compute.depends,
        vec!("age".to_string(), "test".to_string())
    );

    assert_eq!(field_age.name, "age");
    assert_eq!(
        field_age.description.as_deref(),
        Some("This is the age of the person")
    );
    assert!(!field_age.required);
    assert_eq!(field_age.default_value, Some(FieldType::Integer(42)));
    assert!(field_age.compute.is_none());

    // Register a new existing field ("name") should override data
    field_name.register_internal_field(&InternalField {
        name: "name".to_string(),
        kind: FieldKind::String,
        default_value: Some(FieldType::String("1ddlyoko".to_string())),
        label: None,
        description: None,
        required: true,
        private: false,
        asks_for_storage: false,
        compute: None,
        field_ref: None,
        selection: None,
        tracking: false,
        owned: false,
        on_delete: None,
    });

    assert_eq!(field_name.name, "name");
    assert_eq!(field_name.description.as_deref(), Some("This is the name"));
    assert!(field_name.required);
    assert_eq!(
        field_name.default_value,
        Some(FieldType::String("1ddlyoko".to_string()))
    );
    assert!(field_name.compute.is_some());
    let field_name_compute = field_name.compute.as_ref().unwrap();
    assert_eq!(
        field_name_compute.depends,
        vec!("age".to_string(), "test".to_string())
    );

    // Again
    field_name.register_internal_field(&InternalField {
        name: "name".to_string(),
        kind: FieldKind::String,
        default_value: None,
        label: None,
        description: Some("This is another description".to_string()),
        required: true,
        private: false,
        asks_for_storage: false,
        compute: Some(FieldCompute {
            method: "compute_it".to_string(),
            depends: vec!["age", "test2"],
        }),
        field_ref: None,
        selection: None,
        tracking: false,
        owned: false,
        on_delete: None,
    });

    assert_eq!(field_name.name, "name");
    assert_eq!(
        field_name.description.as_deref(),
        Some("This is another description")
    );
    assert!(field_name.required);
    assert_eq!(
        field_name.default_value,
        Some(FieldType::String("1ddlyoko".to_string()))
    );
    assert!(field_name.compute.is_some());
    let field_name_compute = field_name.compute.as_ref().unwrap();
    assert_eq!(
        field_name_compute.depends,
        vec!("age".to_string(), "test".to_string(), "test2".to_string())
    );

    // Again
    field_name.register_internal_field(&InternalField {
        name: "name".to_string(),
        kind: FieldKind::String,
        default_value: None,
        label: None,
        description: Some("This is another description".to_string()),
        required: true,
        private: false,
        asks_for_storage: false,
        compute: Some(FieldCompute {
            method: "compute_it".to_string(),
            depends: vec!["age"],
        }),
        field_ref: None,
        selection: None,
        tracking: false,
        owned: false,
        on_delete: None,
    });

    assert_eq!(field_name.name, "name");
    assert_eq!(
        field_name.description.as_deref(),
        Some("This is another description")
    );
    assert!(field_name.required);
    assert_eq!(
        field_name.default_value,
        Some(FieldType::String("1ddlyoko".to_string()))
    );
    assert!(field_name.compute.is_some());
    let field_name_compute = field_name.compute.as_ref().unwrap();
    assert_eq!(
        field_name_compute.depends,
        vec!("age".to_string(), "test".to_string(), "test2".to_string())
    );
}

#[test]
fn test_register_field_without_default_value_is_allowed() {
    let mut field_name = FinalInternalField::new("field_name");

    // The type is carried by `kind`, so a field no longer needs a default to be declarable.
    field_name.register_internal_field(&InternalField {
        name: "name".to_string(),
        kind: FieldKind::String,
        default_value: None,
        label: None,
        description: Some("This is the name".to_string()),
        required: true,
        private: false,
        asks_for_storage: false,
        compute: None,
        field_ref: None,
        selection: None,
        tracking: false,
        owned: false,
        on_delete: None,
    });

    assert_eq!(field_name.kind, FieldKind::String);
    assert_eq!(field_name.default_value, None);

    // Storage is settled once every struct has been seen, which the registry does for a real
    // model. A plain field nobody computes is kept.
    field_name.settle_storage().expect("nothing computes it");
    assert!(field_name.is_stored());
}

/// Asking to keep a field that nothing computes is refused, and only the merged view can tell.
#[test]
fn test_storage_asked_for_a_field_nothing_computes() {
    let mut field = FinalInternalField::new("field_name");
    field.register_internal_field(&InternalField {
        name: "name".to_string(),
        kind: FieldKind::String,
        default_value: None,
        label: None,
        description: None,
        required: true,
        private: false,
        asks_for_storage: true,
        compute: None,
        field_ref: None,
        selection: None,
        tracking: false,
        owned: false,
        on_delete: None,
    });

    let wrong = field.settle_storage().expect_err("nothing computes it");
    assert!(wrong.contains("nothing computes it"), "got {wrong}");
}

#[test]
#[should_panic]
fn test_register_field_with_another_default_type_should_fail() {
    let mut field_name = FinalInternalField::new("field_name");

    field_name.register_internal_field(&InternalField {
        name: "name".to_string(),
        kind: FieldKind::String,
        default_value: Some(FieldType::String("0ddlyoko".to_string())),
        label: None,
        description: Some("This is the name".to_string()),
        required: true,
        private: false,
        asks_for_storage: false,
        compute: None,
        field_ref: None,
        selection: None,
        tracking: false,
        owned: false,
        on_delete: None,
    });

    field_name.register_internal_field(&InternalField {
        name: "name".to_string(),
        kind: FieldKind::Integer,
        default_value: Some(FieldType::Integer(42)),
        label: None,
        description: None,
        required: true,
        private: false,
        asks_for_storage: false,
        compute: None,
        field_ref: None,
        selection: None,
        tracking: false,
        owned: false,
        on_delete: None,
    });
}

#[test]
fn test_register_fields_with_real_model() -> Result<()> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    app.register_plugin(Box::new(TestPlugin {}))
        .expect("Plugin should load");

    app.load_plugin("test_plugin")?;

    // Should exist as the plugin is registered and loaded
    let model = app.model_manager.get_model("sale_order_test");
    let field = model.get_internal_field("name");
    assert_eq!(field.name, "name");
    // Description should be overridden
    assert_eq!(field.description.as_deref(), Some("New name of the SO"));
    assert!(field.compute.is_none());
    assert!(field.required);
    assert_eq!(
        field.default_value, None,
        "text is given by whoever creates the record"
    );

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

    assert!(
        matches!(so_field_inverse.inverse_field.clone(), FieldReferenceType::O2M { inverse_field } if inverse_field == "order")
    );
    assert!(
        matches!(so_line_field_inverse.inverse_field.clone(), FieldReferenceType::M2O { inverse_fields } if inverse_fields.contains(&"lines".to_string()))
    );

    Ok(())
}
