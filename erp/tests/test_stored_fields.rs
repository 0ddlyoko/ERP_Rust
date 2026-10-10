//! Which fields live in a column.
//!
//! Everything does, except a computed field: its value is worked out on each read unless it asks
//! to be kept. That is the difference between a field the database holds and one the code
//! produces, and it decides what can be searched, sorted and flushed.

use erp::Result;
use erp::app::Application;
use erp_search::{OrderBy, SearchOptions, SearchType};
use erp_search_code_gen::make_domain;
use erp_types::field::{IdMode, MultipleIds, SingleId};
use erp_types::model::MapOfFields;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io;
use std::sync::Once;
use test_utilities::models::{Invoice, SaleOrder, SaleOrderLine, Tag};

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

thread_local! {
    static CAPTURED: RefCell<Option<Vec<u8>>> = const { RefCell::new(None) };
}

/// Writes into the log being captured on the current thread, if any.
struct CapturedLog;

impl io::Write for CapturedLog {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        CAPTURED.with(|captured| {
            if let Some(log) = captured.borrow_mut().as_mut() {
                log.extend_from_slice(buf);
            }
        });
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// What the ORM logged while `work` ran.
///
/// One global subscriber, installed once, that always listens and writes to the calling thread's
/// capture. A subscriber scoped to the test would not do: `tracing` caches per callsite whether
/// anybody listens, and a test running in parallel can leave "nobody" in that cache.
fn logged<T>(work: impl FnOnce() -> T) -> (T, String) {
    static INSTALLED: Once = Once::new();
    INSTALLED.call_once(|| {
        let subscriber = tracing_subscriber::fmt()
            .with_writer(|| CapturedLog)
            .with_max_level(tracing::Level::TRACE)
            .with_ansi(false)
            .finish();
        tracing::subscriber::set_global_default(subscriber).expect("the only subscriber");
    });

    CAPTURED.with(|captured| *captured.borrow_mut() = Some(Vec::new()));
    let outcome = work();
    let written = CAPTURED
        .with(|captured| captured.borrow_mut().take())
        .unwrap_or_default();
    (
        outcome,
        String::from_utf8(written).expect("the log is text"),
    )
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

// ---- keeping a kept value up to date ----

/// An order with one line, committed, so that the next environment starts from the database.
fn an_order_with_a_line(app: &Application) -> Result<(u32, u32)> {
    let mut env = app.new_env()?;
    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("name", "Order");
    let order: SaleOrder<SingleId> = env.create_new_record_from_map(map)?;
    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("order", erp_types::field::FieldType::Ref(order.get_id()));
    map.insert("price", 10);
    map.insert("amount", 2);
    let line: SaleOrderLine<SingleId> = env.create_new_record_from_map(map)?;
    env.close()?;
    Ok((order.get_id(), line.get_id()))
}

/// Loading a row brings the value stored before the change, which must not stand in for the
/// recompute the change asked for.
#[test]
fn test_loading_a_row_does_not_cancel_a_pending_recompute() -> Result<()> {
    let app = new_app();
    let (_, line) = an_order_with_a_line(&app)?;

    let mut env = app.new_env()?;
    let line: SaleOrderLine<SingleId> = env.get_record(line.into());
    line.set_amount(5, &mut env)?;
    env.read("sale_order_line", &line.id, &["price"])?;
    assert_eq!(*line.get_total_price(&mut env)?, 50);
    Ok(())
}

/// A recompute that asks for another model's to be recomputed reaches the database, whichever of
/// the two is flushed first.
///
/// The order models are flushed in is not fixed, so this runs on twenty fresh applications: an
/// unlucky order is then all but certain to come up.
#[test]
fn test_a_recompute_reaching_another_model_is_saved() -> Result<()> {
    for _ in 0..20 {
        let app = new_app();
        let (order, line) = an_order_with_a_line(&app)?;

        let mut env = app.new_env()?;
        let line: SaleOrderLine<SingleId> = env.get_record(line.into());
        line.set_amount(5, &mut env)?;
        env.close()?;

        let mut env = app.new_env()?;
        let order: SaleOrder<SingleId> = env.get_record(order.into());
        assert_eq!(*order.get_total_price(&mut env)?, 50);
    }
    Ok(())
}

/// Searching on a kept value that waits on another model's recompute finds the new value: the
/// order's total is only flagged once its line's is worked out.
#[test]
fn test_a_search_sees_a_value_waiting_on_another_model() -> Result<()> {
    let app = new_app();
    let (order, line) = an_order_with_a_line(&app)?;
    let (other, _) = an_order_with_a_line(&app)?;

    let mut env = app.new_env()?;
    let line: SaleOrderLine<SingleId> = env.get_record(line.into());
    line.set_amount(5, &mut env)?;

    let domain = make_domain!([("total_price", "=", 50)]);
    assert_eq!(env.search_ids("sale_order", &domain)?, vec![order]);
    assert_eq!(env.count("sale_order", &domain)?, 1);
    let sorted = env.search_ids_with(
        "sale_order",
        &SearchType::Nothing,
        &SearchOptions::new().order_by(OrderBy::desc("total_price")),
    )?;
    assert_eq!(sorted, vec![order, other]);
    Ok(())
}

/// Moving a line from one order to another recomputes both totals.
#[test]
fn test_moving_a_line_recomputes_both_orders() -> Result<()> {
    let app = new_app();
    let (from, line) = an_order_with_a_line(&app)?;
    let (to, _) = an_order_with_a_line(&app)?;

    let mut env = app.new_env()?;
    let line: SaleOrderLine<SingleId> = env.get_record(line.into());
    let destination = SaleOrder::<SingleId>::from_id(to, &env);
    line.set_order(&destination, &mut env)?;
    env.close()?;

    let mut env = app.new_env()?;
    let from: SaleOrder<SingleId> = env.get_record(from.into());
    let to: SaleOrder<SingleId> = env.get_record(to.into());
    assert_eq!(*from.get_total_price(&mut env)?, 0);
    assert_eq!(*to.get_total_price(&mut env)?, 40);
    Ok(())
}

/// Deleting a line recomputes the total of the order it belonged to.
#[test]
fn test_deleting_a_line_recomputes_its_order() -> Result<()> {
    let app = new_app();
    let (order, line) = an_order_with_a_line(&app)?;

    let mut env = app.new_env()?;
    env.delete("sale_order_line", &SingleId::from(line))?;
    env.close()?;

    let mut env = app.new_env()?;
    let order: SaleOrder<SingleId> = env.get_record(order.into());
    assert_eq!(*order.get_total_price(&mut env)?, 0);
    Ok(())
}

mod unkept_inverse {
    use code_gen::{Model, erp_methods};
    use erp::environment::Environment;
    use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};
    use std::error::Error;

    #[derive(Model)]
    #[erp(id = "crew")]
    #[allow(dead_code)]
    pub struct Crew<Mode: IdMode> {
        pub id: Mode,
        #[erp(inverse = "crew")]
        sailors: Reference<BaseSailor, MultipleIds>,
    }

    #[derive(Model)]
    #[erp(id = "sailor", methods)]
    #[allow(dead_code)]
    pub struct Sailor<Mode: IdMode> {
        pub id: Mode,
        #[erp(compute = "compute_crew", depends = [])]
        crew: Reference<BaseCrew, SingleId>,
    }

    #[erp_methods]
    impl Sailor<MultipleIds> {
        pub fn compute_crew(
            &self,
            env: &mut Environment,
        ) -> Result<(), Box<dyn Error + Send + Sync>> {
            let _ = env;
            Ok(())
        }
    }
}

/// A one2many is found by searching its many2one's column, so that many2one has to have one.
#[test]
#[should_panic(expected = "Declare it `stored`")]
fn test_a_one2many_needs_its_many2one_kept() {
    let mut app = Application::new_test();
    app.model_manager
        .register_model::<unkept_inverse::Crew<_>>();
    app.model_manager
        .register_model::<unkept_inverse::Sailor<_>>();
    app.model_manager.post_register();
}

mod computed_list {
    use code_gen::{Model, erp_methods};
    use erp::environment::Environment;
    use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};
    use std::error::Error;

    #[derive(Model)]
    #[erp(id = "harbour", methods)]
    #[allow(dead_code)]
    pub struct Harbour<Mode: IdMode> {
        pub id: Mode,
        #[erp(inverse = "harbour")]
        boats: Reference<BaseBoat, MultipleIds>,
        #[erp(compute = "compute_big_boats", depends = ["boats.size"])]
        big_boats: Reference<BaseBoat, MultipleIds>,
        #[erp(compute = "compute_big_count", depends = ["big_boats"])]
        big_count: i32,
    }

    #[erp_methods]
    impl Harbour<MultipleIds> {
        pub fn compute_big_boats(
            &self,
            env: &mut Environment,
        ) -> Result<(), Box<dyn Error + Send + Sync>> {
            for harbour in self {
                let boats: Boat<MultipleIds> = harbour.get_boats(env)?;
                let mut big = Vec::new();
                for boat in &boats {
                    if *boat.get_size(env)? > 4 {
                        big.push(boat.get_id());
                    }
                }
                let big = Boat::<MultipleIds>::from_ids(big, env);
                harbour.set_big_boats(&big, env)?;
            }
            Ok(())
        }

        pub fn compute_big_count(
            &self,
            env: &mut Environment,
        ) -> Result<(), Box<dyn Error + Send + Sync>> {
            for harbour in self {
                let big: Boat<MultipleIds> = harbour.get_big_boats(env)?;
                harbour.set_big_count(big.get_ids_ref().len() as i32, env)?;
            }
            Ok(())
        }
    }

    #[derive(Model)]
    #[erp(id = "boat")]
    #[allow(dead_code)]
    pub struct Boat<Mode: IdMode> {
        pub id: Mode,
        harbour: Reference<BaseHarbour, SingleId>,
        #[erp(default = 0)]
        size: i32,
    }
}

fn harbour_app() -> Application {
    use computed_list::{Boat, Harbour};
    let mut app = Application::new_test();
    app.model_manager.register_model::<Harbour<_>>();
    app.model_manager.register_model::<Boat<_>>();
    app.model_manager.post_register();
    app
}

fn a_boat(env: &mut erp::environment::Environment, harbour: u32, size: i32) -> Result<u32> {
    let mut map = MapOfFields::default();
    map.insert("harbour", erp_types::field::FieldType::Ref(harbour));
    map.insert("size", size);
    Ok(env.create_records("boat", vec![map])?.get_ids_ref()[0])
}

/// The big boats of a harbour, and how many there are.
fn big_boats(env: &mut erp::environment::Environment, harbour: u32) -> Result<(Vec<u32>, i32)> {
    use computed_list::{Boat, Harbour};
    let harbour: Harbour<SingleId> = env.get_record(harbour.into());
    let mut big = harbour.get_big_boats::<Boat<MultipleIds>>(env)?.get_ids();
    big.sort_unstable();
    Ok((big, *harbour.get_big_count(env)?))
}

/// A list of references can be computed, and follows what it depends on like any computed field.
#[test]
fn test_a_computed_list_of_references() -> Result<()> {
    use computed_list::{Boat, Harbour};
    let app = harbour_app();

    let mut env = app.new_env()?;
    let harbour: Harbour<SingleId> = env.create_new_record_from_map(MapOfFields::default())?;
    let boats: Vec<u32> = [1, 5, 9]
        .into_iter()
        .map(|size| {
            let mut map = MapOfFields::default();
            map.insert(
                "harbour",
                erp_types::field::FieldType::Ref(harbour.get_id()),
            );
            map.insert("size", size);
            Ok(env.create_records("boat", vec![map])?.get_ids_ref()[0])
        })
        .collect::<Result<_>>()?;

    let big: Boat<MultipleIds> = harbour.get_big_boats(&mut env)?;
    assert_eq!(big.get_ids(), vec![boats[1], boats[2]]);

    let small: Boat<SingleId> = env.get_record(boats[0].into());
    small.set_size(10, &mut env)?;
    let big: Boat<MultipleIds> = harbour.get_big_boats(&mut env)?;
    assert_eq!(big.get_ids(), boats, "changing a size recomputes the list");
    Ok(())
}

/// The list follows its records wherever they go, and a field computed from the list follows it.
#[test]
fn test_a_computed_list_follows_moves_and_deletes() -> Result<()> {
    use computed_list::Boat;
    let app = harbour_app();
    let mut env = app.new_env()?;
    let first = env
        .create_records("harbour", vec![MapOfFields::default()])?
        .get_ids_ref()[0];
    let second = env
        .create_records("harbour", vec![MapOfFields::default()])?
        .get_ids_ref()[0];
    assert_eq!(
        big_boats(&mut env, first)?,
        (vec![], 0),
        "an empty list is a list"
    );
    let small = a_boat(&mut env, first, 1)?;
    let large = a_boat(&mut env, first, 9)?;
    env.close()?;

    let mut env = app.new_env()?;
    assert_eq!(big_boats(&mut env, first)?, (vec![large], 1));

    let boat: Boat<SingleId> = env.get_record(large.into());
    let harbour = computed_list::Harbour::<SingleId>::from_id(second, &env);
    boat.set_harbour(&harbour, &mut env)?;
    assert_eq!(big_boats(&mut env, first)?, (vec![], 0));
    assert_eq!(big_boats(&mut env, second)?, (vec![large], 1));

    let boat: Boat<SingleId> = env.get_record(small.into());
    boat.set_size(7, &mut env)?;
    assert_eq!(big_boats(&mut env, first)?, (vec![small], 1));

    env.delete("boat", &SingleId::from(small))?;
    assert_eq!(big_boats(&mut env, first)?, (vec![], 0));
    Ok(())
}

/// A list has no column, so it cannot be searched — and asking for `stored` would not help.
#[test]
fn test_a_computed_list_cannot_be_searched() -> Result<()> {
    let app = harbour_app();
    let mut env = app.new_env()?;
    let error = env
        .search_ids("harbour", &make_domain!([("big_boats", "=", 1)]))
        .unwrap_err()
        .to_string();
    assert!(error.contains("worked out on each read"), "got {error}");
    assert!(
        !error.contains("stored"),
        "no advice that cannot be followed: {error}"
    );
    Ok(())
}

mod stored_list {
    use code_gen::{Model, erp_methods};
    use erp::environment::Environment;
    use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};
    use std::error::Error;

    #[derive(Model)]
    #[erp(id = "fleet", methods)]
    #[allow(dead_code)]
    pub struct Fleet<Mode: IdMode> {
        pub id: Mode,
        flagship: Reference<BaseFleet, SingleId>,
        #[erp(compute = "compute_ships", depends = [], stored, inverse = "flagship")]
        ships: Reference<BaseFleet, MultipleIds>,
        #[erp(compute = "compute_size", depends = ["ships.ships"])]
        size: i32,
    }

    #[erp_methods]
    impl Fleet<MultipleIds> {
        pub fn compute_ships(
            &self,
            env: &mut Environment,
        ) -> Result<(), Box<dyn Error + Send + Sync>> {
            let _ = env;
            Ok(())
        }

        pub fn compute_size(
            &self,
            env: &mut Environment,
        ) -> Result<(), Box<dyn Error + Send + Sync>> {
            let _ = env;
            Ok(())
        }
    }
}

/// A one2many has nothing of its own to keep — it is found through the many2one pointing back —
/// so asking to keep it is refused rather than ignored. A many2many is kept in its table of pairs.
#[test]
#[should_panic(expected = "a one2many has nothing of its own to keep")]
fn test_a_computed_list_cannot_be_stored() {
    let mut app = Application::new_test();
    app.model_manager.register_model::<stored_list::Fleet<_>>();
    app.model_manager.post_register();
}

mod through_a_list {
    use code_gen::{Model, erp_methods};
    use erp::environment::Environment;
    use erp::types::field::{IdMode, MultipleIds, Reference};
    use std::error::Error;

    #[derive(Model)]
    #[erp(id = "convoy", methods)]
    #[allow(dead_code)]
    pub struct Convoy<Mode: IdMode> {
        pub id: Mode,
        #[erp(compute = "compute_escorts", depends = [])]
        escorts: Reference<BaseConvoy, MultipleIds>,
        #[erp(compute = "compute_escort_count", depends = ["escorts.escorts"])]
        escort_count: i32,
    }

    #[erp_methods]
    impl Convoy<MultipleIds> {
        pub fn compute_escorts(
            &self,
            env: &mut Environment,
        ) -> Result<(), Box<dyn Error + Send + Sync>> {
            let _ = env;
            Ok(())
        }

        pub fn compute_escort_count(
            &self,
            env: &mut Environment,
        ) -> Result<(), Box<dyn Error + Send + Sync>> {
            let _ = env;
            Ok(())
        }
    }
}

/// A computed list has nothing to follow back, so a dependency cannot cross it — and the refusal
/// names the field whose dependency is wrong, not the list.
#[test]
#[should_panic(expected = "convoy.escort_count depends on \"escorts.escorts\"")]
fn test_a_dependency_cannot_cross_a_computed_list() {
    let mut app = Application::new_test();
    app.model_manager
        .register_model::<through_a_list::Convoy<_>>();
    app.model_manager.post_register();
}

mod counted_computes {
    use code_gen::{Model, erp_methods};
    use erp::environment::Environment;
    use erp::types::field::{IdMode, MultipleIds};
    use std::error::Error;
    use std::sync::Mutex;

    pub static RUNS: Mutex<Vec<(&'static str, Vec<u32>)>> = Mutex::new(Vec::new());

    #[derive(Model)]
    #[erp(id = "gauge", methods)]
    #[allow(dead_code)]
    pub struct Gauge<Mode: IdMode> {
        pub id: Mode,
        #[erp(default = 0)]
        level: i32,
        #[erp(compute = "compute_double", depends = ["level"], stored)]
        double: i32,
        #[erp(compute = "compute_label", depends = ["level"])]
        label: String,
    }

    #[erp_methods]
    impl Gauge<MultipleIds> {
        pub fn compute_double(
            &self,
            env: &mut Environment,
        ) -> Result<(), Box<dyn Error + Send + Sync>> {
            RUNS.lock().unwrap().push(("double", self.get_ids()));
            for gauge in self {
                let level = *gauge.get_level(env)?;
                gauge.set_double(level * 2, env)?;
            }
            Ok(())
        }

        pub fn compute_label(
            &self,
            env: &mut Environment,
        ) -> Result<(), Box<dyn Error + Send + Sync>> {
            RUNS.lock().unwrap().push(("label", self.get_ids()));
            for gauge in self {
                let level = *gauge.get_level(env)?;
                gauge.set_label(format!("level {level}"), env)?;
            }
            Ok(())
        }
    }
}

/// Reading several computed fields runs each compute once, on the records that need it: every
/// record for a value worked out on each read, only the changed ones for a kept value.
#[test]
fn test_reading_several_computed_fields_plans_their_computes() -> Result<()> {
    use counted_computes::{Gauge, RUNS};
    let mut app = Application::new_test();
    app.model_manager.register_model::<Gauge<_>>();
    app.model_manager.post_register();

    let mut env = app.new_env()?;
    let ids: Vec<u32> = (1..=3)
        .map(|level| {
            let mut map = MapOfFields::default();
            map.insert("level", level);
            Ok(env.create_records("gauge", vec![map])?.get_ids_ref()[0])
        })
        .collect::<Result<_>>()?;
    env.close()?;

    let mut env = app.new_env()?;
    let changed: Gauge<SingleId> = env.get_record(ids[1].into());
    changed.set_level(10, &mut env)?;
    RUNS.lock().unwrap().clear();

    let rows = env.read(
        "gauge",
        &MultipleIds::from(ids.clone()),
        &["double", "label"],
    )?;
    let doubles: Vec<i32> = rows.iter().map(|row| *row.get::<&i32>("double")).collect();
    let labels: Vec<String> = rows
        .iter()
        .map(|row| row.get::<&String>("label").clone())
        .collect();
    assert_eq!(doubles, vec![2, 20, 6]);
    assert_eq!(labels, vec!["level 1", "level 10", "level 3"]);

    let mut runs = RUNS.lock().unwrap().clone();
    runs.sort();
    assert_eq!(runs, vec![("double", vec![ids[1]]), ("label", ids.clone())]);
    Ok(())
}
