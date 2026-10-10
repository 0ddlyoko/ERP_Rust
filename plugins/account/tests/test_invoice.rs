//! Invoices, bills and credit notes: their lines filled from products, their totals, the journal
//! items posting makes, their numbering, and what may still change once posted.

mod common;

use account::models::{AccountMove, MoveState, PaymentState};
use common::*;
use erp::Result;
use erp::types::field::SingleId;
use serde_json::json;

fn line(product: u32, quantity: &str) -> serde_json::Value {
    json!({"product": product, "quantity": quantity})
}

/// A customer invoice adds up its lines and posts as the customer owing the total, the sale and
/// the VAT credited.
#[test]
fn test_a_customer_invoice_posts_its_journal_items() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let customer = partner(&mut env, "Customer")?;
    let desk = product(&mut env, "Desk", "100", "60")?;
    let invoice = invoice(&mut env, "out_invoice", customer, vec![line(desk, "2")])?;
    let row = read(
        &mut env,
        "account_move",
        invoice,
        &[
            "name",
            "state",
            "amount_untaxed",
            "amount_tax",
            "amount_total",
        ],
    )?;
    assert_eq!(row["name"], json!("/"));
    assert_eq!(row["state"], json!("draft"));
    assert_eq!(decimal(&row["amount_untaxed"]), d("200"));
    assert_eq!(decimal(&row["amount_tax"]), d("42"));
    assert_eq!(decimal(&row["amount_total"]), d("242"));

    post(&mut env, invoice)?;
    let row = read(
        &mut env,
        "account_move",
        invoice,
        &[
            "name",
            "state",
            "amount_residual",
            "payment_state",
            "payment_reference",
        ],
    )?;
    assert_eq!(row["name"], json!("INV/2026/00001"));
    assert_eq!(row["state"], json!("posted"));
    assert_eq!(decimal(&row["amount_residual"]), d("242"));
    assert_eq!(row["payment_state"], json!("not_paid"));
    assert_eq!(row["payment_reference"], json!("INV/2026/00001"));
    assert_eq!(
        items(&mut env, invoice)?,
        vec![
            ("400000".to_string(), d("242"), d("0")),
            ("451000".to_string(), d("0"), d("42")),
            ("700000".to_string(), d("0"), d("200")),
        ]
    );
    let second = invoice_for(&mut env, customer, desk)?;
    post(&mut env, second)?;
    assert_eq!(
        read(&mut env, "account_move", second, &["name"])?["name"],
        json!("INV/2026/00002")
    );
    check_books(&mut env)
}

fn invoice_for(
    env: &mut erp::environment::Environment,
    customer: u32,
    product: u32,
) -> Result<u32> {
    common::invoice(env, "out_invoice", customer, vec![line(product, "1")])
}

/// A line takes its label, account, price, unit and taxes from its product; a price typed over
/// it is kept.
#[test]
fn test_lines_are_filled_from_the_product() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let customer = partner(&mut env, "Customer")?;
    let s21 = chart(&mut env, "tax_sale_21");
    let desk = create(
        &mut env,
        "product",
        json!({"name": "Desk", "default_code": "D1", "list_price": "100",
        "description_sale": "Oak, 160 cm", "taxes": [s21]}),
    )?;
    let invoice = invoice(&mut env, "out_invoice", customer, vec![line(desk, "1")])?;
    let lines =
        read(&mut env, "account_move", invoice, &["invoice_lines"])?["invoice_lines"].clone();
    let first = lines[0].as_u64().expect("a line") as u32;
    let row = read(
        &mut env,
        "account_invoice_line",
        first,
        &["name", "account", "price_unit", "taxes", "uom"],
    )?;
    assert_eq!(row["name"], json!("[D1] Desk\nOak, 160 cm"));
    assert_eq!(row["account"], json!(chart(&mut env, "a_sales")));
    assert_eq!(decimal(&row["price_unit"]), d("100"));
    assert_eq!(row["taxes"], json!([chart(&mut env, "tax_sale_21")]));
    assert_eq!(
        row["uom"],
        json!(erp_test_support::xml_id(&mut env, "uom.uom_unit"))
    );

    env.call_rpc(
        "account_invoice_line",
        "write",
        &json!({"ids": [first], "values": {"price_unit": "90", "discount": "10"}}),
    )?;
    let row = read(
        &mut env,
        "account_invoice_line",
        first,
        &["price_unit", "price_subtotal", "price_total"],
    )?;
    assert_eq!(
        decimal(&row["price_unit"]),
        d("90"),
        "typed over the product's price"
    );
    assert_eq!(decimal(&row["price_subtotal"]), d("81"));
    assert_eq!(decimal(&row["price_total"]), d("98.01"));
    let haggled = create(
        &mut env,
        "account_invoice_line",
        json!({"move_id": invoice, "product": desk, "price_unit": "75"}),
    )?;
    let row = read(&mut env, "account_invoice_line", haggled, &["price_unit"])?;
    assert_eq!(
        decimal(&row["price_unit"]),
        d("75"),
        "given with the product, the price stays"
    );
    Ok(())
}

/// Lines with different rates make a tax item each; a discount and a half cent round per line.
#[test]
fn test_rates_discounts_and_rounding() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let customer = partner(&mut env, "Customer")?;
    let (s21, s6, s0) = (
        chart(&mut env, "tax_sale_21"),
        chart(&mut env, "tax_sale_6"),
        chart(&mut env, "tax_sale_0"),
    );
    let invoice = invoice(
        &mut env,
        "out_invoice",
        customer,
        vec![
            json!({"name": "Chairs", "quantity": "3", "price_unit": "19.99", "discount": "10", "taxes": [s21]}),
            json!({"name": "Books", "quantity": "1", "price_unit": "15.55", "taxes": [s6]}),
            json!({"name": "Export", "quantity": "1", "price_unit": "10", "taxes": [s0]}),
            json!({"name": "No tax", "quantity": "2", "price_unit": "2.50", "taxes": []}),
        ],
    )?;
    let row = read(
        &mut env,
        "account_move",
        invoice,
        &["amount_untaxed", "amount_tax", "amount_total"],
    )?;
    // 53.97 + 15.55 + 10 + 5; 11.33 (21 % of 53.973) + 0.93 (6 % of 15.55).
    assert_eq!(decimal(&row["amount_untaxed"]), d("84.52"));
    assert_eq!(decimal(&row["amount_tax"]), d("12.26"));
    assert_eq!(decimal(&row["amount_total"]), d("96.78"));
    post(&mut env, invoice)?;
    assert_eq!(
        items(&mut env, invoice)?,
        vec![
            ("400000".to_string(), d("96.78"), d("0")),
            ("451000".to_string(), d("0"), d("0.93")),
            ("451000".to_string(), d("0"), d("11.33")),
            ("700000".to_string(), d("0"), d("5")),
            ("700000".to_string(), d("0"), d("10")),
            ("700000".to_string(), d("0"), d("15.55")),
            ("700000".to_string(), d("0"), d("53.97")),
        ]
    );
    check_books(&mut env)
}

/// A price including its tax keeps the total shown.
#[test]
fn test_a_tax_included_in_the_price() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let customer = partner(&mut env, "Customer")?;
    let included = chart(&mut env, "tax_sale_21_included");
    let invoice = invoice(
        &mut env,
        "out_invoice",
        customer,
        vec![json!({"name": "Ticket", "quantity": "1", "price_unit": "10", "taxes": [included]})],
    )?;
    let row = read(
        &mut env,
        "account_move",
        invoice,
        &["amount_untaxed", "amount_tax", "amount_total"],
    )?;
    assert_eq!(
        (
            decimal(&row["amount_untaxed"]),
            decimal(&row["amount_tax"]),
            decimal(&row["amount_total"])
        ),
        (d("8.26"), d("1.74"), d("10"))
    );
    post(&mut env, invoice)?;
    check_books(&mut env)
}

/// A vendor bill posts the purchase and the deductible VAT debited, the supplier credited.
#[test]
fn test_a_vendor_bill_posts_its_journal_items() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let vendor = partner(&mut env, "Vendor")?;
    let paper = product(&mut env, "Paper", "10", "4.50")?;
    let bill = invoice(&mut env, "in_invoice", vendor, vec![line(paper, "20")])?;
    let row = read(
        &mut env,
        "account_move",
        bill,
        &["amount_untaxed", "amount_total", "journal"],
    )?;
    assert_eq!(decimal(&row["amount_untaxed"]), d("90"), "bought at cost");
    assert_eq!(decimal(&row["amount_total"]), d("108.9"));
    assert_eq!(row["journal"], json!(chart(&mut env, "journal_purchase")));
    post(&mut env, bill)?;
    assert_eq!(
        read(&mut env, "account_move", bill, &["name"])?["name"],
        json!("BILL/2026/00001")
    );
    assert_eq!(
        items(&mut env, bill)?,
        vec![
            ("411000".to_string(), d("18.9"), d("0")),
            ("440000".to_string(), d("0"), d("108.9")),
            ("604000".to_string(), d("90"), d("0")),
        ]
    );
    check_books(&mut env)
}

/// A reverse charge leaves the supplier owed the untaxed amount, the VAT both owed and deducted.
#[test]
fn test_a_reverse_charge_bill() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let vendor = partner(&mut env, "Contractor")?;
    let reverse = chart(&mut env, "tax_purchase_21_reverse");
    let bill = invoice(
        &mut env,
        "in_invoice",
        vendor,
        vec![json!({"name": "Works", "quantity": "1", "price_unit": "1000", "taxes": [reverse]})],
    )?;
    let row = read(
        &mut env,
        "account_move",
        bill,
        &["amount_tax", "amount_total"],
    )?;
    assert_eq!(
        (decimal(&row["amount_tax"]), decimal(&row["amount_total"])),
        (d("0"), d("1000"))
    );
    post(&mut env, bill)?;
    assert_eq!(
        items(&mut env, bill)?,
        vec![
            ("411000".to_string(), d("210"), d("0")),
            ("440000".to_string(), d("0"), d("1000")),
            ("451000".to_string(), d("0"), d("210")),
            ("604000".to_string(), d("1000"), d("0")),
        ]
    );
    check_books(&mut env)
}

/// A credit note is made from the invoice, numbered apart, its items mirrored; settling the
/// invoice with it marks the invoice reversed.
#[test]
fn test_credit_notes() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let customer = partner(&mut env, "Customer")?;
    let desk = product(&mut env, "Desk", "100", "60")?;
    let invoice = invoice(&mut env, "out_invoice", customer, vec![line(desk, "1")])?;
    post(&mut env, invoice)?;

    let answer = call(&mut env, "account_move", "action_reverse", &[invoice])?;
    assert_eq!(answer["type"], json!("open"));
    assert_eq!(answer["action"], json!("account.action_move_out_refund"));
    let refund = answer["id"].as_u64().expect("the credit note") as u32;
    let row = read(
        &mut env,
        "account_move",
        refund,
        &["move_type", "state", "amount_total", "reversed_entry"],
    )?;
    assert_eq!(row["move_type"], json!("out_refund"));
    assert_eq!(row["state"], json!("draft"));
    assert_eq!(decimal(&row["amount_total"]), d("121"));
    assert_eq!(row["reversed_entry"], json!(invoice));
    post(&mut env, refund)?;
    assert_eq!(
        read(&mut env, "account_move", refund, &["name"])?["name"],
        json!("RINV/2026/00001")
    );
    assert_eq!(
        items(&mut env, refund)?,
        vec![
            ("400000".to_string(), d("0"), d("121")),
            ("451000".to_string(), d("21"), d("0")),
            ("700000".to_string(), d("100"), d("0")),
        ]
    );

    let other = common::invoice(&mut env, "out_invoice", customer, vec![line(desk, "2")])?;
    post(&mut env, other)?;
    call(
        &mut env,
        "account_move",
        "action_reverse_and_reconcile",
        &[other],
    )?;
    let row = read(
        &mut env,
        "account_move",
        other,
        &["payment_state", "amount_residual"],
    )?;
    assert_eq!(row["payment_state"], json!("reversed"));
    assert_eq!(decimal(&row["amount_residual"]), d("0"));
    check_books(&mut env)
}

/// A vendor credit note mirrors the bill.
#[test]
fn test_vendor_credit_notes() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let vendor = partner(&mut env, "Vendor")?;
    let paper = product(&mut env, "Paper", "10", "5")?;
    let bill = invoice(&mut env, "in_invoice", vendor, vec![line(paper, "10")])?;
    post(&mut env, bill)?;
    call(
        &mut env,
        "account_move",
        "action_reverse_and_reconcile",
        &[bill],
    )?;
    let refund: AccountMove<erp::types::field::MultipleIds> = env.search(
        &erp_search_code_gen::make_domain!([("move_type", "=", "in_refund")]),
    )?;
    let refund = refund.get_ids_ref()[0];
    assert_eq!(
        read(&mut env, "account_move", refund, &["name"])?["name"],
        json!("RBILL/2026/00001")
    );
    assert_eq!(
        items(&mut env, refund)?,
        vec![
            ("411000".to_string(), d("0"), d("10.5")),
            ("440000".to_string(), d("60.5"), d("0")),
            ("604000".to_string(), d("0"), d("50")),
        ]
    );
    assert_eq!(
        read(&mut env, "account_move", bill, &["payment_state"])?["payment_state"],
        json!("reversed")
    );
    check_books(&mut env)
}

/// Payment terms split what the customer owes into installments, each with its due date.
#[test]
fn test_installments() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let customer = partner(&mut env, "Customer")?;
    let term = chart(&mut env, "term_advance");
    let desk = product(&mut env, "Desk", "1000", "600")?;
    let invoice = create(
        &mut env,
        "account_move",
        json!({"move_type": "out_invoice", "partner": customer,
        "invoice_date": "2026-03-10", "payment_term": term, "invoice_lines": {"create": [line(desk, "1")]}}),
    )?;
    post(&mut env, invoice)?;
    let entry = entry(&mut env, invoice);
    let lines: account::models::AccountMoveLine<erp::types::field::MultipleIds> =
        entry.get_lines(&mut env)?;
    let mut receivables = Vec::new();
    for item in &lines {
        if item.get_debit(&mut env)? > &d("0") {
            receivables.push((
                item.get_date_maturity(&mut env)?
                    .map(|date| date.to_string()),
                *item.get_debit(&mut env)?,
            ));
        }
    }
    receivables.sort();
    assert_eq!(
        receivables,
        vec![
            (Some("2026-03-10".to_string()), d("363")),
            (Some("2026-05-09".to_string()), d("847"))
        ]
    );
    assert_eq!(
        read(&mut env, "account_move", invoice, &["invoice_date_due"])?["invoice_date_due"],
        json!("2026-05-09")
    );
    check_books(&mut env)
}

/// An invoice in dollars is booked in euros at the rate of its date, the dollars kept beside.
#[test]
fn test_an_invoice_in_another_currency() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let usd = erp_test_support::xml_id(&mut env, "currency.currency_usd");
    create(
        &mut env,
        "currency_rate",
        json!({"currency": usd, "date": "2026-01-01", "rate": "1.25"}),
    )?;
    let customer = partner(&mut env, "Customer")?;
    let s21 = chart(&mut env, "tax_sale_21");
    let invoice = create(
        &mut env,
        "account_move",
        json!({"move_type": "out_invoice", "partner": customer,
        "invoice_date": "2026-03-10", "currency": usd,
        "invoice_lines": {"create": [{"name": "Consulting", "price_unit": "125", "taxes": [s21]}]}}),
    )?;
    post(&mut env, invoice)?;
    assert_eq!(
        items(&mut env, invoice)?,
        vec![
            ("400000".to_string(), d("121"), d("0")),
            ("451000".to_string(), d("0"), d("21")),
            ("700000".to_string(), d("0"), d("100")),
        ]
    );
    let row = read(
        &mut env,
        "account_move",
        invoice,
        &["amount_total", "amount_residual", "amount_total_signed"],
    )?;
    assert_eq!(decimal(&row["amount_total"]), d("151.25"), "in dollars");
    assert_eq!(decimal(&row["amount_residual"]), d("151.25"));
    assert_eq!(decimal(&row["amount_total_signed"]), d("121"), "in euros");
    check_books(&mut env)
}

/// A fiscal position changes the taxes the product brings: intra-community, no VAT.
#[test]
fn test_a_fiscal_position_maps_the_taxes() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let position = chart(&mut env, "position_intra_eu");
    let customer = create(
        &mut env,
        "contact",
        json!({"name": "EU customer", "is_company": true, "fiscal_position": position}),
    )?;
    let desk = product(&mut env, "Desk", "100", "60")?;
    let invoice = invoice(&mut env, "out_invoice", customer, vec![line(desk, "1")])?;
    let row = read(
        &mut env,
        "account_move",
        invoice,
        &["fiscal_position", "amount_tax", "amount_total"],
    )?;
    assert_eq!(row["fiscal_position"], json!(position), "from the customer");
    assert_eq!(decimal(&row["amount_tax"]), d("0"));
    assert_eq!(decimal(&row["amount_total"]), d("100"));
    post(&mut env, invoice)?;
    check_books(&mut env)
}

/// Posted, an invoice cannot change nor be deleted; back to draft its items go, and it posts
/// again under the same number; a draft can be cancelled, and is no longer due.
#[test]
fn test_what_a_posted_invoice_allows() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let customer = partner(&mut env, "Customer")?;
    let desk = product(&mut env, "Desk", "100", "60")?;
    let invoice = invoice(&mut env, "out_invoice", customer, vec![line(desk, "1")])?;
    post(&mut env, invoice)?;
    let error = env
        .call_rpc(
            "account_move",
            "write",
            &json!({"ids": [invoice], "values": {"partner": customer}}),
        )
        .expect_err("posted")
        .to_string();
    assert!(error.contains("posted"), "{error}");
    let lines =
        read(&mut env, "account_move", invoice, &["invoice_lines"])?["invoice_lines"].clone();
    assert!(
        env.call_rpc(
            "account_invoice_line",
            "write",
            &json!({"ids": lines, "values": {"quantity": "5"}})
        )
        .is_err()
    );
    let error = env
        .call_rpc("account_move", "delete", &json!({"ids": [invoice]}))
        .expect_err("numbered")
        .to_string();
    assert!(error.contains("cancel"), "{error}");
    assert!(
        call(&mut env, "account_move", "button_cancel", &[invoice]).is_err(),
        "only a draft is cancelled"
    );
    assert!(post(&mut env, invoice).is_err(), "posted once");

    call(&mut env, "account_move", "button_draft", &[invoice])?;
    assert!(
        items(&mut env, invoice)?.is_empty(),
        "its items went with it"
    );
    env.call_rpc(
        "account_invoice_line",
        "write",
        &json!({"ids": lines, "values": {"quantity": "2"}}),
    )?;
    post(&mut env, invoice)?;
    let row = read(&mut env, "account_move", invoice, &["name", "amount_total"])?;
    assert_eq!(row["name"], json!("INV/2026/00001"), "the same number");
    assert_eq!(decimal(&row["amount_total"]), d("242"));

    call(&mut env, "account_move", "button_draft", &[invoice])?;
    call(&mut env, "account_move", "button_cancel", &[invoice])?;
    let entry = entry(&mut env, invoice);
    assert!(matches!(*entry.get_state(&mut env)?, MoveState::Cancel));
    assert!(matches!(
        *entry.get_payment_state(&mut env)?,
        PaymentState::NotPaid
    ));
    let draft = common::invoice(&mut env, "out_invoice", customer, vec![line(desk, "1")])?;
    assert_eq!(
        env.delete("account_move", &SingleId::from(draft))?,
        1,
        "never numbered: deleted"
    );
    check_books(&mut env)
}

/// Nothing posts without a customer, without lines, or with a negative total.
#[test]
fn test_what_cannot_be_posted() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let customer = partner(&mut env, "Customer")?;
    let lonely = create(
        &mut env,
        "account_move",
        json!({"move_type": "out_invoice",
        "invoice_lines": {"create": [{"name": "Thing", "price_unit": "10"}]}}),
    )?;
    let error = post(&mut env, lonely).expect_err("no customer").to_string();
    assert!(error.contains("customer"), "{error}");
    let empty = create(
        &mut env,
        "account_move",
        json!({"move_type": "out_invoice", "partner": customer}),
    )?;
    let error = post(&mut env, empty).expect_err("no line").to_string();
    assert!(error.contains("no line"), "{error}");
    let negative = invoice(
        &mut env,
        "out_invoice",
        customer,
        vec![json!({"name": "Refund", "price_unit": "-10"})],
    )?;
    let error = post(&mut env, negative).expect_err("negative").to_string();
    assert!(error.contains("credit note"), "{error}");
    let row = read(&mut env, "account_move", negative, &["name", "state"])?;
    assert_eq!(
        (row["name"].clone(), row["state"].clone()),
        (json!("/"), json!("draft")),
        "nothing half done"
    );
    Ok(())
}

/// Locked books refuse what is dated within them.
#[test]
fn test_the_lock_date() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let company = account::models::CompanyAccount::current(&mut env)?;
    env.call_rpc(
        "company",
        "write",
        &json!({"ids": [company.get_id()], "values": {"lock_date": "2026-03-31"}}),
    )?;
    let customer = partner(&mut env, "Customer")?;
    let desk = product(&mut env, "Desk", "100", "60")?;
    let invoice = invoice(&mut env, "out_invoice", customer, vec![line(desk, "1")])?;
    let error = post(&mut env, invoice).expect_err("locked").to_string();
    assert!(error.contains("locked"), "{error}");
    env.call_rpc(
        "account_move",
        "write",
        &json!({"ids": [invoice], "values": {"invoice_date": "2026-04-01"}}),
    )?;
    post(&mut env, invoice)?;
    check_books(&mut env)
}

/// An entry made by hand posts when it balances, and is reversed by its mirror.
#[test]
fn test_manual_entries() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let (bank, capital) = (chart(&mut env, "a_bank"), chart(&mut env, "a_capital"));
    let unbalanced = create(
        &mut env,
        "account_move",
        json!({"date": "2026-01-02", "lines": {"create": [
        {"account": bank, "debit": "1000"}, {"account": capital, "credit": "900"}]}}),
    )?;
    let error = post(&mut env, unbalanced)
        .expect_err("unbalanced")
        .to_string();
    assert!(error.contains("does not balance"), "{error}");
    let entry = create(
        &mut env,
        "account_move",
        json!({"date": "2026-01-02", "lines": {"create": [
        {"account": bank, "debit": "1000", "name": "Capital paid in"}, {"account": capital, "credit": "1000"}]}}),
    )?;
    post(&mut env, entry)?;
    let row = read(
        &mut env,
        "account_move",
        entry,
        &["name", "amount_total", "journal"],
    )?;
    assert_eq!(row["name"], json!("MISC/2026/00001"));
    assert_eq!(decimal(&row["amount_total"]), d("1000"));
    assert!(
        create(
            &mut env,
            "account_move_line",
            json!({"move_id": entry, "account": bank, "debit": "5", "credit": "5"})
        )
        .is_err(),
        "a debit or a credit, not both"
    );
    let answer = call(&mut env, "account_move", "action_reverse", &[entry])?;
    let reversal = answer["id"].as_u64().expect("a reversal") as u32;
    assert_eq!(
        read(&mut env, "account_move", reversal, &["state"])?["state"],
        json!("posted")
    );
    assert_eq!(
        items(&mut env, reversal)?,
        vec![
            ("100000".to_string(), d("1000"), d("0")),
            ("550000".to_string(), d("0"), d("1000"))
        ]
    );
    check_books(&mut env)
}

/// Invoicing users make and post invoices; other employees see none; the chart is the
/// accountant's.
#[test]
fn test_access_rights() -> Result<()> {
    let app = new_app()?;
    let (customer, desk) = {
        let mut env = admin_env(&app)?;
        let ids = (
            partner(&mut env, "Customer")?,
            product(&mut env, "Desk", "100", "60")?,
        );
        env.close()?;
        ids
    };
    {
        let mut env = user_env(&app, "employee", &["base.group_user"])?;
        assert!(invoice(&mut env, "out_invoice", customer, vec![line(desk, "1")]).is_err());
        assert!(
            create(
                &mut env,
                "account",
                json!({"code": "999999", "name": "Mine"})
            )
            .is_err()
        );
    }
    let mut env = user_env(
        &app,
        "billing",
        &["base.group_user", "account.group_account_invoice"],
    )?;
    let invoice = invoice(&mut env, "out_invoice", customer, vec![line(desk, "1")])?;
    post(&mut env, invoice)?;
    assert!(
        create(
            &mut env,
            "account",
            json!({"code": "999999", "name": "Mine"})
        )
        .is_err()
    );
    check_books(&mut env)
}
