
use core::field::FieldType;
use core::internal::internal_field::InternalField;
use core::internal::internal_model::FinalInternalModel;
use test_utilities::models::test_model::TestEmptyModel;

#[test]
fn test_get_fields_name() {
    let mut internal_model = FinalInternalModel::new("".to_string());

    internal_model.register_internal_field(&InternalField {
        name: "name".to_string(),
        default_value: Some(FieldType::String("0ddlyoko".to_string())),
        description: Some("This is the name".to_string()),
        required: None,
        compute: None,
    });

    internal_model.register_internal_field(&InternalField {
        name: "age".to_string(),
        default_value: Some(FieldType::Integer(42)),
        description: Some("This is the age of the person".to_string()),
        required: None,
        compute: None,
    });

    assert_eq!({
        let mut fields = internal_model.get_fields_name();
        fields.sort();
        fields
    }, vec!["age", "name"]);
    assert_eq!(internal_model.get_missing_fields(vec!["age"]), vec!["name"]);
}