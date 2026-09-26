//! Which fields live in a column.
//!
//! Everything does, except a computed field: its value is worked out on each read unless it asks
//! to be kept. That is the difference between a field the database holds and one the code
//! produces, and it decides what can be searched, sorted and flushed.

use erp::app::Application;
use erp_search::{OrderBy, SearchOptions};
use erp_search_code_gen::make_domain;
use erp_types::field::{IdMode, MultipleIds, SingleId};
use erp_types::model::MapOfFields;
use std::collections::HashMap;
use std::error::Error;
use std::io;
use std::sync::{Arc, Mutex};
use test_utilities::models::{Invoice, SaleOrder, SaleOrderLine, Tag};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

fn new_app() -> Application {
    let mut app = Application::new_test();
    app.model_manager.register_model::<Invoice<_>>();
    app.model_manager.register_model::<SaleOrder<_>>();
    app.model_manager.register_model::<SaleOrderLine<_>>();
    app.model_manager.register_model::<Tag<_>>();
    app.model_manager.post_register();
    app
}

fn a_line(env: &mut erp::environment::Environment) -> Result<SaleOrderLine<SingleId>> {
    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("price", 10);
    map.insert("amount", 2);
    env.create_new_record_from_map(map)
}

/// A computed field is worked out on each read; one that asks to be kept is not.
#[test]
fn test_a_computed_field_is_not_kept_unless_it_asks() {
    let app = new_app();
    let line = app.model_manager.get_model("sale_order_line");

    assert!(
        line.get_internal_field("total_price").stored,
        "declared `stored`"
    );
    assert!(
        !line.get_internal_field("siblings_total").stored,
        "computed, and did not ask"
    );
    assert!(
        line.get_internal_field("price").stored,
        "a plain field is kept"
    );
    assert!(line.get_internal_field("order").stored, "so is a many2one");
}

/// The two halves of "stored" are separate: a many2many is meant to be kept, but not in a column
/// of its own.
#[test]
fn test_a_relation_is_kept_without_a_column() {
    let app = new_app();
    let order = app.model_manager.get_model("sale_order");

    let lines = order.get_internal_field("lines");
    assert!(lines.stored, "nothing computes it");
    assert!(!lines.is_stored(), "but no column holds it");
}

/// Reading one still answers: it is computed on demand.
#[test]
fn test_an_unkept_field_is_still_readable() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let line = a_line(&mut env)?;

    assert_eq!(*line.get_total_price(&mut env)?, 20, "kept, and computed");
    assert_eq!(
        *line.get_siblings_total(&mut env)?,
        0,
        "not kept, and computed all the same"
    );
    Ok(())
}

/// Nothing searches on one.
///
/// Both backends would answer, and both would answer wrongly: PostgreSQL with an unknown column,
/// the in-memory one by quietly matching nothing.
#[test]
fn test_no_domain_names_an_unkept_field() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    a_line(&mut env)?;

    let refused = env.search_ids(
        "sale_order_line",
        &make_domain!([("siblings_total", "=", 0)]),
    );
    let message = refused.unwrap_err().to_string();
    assert!(message.contains("siblings_total"), "got {message}");
    assert!(message.contains("stored"), "and says what to do: {message}");

    let kept = env.search_ids("sale_order_line", &make_domain!([("total_price", "=", 20)]))?;
    assert_eq!(kept.len(), 1, "a kept computed field still searches");
    Ok(())
}

/// Nor sorts by one.
#[test]
fn test_no_sort_key_names_an_unkept_field() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    a_line(&mut env)?;

    assert!(
        env.search_ids_with(
            "sale_order_line",
            &make_domain!([]),
            &SearchOptions::new().order_by(OrderBy::asc("siblings_total")),
        )
        .is_err()
    );
    assert!(
        env.search_ids_with(
            "sale_order_line",
            &make_domain!([]),
            &SearchOptions::new().order_by(OrderBy::asc("total_price")),
        )
        .is_ok()
    );
    Ok(())
}

/// Counting is a search too.
#[test]
fn test_counting_refuses_it_as_well() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    a_line(&mut env)?;

    assert!(
        env.count(
            "sale_order_line",
            &make_domain!([("siblings_total", "=", 0)])
        )
        .is_err()
    );
    Ok(())
}

/// Every segment of a path, not only the last: a relation that is itself computed has no table to
/// join through.
#[test]
fn test_a_path_is_refused_at_the_segment_that_is_unkept() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    a_line(&mut env)?;

    let refused = env.search_ids(
        "sale_order_line",
        &make_domain!([("order_tags.name", "=", "anything")]),
    );
    let message = refused.unwrap_err().to_string();
    assert!(message.contains("order_tags"), "got {message}");

    let allowed: SaleOrder<MultipleIds> = env.search(&make_domain!([("tags.name", "=", "x")]))?;
    assert!(allowed.id.get_ids_ref().is_empty(), "a kept relation joins");
    Ok(())
}

/// An unkept field never reaches the database, so nothing flushes it.
#[test]
fn test_an_unkept_field_is_never_flushed() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("name", "INV");
    let invoice: Invoice<SingleId> = env.create_new_record_from_map(map)?;
    // Read it, so its value exists in cache and could be flushed if anything wanted to.
    assert_eq!(invoice.get_tag_summary(&mut env)?, "");

    let fields = env.get_fields_to_save("invoice", &vec![])?;
    assert!(
        !fields
            .get("invoice")
            .is_some_and(|names| names.contains(&"tag_summary")),
        "got {fields:?}"
    );
    Ok(())
}

/// What the ORM logged while `work` ran.
fn logged<T>(work: impl FnOnce() -> T) -> (T, String) {
    #[derive(Clone)]
    struct Buffer(Arc<Mutex<Vec<u8>>>);

    impl io::Write for Buffer {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().expect("not poisoned").extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    let buffer = Buffer(Arc::new(Mutex::new(Vec::new())));
    let writer = buffer.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_writer(move || writer.clone())
        .with_max_level(tracing::Level::TRACE)
        .with_ansi(false)
        .finish();
    let outcome = tracing::subscriber::with_default(subscriber, work);
    let written =
        String::from_utf8(buffer.0.lock().expect("not poisoned").clone()).expect("the log is text");
    (outcome, written)
}

/// The refusal reaches the caller, and the reason reaches whoever runs the server.
///
/// One of them can act on it and the other cannot: the caller gets an error it can report, and
/// the operator gets the model, the field, and who kept asking.
#[test]
fn test_the_refusal_is_logged_as_well_as_returned() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env_as(7)?;
    a_line(&mut env)?;

    let (refused, log) = logged(|| {
        env.search_ids(
            "sale_order_line",
            &make_domain!([("siblings_total", "=", 0)]),
        )
    });

    assert!(refused.is_err(), "the caller is told");
    assert!(log.contains("sale_order_line"), "got {log}");
    assert!(log.contains("siblings_total"), "got {log}");
    assert!(log.contains('7'), "and who asked: {log}");
    assert!(log.contains("filter on"), "and what for: {log}");
    Ok(())
}

/// Sorting says so too, and says it differently.
#[test]
fn test_a_refused_sort_says_it_was_a_sort() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    a_line(&mut env)?;

    let (refused, log) = logged(|| {
        env.search_ids_with(
            "sale_order_line",
            &make_domain!([]),
            &SearchOptions::new().order_by(OrderBy::asc("siblings_total")),
        )
    });

    assert!(refused.is_err());
    assert!(log.contains("sort by"), "got {log}");
    Ok(())
}

/// A search that is answered logs nothing.
#[test]
fn test_nothing_is_logged_when_the_search_is_fine() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    a_line(&mut env)?;

    let (found, log) =
        logged(|| env.search_ids("sale_order_line", &make_domain!([("total_price", "=", 20)])));

    assert_eq!(found?.len(), 1);
    assert!(log.is_empty(), "got {log}");
    Ok(())
}

/// One struct computes a field, another asks for it to be kept.
///
/// Storage belongs to the model's field, not to the struct that happens to mention it.
/// `sale_order_test` is built from two structs, and between them they cover the three cases:
///
/// - `narrowed` — computed by the first, and the second asks for a column with nothing but
///   `#[erp(stored)]`. It does not repeat the computation, which would only duplicate its
///   `depends`.
/// - `replaced` — the first asks, the struct registered after it says nothing. The order they
///   register in must not decide.
/// - `label` — computed by the first, and merely *mentioned* by the second. Nobody asked for a
///   column, so there must not be one: on its own, that second struct looks like it is declaring
///   a plain field, and only the merged view knows better.
#[test]
fn test_a_second_struct_can_ask_for_a_column() -> Result<()> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(test_utilities::TestLibPlugin {}))?;
    app.register_plugin(Box::new(test_plugin::TestPlugin {}))?;
    app.load_plugin("test_plugin")?;
    let model = app.model_manager.get_model("sale_order_test");

    assert!(
        model.get_internal_field("narrowed").stored,
        "one struct asking is enough"
    );
    assert!(
        model.get_internal_field("replaced").stored,
        "and so is the first asking, though the struct after it says nothing"
    );
    assert!(
        !model.get_internal_field("label").stored,
        "and merely mentioning a computed field does not give it a column"
    );

    let mut env = app.new_env()?;
    for name in ["skip", "ordinary"] {
        let mut map: MapOfFields = MapOfFields::new(HashMap::new());
        map.insert("name", name);
        let _: MultipleIds = env.create_records("sale_order_test", vec![map])?;
    }

    // What it is kept for: being searchable, on whatever the chain worked out.
    let skipped = env.search_ids(
        "sale_order_test",
        &make_domain!([("narrowed", "=", "skipped")]),
    )?;
    assert_eq!(skipped.len(), 1, "a kept computed field searches");
    let base = env.search_ids(
        "sale_order_test",
        &make_domain!([("narrowed", "=", "base")]),
    )?;
    assert_eq!(base.len(), 1, "and tells the two values apart");

    let replaced = env.search_ids(
        "sale_order_test",
        &make_domain!([("replaced", "=", "derived")]),
    )?;
    assert_eq!(
        replaced.len(),
        2,
        "the field the first struct asked for is kept too"
    );
    assert!(
        env.search_ids("sale_order_test", &make_domain!([("label", "=", "x")]))
            .is_err(),
        "the one nobody asked for does not"
    );
    Ok(())
}

/// Filling a column that has just appeared says so, and says how much there was to do.
///
/// A startup that pauses without explaining itself is a startup somebody will kill. The line goes
/// out before the work, not after, so it explains a wait that is happening rather than one that
/// has already happened.
#[test]
fn test_the_prefill_says_what_it_is_doing() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    a_line(&mut env)?;
    a_line(&mut env)?;

    let (outcome, log) = logged(|| env.fill_stored_field("sale_order_line", "total_price"));
    outcome?;

    assert!(log.contains("sale_order_line"), "got {log}");
    assert!(log.contains("total_price"), "got {log}");
    assert!(log.contains("records=2"), "how much there was to do: {log}");
    assert!(log.contains("took="), "and how long it took: {log}");
    Ok(())
}

/// An empty table is not news.
///
/// Installing a plugin makes every column appear at once, and every one of them would say so on a
/// database where there is nothing to fill.
#[test]
fn test_nothing_is_said_when_there_is_nothing_to_fill() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let (outcome, log) = logged(|| env.fill_stored_field("sale_order_line", "total_price"));
    outcome?;

    assert!(log.is_empty(), "got {log}");
    Ok(())
}
