pub mod errors;
mod internal_plugin;
mod plugin_manager;

pub(crate) use internal_plugin::InternalPluginType;
pub(crate) use internal_plugin::InternalPluginState;
pub(crate) use internal_plugin::InternalPlugin;
pub use plugin_manager::PluginManager;

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
