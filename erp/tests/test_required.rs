//! A required field is never left empty: not when a record is created, nor when it is written,
//! nor when a one2many lets go of a record whose many2one is required.

use erp::Result;
use erp::app::Application;
use erp::environment::Environment;
use erp_types::field::{IdMode, MultipleIds};
use erp_types::model::MapOfFields;
use serde_json::json;

mod storage {
    use code_gen::Model;
    use erp::types::field::{IdMode, MultipleIds, NaiveDate, Reference, SingleId};

    #[derive(Model)]
    #[erp(id = "shelf")]
    #[allow(dead_code)]
    pub struct Shelf<Mode: IdMode> {
        pub id: Mode,
        name: String,
        note: Option<String>,
        #[erp(inverse = "shelf")]
        labels: Reference<BaseLabel, MultipleIds>,
    }

    /// A date has no default: whoever writes in the diary gives it.
    #[derive(Model)]
    #[erp(id = "diary")]
    #[allow(dead_code)]
    pub struct Diary<Mode: IdMode> {
        pub id: Mode,
        day: NaiveDate,
        #[erp(default = 0)]
        pages: i32,
    }

    /// Never empty: a basket holds at least one fruit.
    #[derive(Model)]
    #[erp(id = "basket")]
    #[allow(dead_code)]
    pub struct Basket<Mode: IdMode> {
        pub id: Mode,
        #[erp(inverse = "basket", required)]
        fruits: Reference<BaseFruit, MultipleIds>,
    }

    #[derive(Model)]
    #[erp(id = "fruit")]
    #[allow(dead_code)]
    pub struct Fruit<Mode: IdMode> {
        pub id: Mode,
        basket: Reference<BaseBasket, SingleId>,
    }

    /// Never empty: a recipe holds at least one spice.
    #[derive(Model)]
    #[erp(id = "recipe")]
    #[allow(dead_code)]
    pub struct Recipe<Mode: IdMode> {
        pub id: Mode,
        #[erp(relation = "recipe_spice_rel", required)]
        spices: Reference<BaseSpice, MultipleIds>,
    }

    #[derive(Model)]
    #[erp(id = "spice")]
    #[allow(dead_code)]
    pub struct Spice<Mode: IdMode> {
        pub id: Mode,
        #[erp(relation = "recipe_spice_rel")]
        recipes: Reference<BaseRecipe, MultipleIds>,
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
    app.model_manager.register_model::<storage::Diary<_>>();
    app.model_manager.register_model::<storage::Basket<_>>();
    app.model_manager.register_model::<storage::Fruit<_>>();
    app.model_manager.register_model::<storage::Recipe<_>>();
    app.model_manager.register_model::<storage::Spice<_>>();
    app.model_manager.post_register();
    app
}

fn named(name: &str) -> MapOfFields {
    let mut values = MapOfFields::default();
    values.insert("name", name);
    values
}

fn create(env: &mut Environment, model: &str, values: MapOfFields) -> Result<u32> {
    Ok(env.create_records(model, vec![values])?.get_ids_ref()[0])
}

/// Emptying a required field is refused, over the wire as from code; an optional one is not.
#[test]
fn test_a_required_field_is_not_emptied() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let shelf = create(&mut env, "shelf", named("shelf"))?;

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
    create(&mut env, "shelf", named("shelf"))?;

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
    let shelf = create(&mut env, "shelf", named("shelf"))?;
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

fn refused_as_required(result: Result<serde_json::Value>, model: &str, field: &str) {
    let error = result.expect_err("refused").to_string();
    assert!(
        error.contains(&format!("\"{field}\" of model \"{model}\" is required")),
        "{error}"
    );
}

/// A required one2many holds at least one record: when created, written, or when its last
/// record moves elsewhere or is deleted. Replacing its records is no emptying.
///
/// Each refused change gets a unit of work of its own, rolled back as a request's would be.
#[test]
fn test_a_required_one2many_is_never_empty() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    refused_as_required(
        env.call_rpc("basket", "create", &json!({"values": {}})),
        "basket",
        "fruits",
    );
    drop(env);

    let mut env = app.new_env()?;
    let ids = env.call_rpc(
        "basket",
        "create",
        &json!({"values": [{"fruits": {"create": [{}]}}, {"fruits": {"create": [{}]}}]}),
    )?;
    let (basket, other) = (ids[0].clone(), ids[1].clone());
    let fruits = |env: &mut Environment| -> Result<serde_json::Value> {
        Ok(env.call_rpc(
            "basket",
            "read",
            &json!({"ids": [basket], "fields": ["fruits"]}),
        )?[0]["fruits"]
            .clone())
    };
    let first = fruits(&mut env)?[0].clone();
    env.call_rpc(
        "basket",
        "write",
        &json!({"ids": [basket], "values": {"fruits": {"unlink": [first], "create": [{}]}}}),
    )?;
    let last = fruits(&mut env)?[0].clone();
    env.close()?;

    let mut env = app.new_env()?;
    refused_as_required(
        env.call_rpc(
            "basket",
            "write",
            &json!({"ids": [basket], "values": {"fruits": []}}),
        ),
        "basket",
        "fruits",
    );
    drop(env);
    let mut env = app.new_env()?;
    refused_as_required(
        env.call_rpc(
            "fruit",
            "write",
            &json!({"ids": [last], "values": {"basket": other}}),
        ),
        "basket",
        "fruits",
    );
    drop(env);
    let mut env = app.new_env()?;
    refused_as_required(
        env.call_rpc("fruit", "delete", &json!({"ids": [last]})),
        "basket",
        "fruits",
    );
    drop(env);
    let mut env = app.new_env()?;
    env.call_rpc("basket", "delete", &json!({"ids": [basket]}))?;
    Ok(())
}

/// A required many2many holds at least one record, from whichever side it is written.
#[test]
fn test_a_required_many2many_is_never_empty() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let spice = env.call_rpc("spice", "create", &json!({"values": {}}))?[0].clone();
    let recipe =
        env.call_rpc("recipe", "create", &json!({"values": {"spices": [spice]}}))?[0].clone();
    env.close()?;

    let mut env = app.new_env()?;
    refused_as_required(
        env.call_rpc("recipe", "create", &json!({"values": {"spices": []}})),
        "recipe",
        "spices",
    );
    drop(env);
    let mut env = app.new_env()?;
    refused_as_required(
        env.call_rpc(
            "recipe",
            "write",
            &json!({"ids": [recipe], "values": {"spices": []}}),
        ),
        "recipe",
        "spices",
    );
    drop(env);
    let mut env = app.new_env()?;
    refused_as_required(
        env.call_rpc(
            "spice",
            "write",
            &json!({"ids": [spice], "values": {"recipes": []}}),
        ),
        "recipe",
        "spices",
    );
    Ok(())
}

/// Empty text is no text: refused for a required field, kept as nothing for an optional one.
#[test]
fn test_empty_text_is_no_text() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let shelf = create(&mut env, "shelf", named("shelf"))?;
    env.close()?;

    let mut env = app.new_env()?;
    refused_as_required(
        env.call_rpc(
            "shelf",
            "write",
            &json!({"ids": [shelf], "values": {"name": ""}}),
        ),
        "shelf",
        "name",
    );
    drop(env);
    let mut env = app.new_env()?;
    refused_as_required(
        env.call_rpc("shelf", "create", &json!({"values": {"name": ""}})),
        "shelf",
        "name",
    );
    drop(env);

    let mut env = app.new_env()?;
    env.call_rpc(
        "shelf",
        "write",
        &json!({"ids": [shelf], "values": {"note": ""}}),
    )?;
    let rows = env.call_rpc(
        "shelf",
        "read",
        &json!({"ids": [shelf], "fields": ["note"]}),
    )?;
    assert_eq!(rows[0]["note"], serde_json::Value::Null);
    Ok(())
}

/// A date is given by whoever creates the record; a number starts at its default.
#[test]
fn test_a_date_has_no_implicit_default() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    refused_as_required(
        env.call_rpc("diary", "create", &json!({"values": {}})),
        "diary",
        "day",
    );
    drop(env);
    let mut env = app.new_env()?;
    let ids = env.call_rpc("diary", "create", &json!({"values": {"day": "2026-10-04"}}))?;
    let rows = env.call_rpc("diary", "read", &json!({"ids": ids, "fields": ["pages"]}))?;
    assert_eq!(rows[0]["pages"], 0);
    Ok(())
}
