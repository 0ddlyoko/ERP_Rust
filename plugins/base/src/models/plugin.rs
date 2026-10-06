use code_gen::{Model, erp_methods, selection};
use erp::Result;
use erp::access::Operation;
use erp::environment::Environment;
use erp::serde_json::{Value, json};
use erp::types::field::{IdMode, MultipleIds};

#[selection]
pub enum PluginState {
    #[default]
    NotInstalled,
    Installed,
}

#[derive(Model)]
#[erp(id = "plugin", methods)]
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
    color: Option<String>,
    #[erp(label = "Demo data loaded", default = false)]
    demo_loaded: bool,
}

#[erp_methods]
impl Plugin<MultipleIds> {
    /// Install these plugins, with what they depend on, once this call is committed. The client
    /// is told to load the page again: the menus and the code it runs change with them.
    ///
    /// Held to the right to change the plugins: installing is changing what every user runs.
    #[erp(rpc)]
    pub fn install(&self, env: &mut Environment) -> Result<Value> {
        env.check_access("plugin", Operation::Write, self.id.get_ids_ref(), &[])?;
        for plugin in self {
            if *plugin.get_state(env)? != PluginState::Installed {
                let name = plugin.get_name(env)?.clone();
                env.request_install(&name);
            }
        }
        Ok(json!({"type": "reload"}))
    }
}
