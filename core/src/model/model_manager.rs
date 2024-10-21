use std::collections::HashMap;
use crate::model::{MapOfFields, Model, ModelDescriptor};


struct InternalModel {
    model_name: String,
    model_descriptor: ModelDescriptor,
    create_instance: fn(u32, MapOfFields) -> Box<dyn Model>,
}

pub struct ModelManager {
    models: HashMap<String, Vec<InternalModel>>,
}

impl ModelManager {
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
        }
    }

    pub fn register_model<M>(&mut self) where M: Model + 'static {
        let model_name = M::get_model_name();
        let model_descriptor = M::get_model_descriptor();

        let create_instance: fn(u32, MapOfFields) -> Box<dyn Model> = |id, data| Box::new(M::create_model(id, data));

        let internal_model = InternalModel {
            model_name,
            model_descriptor,
            create_instance,
        };

        self.models.entry(internal_model.model_name.clone()).or_insert_with(Vec::new).push(internal_model);
    }

    pub fn create_instance_from_name(&self, model_name: &str, id: u32, data: MapOfFields) -> Option<Box<dyn Model>> {
        self.models.get(model_name).and_then(|model| {
            model.first().map(|internal_model| {
                (internal_model.create_instance)(id, data)
            })
        })
    }

    pub fn create_instance<M>(&self, id: u32, data: MapOfFields) -> Option<M> where M: Model + 'static {
        Some(M::create_model(id, data))
    }
}
