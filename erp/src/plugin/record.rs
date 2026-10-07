//! Keeping the `plugin` table in step with the plugins the application knows.
//!
//! The table is what the application reads at boot to know which plugins to load again, so a
//! plugin is installed once its row says so, and stays installed across restarts.

use crate::environment::Environment;
use crate::plugin::PluginInfo;
use erp_search_code_gen::make_domain;
use erp_types::field::{FieldType, SingleId};
use erp_types::model::MapOfFields;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

const PLUGIN_MODEL: &str = "plugin";

/// Record a plugin, and mark it installed if it just was.
///
/// A plugin that is only known is recorded as not installed, and a row already saying installed is
/// left as it is: knowing a plugin never uninstalls it. Installing records the version loaded, and
/// says so when it differs from the one installed before — which is what an update is, since
/// loading a plugin brings its schema and data up to date.
///
/// Does nothing before the model describing plugins exists, which is what loading `base` itself
/// looks like until its models are registered.
pub(crate) fn record_plugin(
    env: &mut Environment,
    name: &str,
    info: &PluginInfo,
    installed: bool,
) -> Result<()> {
    if env.model_manager.try_get_model(PLUGIN_MODEL).is_err() {
        return Ok(());
    }
    let mut values = MapOfFields::default();
    values.insert("name", name);
    for (field, value) in [
        ("description", &info.description),
        ("author", &info.author),
        ("category", &info.category),
        ("website", &info.website),
        ("latest_version", &info.version),
        ("color", &info.color),
    ] {
        values.insert_option(field, value.clone().map(FieldType::String));
    }
    if installed {
        values.insert("state", "installed");
        values.insert_option(
            "installed_version",
            info.version.clone().map(FieldType::String),
        );
    }

    let existing = env.search_ids(PLUGIN_MODEL, &make_domain!([("name", "=", name)]))?;
    match existing.first() {
        Some(id) => {
            if installed {
                let rows = env.read(PLUGIN_MODEL, &SingleId::from(*id), &["installed_version"])?;
                let before: Option<String> =
                    rows[0].get_option::<&String>("installed_version").cloned();
                if before.is_some() && before != info.version {
                    tracing::info!(
                        plugin = %name,
                        from = ?before,
                        to = ?info.version,
                        "Updated plugin"
                    );
                }
            }
            env.write_changes(PLUGIN_MODEL, *id, values)?;
        }
        None => {
            if !installed {
                values.insert("state", "not_installed");
            }
            env.create_records(PLUGIN_MODEL, vec![values])?;
        }
    }
    Ok(())
}

/// Whether a plugin is installed, and with which version.
///
/// `None` when it is not — no row, or a row saying otherwise — and also before the model
/// describing plugins exists, which is what a first boot looks like while `base` is loading.
pub(crate) fn installed_version(
    env: &mut Environment,
    name: &str,
) -> Result<Option<Option<String>>> {
    if env.model_manager.try_get_model(PLUGIN_MODEL).is_err() {
        return Ok(None);
    }
    let found = env.search_ids(
        PLUGIN_MODEL,
        &make_domain!([("name", "=", name), ("state", "=", "installed")]),
    )?;
    let Some(id) = found.first() else {
        return Ok(None);
    };
    let rows = env.read(PLUGIN_MODEL, &SingleId::from(*id), &["installed_version"])?;
    Ok(Some(
        rows[0].get_option::<&String>("installed_version").cloned(),
    ))
}
