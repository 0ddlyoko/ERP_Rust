use crate::model::ModelManager;

pub trait Plugin {
    fn init(&mut self);
    fn init_models(&self, model_manager: &ModelManager);
}
