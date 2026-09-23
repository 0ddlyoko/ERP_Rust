use erp::environment::Environment;
use erp::model::ModelManager;
use erp::plugin::Plugin;
use erp::types::field::SingleId;
use std::error::Error;

pub mod models;

/// Password given to the seeded administrator on a fresh database.
pub const DEFAULT_ADMIN_PASSWORD: &str = "admin";

pub struct BasePlugin;

impl Plugin for BasePlugin {
    fn name(&self) -> String {
        "base".to_string()
    }

    fn init_models(&self, model_manager: &mut ModelManager) {
        model_manager.register_model::<models::Company<_>>();
        model_manager.register_model::<models::Group<_>>();
        model_manager.register_model::<models::Users<_>>();
        model_manager.register_model::<models::Contact<_>>();
        model_manager.register_model::<models::Country<_>>();
        model_manager.register_model::<models::Lang<_>>();
        model_manager.register_model::<models::ModelData<_>>();
        model_manager.register_model::<models::Plugin<_>>();
    }

    fn data(&self) -> Vec<&'static str> {
        vec![include_str!("../data/users.xml")]
    }

    /// Give the seeded administrator a password, only if it has none.
    ///
    /// A hash cannot be written in a data file — it is salted per account — so the account is
    /// declared there and finished here. `noupdate` keeps later loads away from it.
    fn post_init(&mut self, env: &mut Environment) -> Result<(), Box<dyn Error + Send + Sync>> {
        let Some(admin) = erp::data::resolve(env, "base.user_admin")? else {
            return Ok(());
        };
        let admin: models::Users<SingleId> = env.get_record(admin.into());
        if !admin.has_password(env)? {
            admin.change_password(env, DEFAULT_ADMIN_PASSWORD)?;
        }
        Ok(())
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn _create_plugin() -> *mut Box<dyn Plugin> {
    let plugin = BasePlugin {};
    let box_plugin = Box::new(plugin);
    Box::into_raw(Box::new(box_plugin))
}
