// Path for a single depend
#[derive(Clone, Debug, PartialEq)]
pub enum FieldDepend {
    /// Field on the current model
    ///
    /// This is always the last in the list
    SameModel { field_name: String },
    /// O2M field from the model you are now.
    AnotherModel {
        /// Model the O2M field is from
        target_model: String,
        /// M2O field from the model you are now
        target_field: String,
    },
    /// M2O field
    CurrentFieldAnotherModel {
        /// Model the M2O field is targeting
        target_model: String,
        /// Field from the model you are currently now
        field_name: String,
    },
}
