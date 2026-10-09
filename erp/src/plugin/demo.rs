//! Demo data: records showing what each plugin does, for a database that asks for them.
//!
//! A database asks through its `demo_data` parameter. From then on, every plugin installed
//! brings its demo documents ([`Plugin::demo`]), and turning the parameter on loads those of the
//! plugins installed before. A plugin's demo is loaded once: its row says so, and neither an
//! update nor turning the parameter on again brings the records back once the user changed them.
//!
//! [`Plugin::demo`]: crate::plugin::Plugin::demo

use crate::environment::Environment;
use erp_types::field::SingleId;
use erp_types::model::MapOfFields;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// The parameter saying whether the database wants demo data.
pub const DEMO_PARAMETER: &str = "demo_data";
const PLUGIN_MODEL: &str = "plugin";

/// Whether the database wants demo data: its `demo_data` parameter is on. Never before the model
/// of parameters exists.
pub fn is_wanted(env: &mut Environment) -> Result<bool> {
    Ok(matches!(
        crate::plugin::parameter(env, DEMO_PARAMETER)?.as_deref(),
        Some("1" | "true")
    ))
}

/// Load a plugin's demo documents, if the database wants demo data and they were not loaded
/// already; the plugin's row then says they were.
pub(crate) fn load_for(env: &mut Environment, plugin_name: &str) -> Result<()> {
    if !is_wanted(env)? {
        return Ok(());
    }
    let documents = env
        .model_manager
        .demo
        .get(plugin_name)
        .cloned()
        .unwrap_or_default();
    let Some(row) = crate::plugin::plugin_row(env, plugin_name)? else {
        return Ok(());
    };
    if row.demo_loaded {
        return Ok(());
    }
    let env = &mut *env.sudo();
    if !documents.is_empty() {
        tracing::info!(plugin = %plugin_name, "Loading demo data");
    }
    for document in &documents {
        crate::data::load(env, plugin_name, document)?;
    }
    let mut values = MapOfFields::default();
    values.insert("demo_loaded", true);
    env.write(PLUGIN_MODEL, &SingleId::from(row.id), values)?;
    crate::plugin::note_plugin(
        env,
        plugin_name,
        crate::plugin::PluginRow {
            demo_loaded: true,
            ..row
        },
    );
    Ok(())
}

/// Load the demo documents of every plugin installed, in the order they were: what turning
/// demo data on does for the plugins already there.
pub fn load_installed(env: &mut Environment) -> Result<()> {
    for plugin_name in env.model_manager.loaded_plugins.clone() {
        load_for(env, &plugin_name)?;
    }
    Ok(())
}
