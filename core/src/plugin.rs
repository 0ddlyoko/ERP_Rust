pub mod errors;
pub mod internal_plugin;
pub mod plugin_manager;

use std::any::Any;
use crate::model::ModelManager;

pub trait Plugin: Any + Send + Sync {
    fn name(&self) -> String;
    fn init(&mut self) {}
    fn init_models(&self, model_manager: &mut ModelManager);
    fn unload(&mut self) {}
    fn get_depends(&self) -> Vec<String> {
        Vec::new()
    }
}
