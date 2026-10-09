use crate::field::FieldDescriptor;

pub struct ModelDescriptor {
    /// Identity of the model, used everywhere but the database layer.
    pub name: String,
    /// Physical table backing the model. Defaults to [`ModelDescriptor::name`].
    pub table_name: String,
    pub description: Option<String>,
    /// The field naming a record, when it is not `name`.
    pub name_field: Option<String>,
    /// How the records come when nobody asks for an order: `date_order desc, id desc`.
    pub order: Option<String>,
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
            order: None,
            fields: Vec::new(),
        }
    }
}
