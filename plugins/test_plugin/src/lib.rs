
pub struct TestPlugin;

impl core::plugin::Plugin for TestPlugin {
    fn init(&mut self) {
        println!("init");
    }

    fn init_models(&self, model_manager: &core::model::model_manager::ModelManager) {
        println!("init_models");
        // feds
    }
}

#[no_mangle]
#[allow(improper_ctypes_definitions)]
pub extern "C" fn _plugin_create() -> Box<dyn core::plugin::Plugin> {
    Box::new(TestPlugin)
}
