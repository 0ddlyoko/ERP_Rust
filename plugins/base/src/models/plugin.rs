use code_gen::Model;
use erp::field::{EnumType, IdMode};

#[derive(Debug, PartialEq, Eq, Default, Copy, Clone)]
pub enum PluginState {
    #[default]
    NotInstalled,
    Installed,
}

impl From<PluginState> for &'static str {
    fn from(value: PluginState) -> &'static str {
        match value {
            PluginState::Installed => "installed",
            PluginState::NotInstalled => "not_installed",
        }
    }
}

impl From<&str> for &PluginState {
    fn from(value: &str) -> Self {
        match value {
            "not_installed" => &PluginState::NotInstalled,
            "installed" => &PluginState::Installed,
            _ => &PluginState::NotInstalled,
        }
    }
}

impl EnumType for PluginState {
}

#[derive(Model)]
#[erp(table_name="plugin")]
#[allow(dead_code)]
pub struct Plugin<Mode: IdMode> {
    id: Mode,
    name: String,
    description: Option<String>,
    website: Option<String>,
    url: Option<String>,
    state: PluginState,
    // TODO Add plugin category
    // TODO Add author
    // TODO Add version (installed, latest, ...) + auto update if new version
}