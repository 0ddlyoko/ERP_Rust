use erp::Result;
use erp::environment::Environment;
use erp::http::ControllerRegistry;
use erp::model::ModelManager;
use erp::plugin::{Plugin, PluginInfo};
use erp::types::field::SingleId;

pub mod controllers;
pub mod models;

/// Password given to the seeded administrator on a fresh database.
pub const DEFAULT_ADMIN_PASSWORD: &str = "admin";

/// How many users, and how many sessions, the caches kept across requests hold at most: the
/// active ones stay, an account seen once makes room for them. A few megabytes at most.
const CACHED_ACCOUNTS: usize = 10_000;

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
            color: Some("#5a5878".to_string()),
            ..PluginInfo::default()
        }
    }

    fn init_models(&self, model_manager: &mut ModelManager) {
        model_manager.register_model::<models::Company<_>>();
        model_manager.register_model::<models::Group<_>>();
        model_manager.register_model::<models::Users<_>>();
        model_manager.register_model::<models::Contact<_>>();
        model_manager.register_model::<models::ContactTag<_>>();
        model_manager.register_model::<models::Country<_>>();
        model_manager.register_model::<models::Lang<_>>();
        model_manager.register_model::<models::ModelData<_>>();
        model_manager.register_model::<models::Plugin<_>>();
        model_manager.register_model::<models::Session<_>>();
        model_manager.register_model::<models::AccessRule<_>>();
        model_manager.register_model::<models::View<_>>();
        model_manager.register_model::<models::Action<_>>();
        model_manager.register_model::<models::Menu<_>>();
        model_manager.register_model::<models::Parameter<_>>();
        model_manager.register_model::<models::Settings<_>>();
        model_manager.set_data_children("menu", "parent");
        model_manager.set_data_body("view", "arch");
        model_manager
            .shared_caches
            .register(models::VIEWS_CACHE, &["view"]);
        model_manager
            .shared_caches
            .register(models::RULES_CACHE, &["access_rule"]);
        model_manager.shared_caches.register_bounded(
            models::GROUPS_CACHE,
            &["users", "group"],
            CACHED_ACCOUNTS,
        );
        model_manager.shared_caches.register_bounded(
            models::SESSIONS_CACHE,
            &["session", "users"],
            CACHED_ACCOUNTS,
        );
        model_manager.load_hooks.push(|env, plugin| {
            models::View::<SingleId>::on_plugin_loaded(env, plugin.to_string())
        });
        // What the core knows about identity is that something answers it. This is the something.
        model_manager
            .identities
            .register(|env, token| models::Session::<SingleId>::resolve(env, token.to_string()));
        model_manager.identities.register_user_model("users");
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
            include_str!("../data/countries.xml"),
            include_str!("../data/company.xml"),
            include_str!("../data/settings.xml"),
            include_str!("../views/users_views.xml"),
            include_str!("../views/contact_views.xml"),
            include_str!("../views/company_views.xml"),
            include_str!("../views/technical_views.xml"),
            include_str!("../views/menus.xml"),
        ]
    }

    fn demo(&self) -> Vec<&'static str> {
        vec![include_str!("../demo/contacts.xml")]
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
    fn post_init(&mut self, env: &mut Environment) -> Result<()> {
        let admin: models::Users<SingleId> = env.named("base.user_admin")?;
        if !admin.has_password(env)? {
            admin.change_password(env, DEFAULT_ADMIN_PASSWORD.to_string())?;
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
