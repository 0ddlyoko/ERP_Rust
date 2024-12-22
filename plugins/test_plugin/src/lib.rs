use crate::models::sale_order_test::{SaleOrderTest, SaleOrderTest2};
use erp::model::ModelManager;
use erp::plugin::Plugin;

mod models;

pub struct TestPlugin;

impl Plugin for TestPlugin {
    fn name(&self) -> String {
        "test_plugin".to_string()
    }

    fn init_models(&self, model_manager: &mut ModelManager) {
        println!("init_models");
        model_manager.register_model::<SaleOrderTest>();
        model_manager.register_model::<SaleOrderTest2>();
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

#[no_mangle]
pub extern "C" fn _create_plugin() -> *mut Box<dyn Plugin> {
    println!("CALLING _create_plugin");
    let object = TestPlugin {};
    let b = Box::new(object);
    Box::into_raw(Box::new(b))
}
