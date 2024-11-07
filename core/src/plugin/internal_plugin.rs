use libloading::Library;
use crate::plugin::Plugin;

pub(crate) enum InternalPluginType {
    Dll(Library),
    Static(),
}

pub(crate) struct InternalPlugin {
    pub(crate) plugin: Box<dyn Plugin>,
    pub(crate) plugin_type: InternalPluginType,
}
