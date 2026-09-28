pub mod errors;
mod internal_plugin;
mod plugin_manager;
mod record;

pub(crate) use internal_plugin::InternalPlugin;
pub(crate) use internal_plugin::InternalPluginState;
pub(crate) use internal_plugin::InternalPluginType;
pub use plugin_manager::PluginManager;
pub(crate) use record::{installed_version, record_plugin};

use crate::environment::Environment;
use crate::http::ControllerRegistry;
use crate::model::ModelManager;
use std::any::Any;
use std::error::Error;

/// What a plugin says about itself, recorded in the database for whoever lists plugins.
///
/// `version` is the version of the code being loaded; the one last installed is kept beside it,
/// so a change between the two is what an update looks like.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PluginInfo {
    pub description: Option<String>,
    pub author: Option<String>,
    pub category: Option<String>,
    pub website: Option<String>,
    pub version: Option<String>,
}

pub trait Plugin: Any + Send + Sync {
    /// Get the name of this plugin
    fn name(&self) -> String;

    /// What this plugin says about itself. Nothing, unless it says otherwise.
    fn info(&self) -> PluginInfo {
        PluginInfo::default()
    }

    /// Pre-Initialize this plugin
    ///
    /// This method is called before models initialized (before the call to init_models)
    fn pre_init(&mut self) {}

    /// Register models created in this plugin
    fn init_models(&self, model_manager: &mut ModelManager);

    /// Register the controllers this plugin declares or extends. Called right after
    /// [`Plugin::init_models`], so a controller extending another finds it already registered.
    fn init_controllers(&self, _controllers: &mut ControllerRegistry) {}

    /// XML documents this plugin ships.
    ///
    /// Returned as contents rather than paths, through `include_str!`: a plugin is a dynamic
    /// library with no reliable base directory at runtime, so embedding removes the problem
    /// instead of working around it.
    fn data(&self) -> Vec<&'static str> {
        Vec::new()
    }

    /// Post-Initialize this plugin
    ///
    /// This method is called once this plugin is fully initialized (after the call to init_models)
    fn post_init(&mut self, _env: &mut Environment) -> Result<(), Box<dyn Error + Send + Sync>> {
        Ok(())
    }

    /// Unload this plugin
    fn unload(&mut self) {}

    /// Returns dependencies of this plugin
    fn get_depends(&self) -> Vec<String> {
        Vec::new()
    }
}
