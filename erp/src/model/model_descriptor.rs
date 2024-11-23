use crate::field::FieldDescriptor;

pub struct ModelDescriptor {
    pub name: String,
    pub description: Option<String>,
    pub fields: Vec<FieldDescriptor>,
}

impl ModelDescriptor {
    pub fn new(name: String) -> Self {
        let description = Some(name.clone());
        ModelDescriptor {
            name,
            description,
            fields: Vec::new(),
        }
    }
}