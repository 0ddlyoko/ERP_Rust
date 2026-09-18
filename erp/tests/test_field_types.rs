use erp::app::Application;
use erp_search_code_gen::make_domain;
use erp_types::field::{Decimal, IdMode, MultipleIds, NaiveDate, SingleId, Timestamp};
use erp_types::model::MapOfFields;
use std::collections::HashMap;
use std::error::Error;
use std::str::FromStr;
use test_utilities::models::Invoice;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

fn new_app() -> Application {
    let mut app = Application::new_test();
    app.model_manager.register_model::<Invoice<_>>();
    app.model_manager.post_register();
    app
}

/// Money must round-trip exactly. `0.1 + 0.2` is the canonical case a float gets wrong.
#[test]
fn test_decimal_arithmetic_is_exact() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("amount_untaxed", Decimal::from_str("0.1")?);
    let invoice: Invoice<SingleId> = env.create_new_record_from_map(map)?;

    let stored = *invoice.get_amount_untaxed(&mut env)?;
    let sum = stored + Decimal::from_str("0.2")?;
    assert_eq!(
        sum,
        Decimal::from_str("0.3")?,
        "decimal arithmetic must be exact"
    );
    Ok(())
}

/// A value a 32-bit float cannot even represent must survive a write/commit/read cycle.
#[test]
fn test_decimal_survives_a_commit() -> Result<()> {
    let app = new_app();
    let amount = Decimal::from_str("99999.99")?;

    let mut env = app.new_env()?;
    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("amount_untaxed", amount);
    let _invoice: Invoice<SingleId> = env.create_new_record_from_map(map)?;
    env.close()?;

    let mut env = app.new_env()?;
    let found: Invoice<MultipleIds> =
        env.search(&make_domain!([("amount_untaxed", "=", amount)]))?;
    assert_eq!(found.id.get_ids_ref().len(), 1);
    assert_eq!(found.get_amount_untaxed(&mut env)?, vec![&amount]);
    Ok(())
}

/// The declared default is carried through as a real `Decimal`, not a float.
#[test]
fn test_decimal_default_value() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let invoice: Invoice<SingleId> =
        env.create_new_record_from_map(MapOfFields::new(HashMap::new()))?;
    assert_eq!(*invoice.get_tax_rate(&mut env)?, Decimal::from_str("0.21")?);
    Ok(())
}

/// Dates and timestamps must store, commit and compare.
#[test]
fn test_date_and_timestamp_round_trip() -> Result<()> {
    let app = new_app();
    let due = NaiveDate::from_str("2026-03-15")?;
    let created = Timestamp::from_str("2026-01-02T10:30:00Z")?;

    let mut env = app.new_env()?;
    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("due_date", due);
    map.insert("created_at", created);
    let _invoice: Invoice<SingleId> = env.create_new_record_from_map(map)?;
    env.close()?;

    let mut env = app.new_env()?;
    let found: Invoice<MultipleIds> = env.search(&make_domain!([("due_date", "=", due)]))?;
    assert_eq!(found.id.get_ids_ref().len(), 1, "a date must be searchable");
    assert_eq!(found.get_created_at(&mut env)?, vec![&created]);
    Ok(())
}

/// Ordering comparisons must work on dates, which is what a due-date filter needs.
#[test]
fn test_dates_are_ordered() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    for day in ["2026-01-10", "2026-02-10", "2026-03-10"] {
        let mut map: MapOfFields = MapOfFields::new(HashMap::new());
        map.insert("due_date", NaiveDate::from_str(day)?);
        let _invoice: Invoice<SingleId> = env.create_new_record_from_map(map)?;
    }
    env.close()?;

    let mut env = app.new_env()?;
    let pivot = NaiveDate::from_str("2026-02-01")?;
    let overdue: Invoice<MultipleIds> = env.search(&make_domain!([("due_date", "<", pivot)]))?;
    assert_eq!(
        overdue.id.get_ids_ref().len(),
        1,
        "one invoice is due before the pivot"
    );

    let later: Invoice<MultipleIds> = env.search(&make_domain!([("due_date", ">", pivot)]))?;
    assert_eq!(
        later.id.get_ids_ref().len(),
        2,
        "two invoices are due after the pivot"
    );
    Ok(())
}

/// Strings gained ordering in the same rewrite; it used to return false unconditionally.
#[test]
fn test_strings_are_ordered() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    for name in ["alpha", "bravo", "charlie"] {
        let mut map: MapOfFields = MapOfFields::new(HashMap::new());
        map.insert("name", name);
        let _invoice: Invoice<SingleId> = env.create_new_record_from_map(map)?;
    }
    env.close()?;

    let mut env = app.new_env()?;
    let found: Invoice<MultipleIds> = env.search(&make_domain!([("name", "<", "bravo")]))?;
    assert_eq!(
        found.id.get_ids_ref().len(),
        1,
        "only \"alpha\" sorts before \"bravo\""
    );
    Ok(())
}
