use std::{error, fmt};

#[derive(Debug, Clone)]
pub struct PluginAlreadyRegisteredError {
    pub(crate) plugin_name: String,
}

impl fmt::Display for PluginAlreadyRegisteredError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Plugin \"{}\" is already registered, or a plugin with the same name already exist",
            self.plugin_name
        )
    }
}

impl error::Error for PluginAlreadyRegisteredError {}


#[derive(Debug, Clone)]
pub struct PluginNotFoundError {
    pub(crate) plugin_name: String,
}

impl fmt::Display for PluginNotFoundError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Plugin \"{}\" doesn't exist. Please check if it's in the plugin path",
            self.plugin_name
        )
    }
}

impl error::Error for PluginNotFoundError {}
