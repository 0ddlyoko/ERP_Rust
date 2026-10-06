//! Records gathered by a field's value or a date's period, counted and summed: what a grouped
//! list shows.

use erp::app::Application;
use erp::database::{FieldType, GroupBy, Period};
use erp::environment::Environment;
use erp_search_code_gen::make_domain;
use erp_types::field::{Decimal, IdMode, NaiveDate};
use erp_types::model::MapOfFields;
use serde_json::json;
use std::error::Error;
use std::str::FromStr;
use test_utilities::models::{Invoice, SaleOrder, SaleOrderLine, Tag};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

fn new_app() -> Application {
    let mut app = Application::new_test();
    app.model_manager.register_model::<Invoice<_>>();
    app.model_manager.register_model::<Tag<_>>();
    app.model_manager.register_model::<SaleOrder<_>>();
    app.model_manager.register_model::<SaleOrderLine<_>>();
    app.model_manager.post_register();
    app
}

fn d(text: &str) -> Decimal {
    Decimal::from_str(text).expect("a decimal")
}

fn date(text: &str) -> NaiveDate {
    NaiveDate::parse_from_str(text, "%Y-%m-%d").expect("a date")
}

/// Two invoices of "a" in January, one of "b" in February, one of "c" with no due date.
fn seed(env: &mut Environment) -> Result<()> {
    for (name, amount, due) in [
        ("a", "10.5", Some("2026-01-15")),
        ("a", "4.5", Some("2026-01-31")),
        ("b", "7", Some("2026-02-03")),
        ("c", "1", None),
    ] {
        let mut values = MapOfFields::default();
        values.insert("name", name);
        values.insert("amount_untaxed", d(amount));
        if let Some(due) = due {
            values.insert("due_date", date(due));
        }
        env.create_records("invoice", vec![values])?;
    }
    Ok(())
}

fn by(text: &str) -> GroupBy {
    GroupBy::parse(text).expect("a grouping")
}

/// `(value, count, sum of amount_untaxed)` of each group.
fn summary(
    env: &mut Environment,
    group_by: Option<&GroupBy>,
) -> Result<Vec<(Option<FieldType>, u32, Decimal)>> {
    Ok(env
        .read_group("invoice", &make_domain!([]), group_by, &["amount_untaxed"])?
        .into_iter()
        .map(|group| (group.key, group.count, group.sums["amount_untaxed"]))
        .collect())
}

/// By a field's value, in its order, counted and summed.
#[test]
fn test_records_are_gathered_by_value() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    seed(&mut env)?;
    assert_eq!(
        summary(&mut env, Some(&by("name")))?,
        vec![
            (Some(FieldType::String("a".into())), 2, d("15.0")),
            (Some(FieldType::String("b".into())), 1, d("7")),
            (Some(FieldType::String("c".into())), 1, d("1")),
        ]
    );
    Ok(())
}

/// By the month a date falls in, the first day of it standing for the month; those with no
/// date last.
#[test]
fn test_dates_are_gathered_by_period() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    seed(&mut env)?;
    assert_eq!(
        summary(&mut env, Some(&by("due_date:month")))?,
        vec![
            (Some(FieldType::Date(date("2026-01-01"))), 2, d("15.0")),
            (Some(FieldType::Date(date("2026-02-01"))), 1, d("7")),
            (None, 1, d("1")),
        ]
    );
    assert_eq!(
        summary(&mut env, Some(&by("due_date:year")))?[0],
        (Some(FieldType::Date(date("2026-01-01"))), 3, d("22.0"))
    );
    Ok(())
}

/// Without grouping, one group of everything the domain finds — even nothing.
#[test]
fn test_without_grouping_everything_is_one_group() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    seed(&mut env)?;
    assert_eq!(summary(&mut env, None)?, vec![(None, 4, d("23.0"))]);
    let groups = env.read_group(
        "invoice",
        &make_domain!([("name", "=", "z")]),
        None,
        &["amount_untaxed"],
    )?;
    assert_eq!(groups.len(), 1);
    assert_eq!(
        (groups[0].count, groups[0].sums["amount_untaxed"]),
        (0, d("0"))
    );
    Ok(())
}

/// What cannot be grouped or summed is refused, saying why.
#[test]
fn test_what_cannot_be_grouped_is_refused() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let all = make_domain!([]);
    let refused = |env: &mut Environment, group_by: Option<&GroupBy>, sums: &[&str]| {
        env.read_group("invoice", &all, group_by, sums)
            .expect_err("refused")
            .to_string()
    };
    assert!(refused(&mut env, Some(&by("tags")), &[]).contains("holds records"));
    assert!(refused(&mut env, Some(&by("name:month")), &[]).contains("is no date"));
    assert!(refused(&mut env, Some(&by("tag_summary")), &[]).contains("worked out on each read"));
    assert!(refused(&mut env, None, &["name"]).contains("is no number"));
    assert!(GroupBy::parse("due_date:century").is_err());
    Ok(())
}

/// Over the protocol, a group says how to find its records; a record it is gathered by is
/// named.
#[test]
fn test_groups_are_answered_with_their_domain() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    seed(&mut env)?;
    let answer = env.call_rpc(
        "invoice",
        "read_group",
        &json!({"group_by": "due_date:month", "sums": ["amount_untaxed"]}),
    )?;
    assert_eq!(
        answer[0],
        json!({"value": "2026-01-01", "count": 2, "sums": {"amount_untaxed": "15.0"},
               "domain": [["due_date", ">=", "2026-01-01"], ["due_date", "<", "2026-02-01"]]})
    );
    assert_eq!(answer[2]["domain"], json!([["due_date", "=", null]]));
    let january = env.call_rpc("invoice", "search", &json!({"domain": answer[0]["domain"]}))?;
    assert_eq!(january.as_array().map(Vec::len), Some(2));

    let mut values = MapOfFields::default();
    values.insert("name", "Order 7");
    let order = env
        .create_records("sale_order", vec![values])?
        .get_ids_ref()[0];
    for price in [3, 4] {
        let mut values = MapOfFields::default();
        values.insert("order", erp_types::field::FieldType::Ref(order));
        values.insert("price", price);
        env.create_records("sale_order_line", vec![values])?;
    }
    let answer = env.call_rpc(
        "sale_order_line",
        "read_group",
        &json!({"group_by": "order", "sums": ["price"]}),
    )?;
    assert_eq!(
        answer,
        json!([{"value": [order, "Order 7"], "count": 2, "sums": {"price": "7"},
                "domain": [["order", "=", order]]}])
    );
    Ok(())
}

/// A week starts on Monday, a quarter every three months.
#[test]
fn test_periods_start_where_they_should() {
    assert_eq!(Period::Week.start(date("2026-10-08")), date("2026-10-05"));
    assert_eq!(Period::Week.next(date("2026-10-05")), date("2026-10-12"));
    assert_eq!(
        Period::Quarter.start(date("2026-08-20")),
        date("2026-07-01")
    );
    assert_eq!(Period::Quarter.next(date("2026-10-01")), date("2027-01-01"));
    assert_eq!(Period::Month.next(date("2026-01-01")), date("2026-02-01"));
}

/// Several domains are counted in one call, a count each in the order asked.
#[test]
fn test_several_domains_are_counted_at_once() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    seed(&mut env)?;
    let counts = env.call_rpc(
        "invoice",
        "count",
        &json!({ "domains": [[], [["due_date", "<", "2026-02-01"]], [["id", "=", 0]]] }),
    )?;
    let all = env.call_rpc("invoice", "count", &json!({}))?;
    assert_eq!(counts, json!([all, 2, 0]));
    Ok(())
}

/// Dates and moments come as text over the protocol, and are compared as what they write.
#[test]
fn test_dates_written_as_text_are_dates() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    seed(&mut env)?;
    let count = |env: &mut Environment, domain: serde_json::Value| -> Result<u64> {
        Ok(env
            .call_rpc("invoice", "count", &json!({ "domain": domain }))?
            .as_u64()
            .unwrap_or_default())
    };
    assert_eq!(
        count(&mut env, json!([["due_date", "<", "2026-02-01"]]))?,
        2
    );
    assert_eq!(
        count(
            &mut env,
            json!([["due_date", "in", ["2026-01-15", "2026-02-03"]]])
        )?,
        2
    );
    let mut values = MapOfFields::default();
    values.insert(
        "created_at",
        erp_types::field::DateTime::parse_from_rfc3339("2026-03-01T10:00:00Z")?
            .with_timezone(&erp_types::field::Utc),
    );
    env.create_records("invoice", vec![values])?;
    assert_eq!(
        count(&mut env, json!([["created_at", ">=", "2026-03-01"]]))?,
        1
    );
    assert_eq!(
        count(
            &mut env,
            json!([["created_at", ">", "2026-03-01 10:00:00"]])
        )?,
        0
    );
    let error = env
        .call_rpc(
            "invoice",
            "count",
            &json!({"domain": [["due_date", "=", "soon"]]}),
        )
        .expect_err("no date")
        .to_string();
    assert!(error.contains("is not a date"), "{error}");
    Ok(())
}
