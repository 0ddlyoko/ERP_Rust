//! Currencies: the seeded ones, rates by date, conversions, the company's currency, the rules a
//! rate follows, who may change them, and their views.

use base::BasePlugin;
use base::models::{Company, View};
use currency::CurrencyPlugin;
use currency::models::{CompanyCurrency, Currency};
use erp::Result;
use erp::app::Application;
use erp::environment::Environment;
use erp::types::field::{IdMode, NaiveDate, SingleId};
use erp::types::model::MapOfFields;
use erp_test_support::{admin_env, d, user_env, xml_id};
use serde_json::json;
use web::WebPlugin;

fn new_app() -> Result<Application> {
    erp_test_support::app(
        vec![
            Box::new(BasePlugin {}),
            Box::new(WebPlugin {}),
            Box::new(CurrencyPlugin {}),
        ],
        "currency",
    )
}

fn date(text: &str) -> NaiveDate {
    NaiveDate::parse_from_str(text, "%Y-%m-%d").expect("a date")
}

fn currency(env: &mut Environment, code: &str) -> Result<Currency<SingleId>> {
    env.named(&format!("currency.currency_{code}"))
}

fn add_rate(env: &mut Environment, code: &str, on: &str, rate: &str) -> Result<u32> {
    let currency = xml_id(env, &format!("currency.currency_{code}"));
    let mut values = MapOfFields::default();
    values.insert("currency", currency);
    values.insert("date", date(on));
    values.insert("rate", d(rate));
    Ok(env
        .create_records("currency_rate", vec![values])?
        .get_ids_ref()[0])
}

/// Installed, the plugin gives the main company the euro.
#[test]
fn test_the_company_keeps_its_books_in_euros() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let euro = currency(&mut env, "eur")?;
    assert_eq!(Currency::of_company(&mut env)?.get_id(), euro.get_id());
    let company = Company::current(&mut env)?;
    let company: CompanyCurrency<SingleId> = env.get_record(company.get_id().into());
    let held: Currency<SingleId> = company.get_currency(&mut env)?;
    assert_eq!(held.get_name(&mut env)?, "EUR");
    Ok(())
}

/// The rate of a day is the latest set on or before it; before any, and for the company's
/// currency, it is 1.
#[test]
fn test_the_rate_in_force_on_a_date() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    add_rate(&mut env, "usd", "2026-01-01", "1.0500")?;
    add_rate(&mut env, "usd", "2026-03-01", "1.0850")?;
    let usd = currency(&mut env, "usd")?;
    let euro = currency(&mut env, "eur")?;
    assert_eq!(
        usd.rate_at(&mut env, date("2025-12-31"))?,
        d("1"),
        "no rate yet"
    );
    assert_eq!(usd.rate_at(&mut env, date("2026-01-01"))?, d("1.05"));
    assert_eq!(usd.rate_at(&mut env, date("2026-02-28"))?, d("1.05"));
    assert_eq!(usd.rate_at(&mut env, date("2026-03-01"))?, d("1.085"));
    assert_eq!(usd.rate_at(&mut env, date("2027-01-01"))?, d("1.085"));
    assert_eq!(euro.rate_at(&mut env, date("2026-03-01"))?, d("1"));
    assert_eq!(*usd.get_rate(&mut env)?, d("1.085"), "today's rate");
    Ok(())
}

/// Amounts convert at the rates of their date, rounded to the target currency.
#[test]
fn test_amounts_convert_at_the_rates_of_their_date() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    add_rate(&mut env, "usd", "2026-01-01", "1.0500")?;
    add_rate(&mut env, "usd", "2026-03-01", "1.0850")?;
    add_rate(&mut env, "gbp", "2026-01-01", "0.8500")?;
    add_rate(&mut env, "jpy", "2026-01-01", "162.37")?;
    let usd = currency(&mut env, "usd")?;
    let euro = currency(&mut env, "eur")?;
    let gbp = currency(&mut env, "gbp")?;
    let jpy = currency(&mut env, "jpy")?;

    assert_eq!(
        euro.convert(&mut env, d("100"), usd.clone(), date("2026-02-01"))?,
        d("105")
    );
    assert_eq!(
        euro.convert(&mut env, d("100"), usd.clone(), date("2026-03-15"))?,
        d("108.5")
    );
    assert_eq!(
        usd.convert(&mut env, d("108.50"), euro.clone(), date("2026-03-15"))?,
        d("100")
    );
    assert_eq!(
        usd.convert(&mut env, d("10"), gbp, date("2026-03-15"))?,
        d("7.83")
    );
    assert_eq!(
        euro.convert(&mut env, d("10.99"), jpy, date("2026-03-15"))?,
        d("1784")
    );
    assert_eq!(
        euro.convert(&mut env, d("-50"), usd, date("2026-03-15"))?,
        d("-54.25")
    );
    assert_eq!(
        euro.convert(&mut env, d("10.005"), euro.clone(), date("2026-03-15"))?,
        d("10.01")
    );
    Ok(())
}

/// Amounts are shown with the currency's symbol where it goes and its decimals.
#[test]
fn test_amounts_are_formatted() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let euro = currency(&mut env, "eur")?;
    let usd = currency(&mut env, "usd")?;
    let jpy = currency(&mut env, "jpy")?;
    assert_eq!(euro.format(&mut env, d("1234.5"))?, "1234.50 €");
    assert_eq!(usd.format(&mut env, d("-3"))?, "$ -3.00");
    assert_eq!(jpy.format(&mut env, d("1784.4"))?, "¥ 1784");
    Ok(())
}

/// A rate is positive, and a currency has one a day.
#[test]
fn test_rates_follow_their_rules() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    assert!(add_rate(&mut env, "usd", "2026-01-01", "0").is_err());
    assert!(add_rate(&mut env, "usd", "2026-01-01", "-1").is_err());
    add_rate(&mut env, "usd", "2026-01-01", "1.05")?;
    let error = add_rate(&mut env, "usd", "2026-01-01", "1.06")
        .expect_err("one a day")
        .to_string();
    assert!(error.contains("2026-01-01"), "{error}");
    add_rate(&mut env, "gbp", "2026-01-01", "0.85")?;
    let count = env.count("currency_rate", &erp_search::SearchType::Nothing)?;
    assert_eq!(count, 2, "the refused rates are not kept");
    Ok(())
}

/// Employees read currencies and rates; administrators change them.
#[test]
fn test_access_rights() -> Result<()> {
    let app = new_app()?;
    let mut env = user_env(&app, "employee", &["base.group_user"])?;
    let found = env.call_rpc(
        "currency",
        "search",
        &json!({"domain": [["name", "=", "USD"]]}),
    )?;
    assert_eq!(found.as_array().map(Vec::len), Some(1));
    assert!(add_rate(&mut env, "usd", "2026-01-01", "1.05").is_err());
    drop(env);
    let mut env = admin_env(&app)?;
    assert!(add_rate(&mut env, "usd", "2026-01-01", "1.05").is_ok());
    Ok(())
}

/// A currency goes with its rates; the company's currency cannot go.
#[test]
fn test_deleting() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    add_rate(&mut env, "usd", "2026-01-01", "1.05")?;
    let usd = xml_id(&mut env, "currency.currency_usd");
    env.delete("currency", &SingleId::from(usd))?;
    assert_eq!(
        env.count("currency_rate", &erp_search::SearchType::Nothing)?,
        0
    );
    let euro = xml_id(&mut env, "currency.currency_eur");
    assert!(env.delete("currency", &SingleId::from(euro)).is_err());
    Ok(())
}

/// The views resolve; the company form shows its currency.
#[test]
fn test_views_load() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    for (model, kind) in [
        ("currency", "list"),
        ("currency", "form"),
        ("currency", "search"),
        ("currency_rate", "list"),
    ] {
        let arch = env.get_empty_record::<View<_>>().load(
            &mut env,
            model.to_string(),
            kind.to_string(),
        )?;
        assert!(
            arch.starts_with(&format!("<{kind}")),
            "{model} {kind}: {arch}"
        );
    }
    let company = env.get_empty_record::<View<_>>().load(
        &mut env,
        "company".to_string(),
        "form".to_string(),
    )?;
    assert!(company.contains(r#"<field name="currency""#), "{company}");
    for action in [
        "currency.action_currencies",
        "currency.action_currency_rates",
    ] {
        xml_id(&mut env, action);
    }
    Ok(())
}
