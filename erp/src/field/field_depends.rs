

// Path for a single depend
#[derive(Clone, Debug, PartialEq)]
pub enum FieldDepend {
    SameModel { field_name: String },
    AnotherModel {
        target_model: String,
        target_field: String,
    },
    AnotherModel2 {
        target_model: String,
        field_name: String,
    },
}
