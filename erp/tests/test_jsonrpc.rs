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
            "create_date",
            "id",
            "order",
            "order_tags",
            "price",
            "siblings_total",
            "total_price",
            "write_date"
        ]
    );
    assert_eq!(
        fields["create_date"],
        json!({"type": "datetime", "label": "Created on", "required": false, "readonly": true,
               "stored": true}),
        "filled in by the ORM"
    );
    assert_eq!(
        fields["price"],
        json!({"type": "integer", "label": "Price", "required": true, "readonly": false,
               "stored": true, "default": 42})
    );
    assert_eq!(
        fields["order_tags"]["label"], "Order Tags",
        "the name, made readable"
    );
    assert!(
        fields["price"].get("description").is_none(),
        "no help unless written"
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

/// A field's own label replaces its name, and its description is the help shown beside it.
#[test]
fn test_fields_get_gives_a_fields_label_and_description() -> Result<()> {
    let app = new_app()?;
    let fields = result(
        &app,
        "meter_reading.fields_get",
        json!({"fields": ["reference", "value"]}),
    );
    assert_eq!(fields["reference"]["label"], "Meter number");
    assert_eq!(
        fields["reference"]["description"],
        "As printed on the meter"
    );
    assert_eq!(fields["value"]["label"], "Value");
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

    let rows = result(
        &app,
        "sale_order.read",
        json!({"ids": [order], "fields": ["lines", "tags"], "names": true}),
    );
    assert_eq!(
        rows[0]["lines"],
        json!([[line, null]]),
        "a line has no name to give"
    );

    let found = result(
        &app,
        "sale_order_line.search",
        json!({"domain": [["order", "ilike", "%s0004%"]]}),
    );
    assert_eq!(
        found,
        json!([line]),
        "a relation is searched by its records' names"
    );

    let tag = create(&app, "tag", json!({"name": "urgent"}));
    result(
        &app,
        "sale_order.write",
        json!({"ids": [order], "values": {"tags": [tag]}}),
    );
    let found = result(
        &app,
        "sale_order.search",
        json!({"domain": [["tags", "ilike", "%urg%"]]}),
    );
    assert_eq!(found, json!([order]), "a many2many too");
    assert_eq!(rows[0]["tags"], json!([]), "no record, an empty list");
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

/// Ids a caller sends are each one record, and one that does not exist is refused by name rather
/// than met halfway through the work.
#[test]
fn test_ids_named_twice_or_missing_are_answered() -> Result<()> {
    let app = new_app()?;
    let order = create(&app, "sale_order", json!({"name": "S1"}));
    let line = create(&app, "sale_order_line", json!({"order": order, "price": 3}));

    let refused = call(
        &app,
        "sale_order_line.write",
        json!({"ids": [999], "values": {"order": null}}),
    );
    let message = refused["error"]["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("sale_order_line #999 does not exist"),
        "{refused}"
    );
    let refused = call(&app, "sale_order_line.delete", json!({"ids": [line, 999]}));
    assert!(
        refused["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("#999"),
        "{refused}"
    );

    let deleted = result(&app, "sale_order_line.delete", json!({"ids": [line, line]}));
    assert_eq!(deleted, json!(1), "named twice, deleted once");
    Ok(())
}

/// A call that panics is answered with an error and affects nothing else: what it wrote is
/// undone, and the other calls of its batch are answered.
#[test]
fn test_a_panic_fails_its_call_alone() -> Result<()> {
    let app = new_app()?;
    let machine = create(
        &app,
        "machine",
        json!({"name": "digger", "base_rate": 1, "days": 1}),
    );
    let body = json!([
        {"jsonrpc": "2.0", "method": "machine.explode", "params": {"ids": [machine], "args": {}}, "id": 1},
        {"jsonrpc": "2.0", "method": "machine.count", "params": {"domain": []}, "id": 2},
    ])
    .to_string();
    let answers = jsonrpc::handle(&app, None, &body).expect("a batch is answered");
    assert_eq!(answers[0]["error"]["code"], -32603, "{answers}");
    assert!(
        !answers[0]["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("on purpose"),
        "what panicked stays in the log: {answers}"
    );
    assert_eq!(answers[1]["result"], json!(1), "{answers}");
    let rows = result(
        &app,
        "machine.read",
        json!({"ids": [machine], "fields": ["name"]}),
    );
    assert_eq!(rows[0]["name"], "digger", "undone");
    Ok(())
}

/// A domain nesting operators too deeply is refused, however it is written: it never reaches
/// anything that would follow it to the end.
#[test]
fn test_a_domain_too_deep_is_refused() -> Result<()> {
    let app = new_app()?;
    let mut domain = Vec::new();
    for level in 0..20_000 {
        domain.push(json!(if level % 2 == 0 { "&" } else { "|" }));
        domain.push(json!(["name", "=", "x"]));
    }
    domain.push(json!(["name", "=", "y"]));
    let refused = call(&app, "machine.search", json!({"domain": domain}));
    assert!(
        refused["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("nests operators"),
        "{refused}"
    );

    let mut flat = vec![json!("|"); 49_999];
    flat.extend((0..50_000).map(|value| json!(["base_rate", "=", value])));
    let found = result(&app, "machine.search", json!({"domain": flat}));
    assert_eq!(found, json!([]), "a long OR is fine");
    Ok(())
}

/// A one2many or a many2many is written as commands, carried out in order on what it holds:
/// records changed by `{"id": ..}`, created by values alone — a line created for an order
/// pointing back to it — and taken out by `{"unlink": id}`, in one call, on create as on write.
#[test]
fn test_lines_are_created_and_changed_with_their_record() -> Result<()> {
    let app = new_app()?;
    let tag = create(&app, "tag", json!({"name": "urgent"}));
    let created = result(
        &app,
        "sale_order.create",
        json!({"values": {
            "name": "S1",
            "lines": {"create": [{"price": 10, "amount": 2}, {"price": 4}]},
            "tags": {"link": [tag], "create": [{"name": "new"}]},
        }}),
    );
    let order = created[0].clone();
    let rows = result(
        &app,
        "sale_order_line.read_matching",
        json!({"domain": [["order", "=", order]], "fields": ["price", "amount"], "order": ["id asc"]}),
    );
    assert_eq!(rows.as_array().map_or(0, Vec::len), 2, "{rows}");
    let first = rows[0]["id"].clone();
    let second = rows[1]["id"].clone();
    assert_eq!(rows[0]["price"], 10);
    let tags = result(
        &app,
        "sale_order.read",
        json!({"ids": [order], "fields": ["tags"], "names": true}),
    );
    assert_eq!(tags[0]["tags"][1][1], "new", "{tags}");

    result(
        &app,
        "sale_order.write",
        json!({"ids": [order], "values": {"lines": {"unlink": [second], "update": [{"id": first, "price": 12}], "create": [{"price": 7}]}}}),
    );
    let rows = result(
        &app,
        "sale_order_line.read_matching",
        json!({"domain": [["order", "=", order]], "fields": ["price"], "order": ["id asc"]}),
    );
    let prices: Vec<Value> = rows
        .as_array()
        .expect("rows")
        .iter()
        .map(|row| row["price"].clone())
        .collect();
    assert_eq!(
        prices,
        [json!(12), json!(7)],
        "changed, created, and the second one let go"
    );
    let let_go = result(
        &app,
        "sale_order_line.read",
        json!({"ids": [second], "fields": ["order"]}),
    );
    assert_eq!(
        let_go[0]["order"],
        Value::Null,
        "a line the order does not own is only detached"
    );
    Ok(())
}

/// `delete` takes a record out and deletes it; `clear` takes every record out — let go, or
/// deleted when the one2many owns them; ids alone replace what the field holds.
#[test]
fn test_records_are_deleted_and_cleared() -> Result<()> {
    let app = new_app()?;
    let created = result(
        &app,
        "sale_order.create",
        json!({"values": {"name": "S3", "lines": {"create": [{"price": 1}, {"price": 2}, {"price": 3}]}}}),
    );
    let order = created[0].clone();
    let lines = result(
        &app,
        "sale_order_line.search",
        json!({"domain": [["order", "=", order]], "order": ["id asc"]}),
    );
    result(
        &app,
        "sale_order.write",
        json!({"ids": [order], "values": {"lines": {"delete": [lines[0]]}}}),
    );
    let gone = result(
        &app,
        "sale_order_line.search",
        json!({"domain": [["id", "=", lines[0]]]}),
    );
    assert_eq!(gone, json!([]), "deleted");
    let held = result(
        &app,
        "sale_order.read",
        json!({"ids": [order], "fields": ["lines"]}),
    );
    assert_eq!(held[0]["lines"], json!([lines[1], lines[2]]));

    result(
        &app,
        "sale_order.write",
        json!({"ids": [order], "values": {"lines": {"clear": true}}}),
    );
    let held = result(
        &app,
        "sale_order.read",
        json!({"ids": [order], "fields": ["lines"]}),
    );
    assert!(
        held[0]["lines"].as_array().is_none_or(Vec::is_empty),
        "cleared: {held}"
    );
    let let_go = result(
        &app,
        "sale_order_line.search",
        json!({"domain": [["id", "in", [lines[1], lines[2]]]]}),
    );
    assert_eq!(
        let_go.as_array().map_or(0, Vec::len),
        2,
        "the order does not own them: let go"
    );

    let notebook = result(
        &app,
        "notebook.create",
        json!({"values": {"name": "N2", "pages": {"create": [{"text": "a"}, {"text": "b"}]}}}),
    )[0]
    .clone();
    result(
        &app,
        "notebook.write",
        json!({"ids": [notebook], "values": {"pages": {"clear": true, "create": [{"text": "c"}]}}}),
    );
    let pages = result(
        &app,
        "page.read_matching",
        json!({"domain": [["notebook", "=", notebook]], "fields": ["text"]}),
    );
    assert_eq!(pages.as_array().map_or(0, Vec::len), 1, "{pages}");
    assert_eq!(pages[0]["text"], "c", "cleared then added to");
    let all = result(
        &app,
        "page.count",
        json!({"domain": [["text", "in", ["a", "b"]]]}),
    );
    assert_eq!(all, json!(0), "owned pages are deleted when cleared");
    Ok(())
}

/// A list of command objects is carried out one object after the other; one object's commands
/// in a fixed order, whatever order they are written in. Ids and commands do not mix.
#[test]
fn test_command_objects_are_carried_out_in_order() -> Result<()> {
    let app = new_app()?;
    let notebook = result(
        &app,
        "notebook.create",
        json!({"values": {"name": "N3", "pages": {"create": [{"text": "a"}]}}}),
    )[0]
    .clone();
    result(
        &app,
        "notebook.write",
        json!({"ids": [notebook], "values": {"pages": [
            {"create": [{"text": "b"}]},
            {"create": [{"text": "c"}], "clear": true},
        ]}}),
    );
    let pages = result(
        &app,
        "page.read_matching",
        json!({"domain": [["notebook", "=", notebook]], "fields": ["text"]}),
    );
    let texts: Vec<Value> = pages
        .as_array()
        .expect("pages")
        .iter()
        .map(|page| page["text"].clone())
        .collect();
    assert_eq!(
        texts,
        [json!("c")],
        "b created, then everything cleared, then c created"
    );

    let refused = call(
        &app,
        "notebook.write",
        json!({"ids": [notebook], "values": {"pages": [1, {"clear": true}]}}),
    );
    assert!(
        refused["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("not both"),
        "{refused}"
    );
    let refused = call(
        &app,
        "notebook.write",
        json!({"ids": [notebook], "values": {"pages": {"remove": [1]}}}),
    );
    assert!(
        refused["error"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("\"remove\" is not a command"),
        "{refused}"
    );
    Ok(())
}

/// Records a one2many owns are deleted once removed from it, not left pointing nowhere.
#[test]
fn test_owned_lines_go_with_their_removal() -> Result<()> {
    let app = new_app()?;
    let created = result(
        &app,
        "notebook.create",
        json!({"values": {"name": "N1", "pages": {"create": [{"text": "keep"}, {"text": "drop"}]}}}),
    );
    let notebook = created[0].clone();
    let pages = result(
        &app,
        "page.read_matching",
        json!({"domain": [], "fields": ["text", "notebook"], "order": ["id asc"]}),
    );
    assert_eq!(
        pages[0]["notebook"], notebook,
        "created pointing back: {pages}"
    );
    let keep = pages[0]["id"].clone();
    result(
        &app,
        "notebook.write",
        json!({"ids": [notebook], "values": {"pages": [keep]}}),
    );
    let left = result(&app, "page.search", json!({"domain": []}));
    assert_eq!(
        left,
        json!([keep]),
        "ids alone replace: the page left out is deleted"
    );

    result(
        &app,
        "notebook.write",
        json!({"ids": [notebook], "values": {"pages": {"unlink": [keep]}}}),
    );
    let left = result(&app, "page.search", json!({"domain": []}));
    assert_eq!(
        left,
        json!([]),
        "a page unlinked from the notebook owning it is deleted"
    );
    Ok(())
}
