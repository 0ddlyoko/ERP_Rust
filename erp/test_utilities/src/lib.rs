use ::config::ConfigError;
use erp::Result;
use erp::model::ModelManager;
use erp::plugin::Plugin;

pub mod config;
pub mod models;

pub fn default_config() -> Result<erp::config::Config, ConfigError> {
    config::build_config()
}

pub struct TestLibPlugin;

impl Plugin for TestLibPlugin {
    fn name(&self) -> String {
        "test_lib_plugin".to_string()
    }

    fn init_models(&self, model_manager: &mut ModelManager) {
        model_manager.register_model::<models::Invoice<_>>();
        model_manager.register_model::<models::Machine<_>>();
        model_manager.register_model::<models::MeterReading<_>>();
        model_manager.register_model::<models::Notebook<_>>();
        model_manager.register_model::<models::Page<_>>();
        model_manager.register_model::<models::Record<_>>();
        model_manager.register_model::<models::SaleOrder<_>>();
        model_manager.register_model::<models::SaleOrderLine<_>>();
        model_manager.register_model::<models::Tag<_>>();
    }
}

/// Ships a data file, to exercise the loader end to end.
///
/// Kept apart from [`TestLibPlugin`] so tests that do not care about data need neither `base`
/// nor the external identifier registry.
pub struct SeedPlugin;

impl Plugin for SeedPlugin {
    fn name(&self) -> String {
        "seed_plugin".to_string()
    }

    fn init_models(&self, _model_manager: &mut ModelManager) {}

    fn data(&self) -> Vec<&'static str> {
        vec![include_str!("../data/orders.xml")]
    }

    /// External identifiers are recorded in `model_data`, which `base` declares.
    fn get_depends(&self) -> Vec<String> {
        vec!["base".to_string(), "test_lib_plugin".to_string()]
    }
}

/// Let a user do anything to these models, through a group made for them.
///
/// Addressed by name so this crate needs no dependency on `base`, which declares the models. Runs
/// as sudo: granting rights cannot depend on the rights being granted.
pub fn grant_everything(
    env: &mut erp::environment::Environment,
    uid: u32,
    models: &[&str],
) -> Result<u32> {
    use erp::types::field::IdMode;
    use erp::types::model::MapOfFields;

    let env = &mut *env.sudo();
    let mut group = MapOfFields::default();
    group.insert("name", format!("Everything for {uid}"));
    group.insert("users", erp::types::field::FieldType::Refs(vec![uid]));
    let group = env.create_records("group", vec![group])?.get_ids_ref()[0];

    let rules = models
        .iter()
        .map(|model| {
            let mut rule = MapOfFields::default();
            rule.insert("name", format!("{model}: everything for {uid}"));
            rule.insert("model", *model);
            rule.insert("group", erp::types::field::FieldType::Ref(group));
            for domain in [
                "domain_read",
                "domain_create",
                "domain_write",
                "domain_delete",
            ] {
                rule.insert(domain, "[]");
            }
            rule
        })
        .collect();
    env.create_records("access_rule", rules)?;
    Ok(group)
}
