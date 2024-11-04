use crate::plugin::Plugin;
use libloading::{Error, Library, Symbol};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use crate::plugin::errors::PluginAlreadyLoadedError;
use crate::plugin::internal_plugin::InternalPlugin;

pub struct PluginManager {
    pub plugins: HashMap<&'static str, InternalPlugin>,
}

impl Default for PluginManager {
    fn default() -> Self {
        PluginManager {
            plugins: HashMap::new(),
        }
    }
}

impl PluginManager {

    pub fn load_plugins(&mut self, directory_path: String) -> Result<(), Box<dyn std::error::Error>> {
        let paths = fs::read_dir(directory_path).unwrap();
        for path in paths {
            let path = path.unwrap().path();
            self.add_plugin(&path)?;
        }

        Ok(())
    }

    unsafe fn read_plugin_from_file(&mut self, path: &PathBuf) -> Result<InternalPlugin, Error> {
        type PluginCreator = unsafe extern "C" fn() -> *mut Box<dyn Plugin>;

        let library = Library::new(path)?;

        let constructor: Symbol<PluginCreator> = library.get(b"_create_plugin")?;
        let boxed_raw = constructor();
        let plugin = *Box::from_raw(boxed_raw);

        Ok(InternalPlugin {
            plugin,
            library,
        })
    }

    fn add_plugin(&mut self, plugin_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let internal_plugin = unsafe { self.read_plugin_from_file(plugin_path)? };

        let plugin_name = internal_plugin.plugin.name();
        if self.plugins.contains_key(plugin_name) {
            let InternalPlugin { plugin, library } = internal_plugin;
            let plugin_name_string = plugin_name.to_string().clone();
            drop(plugin);
            drop(library);
            return Err(PluginAlreadyLoadedError { plugin_name: plugin_name_string }.into());
        }

        self.plugins.insert(plugin_name, internal_plugin);
        let plugin = self.plugins.get_mut(plugin_name).unwrap();
        plugin.plugin.init();

        Ok(())
    }
}

/// We need to first drop the instance of the plugin, then the library as the instance of the
/// plugin is loaded into the library memory chunk.
impl Drop for PluginManager {
    fn drop(&mut self) {
        while let Some((_, internal_plugin)) = self.plugins.drain().next() {
            let InternalPlugin { plugin, library } = internal_plugin;

            drop(plugin);
            drop(library);
        }
    }
}
