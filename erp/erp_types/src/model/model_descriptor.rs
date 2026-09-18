use crate::field::FieldDescriptor;

pub struct ModelDescriptor {
    /// Identity of the model, used everywhere but the database layer.
    pub name: String,
    /// Physical table backing the model. Defaults to [`ModelDescriptor::name`].
    pub table_name: String,
    pub description: Option<String>,
    pub fields: Vec<FieldDescriptor>,
}

impl ModelDescriptor {
    pub fn new(name: String) -> Self {
        let description = Some(name.clone());
        ModelDescriptor {
            table_name: name.clone(),
            name,
            description,
            fields: Vec::new(),
        }
    }
}
