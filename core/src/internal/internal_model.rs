use crate::internal::internal_field::{FinalInternalField, InternalField};
use crate::model::{MapOfFields, Model};
use std::any::TypeId;
use std::collections::HashMap;
use crate::field::FieldDescriptor;

/// Model descriptor represented by a single struct model
pub(crate) struct InternalModel {
    pub name: &'static str,
    pub description: Option<String>,
    pub fields: HashMap<&'static str, InternalField>,
    pub create_instance: fn(u32, MapOfFields) -> Box<dyn Model>,
}

/// Final descriptor of a model.
///
/// Represent all combined InternalModel
pub(crate) struct FinalInternalModel {
    pub name: &'static str,
    pub description: String,
    pub models: HashMap<TypeId, InternalModel>,
    pub fields: HashMap<&'static str, FinalInternalField>,
}

impl FinalInternalModel {
    pub fn new(model_name: &'static str) -> FinalInternalModel {
        FinalInternalModel {
            name: model_name,
            description: "".to_string(),
            models: HashMap::new(),
            fields: HashMap::new(),
        }
    }

    pub fn register_internal_model<M>(&mut self) where M: Model + 'static {
        let name = M::get_model_name();
        let model_descriptor = M::get_model_descriptor();
        let type_id = TypeId::of::<M>();

        let description = model_descriptor.description;
        let mut fields = HashMap::new();
        for field in &model_descriptor.fields {
            fields.insert(field.name, InternalField {
                name: field.name,
                default_value: field.default_value.clone(),
                description: field.description.clone(),
                required: field.required,
            });
            self.register_internal_field(field);
        }

        let create_instance: fn(u32, MapOfFields) -> Box<dyn Model> = |id, data| Box::new(M::create_model(id, data));

        let internal_model = InternalModel {
            name,
            description,
            fields,
            create_instance,
        };
        
        if let Some(description) = &internal_model.description {
            self.description = description.clone();
        }
        self.models.insert(type_id, internal_model);
    }

    fn register_internal_field(&mut self, field_descriptor: &FieldDescriptor) {
        let name = field_descriptor.name;
        let internal_field = self.fields.entry(name).or_insert_with(|| { FinalInternalField::new(name) });
        internal_field.register_internal_field(field_descriptor);
    }

    pub fn first(&self) -> &InternalModel {
        if let Some(first_value) = self.models.values().next() {
            first_value
        } else {
            panic!("Not a single model is present");
        }
    }

    pub fn get_internal_model<M>(&self) -> &InternalModel where M: Model + 'static {
        let type_id = TypeId::of::<M>();
        self.models.get(&type_id).expect("Internal model not registered")
    }

    pub fn get_internal_model_mut<M>(&mut self) -> &mut InternalModel where M: Model + 'static {
        let type_id = TypeId::of::<M>();
        self.models.get_mut(&type_id).expect("Internal model not registered")
    }
}
