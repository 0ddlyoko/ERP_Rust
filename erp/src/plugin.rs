pub mod errors;
mod internal_plugin;
mod plugin_manager;

pub(crate) use internal_plugin::InternalPlugin;
pub(crate) use internal_plugin::InternalPluginState;
pub(crate) use internal_plugin::InternalPluginType;
pub use plugin_manager::PluginManager;

use crate::model::ModelManager;
use std::any::Any;
use std::error::Error;
use crate::environment::Environment;

pub trait Plugin: Any + Send + Sync {
    /// Get the name of this plugin
    fn name(&self) -> String;

    /// Pre-Initialize this plugin
    ///
    /// This method is called before models initialized (before the call to init_models)
    fn pre_init(&mut self) {}

    /// Register models created in this plugin
    fn init_models(&self, model_manager: &mut ModelManager);

    /// Post-Initialize this plugin
    ///
    /// This method is called once this plugin is fully initialized (after the call to init_models)
    fn post_init(&mut self, _env: &mut Environment) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    /// Unload this plugin
    fn unload(&mut self) {}

    /// Returns dependencies of this plugin
    fn get_depends(&self) -> Vec<String> {
        Vec::new()
    }
}
