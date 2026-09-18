use code_gen::Model;
use erp::types::field::IdMode;

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
    #[erp(default = "")]
    module: String,
    /// Identifier within that plugin.
    #[erp(default = "")]
    name: String,
    /// Model of the record designated.
    #[erp(default = "")]
    model: String,
    /// Technical id of that record.
    #[erp(default = 0)]
    res_id: i32,
    /// When set, later loads leave the record alone — it belongs to the user now.
    #[erp(default = false)]
    noupdate: bool,
}
