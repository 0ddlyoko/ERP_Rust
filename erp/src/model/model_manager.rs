use crate::field::MultipleIds;
use crate::internal::internal_model::{FinalInternalModel, InternalModel};
use crate::model::Model;
use std::collections::HashMap;

#[derive(Default)]
pub struct ModelManager {
    models: HashMap<String, FinalInternalModel>,
    pub(crate) current_plugin_loading: Option<String>,
}

impl ModelManager {
    pub fn register_model<M>(&mut self)
    where
        M: Model<MultipleIds> + 'static,
    {
        let plugin_name = match &self.current_plugin_loading {
            Some(plugin_name) => plugin_name,
            None => "Unknown",
        };
        let model_name = M::get_model_name();

        self.models
            .entry(model_name.to_string())
            .or_insert_with(|| FinalInternalModel::new(model_name))
            .register_internal_model::<M>(plugin_name);
    }

    pub fn get_models(&self) -> &HashMap<String, FinalInternalModel> {
        &self.models
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

    /// Retrieves all models created by a specific plugin
    pub fn get_all_models_for_plugin(&self, plugin_name: &str) -> Vec<&InternalModel> {
        let mut result = vec![];
        for model in self.models.values() {
            result.extend(model.get_all_models_for_plugin(plugin_name));
        }
        result
    }
}
