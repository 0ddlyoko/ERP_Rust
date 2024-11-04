use libloading::Library;
use crate::plugin::Plugin;

pub(crate) struct InternalPlugin {
    pub(crate) plugin: Box<dyn Plugin>,
    pub(crate) library: Library,
}
