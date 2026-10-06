use code_gen::Model;
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{IdMode, MultipleIds, SingleId};
use erp_search_code_gen::make_domain;
use std::collections::HashSet;

/// Maps a stable external identifier to the record it designates.
///
/// A technical id changes between installations; `module.name` does not, which is what lets a
/// plugin reference another plugin's data and lets a data file be reloaded without duplicating
/// anything.
#[derive(Model)]
#[erp(id = "model_data")]
#[allow(dead_code)]
pub struct ModelData<Mode: IdMode> {
    id: Mode,
    /// Plugin the identifier belongs to.
    #[erp(index)]
    module: String,
    /// Identifier within that plugin.
    #[erp(index)]
    name: String,
    /// Model of the record designated.
    #[erp(index)]
    model: String,
    /// Technical id of that record.
    #[erp(default = 0, index)]
    res_id: i32,
    /// When set, later loads leave the record alone — it belongs to the user now.
    #[erp(default = false)]
    noupdate: bool,
}

impl ModelData<SingleId> {
    /// The records of `model` that installed plugins not loaded yet in this application declare:
    /// while it loads, plugins come one at a time, and what such a record names may come with its
    /// plugin.
    pub fn of_plugins_not_loaded(env: &mut Environment, model: &str) -> Result<HashSet<u32>> {
        let loaded = env.model_manager.loaded_plugins().to_vec();
        let env = &mut *env.sudo();
        let entries: ModelData<MultipleIds> = env.search(&make_domain!([("model", "=", model)]))?;
        let mut records = HashSet::new();
        for entry in entries {
            if !loaded.contains(entry.get_module(env)?)
                && let Ok(id) = u32::try_from(*entry.get_res_id(env)?)
            {
                records.insert(id);
            }
        }
        Ok(records)
    }
}
