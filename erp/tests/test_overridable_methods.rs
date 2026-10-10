use erp::Result;
use erp::app::Application;
use erp_types::field::{IdMode, MultipleIds, SingleId};
use erp_types::model::{CommonModel, MapOfFields};
use std::collections::HashMap;
use test_plugin::TestPlugin;
use test_plugin::models::machine_discounted::MachineDiscounted;
use test_utilities::TestLibPlugin;
use test_utilities::models::Machine;

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

/// Methods that call each other without returning.
///
/// Dispatch always restarts from the head of the chain, so a method reaching its own name — or
/// two of them reaching each other — never comes back. The stack dies around 1300 frames, which
/// takes the process with it; this has to be caught before that.
mod cycles {
    use code_gen::{Model, erp_methods};
    use erp::environment::Environment;
    use erp::types::field::{IdMode, MultipleIds};
    use std::error::Error;

    #[derive(Model)]
    #[erp(id = "cycle", methods)]
    #[allow(dead_code)]
    pub struct Cycle<Mode: IdMode> {
        pub id: Mode,
        #[erp(default = 0)]
        depth: i32,
    }

    #[erp_methods]
    impl Cycle<MultipleIds> {
        /// Reaches its own name, which dispatch sends back to the head.
        pub fn straight_at_itself(
            &self,
            env: &mut Environment,
        ) -> Result<i32, Box<dyn Error + Send + Sync>> {
            self.straight_at_itself(env)
        }

        /// The shape a real cycle takes: two methods, neither of them obviously wrong.
        pub fn ping(&self, env: &mut Environment) -> Result<i32, Box<dyn Error + Send + Sync>> {
            self.pong(env)
        }

        pub fn pong(&self, env: &mut Environment) -> Result<i32, Box<dyn Error + Send + Sync>> {
            self.ping(env)
        }

        /// Nesting that terminates must stay unaffected.
        pub fn shallow(&self, env: &mut Environment) -> Result<i32, Box<dyn Error + Send + Sync>> {
            Ok(*self.get_depth(env)?.first().copied().unwrap_or(&0))
        }

        pub fn calls_shallow(
            &self,
            env: &mut Environment,
        ) -> Result<i32, Box<dyn Error + Send + Sync>> {
            Ok(self.shallow(env)? + 1)
        }
    }
}

fn cycling_app() -> Result<(Application, MultipleIds)> {
    let mut app = Application::new_test();
    app.model_manager.register_model::<cycles::Cycle<_>>();
    app.model_manager.post_register();
    let mut env = app.new_env()?;
    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("depth", 41);
    let ids = env.create_records("cycle", vec![map])?;
    env.close()?;
    Ok((app, ids))
}

/// A method reaching its own name is reported, not crashed on.
#[test]
fn test_a_direct_cycle_is_refused() -> Result<()> {
    let (app, ids) = cycling_app()?;
    let mut env = app.new_env()?;
    let record: cycles::Cycle<MultipleIds> = env.get_record(ids);

    let err = record.straight_at_itself(&mut env).unwrap_err().to_string();
    assert!(
        err.contains("Maximum call depth"),
        "expected the depth guard, got: {err}"
    );
    assert!(
        err.contains("cycle.straight_at_itself"),
        "the error must name the method, got: {err}"
    );
    Ok(())
}

/// The shape that actually happens: two methods calling each other.
#[test]
fn test_an_indirect_cycle_names_both_methods() -> Result<()> {
    let (app, ids) = cycling_app()?;
    let mut env = app.new_env()?;
    let record: cycles::Cycle<MultipleIds> = env.get_record(ids);

    let err = record.ping(&mut env).unwrap_err().to_string();
    assert!(
        err.contains("cycle.ping") && err.contains("cycle.pong"),
        "the error must show the cycle, not just a depth, got: {err}"
    );
    Ok(())
}

/// Nesting that returns is untouched, and the stack is left clean for the next call.
#[test]
fn test_nesting_that_terminates_still_works() -> Result<()> {
    let (app, ids) = cycling_app()?;
    let mut env = app.new_env()?;
    let record: cycles::Cycle<MultipleIds> = env.get_record(ids.clone());

    assert_eq!(record.calls_shallow(&mut env)?, 42);

    // A refused cycle must not leave frames behind.
    let _ = record.ping(&mut env);
    assert_eq!(
        record.calls_shallow(&mut env)?,
        42,
        "the call stack must unwind even when a call fails"
    );
    Ok(())
}

fn named_machine(env: &mut erp::environment::Environment, name: &str) -> Result<u32> {
    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("name", name);
    map.insert("base_rate", 100);
    Ok(env.create_records("machine", vec![map])?.get_ids_ref()[0])
}

/// A method of one record — on `Model<SingleId>` — is overridden like any other: the override
/// runs, and reaches the one below through `super`.
#[test]
fn test_a_method_of_one_record_is_overridden() -> Result<()> {
    let app = base_only()?;
    let mut env = app.new_env()?;
    let id = named_machine(&mut env, "Drill")?;
    let record: Machine<SingleId> = env.get_record(SingleId::from(id));
    assert_eq!(record.label(&mut env)?, "Drill at 100");

    let app = with_override()?;
    let mut env = app.new_env()?;
    let id = named_machine(&mut env, "Drill")?;
    let record: Machine<SingleId> = env.get_record(SingleId::from(id));
    assert_eq!(record.label(&mut env)?, "Drill at 100 (discounted)");
    let answer = env.call_rpc("machine", "label", &serde_json::json!({ "ids": [id] }))?;
    assert_eq!(answer, "Drill at 100 (discounted)");
    Ok(())
}

/// Called remotely on several records, a method of one record is refused rather than run on
/// the first.
#[test]
fn test_a_method_of_one_record_refuses_several() -> Result<()> {
    let app = with_override()?;
    let mut env = app.new_env()?;
    let first = named_machine(&mut env, "Drill")?;
    let second = named_machine(&mut env, "Saw")?;
    let error = env
        .call_rpc(
            "machine",
            "label",
            &serde_json::json!({ "ids": [first, second] }),
        )
        .expect_err("refused")
        .to_string();
    assert!(
        error.contains("label works on one record, not 2"),
        "{error}"
    );
    Ok(())
}

/// A method of the model — without `self` — is overridden too, and called remotely without
/// records.
#[test]
fn test_a_method_of_the_model_is_overridden() -> Result<()> {
    let app = base_only()?;
    let mut env = app.new_env()?;
    assert_eq!(Machine::<MultipleIds>::standard_rate(&mut env)?, 100);

    let app = with_override()?;
    let mut env = app.new_env()?;
    assert_eq!(Machine::<MultipleIds>::standard_rate(&mut env)?, 90);
    assert_eq!(
        MachineDiscounted::<MultipleIds>::standard_rate(&mut env)?,
        90
    );
    let answer = env.call_rpc("machine", "standard_rate", &serde_json::json!({}))?;
    assert_eq!(answer, 90);
    Ok(())
}

fn one_link(
    _ids: MultipleIds,
    _env: &mut dyn erp_types::environment::ErasedEnvironment,
    _args: &(),
    _sup: erp_types::method::Super<'_, (), i32>,
) -> std::result::Result<i32, Box<dyn std::error::Error + Send + Sync>> {
    Ok(1)
}

/// Overriding a method declared on one record with one on records — or the other way round — is
/// refused when the plugin loads, naming both.
#[test]
#[should_panic(expected = "declared on two different receivers")]
fn test_an_override_on_another_receiver_is_refused() {
    let mut registry = erp::internal_types::method::MethodRegistry::default();
    registry.register(
        "machine",
        "label",
        one_link,
        erp_types::method::Receiver::Record,
        "test_lib_plugin",
    );
    registry.register(
        "machine",
        "label",
        one_link,
        erp_types::method::Receiver::Records,
        "test_plugin",
    );
}
