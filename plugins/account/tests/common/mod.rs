//! What the accounting tests share: the application with the test chart, and records made the
//! way a user makes them.
#![allow(dead_code)]

use account::AccountPlugin;
use account::models::{Move, MoveLine, Payment};
use account::testing::TestChartPlugin;
use base::BasePlugin;
use contacts::ContactsPlugin;
use currency::CurrencyPlugin;
use erp::Result;
use erp::app::Application;
use erp::environment::Environment;
use erp::types::field::{Decimal, MultipleIds, SingleId};
use erp_test_support::xml_id;
use mail::MailPlugin;
use product::ProductPlugin;
use sequence::SequencePlugin;
use serde_json::{Value, json};
use uom::UomPlugin;
use web::WebPlugin;

#[allow(unused_imports)]
pub use erp_test_support::{admin_env, d, user_env};

pub fn new_app() -> Result<Application> {
    erp_test_support::app(
        || -> Vec<Box<dyn erp::plugin::Plugin>> {
            vec![
                Box::new(BasePlugin {}),
                Box::new(WebPlugin {}),
                Box::new(MailPlugin {}),
                Box::new(ContactsPlugin {}),
                Box::new(UomPlugin {}),
                Box::new(CurrencyPlugin {}),
                Box::new(SequencePlugin {}),
                Box::new(ProductPlugin {}),
                Box::new(AccountPlugin {}),
                Box::new(TestChartPlugin {}),
            ]
        },
        &["account_test_chart"],
    )
}

/// The id of a record of the test chart, `a_sales`, `tax_sale_21`.
pub fn chart(env: &mut Environment, name: &str) -> u32 {
    xml_id(env, &format!("account_test_chart.{name}"))
}

pub fn create(env: &mut Environment, model: &str, values: Value) -> Result<u32> {
    let ids = env.call_rpc(model, "create", &json!({ "values": values }))?;
    Ok(ids[0].as_u64().ok_or("an id")? as u32)
}

pub fn call(env: &mut Environment, model: &str, method: &str, ids: &[u32]) -> Result<Value> {
    env.call_rpc(model, method, &json!({ "ids": ids }))
}

pub fn read(env: &mut Environment, model: &str, id: u32, fields: &[&str]) -> Result<Value> {
    let rows = env.call_rpc(model, "read", &json!({"ids": [id], "fields": fields}))?;
    Ok(rows[0].clone())
}

pub fn partner(env: &mut Environment, name: &str) -> Result<u32> {
    create(env, "contact", json!({"name": name, "is_company": true}))
}

/// A product sold at `price`, bought at `cost`, with the 21 % taxes of the chart.
pub fn product(env: &mut Environment, name: &str, price: &str, cost: &str) -> Result<u32> {
    let sale = chart(env, "tax_sale_21");
    let purchase = chart(env, "tax_purchase_21");
    create(
        env,
        "product",
        json!({"name": name, "list_price": price, "standard_price": cost,
               "taxes": [sale], "supplier_taxes": [purchase]}),
    )
}

/// A draft invoice of `move_type` for `partner`, its lines given as JSON values.
pub fn invoice(
    env: &mut Environment,
    move_type: &str,
    partner: u32,
    lines: Vec<Value>,
) -> Result<u32> {
    create(
        env,
        "account_move",
        json!({"move_type": move_type, "partner": partner, "invoice_date": "2026-03-10",
               "invoice_lines": {"create": lines}}),
    )
}

pub fn post(env: &mut Environment, entry: u32) -> Result<()> {
    call(env, "account_move", "action_post", &[entry])?;
    Ok(())
}

pub fn entry(env: &mut Environment, id: u32) -> Move<SingleId> {
    env.get_record(id.into())
}

pub fn decimal(value: &Value) -> Decimal {
    match value {
        Value::String(text) => d(text),
        Value::Number(number) => d(&number.to_string()),
        other => panic!("{other} is no amount"),
    }
}

/// The journal items of an entry, as `(account code, debit, credit)` by account code.
pub fn items(env: &mut Environment, id: u32) -> Result<Vec<(String, Decimal, Decimal)>> {
    let entry = entry(env, id);
    let lines: MoveLine<MultipleIds> = entry.get_lines(env)?;
    let mut rows = Vec::new();
    for line in &lines {
        let account: account::models::Account<SingleId> = line.get_account(env)?;
        rows.push((
            account.get_code(env)?.clone(),
            *line.get_debit(env)?,
            *line.get_credit(env)?,
        ));
    }
    rows.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    Ok(rows)
}

/// Every invariant of the books, checked after a scenario.
pub fn check_books(env: &mut Environment) -> Result<()> {
    account::invariants::check_books(env)
}

/// Register and confirm the payment of what is left on invoices.
pub fn pay(env: &mut Environment, invoices: &[u32]) -> Result<Payment<SingleId>> {
    let answer = call(env, "account_move", "action_register_payment", invoices)?;
    let payment = answer["id"].as_u64().ok_or("a payment")? as u32;
    call(env, "account_payment", "action_post", &[payment])?;
    Ok(env.get_record(payment.into()))
}
