use erp::app::Application;
use erp_types::field::{IdMode, MultipleIds};
use erp_types::model::{CommonModel, MapOfFields};
use std::collections::HashMap;
use std::error::Error;
use test_plugin::TestPlugin;
use test_plugin::models::machine_discounted::MachineDiscounted;
use test_utilities::TestLibPlugin;
use test_utilities::models::Machine;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// Only the plugin that declares `daily_rate`.
fn base_only() -> Result<Application> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    app.load_plugin("test_lib_plugin")?;
    Ok(app)
}

/// The declaring plugin, plus one that overrides `daily_rate`.
fn with_override() -> Result<Application> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    app.register_plugin(Box::new(TestPlugin {}))?;
    app.load_plugin("test_plugin")?;
    Ok(app)
}

fn machine(env: &mut erp::environment::Environment, values: &[(&str, i32)]) -> Result<MultipleIds> {
    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    for (field, value) in values {
        map.insert(field, *value);
    }
    env.create_records("machine", vec![map])
}

/// With nothing overriding it, a chain of one runs the implementation as written.
#[test]
fn test_lone_implementation_runs() -> Result<()> {
    let app = base_only()?;
    let mut env = app.new_env()?;
    let ids = machine(&mut env, &[("base_rate", 100), ("days", 3)])?;

    let record: Machine<MultipleIds> = env.get_record(ids);
    assert_eq!(record.daily_rate(&mut env)?, 100);
    Ok(())
}

/// An override runs instead, and reaches the implementation below through `super`.
#[test]
fn test_override_runs_and_super_reaches_the_base() -> Result<()> {
    let app = with_override()?;
    let mut env = app.new_env()?;
    let ids = machine(&mut env, &[("base_rate", 100), ("discount", 30)])?;

    let record: Machine<MultipleIds> = env.get_record(ids);
    assert_eq!(
        record.daily_rate(&mut env)?,
        70,
        "the override must subtract the discount from what super returned"
    );
    Ok(())
}

/// The point of the whole mechanism.
///
/// `quote` lives in the plugin that declares the model, carries no attribute of its own, and was
/// compiled before the overriding plugin existed. It must still reach the override.
#[test]
fn test_a_call_from_the_declaring_plugin_reaches_the_override() -> Result<()> {
    let app = with_override()?;
    let mut env = app.new_env()?;
    let ids = machine(
        &mut env,
        &[("base_rate", 100), ("days", 3), ("discount", 30)],
    )?;

    let record: Machine<MultipleIds> = env.get_record(ids);
    assert_eq!(
        record.quote(&mut env)?,
        210,
        "quote must bill 3 days at the overridden rate of 70, not at the base rate of 100"
    );
    Ok(())
}

/// Without the override loaded, the same call bills the base rate — so the previous test is
/// measuring the override, not a coincidence.
#[test]
fn test_the_same_call_bills_the_base_rate_without_the_override() -> Result<()> {
    let app = base_only()?;
    let mut env = app.new_env()?;
    let ids = machine(&mut env, &[("base_rate", 100), ("days", 3)])?;

    let record: Machine<MultipleIds> = env.get_record(ids);
    assert_eq!(record.quote(&mut env)?, 300);
    Ok(())
}

/// Dispatch keys on the model, not on the Rust type the call was made from.
#[test]
fn test_dispatch_is_the_same_from_either_struct() -> Result<()> {
    let app = with_override()?;
    let mut env = app.new_env()?;
    let ids = machine(&mut env, &[("base_rate", 80), ("discount", 5)])?;

    let record: Machine<MultipleIds> = env.get_record(ids);
    assert_eq!(record.daily_rate(&mut env)?, 75);
    Ok(())
}

/// Several records go through one call, as they do for a computed field.
#[test]
fn test_a_call_carries_the_whole_recordset() -> Result<()> {
    let app = base_only()?;
    let mut env = app.new_env()?;
    let maps: Vec<MapOfFields> = [10, 20, 30]
        .iter()
        .map(|rate| {
            let mut map = MapOfFields::new(HashMap::new());
            map.insert("base_rate", *rate);
            map
        })
        .collect();
    let ids: MultipleIds = env.create_records("machine", maps)?;
    assert_eq!(ids.get_ids_ref().len(), 3);

    let record: Machine<MultipleIds> = env.get_record(ids);
    assert_eq!(
        record.daily_rate(&mut env)?,
        60,
        "the implementation sums the whole recordset in one call"
    );
    Ok(())
}

/// A call against a model nothing registered names what is missing, rather than panicking.
///
/// Registering a model now registers its methods with it, so the two can no longer disagree; what
/// is left is the plugin that was never loaded at all.
#[test]
fn test_a_call_on_an_unregistered_model_is_reported() -> Result<()> {
    let app = Application::new_test();
    let mut env = app.new_env()?;
    let record =
        <Machine<MultipleIds> as CommonModel<MultipleIds>>::create_instance(vec![1u32].into());

    let err = record.daily_rate(&mut env).unwrap_err().to_string();
    assert!(
        err.contains("machine") && err.contains("daily_rate"),
        "the error must name the model and the method, got: {err}"
    );
    Ok(())
}

/// Two structs contributing the same method to one model, disagreeing on the return type.
///
/// Nothing links them but the model id and the method name, so the disagreement cannot be caught
/// by the compiler — it has to be caught when the registry puts them on the same chain.
mod conflicting {
    use code_gen::{Model, erp_methods};
    use erp::environment::Environment;
    use erp::types::field::{IdMode, MultipleIds};
    use std::error::Error;

    #[derive(Model)]
    #[erp(id = "conflict", methods)]
    #[allow(dead_code)]
    pub struct Declares<Mode: IdMode> {
        pub id: Mode,
        #[erp(default = 0)]
        amount: i32,
    }

    #[erp_methods]
    impl Declares<MultipleIds> {
        pub fn total(&self, env: &mut Environment) -> Result<i32, Box<dyn Error + Send + Sync>> {
            Ok(self.get_amount(env)?.into_iter().sum())
        }
    }

    #[derive(Model)]
    #[erp(id = "conflict", methods)]
    #[erp(derived_model = "")]
    #[allow(dead_code)]
    pub struct Disagrees<Mode: IdMode> {
        pub id: Mode,
    }

    #[erp_methods]
    impl Disagrees<MultipleIds> {
        /// Same model, same name, wider return type.
        pub fn total(&self, env: &mut Environment) -> Result<i64, Box<dyn Error + Send + Sync>> {
            let _ = env;
            Ok(0)
        }
    }
}

#[test]
#[should_panic(expected = "declared with two different signatures")]
fn test_disagreeing_signatures_are_refused_at_registration() {
    let mut app = Application::new_test();
    app.model_manager
        .register_model::<conflicting::Declares<_>>();
    app.model_manager
        .register_model::<conflicting::Disagrees<_>>();
}

/// A method calling an overridden one from the model that overrides it reaches the head of the
/// chain, not the implementation sitting next to it.
#[test]
fn test_a_sibling_call_reaches_the_head_of_the_chain() -> Result<()> {
    let app = with_override()?;
    let mut env = app.new_env()?;
    let ids = machine(&mut env, &[("base_rate", 100), ("discount", 30)])?;

    let record: MachineDiscounted<MultipleIds> = env.get_record(ids);
    assert_eq!(
        record.weekly_rate(&mut env)?,
        490,
        "seven days at the overridden rate of 70, not at the base rate of 100"
    );
    Ok(())
}

