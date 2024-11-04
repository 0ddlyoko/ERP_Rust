use std::{error, fmt};

#[derive(Debug, Clone)]
pub struct PluginAlreadyLoadedError {
    pub(crate) plugin_name: String,
}

impl fmt::Display for PluginAlreadyLoadedError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Plugin \"{}\" is already loaded, or a plugin with the same name already exist", self.plugin_name)
    }
}

impl error::Error for PluginAlreadyLoadedError {}
