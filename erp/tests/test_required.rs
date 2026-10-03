//! A required field is never left empty: not when a record is created, nor when it is written,
//! nor when a one2many lets go of a record whose many2one is required.

use erp::app::Application;
use erp::environment::Environment;
use erp_types::field::{IdMode, MultipleIds};
use erp_types::model::MapOfFields;
use serde_json::json;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

mod storage {
    use code_gen::Model;
    use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};

    #[derive(Model)]
    #[erp(id = "shelf")]
    #[allow(dead_code)]
    pub struct Shelf<Mode: IdMode> {
        pub id: Mode,
        #[erp(default = "")]
        name: String,
        note: Option<String>,
        #[erp(inverse = "shelf")]
        labels: Reference<BaseLabel, MultipleIds>,
    }

    #[derive(Model)]
    #[erp(id = "label")]
    #[allow(dead_code)]
    pub struct Label<Mode: IdMode> {
        pub id: Mode,
        #[erp(required)]
        shelf: Reference<BaseShelf, SingleId>,
    }
}

fn new_app() -> Application {
    let mut app = Application::new_test();
    app.model_manager.register_model::<storage::Shelf<_>>();
    app.model_manager.register_model::<storage::Label<_>>();
    app.model_manager.post_register();
    app
}

fn create(env: &mut Environment, model: &str, values: MapOfFields) -> Result<u32> {
    Ok(env.create_records(model, vec![values])?.get_ids_ref()[0])
}

/// Emptying a required field is refused, over the wire as from code; an optional one is not.
#[test]
fn test_a_required_field_is_not_emptied() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let shelf = create(&mut env, "shelf", MapOfFields::default())?;

    let refused = env.call_rpc(
        "shelf",
        "write",
        &json!({"ids": [shelf], "values": {"name": null}}),
    );
    assert_eq!(
        refused.expect_err("refused").to_string(),
        "Field \"name\" of model \"shelf\" is required: it cannot be left empty"
    );
    env.call_rpc(
        "shelf",
        "write",
        &json!({"ids": [shelf], "values": {"note": null}}),
    )?;
    Ok(())
}

/// A record is not created with a required field left empty, its defaults given first.
#[test]
fn test_a_record_is_not_created_without_a_required_field() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    create(&mut env, "shelf", MapOfFields::default())?;

    let refused = env.call_rpc("label", "create", &json!({"values": {}}));
    assert!(
        refused
            .expect_err("refused")
            .to_string()
            .contains("\"shelf\" of model \"label\" is required")
    );
    Ok(())
}

/// A one2many cannot let go of a record whose many2one is required: it would point nowhere.
/// Deleting that record is still possible.
#[test]
fn test_a_one2many_keeps_what_needs_it() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let shelf = create(&mut env, "shelf", MapOfFields::default())?;
    let mut values = MapOfFields::default();
    values.insert("shelf", shelf);
    let label = create(&mut env, "label", values)?;

    let refused = env.call_rpc(
        "shelf",
        "write",
        &json!({"ids": [shelf], "values": {"labels": []}}),
    );
    assert!(
        refused
            .expect_err("the label needs its shelf")
            .to_string()
            .contains("\"shelf\" of model \"label\" is required")
    );

    env.delete("label", &MultipleIds::from(vec![label]))?;
    env.call_rpc(
        "shelf",
        "write",
        &json!({"ids": [shelf], "values": {"labels": []}}),
    )?;
    Ok(())
}
