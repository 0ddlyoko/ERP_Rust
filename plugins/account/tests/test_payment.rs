//! Payments and matching: invoices paid in full, in part, together; payments cancelled; vendor
//! bills paid out; exchange differences; and what a paid invoice no longer allows.

mod common;

use common::*;
use erp::Result;
use erp::environment::Environment;
use serde_json::json;

fn sold(env: &mut Environment, customer: u32, price: &str) -> Result<u32> {
    let s21 = chart(env, "tax_sale_21");
    let invoice = invoice(
        env,
        "out_invoice",
        customer,
        vec![json!({"name": "Goods", "price_unit": price, "taxes": [s21]})],
    )?;
    post(env, invoice)?;
    Ok(invoice)
}

fn state(env: &mut Environment, invoice: u32) -> Result<(String, erp::types::field::Decimal)> {
    let row = read(
        env,
        "account_move",
        invoice,
        &["payment_state", "amount_residual"],
    )?;
    Ok((
        row["payment_state"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        decimal(&row["amount_residual"]),
    ))
}

/// Registering a payment prepares it for what is left; confirmed, it books the money waiting
/// for the bank and settles the invoice, which is paid.
#[test]
fn test_an_invoice_paid_in_full() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let customer = partner(&mut env, "Customer")?;
    let invoice = sold(&mut env, customer, "200")?;
    let answer = call(
        &mut env,
        "account_move",
        "action_register_payment",
        &[invoice],
    )?;
    assert_eq!(answer["action"], json!("account.action_payments_received"));
    let payment = answer["id"].as_u64().expect("a payment") as u32;
    let row = read(
        &mut env,
        "account_payment",
        payment,
        &[
            "amount",
            "partner",
            "payment_type",
            "state",
            "memo",
            "journal",
        ],
    )?;
    assert_eq!(decimal(&row["amount"]), d("242"));
    assert_eq!(row["partner"], json!(customer));
    assert_eq!(row["payment_type"], json!("inbound"));
    assert_eq!(row["state"], json!("draft"));
    assert_eq!(row["memo"], json!("INV/2026/00001"));
    assert_eq!(row["journal"], json!(chart(&mut env, "journal_bank")));
    call(&mut env, "account_payment", "action_post", &[payment])?;
    let row = read(
        &mut env,
        "account_payment",
        payment,
        &["name", "state", "move_id"],
    )?;
    assert_eq!(row["name"], json!("BNK1/2026/00001"));
    assert_eq!(row["state"], json!("posted"));
    let entry = row["move_id"].as_u64().expect("its entry") as u32;
    assert_eq!(
        items(&mut env, entry)?,
        vec![
            ("400000".to_string(), d("0"), d("242")),
            ("499001".to_string(), d("242"), d("0"))
        ]
    );
    assert_eq!(state(&mut env, invoice)?, ("paid".to_string(), d("0")));
    check_books(&mut env)
}

/// Paid in two goes, an invoice is partly paid, then paid.
#[test]
fn test_an_invoice_paid_in_part() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let customer = partner(&mut env, "Customer")?;
    let invoice = sold(&mut env, customer, "100")?;
    let first = create(
        &mut env,
        "account_payment",
        json!({"partner": customer, "amount": "100", "invoices": [invoice]}),
    )?;
    call(&mut env, "account_payment", "action_post", &[first])?;
    assert_eq!(state(&mut env, invoice)?, ("partial".to_string(), d("21")));
    pay(&mut env, &[invoice])?;
    assert_eq!(state(&mut env, invoice)?, ("paid".to_string(), d("0")));
    check_books(&mut env)
}

/// One payment settles two invoices of a customer; a credit note lowers what is asked.
#[test]
fn test_one_payment_for_several_documents() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let customer = partner(&mut env, "Customer")?;
    let first = sold(&mut env, customer, "100")?;
    let second = sold(&mut env, customer, "50")?;
    let third = sold(&mut env, customer, "10")?;
    let answer = call(&mut env, "account_move", "action_reverse", &[third])?;
    let refund = answer["id"].as_u64().expect("a credit note") as u32;
    post(&mut env, refund)?;
    let payment = pay(&mut env, &[first, second, third, refund])?;
    assert_eq!(
        *payment.get_amount(&mut env)?,
        d("181.5"),
        "121 + 60.50 + 12.10 - 12.10"
    );
    for invoice in [first, second, third] {
        assert_eq!(state(&mut env, invoice)?.0, "paid", "invoice {invoice}");
    }
    assert_eq!(state(&mut env, refund)?.0, "paid");
    let other = partner(&mut env, "Someone else")?;
    let theirs = sold(&mut env, other, "10")?;
    let error = call(
        &mut env,
        "account_move",
        "action_register_payment",
        &[first, theirs],
    )
    .expect_err("two partners")
    .to_string();
    assert!(error.contains("separately"), "{error}");
    check_books(&mut env)
}

/// Cancelling a payment opens the invoice again, and its entry is cancelled; a paid invoice
/// cannot go back to draft.
#[test]
fn test_cancelling_a_payment() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let customer = partner(&mut env, "Customer")?;
    let invoice = sold(&mut env, customer, "100")?;
    let payment = pay(&mut env, &[invoice])?;
    let error = call(&mut env, "account_move", "button_draft", &[invoice])
        .expect_err("paid")
        .to_string();
    assert!(error.contains("matched"), "{error}");
    call(
        &mut env,
        "account_payment",
        "action_cancel",
        &[payment.get_id()],
    )?;
    assert_eq!(
        state(&mut env, invoice)?,
        ("not_paid".to_string(), d("121"))
    );
    let row = read(
        &mut env,
        "account_payment",
        payment.get_id(),
        &["state", "move_id"],
    )?;
    assert_eq!(row["state"], json!("cancel"));
    let entry = row["move_id"].as_u64().expect("its entry") as u32;
    assert_eq!(
        read(&mut env, "account_move", entry, &["state"])?["state"],
        json!("cancel")
    );
    call(
        &mut env,
        "account_payment",
        "action_draft",
        &[payment.get_id()],
    )?;
    call(
        &mut env,
        "account_payment",
        "action_post",
        &[payment.get_id()],
    )?;
    assert_eq!(
        state(&mut env, invoice)?.0,
        "paid",
        "confirmed again, it pays again"
    );
    check_books(&mut env)
}

/// A vendor bill is paid out of the bank: the supplier debited, the money waiting to leave.
#[test]
fn test_a_vendor_bill_paid() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let vendor = partner(&mut env, "Vendor")?;
    let p21 = chart(&mut env, "tax_purchase_21");
    let bill = invoice(
        &mut env,
        "in_invoice",
        vendor,
        vec![json!({"name": "Paper", "price_unit": "50", "taxes": [p21]})],
    )?;
    post(&mut env, bill)?;
    let answer = call(&mut env, "account_move", "action_register_payment", &[bill])?;
    assert_eq!(answer["action"], json!("account.action_payments_sent"));
    let payment = answer["id"].as_u64().expect("a payment") as u32;
    let row = read(
        &mut env,
        "account_payment",
        payment,
        &["payment_type", "partner_type", "amount"],
    )?;
    assert_eq!(
        (row["payment_type"].clone(), row["partner_type"].clone()),
        (json!("outbound"), json!("supplier"))
    );
    call(&mut env, "account_payment", "action_post", &[payment])?;
    let entry = read(&mut env, "account_payment", payment, &["move_id"])?["move_id"]
        .as_u64()
        .expect("entry") as u32;
    assert_eq!(
        items(&mut env, entry)?,
        vec![
            ("440000".to_string(), d("60.5"), d("0")),
            ("499002".to_string(), d("0"), d("60.5"))
        ]
    );
    assert_eq!(state(&mut env, bill)?.0, "paid");
    check_books(&mut env)
}

/// A customer's credit note is paid back to them.
#[test]
fn test_a_credit_note_paid_back() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let customer = partner(&mut env, "Customer")?;
    let invoice = sold(&mut env, customer, "100")?;
    pay(&mut env, &[invoice])?;
    let answer = call(&mut env, "account_move", "action_reverse", &[invoice])?;
    let refund = answer["id"].as_u64().expect("a credit note") as u32;
    post(&mut env, refund)?;
    let payment = pay(&mut env, &[refund])?;
    assert!(matches!(
        *payment.get_payment_type(&mut env)?,
        account::models::PaymentType::Outbound
    ));
    assert_eq!(*payment.get_amount(&mut env)?, d("121"));
    assert_eq!(state(&mut env, refund)?.0, "paid");
    check_books(&mut env)
}

/// An invoice in dollars paid when the dollar is worth more: the dollars settle, the euros
/// differ, and the difference is booked as an exchange gain.
#[test]
fn test_an_exchange_difference() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let usd = erp_test_support::xml_id(&mut env, "currency.currency_usd");
    create(
        &mut env,
        "currency_rate",
        json!({"currency": usd, "date": "2026-01-01", "rate": "1.25"}),
    )?;
    create(
        &mut env,
        "currency_rate",
        json!({"currency": usd, "date": "2026-04-01", "rate": "1.00"}),
    )?;
    let customer = partner(&mut env, "Customer")?;
    let invoice = create(
        &mut env,
        "account_move",
        json!({"move_type": "out_invoice", "partner": customer,
        "invoice_date": "2026-03-10", "currency": usd,
        "invoice_lines": {"create": [{"name": "Consulting", "price_unit": "125", "taxes": []}]}}),
    )?;
    post(&mut env, invoice)?;
    let payment = create(
        &mut env,
        "account_payment",
        json!({"partner": customer, "amount": "125",
        "currency": usd, "date": "2026-04-15", "invoices": [invoice]}),
    )?;
    call(&mut env, "account_payment", "action_post", &[payment])?;
    assert_eq!(state(&mut env, invoice)?, ("paid".to_string(), d("0")));
    let exchange = chart(&mut env, "journal_exchange");
    let entries = env.call_rpc(
        "account_move",
        "search",
        &json!({"domain": [["journal", "=", exchange]]}),
    )?;
    let entries = entries.as_array().expect("entries").clone();
    assert_eq!(entries.len(), 1, "one exchange difference");
    let entry = entries[0].as_u64().expect("an entry") as u32;
    assert_eq!(
        items(&mut env, entry)?,
        vec![
            ("400000".to_string(), d("25"), d("0")),
            ("754000".to_string(), d("0"), d("25"))
        ]
    );
    check_books(&mut env)
}

/// Journal items are matched by hand, on one account only, and unmatched.
#[test]
fn test_matching_by_hand() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let customer = partner(&mut env, "Customer")?;
    let invoice = sold(&mut env, customer, "100")?;
    let (receivable, bank) = (chart(&mut env, "a_receivable"), chart(&mut env, "a_bank"));
    let cash_in = create(
        &mut env,
        "account_move",
        json!({"date": "2026-03-20", "lines": {"create": [
        {"account": bank, "debit": "121"}, {"account": receivable, "credit": "121", "partner": customer}]}}),
    )?;
    post(&mut env, cash_in)?;
    let receivables = env.call_rpc(
        "account_move_line",
        "search",
        &json!({"domain": [["account", "=", receivable]]}),
    )?;
    let ids: Vec<u32> = receivables
        .as_array()
        .expect("ids")
        .iter()
        .map(|id| id.as_u64().expect("id") as u32)
        .collect();
    assert_eq!(ids.len(), 2);
    call(&mut env, "account_move_line", "reconcile", &ids)?;
    assert_eq!(state(&mut env, invoice)?.0, "paid");
    let matched = read(
        &mut env,
        "account_move_line",
        ids[0],
        &["full_reconcile", "reconciled"],
    )?;
    assert_eq!(matched["reconciled"], json!(true));
    assert!(
        matched["full_reconcile"].is_array() || matched["full_reconcile"].is_number(),
        "{matched}"
    );
    let all = env.call_rpc(
        "account_move_line",
        "search",
        &json!({"domain": [["move_id", "=", cash_in]]}),
    )?;
    let all: Vec<u32> = all
        .as_array()
        .expect("ids")
        .iter()
        .map(|id| id.as_u64().expect("id") as u32)
        .collect();
    let error = call(&mut env, "account_move_line", "reconcile", &all)
        .expect_err("two accounts")
        .to_string();
    assert!(
        error.contains("does not allow matching") || error.contains("same account"),
        "{error}"
    );
    call(
        &mut env,
        "account_move_line",
        "remove_move_reconcile",
        &ids[..1],
    )?;
    assert_eq!(
        state(&mut env, invoice)?,
        ("not_paid".to_string(), d("121"))
    );
    check_books(&mut env)
}

/// Yen have no cents: an invoice in yen paid in two goes at another rate settles in yen, the
/// euros matched rounded to the cent, and each payment's difference booked on its own.
#[test]
fn test_an_exchange_difference_in_a_currency_without_cents() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let jpy = erp_test_support::xml_id(&mut env, "currency.currency_jpy");
    create(
        &mut env,
        "currency_rate",
        json!({"currency": jpy, "date": "2026-01-01", "rate": "160"}),
    )?;
    create(
        &mut env,
        "currency_rate",
        json!({"currency": jpy, "date": "2026-04-01", "rate": "150"}),
    )?;
    let customer = partner(&mut env, "Customer")?;
    let invoice = create(
        &mut env,
        "account_move",
        json!({"move_type": "out_invoice", "partner": customer,
        "invoice_date": "2026-03-10", "currency": jpy,
        "invoice_lines": {"create": [{"name": "Consulting", "price_unit": "10000", "taxes": []}]}}),
    )?;
    post(&mut env, invoice)?;
    assert_eq!(
        items(&mut env, invoice)?[0],
        ("400000".to_string(), d("62.5"), d("0"))
    );
    for amount in ["3333", "6667"] {
        let payment = create(
            &mut env,
            "account_payment",
            json!({"partner": customer, "amount": amount,
            "currency": jpy, "date": "2026-04-15", "invoices": [invoice]}),
        )?;
        call(&mut env, "account_payment", "action_post", &[payment])?;
    }
    assert_eq!(state(&mut env, invoice)?, ("paid".to_string(), d("0")));
    let exchange = chart(&mut env, "journal_exchange");
    let entries = env.call_rpc(
        "account_move",
        "search",
        &json!({"domain": [["journal", "=", exchange]]}),
    )?;
    let entries: Vec<u32> = entries
        .as_array()
        .expect("entries")
        .iter()
        .map(|id| id.as_u64().expect("an id") as u32)
        .collect();
    // 3333 yen paid 22.22 euros for 20.83 invoiced, 6667 yen 44.45 for the 41.67 left.
    let mut gains = Vec::new();
    for entry in entries {
        gains.push(items(&mut env, entry)?);
    }
    gains.sort();
    assert_eq!(
        gains,
        vec![
            vec![
                ("400000".to_string(), d("1.39"), d("0")),
                ("754000".to_string(), d("0"), d("1.39"))
            ],
            vec![
                ("400000".to_string(), d("2.78"), d("0")),
                ("754000".to_string(), d("0"), d("2.78"))
            ],
        ]
    );
    check_books(&mut env)
}
