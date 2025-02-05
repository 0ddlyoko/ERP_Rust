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
        model_manager.register_model::<models::SaleOrder<_>>();
        model_manager.register_model::<models::SaleOrderLine<_>>();
    }
}
