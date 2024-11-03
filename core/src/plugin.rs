pub mod plugin_manager;

use std::any::Any;
use crate::model::ModelManager;

pub trait Plugin: Any + Send + Sync {
    fn name(&self) -> &'static str;
    fn init(&mut self);
    fn init_models(&self, model_manager: &ModelManager);
}
