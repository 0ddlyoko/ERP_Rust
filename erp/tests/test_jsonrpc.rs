//! The JSON-RPC 2.0 interface, exercised without a socket.
//!
//! The protocol layer knows nothing about HTTP, so neither does this.

use erp::app::Application;
use erp::jsonrpc;
use serde_json::{Value, json};
use std::error::Error;
use test_plugin::TestPlugin;
use test_utilities::TestLibPlugin;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

fn new_app() -> Result<Application> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    app.register_plugin(Box::new(TestPlugin {}))?;
    app.load_plugin("test_plugin")?;
    Ok(app)
}

fn call(app: &Application, method: &str, params: Value) -> Value {
    let body = json!({"jsonrpc": "2.0", "method": method, "params": params, "id": 1}).to_string();
    jsonrpc::handle(app, None, &body).expect("a request with an id is owed an answer")
}

fn result(app: &Application, method: &str, params: Value) -> Value {
    let answer = call(app, method, params);
    assert!(
        answer.get("error").is_none(),
        "expected a result, got {answer}"
    );
    answer["result"].clone()
}

fn error_code(answer: &Value) -> i64 {
    answer["error"]["code"].as_i64().expect("an error code")
}

/// Create, then find what was created, then read it back.
#[test]
fn test_the_built_in_operations_round_trip() -> Result<()> {
    let app = new_app()?;

    let created = result(
        &app,
        "machine.create",
        json!({"values": {"name": "digger", "base_rate": 120, "days": 2}}),
    );
    let ids = created.as_array().expect("a list of ids");
    assert_eq!(ids.len(), 1);

    let found = result(
        &app,
        "machine.search",
        json!({"domain": [["name", "=", "digger"]]}),
    );
    assert_eq!(
        found, created,
        "the search must find exactly what was created"
    );

    let rows = result(
        &app,
        "machine.read",
        json!({"ids": ids, "fields": ["name", "base_rate"]}),
    );
    assert_eq!(rows[0]["name"], json!("digger"));
    assert_eq!(rows[0]["base_rate"], json!(120));
    Ok(())
}

/// Each request runs in its own transaction, and a successful one commits.
#[test]
fn test_a_call_commits_and_the_next_one_sees_it() -> Result<()> {
    let app = new_app()?;
    result(&app, "machine.create", json!({"values": {"name": "kept"}}));

    assert_eq!(
        result(&app, "machine.count", json!({"domain": []})),
        json!(1),
        "a later request opens its own environment and must see the committed record"
    );
    Ok(())
}

/// A call that failed after writing leaves nothing behind.
///
/// The environment is dropped rather than closed, so its transaction rolls back — which only
/// means anything for a call that got far enough to write.
#[test]
fn test_a_failed_call_rolls_back_what_it_wrote() -> Result<()> {
    let app = new_app()?;
    let created = result(
        &app,
        "machine.create",
        json!({"values": {"name": "before"}}),
    );

    let answer = call(
        &app,
        "machine.refuse_after_writing",
        json!({"ids": created}),
    );
    assert_eq!(error_code(&answer), -32000, "got {answer}");

    let rows = result(
        &app,
        "machine.read",
        json!({"ids": created, "fields": ["name"]}),
    );
    assert_eq!(
        rows[0]["name"],
        json!("before"),
        "the write that preceded the failure must have gone with it"
    );
    Ok(())
}

/// A call refused before it wrote anything is reported as the caller's mistake.
#[test]
fn test_unreadable_values_are_the_callers_mistake() -> Result<()> {
    let app = new_app()?;
    let answer = call(
        &app,
        "machine.create",
        json!({"values": {"name": "half", "base_rate": "not a number"}}),
    );
    assert_eq!(error_code(&answer), -32602, "got {answer}");
    assert_eq!(
        result(&app, "machine.count", json!({"domain": []})),
        json!(0)
    );
    Ok(())
}

/// Write and delete, with paging and ordering on the way.
#[test]
fn test_write_delete_and_paging() -> Result<()> {
    let app = new_app()?;
    for rate in [30, 10, 20] {
        result(
            &app,
            "machine.create",
            json!({"values": {"name": format!("m{rate}"), "base_rate": rate}}),
        );
    }

    let ordered = result(
        &app,
        "machine.read_matching",
        json!({"domain": [], "fields": ["base_rate"], "order": ["base_rate asc"], "limit": 2}),
    );
    let rates: Vec<i64> = ordered
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["base_rate"].as_i64().unwrap())
        .collect();
    assert_eq!(rates, vec![10, 20], "ordered, then limited");

    let all = result(&app, "machine.search", json!({"domain": []}));
    let ids = all.as_array().unwrap().clone();
    result(
        &app,
        "machine.write",
        json!({"ids": ids, "values": {"name": "renamed"}}),
    );
    assert_eq!(
        result(
            &app,
            "machine.count",
            json!({"domain": [["name", "=", "renamed"]]})
        ),
        json!(3)
    );

    assert_eq!(
        result(&app, "machine.delete", json!({"ids": ids})),
        json!(3)
    );
    assert_eq!(
        result(&app, "machine.count", json!({"domain": []})),
        json!(0)
    );
    Ok(())
}

/// An exposed method is reachable, and lands on the override chain.
#[test]
fn test_an_exposed_method_is_reachable_and_overridable() -> Result<()> {
    let app = new_app()?;
    let created = result(
        &app,
        "machine.create",
        json!({"values": {"name": "m", "base_rate": 100, "days": 3, "discount": 30}}),
    );

    assert_eq!(
        result(&app, "machine.quote", json!({"ids": created})),
        json!(210),
        "three days at the overridden rate of 70"
    );
    assert_eq!(
        result(
            &app,
            "machine.quote_for",
            json!({"ids": created, "args": {"days": 2}})
        ),
        json!(140)
    );
    Ok(())
}

/// What is not reachable answers as if it were not there.
#[test]
fn test_what_is_not_exposed_is_not_found() -> Result<()> {
    let app = new_app()?;
    for method in [
        "machine.internal_rate",
        "machine.no_such_thing",
        "no_such_model.search",
    ] {
        let answer = call(&app, method, json!({"ids": []}));
        assert_eq!(error_code(&answer), -32601, "for {method}, got {answer}");
    }
    Ok(())
}

/// The envelope is checked, not trusted.
#[test]
fn test_a_malformed_envelope_is_refused() -> Result<()> {
    let app = new_app()?;

    let broken = jsonrpc::handle(&app, None, "{ not json").expect("an answer");
    assert_eq!(error_code(&broken), -32700);

    let wrong_version = jsonrpc::handle(
        &app,
        None,
        &json!({"jsonrpc": "1.0", "method": "machine.count", "params": {}, "id": 1}).to_string(),
    )
    .expect("an answer");
    assert_eq!(error_code(&wrong_version), -32600);

    let bad_params = call(&app, "machine.read", json!({"ids": [1]}));
    assert_eq!(
        bad_params["error"]["code"],
        json!(-32602),
        "fields is required"
    );
    Ok(())
}

/// A notification is owed nothing, and still runs.
#[test]
fn test_a_notification_runs_and_answers_nothing() -> Result<()> {
    let app = new_app()?;
    let body = json!({
        "jsonrpc": "2.0",
        "method": "machine.create",
        "params": {"values": {"name": "quiet"}}
    })
    .to_string();

    assert!(
        jsonrpc::handle(&app, None, &body).is_none(),
        "no id, no answer"
    );
    assert_eq!(
        result(&app, "machine.count", json!({"domain": []})),
        json!(1),
        "but the work was done"
    );
    Ok(())
}

/// A batch answers each request, and one failing does not undo the others.
#[test]
fn test_a_batch_answers_each_request_independently() -> Result<()> {
    let app = new_app()?;
    let body = json!([
        {"jsonrpc": "2.0", "method": "machine.create", "params": {"values": {"name": "one"}}, "id": 1},
        {"jsonrpc": "2.0", "method": "machine.nope", "params": {}, "id": 2},
        {"jsonrpc": "2.0", "method": "machine.create", "params": {"values": {"name": "two"}}, "id": 3}
    ])
    .to_string();

    let answers = jsonrpc::handle(&app, None, &body).expect("an answer per request");
    let answers = answers.as_array().expect("a list");
    assert_eq!(answers.len(), 3);
    assert_eq!(answers[1]["error"]["code"], json!(-32601));
    assert_eq!(answers[0]["id"], json!(1));
    assert_eq!(answers[2]["id"], json!(3));

    assert_eq!(
        result(&app, "machine.count", json!({"domain": []})),
        json!(2),
        "the two that worked must have kept their work"
    );
    Ok(())
}

/// An empty batch is a malformed request, per the specification.
#[test]
fn test_an_empty_batch_is_refused() -> Result<()> {
    let app = new_app()?;
    let answer = jsonrpc::handle(&app, None, "[]").expect("an answer");
    assert_eq!(error_code(&answer), -32600);
    Ok(())
}

/// Every operation the protocol claims to answer to, answers.
///
/// The compiler keeps the set of names and the set the dispatcher handles in step; what it
/// cannot check is that a variant was left out of `Verb::ALL`, so that is checked here.
#[test]
fn test_every_reserved_name_reaches_an_operation() -> Result<()> {
    let app = new_app()?;
    for name in erp::jsonrpc::reserved_names() {
        let answer = call(
            &app,
            &format!("machine.{name}"),
            json!({"ids": [], "fields": []}),
        );
        assert_ne!(
            answer["error"]["code"],
            json!(-32601),
            "\"{name}\" is reserved, so it must not answer as if it did not exist: {answer}"
        );
    }
    Ok(())
}

/// And a model method may not take one of those names.
#[test]
fn test_reserved_names_are_not_model_methods() -> Result<()> {
    let app = new_app()?;
    let exposed = app.model_manager.rpc.names();
    for name in erp::jsonrpc::reserved_names() {
        assert!(
            !exposed
                .iter()
                .any(|entry| entry.ends_with(&format!(".{name}"))),
            "\"{name}\" is reserved but a model exposed it: {exposed:?}"
        );
    }
    Ok(())
}

// ---- describing fields ----

/// Each field's kind, label, constraints, relation and default, as a client needs to show and
/// edit it — `id` included, since every record has one.
#[test]
fn test_fields_get_describes_every_field() -> Result<()> {
    let app = new_app()?;
    let fields = result(&app, "sale_order_line.fields_get", json!({}));
    let names: Vec<&String> = fields.as_object().expect("by name").keys().collect();
    assert_eq!(
        names,
        vec![
            "amount",
            "id",
            "order",
            "order_tags",
            "price",
            "siblings_total",
            "total_price"
        ]
    );
    assert_eq!(
        fields["price"],
        json!({"type": "integer", "label": "price", "required": true, "readonly": false,
               "stored": true, "default": 42})
    );
    assert_eq!(fields["order"]["relation"], "sale_order");
    assert_eq!(fields["order"]["relation_kind"], "many2one");
    assert_eq!(fields["id"]["readonly"], true);
    assert_eq!(fields["total_price"]["readonly"], true, "computed");
    assert_eq!(fields["total_price"]["stored"], true);
    assert_eq!(fields["siblings_total"]["stored"], false);

    let lines = result(&app, "sale_order.fields_get", json!({"fields": ["lines"]}));
    assert_eq!(lines["lines"]["relation_kind"], "one2many");
    assert_eq!(lines["lines"]["relation"], "sale_order_line");
    Ok(())
}

/// Only the fields asked for; one the model does not have is refused, naming it.
#[test]
fn test_fields_get_describes_what_was_asked() -> Result<()> {
    let app = new_app()?;
    let fields = result(
        &app,
        "sale_order_line.fields_get",
        json!({"fields": ["price", "id"]}),
    );
    assert_eq!(fields.as_object().map(|fields| fields.len()), Some(2));

    let answer = call(
        &app,
        "sale_order_line.fields_get",
        json!({"fields": ["nowhere"]}),
    );
    assert!(
        answer["error"]["message"]
            .as_str()
            .is_some_and(|m| m.contains("nowhere")),
        "{answer}"
    );
    assert_eq!(
        error_code(&call(&app, "nobody.fields_get", json!({}))),
        -32601
    );
    Ok(())
}

// ---- naming records ----

fn create(app: &Application, model: &str, values: Value) -> u64 {
    result(app, &format!("{model}.create"), json!({"values": values}))[0]
        .as_u64()
        .expect("an id")
}

/// A record is named by its `name`, or by the field its model declares; a model naming none, and
/// a record that does not exist, have no name.
#[test]
fn test_names_are_read_from_the_naming_field() -> Result<()> {
    let app = new_app()?;
    let order = create(&app, "sale_order", json!({"name": "S00042"}));
    assert_eq!(
        result(&app, "sale_order.names", json!({"ids": [order, 999]})),
        json!([[order, "S00042"], [999, null]])
    );

    let reading = create(
        &app,
        "meter_reading",
        json!({"reference": "M-7", "value": "1.5", "read_on": "2026-10-02"}),
    );
    assert_eq!(
        result(&app, "meter_reading.names", json!({"ids": [reading]})),
        json!([[reading, "M-7"]]),
        "named by the field it declares"
    );

    let line = create(&app, "sale_order_line", json!({"order": order}));
    assert_eq!(
        result(&app, "sale_order_line.names", json!({"ids": [line]})),
        json!([[line, null]]),
        "a model with no name field names nothing"
    );
    Ok(())
}

/// Asked for, every many2one read comes as `[id, name]`, so a list shows names in one call.
#[test]
fn test_a_read_names_its_references_when_asked() -> Result<()> {
    let app = new_app()?;
    let order = create(&app, "sale_order", json!({"name": "S00042"}));
    let line = create(&app, "sale_order_line", json!({"order": order, "price": 3}));
    let empty = create(&app, "sale_order_line", json!({"price": 4}));

    let rows = result(
        &app,
        "sale_order_line.read_matching",
        json!({"domain": [], "fields": ["order", "price"], "order": ["id asc"], "names": true}),
    );
    assert_eq!(rows[0]["id"], json!(line));
    assert_eq!(rows[0]["order"], json!([order, "S00042"]));
    assert_eq!(rows[0]["price"], json!(3), "only references change");
    assert_eq!(rows[1]["id"], json!(empty));
    assert_eq!(
        rows[1]["order"],
        Value::Null,
        "no reference, nothing to name"
    );

    let rows = result(
        &app,
        "sale_order_line.read",
        json!({"ids": [line], "fields": ["order"], "names": true}),
    );
    assert_eq!(rows[0]["order"], json!([order, "S00042"]));
    let plain = result(
        &app,
        "sale_order_line.read",
        json!({"ids": [line], "fields": ["order"]}),
    );
    assert_eq!(plain[0]["order"], json!(order), "the id alone unless asked");
    Ok(())
}

mod misnamed {
    use code_gen::Model;
    use erp::types::field::IdMode;

    #[derive(Model)]
    #[erp(id = "misnamed", name_field = "title")]
    #[allow(dead_code)]
    pub struct Misnamed<Mode: IdMode> {
        pub id: Mode,
        #[erp(default = "")]
        name: String,
    }
}

/// A model named by a field it does not have is refused when registered, not shown as ids.
#[test]
#[should_panic(expected = "named by its field \"title\", which it does not have")]
fn test_a_name_field_the_model_lacks_is_refused() {
    let mut app = Application::new_test();
    app.model_manager.register_model::<misnamed::Misnamed<_>>();
    app.model_manager.post_register();
}
