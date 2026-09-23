//! Fields that never leave the process.
//!
//! A caller asking for one gets it back empty rather than an error, so the answer keeps the shape
//! it asked for. Filtering on one selects nothing and sorting by one is ignored — neither can be
//! answered with an empty field, and both would otherwise hand over the comparison that reading
//! was denied: comparing a value reconstructs it without ever reading it.
//!
//! Writing is the one that is refused outright, because there is no empty answer for it: a value
//! dropped in silence would report a change that never happened.

use erp::app::Application;
use erp::jsonrpc;
use erp_types::field::{IdMode, MultipleIds};
use erp_types::model::MapOfFields;
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
    let answer = jsonrpc::handle(app, None, &body).expect("an answer");
    assert!(
        answer.get("error").is_none(),
        "{method} must not fail: {answer}"
    );
    answer["result"].clone()
}

fn raw(app: &Application, method: &str, params: Value) -> Value {
    let body = json!({"jsonrpc": "2.0", "method": method, "params": params, "id": 1}).to_string();
    jsonrpc::handle(app, None, &body).expect("an answer")
}

/// A machine with an unlock code, made from inside the process.
///
/// Not over the wire: setting a hidden field from outside is refused, which
/// `test_writing_a_private_field_is_refused` is about.
fn make(app: &Application, name: &str, code: &str) -> Value {
    let mut env = app.new_env().expect("an environment");
    let mut values = MapOfFields::default();
    values.insert("name", name);
    values.insert("base_rate", 10);
    values.insert("unlock_code", code);
    let ids: MultipleIds = env
        .create_records("machine", vec![values])
        .expect("a machine");
    let ids = ids.get_ids_ref().clone();
    env.close().expect("a commit");
    json!(ids)
}

/// A hidden field comes back present and empty, beside the ones that are not.
#[test]
fn test_a_private_field_comes_back_empty() -> Result<()> {
    let app = new_app()?;
    let ids = make(&app, "m", "s3cret");

    let rows = call(
        &app,
        "machine.read",
        json!({"ids": ids, "fields": ["name", "unlock_code"]}),
    );
    assert_eq!(rows[0]["name"], json!("m"), "the visible field is there");
    assert!(
        rows[0].get("unlock_code").is_some(),
        "the hidden one keeps its place: {rows}"
    );
    assert_eq!(rows[0]["unlock_code"], Value::Null, "and holds nothing");
    Ok(())
}

/// The same through a search that reads.
#[test]
fn test_read_matching_answers_it_empty_too() -> Result<()> {
    let app = new_app()?;
    make(&app, "m", "s3cret");

    let rows = call(
        &app,
        "machine.read_matching",
        json!({"domain": [], "fields": ["unlock_code"]}),
    );
    assert_eq!(rows[0]["unlock_code"], Value::Null);
    Ok(())
}

/// Asking for nothing but the hidden field still answers, rather than failing.
#[test]
fn test_asking_only_for_a_private_field_still_answers() -> Result<()> {
    let app = new_app()?;
    make(&app, "m", "s3cret");

    let rows = call(
        &app,
        "machine.read_matching",
        json!({"domain": [], "fields": ["unlock_code"]}),
    );
    assert_eq!(rows.as_array().map(Vec::len), Some(1), "one record: {rows}");
    assert_eq!(rows[0]["unlock_code"], Value::Null);
    Ok(())
}

/// Filtering on one selects nothing, whatever is compared against it.
///
/// Nothing rather than everything: a filter that was ignored would look as though every record
/// matched the value the caller guessed.
#[test]
fn test_filtering_on_a_private_field_selects_nothing() -> Result<()> {
    let app = new_app()?;
    make(&app, "m", "s3cret");

    for domain in [
        json!([["unlock_code", "=", "s3cret"]]),
        json!([["unlock_code", "like", "s%"]]),
        json!([["unlock_code", "!=", null]]),
    ] {
        let found = call(&app, "machine.search", json!({"domain": domain.clone()}));
        assert_eq!(found, json!([]), "for {domain}");
    }
    assert_eq!(
        call(
            &app,
            "machine.count",
            json!({"domain": [["unlock_code", "=", "s3cret"]]})
        ),
        json!(0)
    );
    Ok(())
}

/// The answer does not depend on the hidden value, which is what makes it safe.
///
/// A right guess and a wrong one are answered identically; otherwise the filter would say
/// whether the guess was right, which is the whole of what reading was denied.
#[test]
fn test_a_right_guess_and_a_wrong_one_are_answered_the_same() -> Result<()> {
    let app = new_app()?;
    make(&app, "m", "s3cret");

    let right = call(
        &app,
        "machine.search",
        json!({"domain": [["unlock_code", "=", "s3cret"]]}),
    );
    let wrong = call(
        &app,
        "machine.search",
        json!({"domain": [["unlock_code", "=", "nowhere near"]]}),
    );
    assert_eq!(right, wrong, "the answer may not depend on the value");
    assert_eq!(right, json!([]));
    Ok(())
}

/// A blinded condition composes with the rest as a false one does.
#[test]
fn test_it_composes_with_the_conditions_around_it() -> Result<()> {
    let app = new_app()?;
    let ids = make(&app, "m", "s3cret");
    let id = ids[0].clone();

    assert_eq!(
        call(
            &app,
            "machine.search",
            json!({"domain": ["|", ["name", "=", "m"], ["unlock_code", "=", "x"]]})
        ),
        json!([id]),
        "an or keeps the side it can see"
    );
    assert_eq!(
        call(
            &app,
            "machine.search",
            json!({"domain": ["&", ["name", "=", "m"], ["unlock_code", "=", "x"]]})
        ),
        json!([]),
        "an and is killed by the side it cannot"
    );
    Ok(())
}

/// Sorting by one is dropped, and the rest of the ordering still applies.
#[test]
fn test_sorting_by_a_private_field_is_dropped() -> Result<()> {
    let app = new_app()?;
    for (name, code) in [("c", "z"), ("a", "y"), ("b", "x")] {
        make(&app, name, code);
    }

    let rows = call(
        &app,
        "machine.read_matching",
        json!({"domain": [], "fields": ["name"], "order": ["unlock_code asc", "name asc"]}),
    );
    let names: Vec<&str> = rows
        .as_array()
        .expect("rows")
        .iter()
        .map(|row| row["name"].as_str().unwrap_or_default())
        .collect();
    assert_eq!(
        names,
        vec!["a", "b", "c"],
        "sorted by name, not by the hidden code"
    );
    Ok(())
}

/// Writing one is refused, and the record keeps what it had.
///
/// The field is named in the refusal. That gives nothing away: a read already answers it as
/// present and empty, so its existence was never the secret — its value was.
#[test]
fn test_writing_a_private_field_is_refused() -> Result<()> {
    let app = new_app()?;
    let ids = make(&app, "m", "first");

    let written = raw(
        &app,
        "machine.write",
        json!({"ids": ids, "values": {"unlock_code": "second"}}),
    );
    assert!(written.get("result").is_none(), "got {written}");
    assert!(
        written["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("unlock_code")),
        "got {written}"
    );

    let mut env = app.new_env()?;
    let ids: Vec<u32> = serde_json::from_value(ids)?;
    let rows = env.read("machine", &MultipleIds::from(ids), &["unlock_code"])?;
    assert_eq!(rows[0].get::<&String>("unlock_code"), &"first".to_string());
    Ok(())
}

/// A write that touches nothing hidden is unaffected.
#[test]
fn test_writing_the_rest_of_the_model_still_works() -> Result<()> {
    let app = new_app()?;
    let ids = make(&app, "m", "first");

    let written = call(
        &app,
        "machine.write",
        json!({"ids": ids, "values": {"name": "renamed"}}),
    );
    assert_eq!(written, json!(true));
    Ok(())
}

/// Everything else on the model reads and sorts normally.
#[test]
fn test_the_rest_of_the_model_is_unaffected() -> Result<()> {
    let app = new_app()?;
    make(&app, "m", "s3cret");

    let rows = call(
        &app,
        "machine.read_matching",
        json!({"domain": [["base_rate", "=", 10]], "fields": ["name", "base_rate"],
               "order": ["base_rate asc"]}),
    );
    assert_eq!(rows[0]["base_rate"], json!(10), "got {rows}");
    Ok(())
}

/// The ORM itself is not restricted: the rule is about the wire, not about the code.
#[test]
fn test_rust_can_still_read_it() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env()?;
    let mut values = erp_types::model::MapOfFields::default();
    values.insert("unlock_code", "s3cret");
    let ids: erp_types::field::MultipleIds = env.create_records("machine", vec![values])?;

    let rows = env.read("machine", &ids, &["unlock_code"])?;
    assert_eq!(
        rows[0].get::<&String>("unlock_code"),
        &"s3cret".to_string(),
        "a private field is hidden from callers, not from the program"
    );
    Ok(())
}

/// The primary key is not a field any model declares, and the checks that guard hidden fields
/// must let it through rather than fail on it.
///
/// A caller holding a record already holds its id, so there is nothing to hide — but every path
/// that asks "is this one hidden?" asks the registry, which has never heard of it.
#[test]
fn test_the_primary_key_reaches_the_api() -> Result<()> {
    let app = new_app()?;
    let ids = make(&app, "m", "s3cret");
    let ids: Vec<u32> = serde_json::from_value(ids)?;

    let found = call(
        &app,
        "machine.search",
        json!({"domain": [["id", "=", ids[0]]]}),
    );
    assert_eq!(found, json!(ids), "a domain on the primary key");

    let rows = call(
        &app,
        "machine.read_matching",
        json!({"domain": [], "fields": ["name"], "order": ["id desc"]}),
    );
    assert_eq!(rows[0]["name"], json!("m"), "sorting by it: {rows}");
    Ok(())
}
