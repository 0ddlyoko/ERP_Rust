use crate::field::FieldDescriptor;
use crate::model::Model;

pub struct ModelDescriptor<M> where M: Model + Default {
    pub name: String,
    pub description: Option<String>,
    pub fields: Vec<FieldDescriptor<M>>,
}

impl<M> ModelDescriptor<M> where M: Model + Default {
    pub fn new(name: String) -> Self {
        let description = Some(name.clone());
        ModelDescriptor {
            name,
            description,
            fields: Vec::new(),
        }
    }
}