use crate::models::sale_order::SaleOrder;

mod models;

pub struct TestPlugin;

impl core::plugin::Plugin for TestPlugin {
    fn name(&self) -> &'static str {
        "test_plugin"
    }

    fn init(&mut self) {
        println!("init");
    }

    fn init_models(&self, model_manager: &mut core::model::model_manager::ModelManager) {
        println!("init_models");
        model_manager.register_model::<SaleOrder>();
    }

    fn unload(&mut self) {

    }
}

#[no_mangle]
pub extern "C" fn _create_plugin() -> *mut Box<dyn core::plugin::Plugin> {
    println!("CALLING _create_plugin");
    let object = TestPlugin {};
    let b = Box::new(object);
    Box::into_raw(Box::new(b))
}
