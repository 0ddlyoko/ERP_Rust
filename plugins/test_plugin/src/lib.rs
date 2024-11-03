pub struct TestPlugin;

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
// #[allow(improper_ctypes_definitions)]
pub extern "C" fn _create_plugin() -> *mut Box<dyn core::plugin::Plugin> {
    println!("CALLING _create_plugin");
    // let object = TestPlugin {
    //     name: "test_plugin".to_string(),
    // };
    // let name = object.name();
    let boxed: Box<dyn core::plugin::Plugin> = Box::new(TestPlugin);
    Box::into_raw(Box::new(boxed))
}

#[no_mangle]
// #[allow(improper_ctypes_definitions)]
pub extern "C" fn _create_plugin_2() -> u32 {
    println!("CALLING _create_plugin_2");
    // let object = TestPlugin {
    //     name: "test_plugin".to_string(),
    // };
    // let name = object.name();
    42
}
