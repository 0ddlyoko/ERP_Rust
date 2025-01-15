use code_gen::Model;
use erp::field::EnumType;

#[derive(Debug, PartialEq, Eq, Default, Copy, Clone)]
pub enum PluginState {
    #[default]
    NotInstalled,
    Installed,
}

impl EnumType for PluginState {
    fn to_string(&self) -> String {
        match self {
            PluginState::Installed => String::from("installed"),
            PluginState::NotInstalled => String::from("not_installed"),
        }
    }

    fn from_string(t: &str) -> &Self {
        match t {
            "not_installed" => &PluginState::NotInstalled,
            "installed" => &PluginState::Installed,
            _ => &PluginState::NotInstalled,
        }
    }
}

#[derive(Model)]
#[erp(table_name="plugin")]
pub struct Plugin {
    id: u32,
    name: String,
    description: Option<String>,
    website: Option<String>,
    url: Option<String>,
    // state: PluginState,
    // TODO Add module category
    // TODO Add author
    // TODO Add version (installed, latest, ...) + auto update if new version
}