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
