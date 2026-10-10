//! Enums a field holds: declared with `#[selection]`, stored by key, extended by other plugins.

use code_gen::selection;
use erp::app::Application;
use erp::jsonrpc;
use erp::model::{ModelManager, Selections};
use erp::plugin::Plugin;
use erp::types::field::{Selection, SingleId};
use erp_types::model::MapOfFields;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::error::Error;
use test_utilities::TestLibPlugin;
use test_utilities::models::{SaleOrder, SaleOrderState};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

#[selection]
pub enum A {
    A,
    B,
    C,
}

#[selection(extends = A)]
pub enum B {
    A,
    #[selection(after = "b")]
    D,
}

#[selection(extends = A)]
pub enum C {
    #[selection(after = "b")]
    E,
}

#[selection(extends = B)]
pub enum D {
    B,
    C,
    D,
    #[selection(after = "e")]
    F,
}

fn keys(selections: &Selections) -> Vec<String> {
    selections
        .choices(A::FAMILY)
        .iter()
        .map(|choice| choice.key.clone())
        .collect()
}

/// Enums extending one another name the same values, add theirs where they say, and keep the
/// order they were registered in for values placed at the same spot.
#[test]
fn test_extending_enums_build_one_family() {
    let mut selections = Selections::default();
    selections.extend::<C>();
    assert_eq!(keys(&selections), ["a", "b", "e", "c"]);
    selections.extend::<D>();
    assert_eq!(
        keys(&selections),
        ["a", "b", "e", "f", "d", "c"],
        "D applies B first, which places d after b behind e"
    );
    selections.extend::<D>();
    assert_eq!(
        keys(&selections),
        ["a", "b", "e", "f", "d", "c"],
        "once only"
    );

    let mut selections = Selections::default();
    selections.extend::<B>();
    selections.extend::<C>();
    assert_eq!(
        keys(&selections),
        ["a", "b", "d", "e", "c"],
        "d was placed after b first"
    );
}

/// Naming a value of the family is the same value, whichever enum names it.
#[test]
fn test_every_enum_of_a_family_names_the_same_values() {
    assert_eq!(A::B.key(), D::B.key());
    assert!(A::C.is(D::C));
    assert_eq!(D::B.to::<A>(), A::B);
    assert_eq!(A::C.to::<D>(), D::C);
    assert_eq!(A::C.to::<B>(), B::Extended(A::C.key()), "B does not name c");
    assert_eq!(D::F.to::<A>().key(), "f");
    assert_eq!(
        A::from_key("f"),
        A::from_key("f"),
        "the same key, the same value"
    );
}

#[selection(extends = A)]
pub enum Moved {
    #[selection(before = "a", label = "First C")]
    C,
    #[selection(label = "Bee")]
    B,
}

/// Naming a value may move it or relabel it; its key never changes.
#[test]
fn test_an_extension_moves_and_relabels() {
    let mut selections = Selections::default();
    selections.extend::<Moved>();
    let choices: Vec<(String, String)> = selections
        .choices(A::FAMILY)
        .iter()
        .map(|choice| (choice.key.clone(), choice.label.clone()))
        .collect();
    assert_eq!(
        choices,
        [
            ("c".to_string(), "First C".to_string()),
            ("a".to_string(), "A".to_string()),
            ("b".to_string(), "Bee".to_string()),
        ]
    );
}

#[selection(extends = A)]
pub enum Nowhere {
    #[selection(after = "zz")]
    G,
}

#[test]
#[should_panic(expected = "\"zz\", which its family does not have")]
fn test_a_value_placed_next_to_nothing_is_refused() {
    Selections::default().extend::<Nowhere>();
}

#[selection(extends = A)]
pub enum Circle {
    #[selection(after = "y")]
    X,
    #[selection(after = "x")]
    Y,
}

#[test]
#[should_panic(expected = "in a circle")]
fn test_values_placed_in_a_circle_are_refused() {
    Selections::default().extend::<Circle>();
}

/// Keys and labels come from the variants' names.
#[test]
fn test_keys_and_labels_come_from_names() {
    #[selection]
    pub enum Stage {
        QuotationSent,
        #[selection(key = "done", label = "All done")]
        Finished,
    }
    let values: Vec<(&str, &str)> = Stage::VALUES
        .iter()
        .map(|value| (value.key, value.label))
        .collect();
    assert_eq!(
        values,
        [("quotation_sent", "Quotation sent"), ("done", "All done")]
    );
    assert_eq!(Stage::Finished.key(), "done");
    assert_eq!(Stage::from_key("done"), Stage::Finished);
}

#[selection(extends = SaleOrderState)]
pub enum Shipping {
    Sent,
    #[selection(after = "sent", label = "Shipped")]
    Shipped,
}

struct ShippingPlugin;

impl Plugin for ShippingPlugin {
    fn name(&self) -> String {
        "shipping".to_string()
    }

    fn init_models(&self, model_manager: &mut ModelManager) {
        model_manager.register_selection::<Shipping>();
    }

    fn get_depends(&self) -> Vec<String> {
        vec!["test_lib_plugin".to_string()]
    }
}

fn new_app() -> Result<Application> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    app.register_plugin(Box::new(ShippingPlugin {}))?;
    app.load_plugin("shipping")?;
    Ok(app)
}

fn call(app: &Application, method: &str, params: Value) -> Value {
    let body = json!({"jsonrpc": "2.0", "method": method, "params": params, "id": 1}).to_string();
    jsonrpc::handle(app, None, &body).expect("an answer")
}

/// A field holding an enum is described with its values in order, those added by other plugins
/// among them.
#[test]
fn test_a_selection_is_described_with_its_values() -> Result<()> {
    let app = new_app()?;
    let answer = call(&app, "sale_order.fields_get", json!({"fields": ["state"]}));
    let state = &answer["result"]["state"];
    assert_eq!(state["type"], "selection", "{answer}");
    assert_eq!(
        state["values"],
        json!([
            ["draft", "Draft"],
            ["sent", "Sent"],
            ["shipped", "Shipped"],
            ["paid", "Paid"],
            ["cancelled", "Cancelled"]
        ])
    );
    assert_eq!(state["default"], "draft");
    Ok(())
}

/// A key the family lacks is refused, written or searched for; one another plugin added is not.
#[test]
fn test_unknown_keys_are_refused() -> Result<()> {
    let app = new_app()?;
    let created = call(
        &app,
        "sale_order.create",
        json!({"values": {"name": "S1", "state": "shipped"}}),
    );
    let id = created["result"][0].clone();
    assert!(
        id.is_number(),
        "a key added by a plugin is a value: {created}"
    );

    let refused = call(
        &app,
        "sale_order.write",
        json!({"ids": [id], "values": {"state": "lost"}}),
    );
    let message = refused["error"]["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("\"lost\"") && message.contains("draft, sent, shipped"),
        "{refused}"
    );

    let refused = call(
        &app,
        "sale_order.search",
        json!({"domain": [["state", "in", ["paid", "payed"]]]}),
    );
    assert!(
        refused["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("\"payed\""),
        "{refused}"
    );
    let found = call(
        &app,
        "sale_order.search",
        json!({"domain": [["state", "=", "shipped"]]}),
    );
    assert_eq!(found["result"], json!([id]));
    Ok(())
}

/// In Rust, a field takes a value of any enum of its family, and reads back the same value.
#[test]
fn test_a_field_takes_any_enum_of_its_family() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env()?;
    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("name", "S2");
    let order: SaleOrder<SingleId> = env.create_new_record_from_map(map)?;
    assert_eq!(*order.get_state(&mut env)?, SaleOrderState::Draft);

    order.set_state(Shipping::Shipped, &mut env)?;
    let state = *order.get_state(&mut env)?;
    assert!(state.is(Shipping::Shipped));
    assert!(matches!(state, SaleOrderState::Extended(key) if key == "shipped"));

    order.set_state(SaleOrderState::Paid, &mut env)?;
    assert!(order.get_state(&mut env)?.is(SaleOrderState::Paid));
    env.close()?;
    Ok(())
}

/// An enum is compared with as it is in a domain, alone or in a list.
#[test]
fn test_an_enum_is_searched_as_it_is() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env()?;
    for state in ["draft", "sent", "paid"] {
        let mut order = MapOfFields::new(HashMap::new());
        order.insert("name", state);
        order.insert("state", SaleOrderState::from_key(state));
        env.create_records("sale_order", vec![order])?;
    }
    let paid = env.search_ids(
        "sale_order",
        &erp_search_code_gen::make_domain!([("state", "=", SaleOrderState::Paid)]),
    )?;
    assert_eq!(paid.len(), 1);
    let open = env.search_ids(
        "sale_order",
        &erp_search_code_gen::make_domain!([(
            "state",
            "in",
            vec![SaleOrderState::Draft, SaleOrderState::Sent]
        )]),
    )?;
    assert_eq!(open.len(), 2);
    Ok(())
}
