pub mod errors;
mod internal_plugin;
mod plugin_manager;
mod record;

pub(crate) use internal_plugin::InternalPlugin;
pub(crate) use internal_plugin::InternalPluginState;
pub(crate) use internal_plugin::InternalPluginType;
pub use plugin_manager::{PluginManager, plugin_build_symbol, plugin_symbol};
pub(crate) use record::{installed_version, record_plugin};

use crate::assets::{BundleContribution, StaticFiles, TemplateFiles};
use crate::environment::Environment;
use crate::http::ControllerRegistry;
use crate::model::ModelManager;
use std::any::Any;
use std::error::Error;

/// Identifies the build of `erp` the calling code was compiled against.
///
/// Every plugin library carries its own copy of `erp`, and a type is only the same type across
/// libraries when those copies come from one build: Cargo builds `erp` again whenever the features
/// around it differ, and each build gives identically named types different identities. A plugin
/// from another build looks fine until it extends another plugin's method and the signatures, the
/// same on paper, do not match — so the loader compares this first, and refuses the library.
pub fn build_id() -> u64 {
    struct Build;
    let mut hasher = std::hash::DefaultHasher::new();
    std::hash::Hash::hash(&std::any::TypeId::of::<Build>(), &mut hasher);
    std::hash::Hasher::finish(&hasher)
}

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

    /// The files this plugin serves, embedded when it was compiled:
    /// `include!(concat!(env!("OUT_DIR"), "/static_files.rs"))` then `STATIC_FILES`, from a build
    /// script calling `erp_assets_build::compile`. Reachable as `<plugin>/static/<path>`.
    fn static_files(&self) -> StaticFiles {
        &[]
    }

    /// The templates this plugin renders on the server, embedded when it was compiled:
    /// `include!(concat!(env!("OUT_DIR"), "/template_files.rs"))` then `TEMPLATE_FILES`, from a
    /// build script calling `erp_assets_build::templates`. Never served to a browser.
    fn template_files(&self) -> TemplateFiles {
        &[]
    }

    /// What this plugin adds to which bundle: globs over public paths, its own files or other
    /// plugins'. Counted only while the plugin is installed.
    fn assets(&self) -> Vec<BundleContribution> {
        Vec::new()
    }

    /// Names scripts import instead of a path, and the public path each stands for:
    /// `("trame", "web/static/lib/trame.js")` lets any plugin write `import … from "trame"`.
    fn imports(&self) -> Vec<(&'static str, &'static str)> {
        Vec::new()
    }

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

    /// Whether this plugin installs itself once every plugin it depends on is installed.
    ///
    /// For the glue between plugins, which only makes sense when they are all there: nobody has
    /// to remember to install it, and it never lands without what it glues.
    fn auto_install(&self) -> bool {
        false
    }
}
