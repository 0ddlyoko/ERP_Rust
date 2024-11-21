use crate::environment::Environment;
use crate::model::MapOfFields;
use std::collections::HashMap;
use crate::internal::internal_model::FinalInternalModel;
use crate::model::Model;

#[derive(Default)]
pub struct ModelManager {
    models: HashMap<String, FinalInternalModel>,
}

impl ModelManager {
    pub fn register_model<M>(&mut self) where M: Model + Default + 'static {
        let model_name = M::get_model_name();

        self.models.entry(model_name.to_string()).or_insert_with(|| {
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
    
    pub fn get_model(&self, model_name: &str) -> Option<&FinalInternalModel> {
        self.models.get(model_name)
    }
    
    pub fn get_model_mut(&mut self, model_name: &str) -> Option<&mut FinalInternalModel> {
        self.models.get_mut(model_name)
    }
}