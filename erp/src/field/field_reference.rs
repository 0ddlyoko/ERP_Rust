

#[derive(Clone)]
pub struct FieldReference {
    // TODO Use &'static instead of String
    pub target_model: String,
    pub inverse_field: Option<String>,
}
