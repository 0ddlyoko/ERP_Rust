use erp::environment::Environment;
use erp::http::ControllerRegistry;
use erp::model::ModelManager;
use erp::plugin::{Plugin, PluginInfo};
use erp::types::field::SingleId;
use std::error::Error;

pub mod controllers;
pub mod models;

/// Password given to the seeded administrator on a fresh database.
pub const DEFAULT_ADMIN_PASSWORD: &str = "admin";

pub struct BasePlugin;

impl Plugin for BasePlugin {
    fn name(&self) -> String {
        "base".to_string()
    }

    fn info(&self) -> PluginInfo {
        PluginInfo {
            description: Some(
                "What every application starts from: users, groups, access rights, contacts."
                    .to_string(),
            ),
            category: Some("Technical".to_string()),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            ..PluginInfo::default()
        }
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
        model_manager.register_model::<models::AccessRule<_>>();
        model_manager.register_model::<models::View<_>>();
        model_manager.set_data_body("view", "arch");
        model_manager
            .shared_caches
            .register(models::VIEWS_CACHE, &["view"]);
        model_manager
            .load_hooks
            .push(models::View::<SingleId>::on_plugin_loaded);
        // What the core knows about identity is that something answers it. This is the something.
        model_manager
            .identities
            .register(models::Session::<SingleId>::resolve);
        model_manager
            .access
            .register(models::AccessRule::<SingleId>::source());
    }

    fn init_controllers(&self, controllers: &mut ControllerRegistry) {
        controllers.register::<controllers::Home>();
    }

    fn data(&self) -> Vec<&'static str> {
        vec![
            include_str!("../data/users.xml"),
            include_str!("../data/access.xml"),
            include_str!("../views/users_views.xml"),
        ]
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
    ///
    /// None of the three is looked up defensively. They come from the file loaded a moment ago,
    /// so one missing is a broken plugin, and failing here says so — where carrying on would
    /// leave the administrator without a password and the framework without a default user.
    fn post_init(&mut self, env: &mut Environment) -> Result<(), Box<dyn Error + Send + Sync>> {
        let admin: models::Users<SingleId> = env.named("base.user_admin")?;
        if !admin.has_password(env)? {
            admin.change_password(env, DEFAULT_ADMIN_PASSWORD)?;
        }

        let portal: models::Users<SingleId> = env.named("base.user_portal")?;
        env.model_manager
            .identities
            .set_default_user(portal.get_id())?;

        let root: models::Users<SingleId> = env.named("base.user_root")?;
        env.model_manager.identities.set_root_user(root.get_id())?;
        Ok(())
    }
}

code_gen::export_plugin!(BasePlugin {});
