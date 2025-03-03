use std::any::TypeId;

#[derive(Clone)]
pub struct FieldCompute {
    pub type_id: TypeId,
    // TODO Use &'static instead of String
    pub depends: Vec<String>,
}
