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
        model_manager.register_model::<models::Session<_>>();
        // What the core knows about identity is that something answers it. This is the something.
        model_manager
            .identities
            .register(models::Session::<SingleId>::resolve);
    }

    fn data(&self) -> Vec<&'static str> {
        vec![include_str!("../data/users.xml")]
    }

    /// Finish the seeded accounts, and say which two of them the framework needs by name.
    ///
    /// The administrator's password cannot go in a data file — a hash is salted per account — so
    /// the account is declared there and finished here, only if it has none. `noupdate` keeps
    /// later loads away from it.
    ///
    /// The other two are told to the core rather than looked up by it: `erp` knows that somebody
    /// is the caller before authentication and somebody bypasses every rule, and nothing about
    /// which records those are.
    fn post_init(&mut self, env: &mut Environment) -> Result<(), Box<dyn Error + Send + Sync>> {
        if let Some(admin) = erp::data::resolve(env, "base.user_admin")? {
            let admin: models::Users<SingleId> = env.get_record(admin.into());
            if !admin.has_password(env)? {
                admin.change_password(env, DEFAULT_ADMIN_PASSWORD)?;
            }
        }
        if let Some(portal) = erp::data::resolve(env, "base.user_portal")? {
            env.model_manager.identities.set_default_user(portal)?;
        }
        if let Some(root) = erp::data::resolve(env, "base.user_root")? {
            env.model_manager.identities.set_root_user(root)?;
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
