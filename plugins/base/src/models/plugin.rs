use code_gen::{Model, selection};
use erp::types::field::IdMode;

#[selection]
pub enum PluginState {
    #[default]
    NotInstalled,
    Installed,
}

#[derive(Model)]
#[erp(id = "plugin")]
#[allow(dead_code)]
pub struct Plugin<Mode: IdMode> {
    id: Mode,
    name: String,
    description: Option<String>,
    website: Option<String>,
    url: Option<String>,
    #[erp(tracking)]
    state: PluginState,
    category: Option<String>,
    author: Option<String>,
    installed_version: Option<String>,
    latest_version: Option<String>,
}
