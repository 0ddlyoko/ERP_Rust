

// Path for a single depend
#[derive(Clone, Debug, PartialEq)]
pub enum FieldDepend {
    /// Field on the current model
    ///
    /// This is always the last in the list
    SameModel { field_name: String },
    /// O2M field from the model you are now.
    ///
    /// "target_field" from model "target_model" is a M2O
    AnotherModel {
        target_model: String,
        target_field: String,
    },
    /// M2O field
    ///
    /// target_model is the model the M2O field is targeting, and field_name is the field from the
    /// model you are currently now
    CurrentFieldAnotherModel {
        target_model: String,
        field_name: String,
    },
}
