use crate::environment::Environment;
use crate::field::MultipleIds;
use crate::internal::internal_model::FinalInternalModel;
use crate::model::Model;
use std::collections::HashMap;

#[derive(Default)]
pub struct ModelManager {
    models: HashMap<String, FinalInternalModel>,
}

impl ModelManager {
    pub fn register_model<M>(&mut self)
    where
        M: Model<MultipleIds> + 'static,
    {
        let model_name = M::get_model_name();

        self.models
            .entry(model_name.to_string())
            .or_insert_with(|| FinalInternalModel::new(model_name))
            .register_internal_model::<M>();
    }

    pub fn get_models(&self) -> &HashMap<String, FinalInternalModel> {
        &self.models
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

    pub fn is_valid_model(&self, model_name: &str) -> bool {
        self.models.contains_key(model_name)
    }
}
