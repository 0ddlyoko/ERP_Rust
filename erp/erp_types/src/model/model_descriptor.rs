use crate::field::FieldDescriptor;

pub struct ModelDescriptor {
    /// Identity of the model, used everywhere but the database layer.
    pub name: String,
    /// Physical table backing the model. Defaults to [`ModelDescriptor::name`].
    pub table_name: String,
    pub description: Option<String>,
    /// The field naming a record, when it is not `name`.
    pub name_field: Option<String>,
    pub fields: Vec<FieldDescriptor>,
}

impl ModelDescriptor {
    pub fn new(name: String) -> Self {
        let description = Some(name.clone());
        ModelDescriptor {
            table_name: name.clone(),
            name,
            description,
            name_field: None,
            fields: Vec::new(),
        }
    }
}
