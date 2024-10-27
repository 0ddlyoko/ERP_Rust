use crate::environment::Environment;
use crate::internal::internal_model::FinalInternalModel;
use crate::model::{MapOfFields, Model};
use std::collections::HashMap;

pub struct ModelManager {
    models: HashMap<&'static str, FinalInternalModel>,
}

impl ModelManager {
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
        }
    }

    pub fn register_model<M>(&mut self) where M: Model + 'static {
        let model_name = M::get_model_name();

        self.models.entry(model_name).or_insert_with(|| {
            FinalInternalModel::new(model_name)
        }).register_internal_model::<M>();
    }

    pub fn create_instance_from_name(&self, model_name: &str, id: u32, data: MapOfFields) -> Option<Box<dyn Model>> {
        self.models.get(model_name).and_then(|model| {
            Option::from((model.first().create_instance)(id, data))
        })
    }

    pub fn create_instance<M>(&self, id: u32, data: MapOfFields) -> Option<M> where M: Model + 'static {
        Some(M::create_model(id, data))
    }

    pub fn new_environment(&self) -> Environment {
        Environment::new(self)
    }
}
