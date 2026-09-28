/// Where a relational field points.
///
/// `target_model` is a name the model's code declares, so it lives as long as that code — which
/// the registry never outlives: it is cleared before any plugin library is unloaded.
#[derive(Clone)]
pub struct FieldReference {
    pub target_model: &'static str,
    pub inverse_field: FieldReferenceType,
}

#[derive(Clone)]
pub enum FieldReferenceType {
    O2M {
        inverse_field: String,
    },
    // If it's a M2O, this list will only be empty if there is no fields in the target model that is the linked O2M of this field
    M2O {
        inverse_fields: Vec<String>,
    },
    /// Both sides live in a table of pairs rather than in a column.
    ///
    /// The two models must name the same `relation`; each sees its own id under `column` and the
    /// other's under `target_column`.
    M2M {
        relation: String,
        column: String,
        target_column: String,
    },
}

impl FieldReferenceType {
    /// Column naming a model's own id inside a relation table.
    pub fn relation_column(model_name: &str) -> String {
        format!("{model_name}_id")
    }
}
