use crate::models::sale_order_test::SaleOrderTest;

mod models;

pub struct TestPlugin;

impl core::plugin::Plugin for TestPlugin {
    fn name(&self) -> &'static str {
        "test_plugin"
    }

    fn init_models(&self, model_manager: &mut core::model::model_manager::ModelManager) {
        println!("init_models");
        model_manager.register_model::<SaleOrderTest>();
    }
}

pub struct TestPlugin2;

impl core::plugin::Plugin for TestPlugin2 {
    fn name(&self) -> &'static str {
        "test_plugin2"
    }

    fn init_models(&self, _model_manager: &mut core::model::model_manager::ModelManager) {
    }

    fn get_depends(&self) -> Vec<&'static str> {
        vec!["test_plugin"]
    }
}

pub struct TestPlugin3;

impl core::plugin::Plugin for TestPlugin3 {
    fn name(&self) -> &'static str {
        "test_plugin3"
    }

    fn init_models(&self, _model_manager: &mut core::model::model_manager::ModelManager) {
    }

    fn get_depends(&self) -> Vec<&'static str> {
        vec!["test_plugin", "test_plugin2"]
    }
}

#[no_mangle]
pub extern "C" fn _create_plugin() -> *mut Box<dyn core::plugin::Plugin> {
    println!("CALLING _create_plugin");
    let object = TestPlugin {};
    let b = Box::new(object);
    Box::into_raw(Box::new(b))
}
