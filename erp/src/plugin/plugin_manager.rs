use crate::plugin::errors::{PluginAlreadyRegisteredError, PluginNotFoundError};
use crate::plugin::InternalPluginState::{Installed, NotInstalled};
use crate::plugin::Plugin;
use crate::plugin::{InternalPlugin, InternalPluginType};
use crate::util::dependency;
use libloading::{Error, Library, Symbol};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::PathBuf;
use std::{env, error, fs};

unsafe fn read_plugin_from_file(path: &PathBuf) -> Result<InternalPlugin, Error> {
    type PluginCreator = unsafe extern "C" fn() -> *mut Box<dyn Plugin>;

    let library = Library::new(path)?;
    let constructor: Symbol<PluginCreator> = library.get(b"_create_plugin")?;
    let boxed_raw = constructor();

    let plugin = *Box::from_raw(boxed_raw);
    let plugin_type = InternalPluginType::Dll(library);
    let depends = plugin.get_depends();

    Ok(InternalPlugin {
        plugin,
        plugin_type,
        depends,
        state: NotInstalled,
    })
}

#[derive(Default)]
pub struct PluginManager {
    pub(crate) plugins: HashMap<String, InternalPlugin>,
}

impl PluginManager {
    pub fn register_plugins(
        &mut self,
        directory_path: &String,
    ) -> Result<(), Box<dyn error::Error>> {
        let dll_extension = env::consts::DLL_EXTENSION;
        let paths = fs::read_dir(directory_path)?;
        for path in paths {
            let path = path?.path();
            let extension = path.extension();
            if extension == Some(OsStr::new(dll_extension)) {
                self.register_plugin_from_file(&path)?;
            }
        }

        Ok(())
    }

    pub fn register_plugin(
        &mut self,
        plugin: Box<dyn Plugin>,
    ) -> Result<(), Box<dyn error::Error>> {
        let plugin_name = plugin.name();
        println!("Registering plugin: {}", plugin_name);
        if self.plugins.contains_key(&plugin_name) {
            return Err(PluginAlreadyRegisteredError {
                plugin_name: plugin_name.to_string(),
            }
            .into());
        }
        let plugin_type = InternalPluginType::Static();
        let depends = plugin.get_depends();
        let internal_plugin = InternalPlugin {
            plugin,
            plugin_type,
            depends,
            state: NotInstalled,
        };
        self.plugins.insert(plugin_name, internal_plugin);

        Ok(())
    }

    pub fn register_plugin_from_file(
        &mut self,
        plugin_path: &PathBuf,
    ) -> Result<(), Box<dyn error::Error>> {
        // TODO Add a custom exception here with the path, if error
        let internal_plugin = unsafe { read_plugin_from_file(plugin_path)? };

        let plugin_name = internal_plugin.plugin.name();
        println!("Registering plugin: {}", plugin_name);
        if self.plugins.contains_key(&plugin_name) {
            let InternalPlugin {
                plugin,
                plugin_type,
                ..
            } = internal_plugin;
            let plugin_name_string = plugin_name.to_string().clone();
            drop(plugin);
            drop(plugin_type);
            return Err(PluginAlreadyRegisteredError {
                plugin_name: plugin_name_string,
            }
            .into());
        }

        self.plugins.insert(plugin_name, internal_plugin);

        Ok(())
    }

    pub(crate) fn load_plugin(
        &mut self,
        plugin_name: &str,
    ) -> Result<&mut InternalPlugin, Box<dyn error::Error>> {
        let plugin = self
            .get_plugin_mut(plugin_name)
            .unwrap_or_else(|| panic!("Plugin {} is not registered", plugin_name));
        println!("Loading plugin: {}", plugin_name);
        plugin.state = Installed;
        Ok(plugin)
    }

    pub(crate) fn unload_plugin(&mut self, plugin_name: &str) {
        let plugin = self.plugins.get_mut(plugin_name);
        let Some(plugin) = plugin else { return };
        plugin.plugin.unload();

        let plugin = self.plugins.remove(plugin_name);
        let Some(internal_plugin) = plugin else {
            return;
        };
        let InternalPlugin {
            plugin,
            plugin_type,
            ..
        } = internal_plugin;

        drop(plugin);
        drop(plugin_type);
    }

    pub(crate) fn unload(&mut self) {
        let plugin_names = self.plugins.keys().cloned().collect::<Vec<_>>();
        for name in plugin_names {
            self.unload_plugin(name.as_str());
        }
    }

    pub(crate) fn get_plugin(&self, plugin_name: &str) -> Option<&InternalPlugin> {
        self.plugins.get(plugin_name)
    }

    pub(crate) fn get_plugin_mut(&mut self, plugin_name: &str) -> Option<&mut InternalPlugin> {
        self.plugins.get_mut(plugin_name)
    }

    pub(crate) fn _get_ordered_dependencies_of_all_plugins(&self) -> Result<Vec<&str>, Box<dyn error::Error>> {
        let plugins = self.plugins.keys().collect::<Vec<_>>();
        self._get_ordered_dependencies(plugins)
    }

    pub(crate) fn _get_ordered_dependencies<'a>(&self, plugins: Vec<&'a String>) -> Result<Vec<&'a str>, Box<dyn error::Error>> {
        let dependencies: Vec<(&'a str, Vec<&str>)> = plugins
            .iter()
            .map(|&plugin_name| {
                let internal_plugin = self.plugins.get(plugin_name);
                if let Some(internal_plugin) = internal_plugin {
                    let depends = internal_plugin
                        .depends
                        .iter()
                        .map(|str| str.as_str())
                        .collect::<Vec<&str>>();
                    Ok((plugin_name.as_str(), depends))
                } else {
                    Err(PluginNotFoundError {
                        plugin_name: plugin_name.to_string(),
                    })
                }
            })
            .collect::<Result<Vec<_>, _>>()?;

        let dependencies = dependencies.into_iter().collect();

        let sorted_dependencies: Result<Vec<&'a str>, Box<dyn error::Error>> = dependency::sort_dependencies(&dependencies);
        sorted_dependencies
    }

    pub fn is_installed(&self, plugin_name: &str) -> bool {
        let plugin = self.plugins.get(plugin_name);
        let Some(plugin) = plugin else {
            return false;
        };
        plugin.state == Installed
    }
}

/// We need to first drop the instance of the plugin, then the library as the instance of the
/// plugin is loaded into the library memory chunk.
impl Drop for PluginManager {
    fn drop(&mut self) {
        self.unload();
    }
}
