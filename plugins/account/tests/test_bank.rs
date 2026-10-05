//! Bank statements: transactions matched with what they pay — invoices by their reference,
//! payments waiting for the bank — validated once balanced; and the trial balance.

mod common;

use common::*;
use erp::Result;
use serde_json::json;

/// A transaction naming an invoice pays it straight from the bank; one matching a payment
/// clears the money waiting for the bank. The statement validates once balanced.
#[test]
fn test_a_statement_matches_and_validates() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let customer = partner(&mut env, "Customer")?;
    let s21 = chart(&mut env, "tax_sale_21");
    let first = invoice(
        &mut env,
        "out_invoice",
        customer,
        vec![json!({"name": "A", "price_unit": "100", "taxes": [s21]})],
    )?;
    post(&mut env, first)?;
    let second = invoice(
        &mut env,
        "out_invoice",
        customer,
        vec![json!({"name": "B", "price_unit": "50", "taxes": [s21]})],
    )?;
    post(&mut env, second)?;
    pay(&mut env, &[second])?;

    let bank = chart(&mut env, "journal_bank");
    let statement = create(
        &mut env,
        "account_bank_statement",
        json!({"name": "2026-03", "journal": bank,
        "date": "2026-03-31", "balance_start": "1000", "balance_end_real": "1181.5", "lines": {"create": [
            {"date": "2026-03-20", "payment_ref": "INV/2026/00001", "amount": "121"},
            {"date": "2026-03-21", "payment_ref": "Transfer", "partner": customer, "amount": "60.5"}
        ]}}),
    )?;
    let row = read(
        &mut env,
        "account_bank_statement",
        statement,
        &["balance_end"],
    )?;
    assert_eq!(decimal(&row["balance_end"]), d("1181.5"));
    call(
        &mut env,
        "account_bank_statement",
        "action_validate",
        &[statement],
    )?;
    assert_eq!(
        read(&mut env, "account_bank_statement", statement, &["state"])?["state"],
        json!("confirm")
    );
    assert_eq!(
        read(&mut env, "account_move", first, &["payment_state"])?["payment_state"],
        json!("paid")
    );
    let outstanding = chart(&mut env, "a_outstanding_receipts");
    let open = env.call_rpc(
        "account_move_line",
        "search",
        &json!({"domain": [
        ["account", "=", outstanding], ["reconciled", "=", false]]}),
    )?;
    assert_eq!(open, json!([]), "the payment no longer waits for the bank");
    check_books(&mut env)
}

/// A transaction matching nothing says so; given the items it settles, it matches them; its
/// matching can be undone; a statement whose balances disagree is not validated.
#[test]
fn test_matching_by_hand_and_undoing() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let customer = partner(&mut env, "Customer")?;
    let s21 = chart(&mut env, "tax_sale_21");
    let invoice = invoice(
        &mut env,
        "out_invoice",
        customer,
        vec![json!({"name": "A", "price_unit": "100", "taxes": [s21]})],
    )?;
    post(&mut env, invoice)?;
    let bank = chart(&mut env, "journal_bank");
    let statement = create(
        &mut env,
        "account_bank_statement",
        json!({"name": "2026-04", "journal": bank,
        "date": "2026-04-30", "balance_start": "0", "balance_end_real": "999", "lines": {"create": [
            {"date": "2026-04-02", "payment_ref": "Unknown", "amount": "121"}]}}),
    )?;
    let line = read(&mut env, "account_bank_statement", statement, &["lines"])?["lines"][0]
        .as_u64()
        .expect("a line") as u32;
    let error = call(
        &mut env,
        "account_bank_statement_line",
        "action_reconcile",
        &[line],
    )
    .expect_err("no match")
    .to_string();
    assert!(error.contains("Unknown"), "{error}");
    let receivable = chart(&mut env, "a_receivable");
    let open = env.call_rpc(
        "account_move_line",
        "search",
        &json!({"domain": [["account", "=", receivable]]}),
    )?;
    env.call_rpc(
        "account_bank_statement_line",
        "write",
        &json!({"ids": [line], "values": {"to_match": open}}),
    )?;
    call(
        &mut env,
        "account_bank_statement_line",
        "action_reconcile",
        &[line],
    )?;
    assert_eq!(
        read(&mut env, "account_move", invoice, &["payment_state"])?["payment_state"],
        json!("paid")
    );
    let error = call(
        &mut env,
        "account_bank_statement",
        "action_validate",
        &[statement],
    )
    .expect_err("unbalanced")
    .to_string();
    assert!(error.contains("999"), "{error}");
    call(
        &mut env,
        "account_bank_statement_line",
        "action_undo_reconciliation",
        &[line],
    )?;
    assert_eq!(
        read(&mut env, "account_move", invoice, &["payment_state"])?["payment_state"],
        json!("not_paid")
    );
    check_books(&mut env)
}

/// The trial balance of a period: what each account held before, what moved, what it holds;
/// debits and credits agree.
#[test]
fn test_the_trial_balance() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let (bank, capital) = (chart(&mut env, "a_bank"), chart(&mut env, "a_capital"));
    let opening = create(
        &mut env,
        "account_move",
        json!({"date": "2026-01-02", "lines": {"create": [
        {"account": bank, "debit": "1000"}, {"account": capital, "credit": "1000"}]}}),
    )?;
    post(&mut env, opening)?;
    let customer = partner(&mut env, "Customer")?;
    let s21 = chart(&mut env, "tax_sale_21");
    let invoice = invoice(
        &mut env,
        "out_invoice",
        customer,
        vec![json!({"name": "A", "price_unit": "100", "taxes": [s21]})],
    )?;
    post(&mut env, invoice)?;
    let report = create(
        &mut env,
        "account_trial_balance",
        json!({"date_from": "2026-03-01", "date_to": "2026-03-31"}),
    )?;
    call(
        &mut env,
        "account_trial_balance",
        "action_compute",
        &[report],
    )?;
    let row = read(
        &mut env,
        "account_trial_balance",
        report,
        &["total_debit", "total_credit", "lines"],
    )?;
    assert_eq!(decimal(&row["total_debit"]), d("121"));
    assert_eq!(decimal(&row["total_credit"]), d("121"));
    let mut lines = Vec::new();
    for id in row["lines"].as_array().expect("lines") {
        let id = id.as_u64().expect("a line") as u32;
        let line = read(
            &mut env,
            "account_trial_balance_line",
            id,
            &[
                "account",
                "initial_balance",
                "debit",
                "credit",
                "ending_balance",
            ],
        )?;
        let account = line["account"].as_u64().expect("an account") as u32;
        let code = read(&mut env, "account", account, &["code"])?["code"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        lines.push((
            code,
            decimal(&line["initial_balance"]),
            decimal(&line["debit"]),
            decimal(&line["credit"]),
            decimal(&line["ending_balance"]),
        ));
    }
    assert_eq!(
        lines,
        vec![
            ("100000".to_string(), d("-1000"), d("0"), d("0"), d("-1000")),
            ("400000".to_string(), d("0"), d("121"), d("0"), d("121")),
            ("451000".to_string(), d("0"), d("0"), d("21"), d("-21")),
            ("550000".to_string(), d("1000"), d("0"), d("0"), d("1000")),
            ("700000".to_string(), d("0"), d("0"), d("100"), d("-100")),
        ]
    );
    let ending: erp::types::field::Decimal = lines.iter().map(|line| line.4).sum();
    assert_eq!(ending, d("0"), "the balance balances");
    check_books(&mut env)
}
