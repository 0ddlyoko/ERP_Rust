use crate::plugin::Plugin;
use libloading::Library;

#[allow(dead_code)]
pub(crate) enum InternalPluginType {
    Dll(Library),
    Static(),
}

#[derive(PartialEq, Eq)]
pub(crate) enum InternalPluginState {
    NotInstalled,
    Installed,
}

pub(crate) struct InternalPlugin {
    pub(crate) plugin: Box<dyn Plugin>,
    pub(crate) plugin_type: InternalPluginType,
    pub(crate) depends: Vec<String>,
    pub(crate) state: InternalPluginState,
}
