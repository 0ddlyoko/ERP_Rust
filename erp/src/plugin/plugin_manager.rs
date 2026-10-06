use crate::plugin::InternalPluginState::{Installed, NotInstalled};
use crate::plugin::Plugin;
use crate::plugin::errors::{
    PluginAlreadyRegisteredError, PluginBuildMismatchError, PluginLoadError, PluginNotFoundError,
};
use crate::plugin::{InternalPlugin, InternalPluginType};
use crate::util::dependency;
use libloading::{Library, Symbol};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::{env, error, fs};

/// The symbol a plugin library exports its plugin under, from the library's file name.
///
/// `libweb.so`, `libweb.dylib` and `web.dll` all come from the crate `web`, whose
/// `export_plugin!` defines `erp_create_plugin_web`. One name per crate, so that plugins linked
/// together never define the same symbol twice.
pub fn plugin_symbol(path: &Path) -> String {
    format!("erp_create_plugin_{}", crate_of(path))
}

/// The symbol a plugin library says which build of `erp` it carries under: `erp_plugin_build_web`.
pub fn plugin_build_symbol(path: &Path) -> String {
    format!("erp_plugin_build_{}", crate_of(path))
}

fn crate_of(path: &Path) -> String {
    let stem = path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let crate_name = match env::consts::DLL_PREFIX {
        "" => stem.as_str(),
        prefix => stem.strip_prefix(prefix).unwrap_or(&stem),
    };
    crate_name.replace('-', "_")
}

/// Open a plugin library, and create its plugin.
///
/// Its build of `erp` is checked before any of its code runs: a plugin from another build would
/// fail later and far from the cause, and a panic in a library cannot even reach the application —
/// each carries its own standard library, so the process aborts instead.
unsafe fn read_plugin_from_file(
    path: &PathBuf,
) -> Result<InternalPlugin, Box<dyn error::Error + Send + Sync>> {
    type PluginCreator = unsafe extern "C" fn() -> *mut Box<dyn Plugin>;
    type BuildId = unsafe extern "C" fn() -> u64;

    let library = unsafe { Library::new(path)? };
    let build: Symbol<BuildId> = unsafe { library.get(plugin_build_symbol(path).as_bytes())? };
    if unsafe { build() } != crate::plugin::build_id() {
        return Err(PluginBuildMismatchError { path: path.clone() }.into());
    }
    let symbol = plugin_symbol(path);
    let constructor: Symbol<PluginCreator> = unsafe { library.get(symbol.as_bytes())? };
    let boxed_raw = unsafe { constructor() };

    let plugin = unsafe { *Box::from_raw(boxed_raw) };
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
    /// Register the plugin libraries found in `directory_path`; none when it is empty, as for an
    /// application whose plugins are all registered by hand.
    pub fn register_plugins(
        &mut self,
        directory_path: &String,
    ) -> Result<(), Box<dyn error::Error + Send + Sync>> {
        if directory_path.is_empty() {
            return Ok(());
        }
        tracing::info!(directory = %directory_path, "Registering plugins from directory");
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
    ) -> Result<(), Box<dyn error::Error + Send + Sync>> {
        let plugin_name = plugin.name();
        tracing::debug!(plugin = %plugin_name, "Registering plugin");
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
    ) -> Result<(), Box<dyn error::Error + Send + Sync>> {
        let internal_plugin =
            unsafe { read_plugin_from_file(plugin_path) }.map_err(|error| match error
                .downcast::<libloading::Error>(
            ) {
                Ok(source) => PluginLoadError {
                    path: plugin_path.clone(),
                    source: *source,
                }
                .into(),
                Err(error) => error,
            })?;

        let plugin_name = internal_plugin.plugin.name();
        tracing::debug!(plugin = %plugin_name, "Registering plugin");
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
    ) -> Result<&mut InternalPlugin, Box<dyn error::Error + Send + Sync>> {
        let plugin = self
            .get_plugin_mut(plugin_name)
            .unwrap_or_else(|| panic!("Plugin {} is not registered", plugin_name));
        tracing::info!(plugin = %plugin_name, "Loading plugin");
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

    pub(crate) fn _get_ordered_dependencies_of_all_plugins(
        &self,
    ) -> Result<Vec<&str>, Box<dyn error::Error + Send + Sync>> {
        let plugins = self.plugins.keys().collect::<Vec<_>>();
        self._get_ordered_dependencies(plugins)
    }

    pub(crate) fn _get_ordered_dependencies<'a>(
        &self,
        plugins: Vec<&'a String>,
    ) -> Result<Vec<&'a str>, Box<dyn error::Error + Send + Sync>> {
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

        let sorted_dependencies: Result<Vec<&'a str>, Box<dyn error::Error + Send + Sync>> =
            dependency::sort_dependencies(&dependencies);
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
