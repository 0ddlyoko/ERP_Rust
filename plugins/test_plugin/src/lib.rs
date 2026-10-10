use crate::models::machine::MachineDiscounted;
use crate::models::sale_order_test::{SaleOrderTest, SaleOrderTest2};
use erp::assets::{BundleContribution, ModuleFiles, StaticFiles};
use erp::model::ModelManager;
use erp::plugin::Plugin;

pub mod models;

include!(concat!(env!("OUT_DIR"), "/static_files.rs"));

pub struct TestPlugin;

impl Plugin for TestPlugin {
    fn name(&self) -> String {
        "test_plugin".to_string()
    }

    fn init_models(&self, model_manager: &mut ModelManager) {
        tracing::debug!("init_models");
        model_manager.register_model::<SaleOrderTest<_>>();
        model_manager.register_model::<SaleOrderTest2<_>>();
        model_manager.register_model::<MachineDiscounted<_>>();
    }

    /// `machine` is declared by `test_lib_plugin`, which must therefore be loaded first.
    fn get_depends(&self) -> Vec<String> {
        vec!["test_lib_plugin".to_string()]
    }

    fn static_files(&self) -> StaticFiles {
        STATIC_FILES
    }

    fn module_files(&self) -> ModuleFiles {
        MODULE_FILES
    }

    /// Two bundles from the same files, the way the back office and the site share a plugin.
    fn assets(&self) -> Vec<BundleContribution> {
        vec![
            BundleContribution::new(
                "test.backend",
                &[
                    "test_plugin/static/src/**/*.js",
                    "test_plugin/static/src/**/*.xml",
                ],
            ),
            BundleContribution::new(
                "test.frontend",
                &[
                    "test_plugin/static/lib/*.js",
                    "test_plugin/static/css/*.css",
                ],
            ),
        ]
    }
}

pub struct TestPlugin2;

impl Plugin for TestPlugin2 {
    fn name(&self) -> String {
        "test_plugin2".to_string()
    }

    fn init_models(&self, _model_manager: &mut ModelManager) {}

    fn get_depends(&self) -> Vec<String> {
        vec!["test_plugin".to_string()]
    }
}

pub struct TestPlugin3;

impl Plugin for TestPlugin3 {
    fn name(&self) -> String {
        "test_plugin3".to_string()
    }

    fn init_models(&self, _model_manager: &mut ModelManager) {}

    fn get_depends(&self) -> Vec<String> {
        vec!["test_plugin".to_string(), "test_plugin2".to_string()]
    }
}

code_gen::export_plugin!(TestPlugin {});
