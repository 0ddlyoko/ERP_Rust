use libloading::Library;
use crate::plugin::Plugin;

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
    pub(crate) depends: Vec<&'static str>,
    pub(crate) state: InternalPluginState,
}
