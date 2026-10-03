use erp_internal_types::{FinalInternalModel, InternalField};
use erp_types::field::{FieldKind, FieldType};

#[test]
fn test_get_fields_name() {
    let mut internal_model = FinalInternalModel::new("");

    internal_model.register_internal_field(&InternalField {
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
    });

    internal_model.register_internal_field(&InternalField {
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
    });

    internal_model.settle_storage().expect("plain fields");

    assert_eq!(
        {
            let mut fields = internal_model.get_fields_name();
            fields.sort();
            fields
        },
        vec!["age", "name"]
    );
    assert_eq!(internal_model.get_missing_fields(vec!["age"]), vec!["name"]);
}
