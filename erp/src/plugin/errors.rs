use thiserror::Error;

#[derive(Debug, Clone, Error)]
#[error(
    "Plugin \"{plugin_name}\" is already registered, or a plugin with the same name already exist"
)]
pub struct PluginAlreadyRegisteredError {
    pub(crate) plugin_name: String,
}

#[derive(Debug, Clone, Error)]
#[error("Plugin \"{plugin_name}\" doesn't exist. Please check if it's in the plugin path")]
pub struct PluginNotFoundError {
    pub(crate) plugin_name: String,
}

/// A plugin file that could not be opened as one: not a library, built for another platform, or
/// missing the entry point every plugin exports.
#[derive(Debug, Error)]
#[error("Cannot load plugin {}: {source}", .path.display())]
pub struct PluginLoadError {
    pub path: std::path::PathBuf,
    pub source: libloading::Error,
}

/// A plugin library compiled against another build of `erp` than the application loading it.
///
/// Types would not be the same types across the two, however alike they are named. Building the
/// server and its plugins with one Cargo command gives them one build of `erp`.
#[derive(Debug, Error)]
#[error(
    "Plugin {} was compiled against another build of erp than this application. Build the \
     application and its plugins with the same cargo command.",
    .path.display()
)]
pub struct PluginBuildMismatchError {
    pub path: std::path::PathBuf,
}
