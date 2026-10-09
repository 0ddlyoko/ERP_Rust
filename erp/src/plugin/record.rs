//! Keeping the `plugin` table in step with the plugins the application knows.
//!
//! The table is what the application reads at boot to know which plugins to load again, so a
//! plugin is installed once its row says so, and stays installed across restarts.

use crate::environment::Environment;
use crate::plugin::PluginInfo;
use erp_search::SearchType;
use erp_search_code_gen::make_domain;
use erp_types::field::{FieldType, IdMode, MultipleIds, SingleId};
use erp_types::model::MapOfFields;
use std::collections::HashMap;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

const PLUGIN_MODEL: &str = "plugin";
const PARAMETER_MODEL: &str = "parameter";

/// What the plugins' rows and the database's parameters hold, read once while the application
/// loads and kept in step with what it writes there, rather than asked again for each plugin.
#[derive(Default)]
pub struct BootRecords {
    plugins: Option<HashMap<String, PluginRow>>,
    parameters: Option<HashMap<String, Option<String>>>,
}

/// A plugin's row, as loading reads it.
#[derive(Clone, Default)]
pub(crate) struct PluginRow {
    pub id: u32,
    pub installed: bool,
    pub installed_version: Option<String>,
    pub demo_loaded: bool,
    /// What the row says of the plugin, as [`record_plugin`] writes it: to write only what changed.
    pub described: MapOfFields,
}

/// The fields of a plugin's row that say what it is, as [`record_plugin`] writes them.
const DESCRIBED: [&str; 9] = [
    "name",
    "description",
    "author",
    "category",
    "website",
    "latest_version",
    "color",
    "state",
    "installed_version",
];

/// A plugin's row, if it has one — every row read at once while the application loads.
pub(crate) fn plugin_row(env: &mut Environment, name: &str) -> Result<Option<PluginRow>> {
    if env.model_manager.try_get_model(PLUGIN_MODEL).is_err() {
        return Ok(None);
    }
    let booting = env.boot.is_some();
    let loaded = env.boot.as_ref().is_some_and(|boot| boot.plugins.is_some());
    if booting && !loaded {
        let rows = plugin_rows(env, &SearchType::Nothing)?;
        if let Some(boot) = env.boot.as_mut() {
            boot.plugins = Some(rows);
        }
    }
    if booting {
        return Ok(env
            .boot
            .as_ref()
            .and_then(|boot| boot.plugins.as_ref())
            .and_then(|rows| rows.get(name))
            .cloned());
    }
    Ok(plugin_rows(env, &make_domain!([("name", "=", name)]))?.remove(name))
}

/// The plugins' rows a domain finds, by name.
fn plugin_rows(env: &mut Environment, domain: &SearchType) -> Result<HashMap<String, PluginRow>> {
    let env = &mut *env.sudo();
    let ids = env.search_ids(PLUGIN_MODEL, domain)?;
    let rows = env.read(
        PLUGIN_MODEL,
        &MultipleIds::from(ids),
        &[&DESCRIBED[..], &["demo_loaded"]].concat(),
    )?;
    let mut found = HashMap::with_capacity(rows.len());
    for row in &rows {
        let (Some(id), Some(name)) = (
            row.get_option::<&u32>("id"),
            row.get_option::<&String>("name"),
        ) else {
            continue;
        };
        found.insert(
            name.clone(),
            PluginRow {
                id: *id,
                installed: row
                    .get_option::<&String>("state")
                    .is_some_and(|state| state == "installed"),
                installed_version: row.get_option::<&String>("installed_version").cloned(),
                demo_loaded: row
                    .get_option::<&bool>("demo_loaded")
                    .copied()
                    .unwrap_or(false),
                described: MapOfFields::new(
                    DESCRIBED
                        .iter()
                        .map(|field| (field.to_string(), row.fields.get(*field).cloned().flatten()))
                        .collect(),
                ),
            },
        );
    }
    Ok(found)
}

/// Keep what loading read of a plugin's row in step with what was written to it.
pub(crate) fn note_plugin(env: &mut Environment, name: &str, row: PluginRow) {
    if let Some(rows) = env.boot.as_mut().and_then(|boot| boot.plugins.as_mut()) {
        rows.insert(name.to_string(), row);
    }
}

/// The value of a parameter of the database — every one read at once while the application
/// loads. `None` before the model of parameters exists.
pub fn parameter(env: &mut Environment, key: &str) -> Result<Option<String>> {
    if env.model_manager.try_get_model(PARAMETER_MODEL).is_err() {
        return Ok(None);
    }
    let booting = env.boot.is_some();
    let loaded = env
        .boot
        .as_ref()
        .is_some_and(|boot| boot.parameters.is_some());
    if booting && !loaded {
        let values = parameters(env, &SearchType::Nothing)?;
        if let Some(boot) = env.boot.as_mut() {
            boot.parameters = Some(values);
        }
    }
    if booting {
        return Ok(env
            .boot
            .as_ref()
            .and_then(|boot| boot.parameters.as_ref())
            .and_then(|values| values.get(key))
            .cloned()
            .flatten());
    }
    Ok(parameters(env, &make_domain!([("key", "=", key)]))?
        .remove(key)
        .flatten())
}

/// The parameters a domain finds, by key.
fn parameters(
    env: &mut Environment,
    domain: &SearchType,
) -> Result<HashMap<String, Option<String>>> {
    let env = &mut *env.sudo();
    let ids = env.search_ids(PARAMETER_MODEL, domain)?;
    let rows = env.read(PARAMETER_MODEL, &MultipleIds::from(ids), &["key", "value"])?;
    Ok(rows
        .iter()
        .filter_map(|row| {
            let key = row.get_option::<&String>("key")?.clone();
            Some((key, row.get_option::<&String>("value").cloned()))
        })
        .collect())
}

/// Keep what loading read of the parameters in step with a value written under a key.
pub fn note_parameter(env: &mut Environment, key: &str, value: Option<String>) {
    if let Some(values) = env.boot.as_mut().and_then(|boot| boot.parameters.as_mut()) {
        values.insert(key.to_string(), value);
    }
}

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

    match plugin_row(env, name)? {
        Some(row) => {
            if installed {
                let before = row.installed_version.clone();
                if before.is_some() && before != info.version {
                    tracing::info!(
                        plugin = %name,
                        from = ?before,
                        to = ?info.version,
                        "Updated plugin"
                    );
                }
            }
            let changed = MapOfFields::new(
                values
                    .fields
                    .iter()
                    .filter(|(field, value)| row.described.fields.get(*field) != Some(*value))
                    .map(|(field, value)| (field.clone(), value.clone()))
                    .collect(),
            );
            if !changed.is_empty() {
                env.write(PLUGIN_MODEL, &SingleId::from(row.id), changed)?;
            }
            let mut described = row.described.clone();
            described.fields.extend(values.fields);
            let row = PluginRow {
                installed: row.installed || installed,
                installed_version: if installed {
                    info.version.clone()
                } else {
                    row.installed_version
                },
                described,
                ..row
            };
            note_plugin(env, name, row);
        }
        None => {
            if !installed {
                values.insert("state", "not_installed");
            }
            let created: MultipleIds = env.create_records(PLUGIN_MODEL, vec![values])?;
            let id = created.get_ids_ref().first().copied().unwrap_or_default();
            let row = PluginRow {
                id,
                installed,
                installed_version: if installed {
                    info.version.clone()
                } else {
                    None
                },
                demo_loaded: false,
                described: MapOfFields::default(),
            };
            note_plugin(env, name, row);
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
    Ok(plugin_row(env, name)?
        .filter(|row| row.installed)
        .map(|row| row.installed_version))
}
