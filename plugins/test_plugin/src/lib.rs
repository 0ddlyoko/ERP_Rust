
pub struct TestPlugin {
    name: String,
}

impl core::plugin::Plugin for TestPlugin {
    fn name(&self) -> &'static str {
        "test_plugin"
    }

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
pub extern "C" fn _create_plugin() -> *mut dyn core::plugin::Plugin {
    println!("CALLING _create_plugin");
    let object = TestPlugin {
        name: "test_plugin".to_string(),
    };
    // let name = object.name();
    let boxed: Box<dyn core::plugin::Plugin> = Box::new(object);
    Box::into_raw(boxed)
}
