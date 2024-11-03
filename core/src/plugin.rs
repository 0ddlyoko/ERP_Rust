pub mod plugin_manager;

use std::any::Any;
use crate::model::ModelManager;

pub trait Plugin {
    fn name(&self) -> &'static str;
    fn init(&mut self);
    fn init_models(&self, model_manager: &ModelManager);
}
