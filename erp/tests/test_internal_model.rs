use erp_internal_types::{FinalInternalModel, InternalField};
use erp_types::field::{FieldKind, FieldType};
use std::any::TypeId;
use std::error::Error;

/// Stand-in for a real compute: this test exercises registration, not execution.
fn no_compute(
    _field: &str,
    _ids: erp_types::field::MultipleIds,
    _env: &mut dyn erp_types::environment::ErasedEnvironment,
    _parent: erp_types::field::Super,
) -> std::result::Result<(), Box<dyn Error + Send + Sync>> {
    Ok(())
}

#[test]
fn test_get_fields_name() {
    let type_id = TypeId::of::<InternalField>();
    let mut internal_model = FinalInternalModel::new("");

    internal_model.register_internal_field(
        &InternalField {
            name: "name".to_string(),
            kind: FieldKind::String,
            default_value: Some(FieldType::String("0ddlyoko".to_string())),
            description: Some("This is the name".to_string()),
            required: true,
            compute: None,
            field_ref: None,
        },
        &type_id,
        no_compute,
    );

    internal_model.register_internal_field(
        &InternalField {
            name: "age".to_string(),
            kind: FieldKind::Integer,
            default_value: Some(FieldType::Integer(42)),
            description: Some("This is the age of the person".to_string()),
            required: false,
            compute: None,
            field_ref: None,
        },
        &type_id,
        no_compute,
    );

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
