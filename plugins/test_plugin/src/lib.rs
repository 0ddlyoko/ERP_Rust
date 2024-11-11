use crate::models::sale_order_test::SaleOrderTest;

mod models;

pub struct TestPlugin;

impl core::plugin::Plugin for TestPlugin {
    fn name(&self) -> String {
        "test_plugin".to_string()
    }

    fn init_models(&self, model_manager: &mut core::model::model_manager::ModelManager) {
        println!("init_models");
        model_manager.register_model::<SaleOrderTest>();
    }
}

pub struct TestPlugin2;

impl core::plugin::Plugin for TestPlugin2 {
    fn name(&self) -> String {
        "test_plugin2".to_string()
    }

    fn init_models(&self, _model_manager: &mut core::model::model_manager::ModelManager) {
    }

    fn get_depends(&self) -> Vec<String> {
        vec![
            "test_plugin".to_string(),
        ]
    }
}

pub struct TestPlugin3;

impl core::plugin::Plugin for TestPlugin3 {
    fn name(&self) -> String {
        "test_plugin3".to_string()
    }

    fn init_models(&self, _model_manager: &mut core::model::model_manager::ModelManager) {
    }

    fn get_depends(&self) -> Vec<String> {
        vec![
            "test_plugin".to_string(),
            "test_plugin2".to_string(),
        ]
    }
}

#[no_mangle]
pub extern "C" fn _create_plugin() -> *mut Box<dyn core::plugin::Plugin> {
    println!("CALLING _create_plugin");
    let object = TestPlugin {};
    let b = Box::new(object);
    Box::into_raw(Box::new(b))
}
