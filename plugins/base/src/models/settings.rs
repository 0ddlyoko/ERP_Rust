use crate::models::Parameter;
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::plugin::demo::{self, DEMO_PARAMETER};
use erp::types::field::{IdMode, MultipleIds};
use erp::types::model::MapOfFields;

/// The settings of the database, as one page: a single record, `base.settings`, each field
/// keeping a parameter — what plugins read.
#[derive(Model)]
#[erp(id = "settings", methods)]
#[allow(dead_code)]
pub struct Settings<Mode: IdMode> {
    pub id: Mode,
    #[erp(default = "Settings")]
    name: String,
    #[erp(
        label = "Demo data",
        description = "Customers, products and orders showing what each plugin does",
        default = false
    )]
    demo_data: bool,
}

#[erp_methods]
impl Settings<MultipleIds> {
    /// Each setting changed keeps its parameter. Turning demo data on loads the demo data of the
    /// plugins installed, as every plugin installed from then on will; turning it off leaves the
    /// records loaded where they are.
    pub fn write(&self, env: &mut Environment, values: MapOfFields, sup: Super) -> Result<()> {
        let demo_data = values.get_option::<&bool>("demo_data").copied();
        sup.call_with(values, env)?;
        if let Some(wanted) = demo_data {
            let value = if wanted { "1" } else { "0" };
            Parameter::<MultipleIds>::keep(env, DEMO_PARAMETER.to_string(), value.to_string())?;
            if wanted {
                demo::load_installed(env)?;
            }
        }
        Ok(())
    }
}
