use std::any::TypeId;
use erp::field::FieldType;
use erp::internal::internal_field::InternalField;
use erp::internal::internal_model::FinalInternalModel;

#[test]
fn test_get_fields_name() {
    let type_id = TypeId::of::<InternalField>();
    let mut internal_model = FinalInternalModel::new("".to_string());

    internal_model.register_internal_field(&InternalField {
        name: "name".to_string(),
        default_value: Some(FieldType::String("0ddlyoko".to_string())),
        description: Some("This is the name".to_string()),
        required: None,
        compute: None,
    }, &type_id);

    internal_model.register_internal_field(&InternalField {
        name: "age".to_string(),
        default_value: Some(FieldType::Integer(42)),
        description: Some("This is the age of the person".to_string()),
        required: None,
        compute: None,
    }, &type_id);

    assert_eq!({
        let mut fields = internal_model.get_fields_name();
        fields.sort();
        fields
    }, vec!["age", "name"]);
    assert_eq!(internal_model.get_missing_fields(vec!["age"]), vec!["name"]);
}