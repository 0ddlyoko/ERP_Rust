use std::{error, fmt};

#[derive(Debug, Clone)]
pub struct PluginAlreadyRegisteredError {
    pub plugin_name: String,
}

impl fmt::Display for PluginAlreadyRegisteredError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Plugin \"{}\" is already registered, or a plugin with the same name already exist", self.plugin_name)
    }
}

impl error::Error for PluginAlreadyRegisteredError {}
