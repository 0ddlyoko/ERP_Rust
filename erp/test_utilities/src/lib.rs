use ::config::ConfigError;
use erp::model::ModelManager;
use erp::plugin::Plugin;

pub mod config;
pub mod models;

pub fn default_config() -> Result<erp::config::Config, ConfigError> {
    config::build_config()
}

pub struct TestLibPlugin;

impl Plugin for TestLibPlugin {
    fn name(&self) -> String {
        "test_lib_plugin".to_string()
    }

    fn init_models(&self, model_manager: &mut ModelManager) {
        model_manager.register_model::<models::Invoice<_>>();
        model_manager.register_model::<models::MeterReading<_>>();
        model_manager.register_model::<models::SaleOrder<_>>();
        model_manager.register_model::<models::SaleOrderLine<_>>();
        model_manager.register_model::<models::Tag<_>>();
    }
}

/// Ships a data file, to exercise the loader end to end.
///
/// Kept apart from [`TestLibPlugin`] so tests that do not care about data need neither `base`
/// nor the external identifier registry.
pub struct SeedPlugin;

impl Plugin for SeedPlugin {
    fn name(&self) -> String {
        "seed_plugin".to_string()
    }

    fn init_models(&self, _model_manager: &mut ModelManager) {}

    fn data(&self) -> Vec<&'static str> {
        vec![include_str!("../data/orders.xml")]
    }

    /// External identifiers are recorded in `model_data`, which `base` declares.
    fn get_depends(&self) -> Vec<String> {
        vec!["base".to_string(), "test_lib_plugin".to_string()]
    }
}
