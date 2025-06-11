

#[derive(Clone)]
pub struct FieldReference {
    // TODO Use &'static instead of String
    pub target_model: String,
    pub inverse_field: FieldReferenceType,
}

#[derive(Clone)]
pub enum FieldReferenceType {
    O2M { inverse_field: String },
    // If it's a M2O, this list will only be empty if there is no fields in the target model that is the linked O2M of this field
    M2O { inverse_fields: Vec<String> },
}
