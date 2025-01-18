use code_gen::Model;
use erp::field::EnumType;

#[derive(Debug, PartialEq, Eq, Default, Copy, Clone)]
pub enum PluginState {
    #[default]
    NotInstalled,
    Installed,
}

impl<'a> From<PluginState> for &'a str {
    fn from(value: PluginState) -> &'a str {
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
pub struct Plugin {
    id: u32,
    name: String,
    description: Option<String>,
    website: Option<String>,
    url: Option<String>,
    state: PluginState,
    // TODO Add module category
    // TODO Add author
    // TODO Add version (installed, latest, ...) + auto update if new version
}