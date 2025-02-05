use erp::model::ModelManager;
use erp::plugin::Plugin;

pub mod models;

pub struct BasePlugin;

impl Plugin for BasePlugin {
    fn name(&self) -> String {
        "base".to_string()
    }

    fn init_models(&self, model_manager: &mut ModelManager) {
        model_manager.register_model::<models::Company<_>>();
        model_manager.register_model::<models::Contact<_>>();
        model_manager.register_model::<models::Country<_>>();
        model_manager.register_model::<models::Lang<_>>();
        model_manager.register_model::<models::Plugin<_>>();
    }
}

#[no_mangle]
pub extern "C" fn _create_plugin() -> *mut Box<dyn Plugin> {
    let plugin = BasePlugin {};
    let box_plugin = Box::new(plugin);
    Box::into_raw(Box::new(box_plugin))
}
