//! Fields that never leave the process.
//!
//! A caller asking for one gets it back empty rather than an error, so the answer keeps the shape
//! it asked for. Filtering on one selects nothing and sorting by one is ignored — neither can be
//! answered with an empty field, and both would otherwise hand over the comparison that reading
//! was denied: comparing a value reconstructs it without ever reading it.

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
    let answer = jsonrpc::handle(app, None, &body).expect("an answer");
    assert!(
        answer.get("error").is_none(),
        "{method} must not fail: {answer}"
    );
    answer["result"].clone()
}

fn make(app: &Application, name: &str, code: &str) -> Value {
    call(
        app,
        "machine.create",
        json!({"values": {"name": name, "base_rate": 10, "unlock_code": code}}),
    )
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

/// Writing one is allowed: setting a password is the whole point of having the field.
#[test]
fn test_a_private_field_can_still_be_written() -> Result<()> {
    let app = new_app()?;
    let ids = make(&app, "m", "first");

    let written = call(
        &app,
        "machine.write",
        json!({"ids": ids, "values": {"unlock_code": "second"}}),
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
