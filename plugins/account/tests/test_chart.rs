//! The chart: accounts, taxes, journals, terms and fiscal positions, their rules and views.

mod common;

use account::models::{FiscalPosition, Journal, PaymentTerm, Tax, TaxDocument};
use common::*;
use erp::Result;
use erp::types::field::{NaiveDate, SingleId};
use serde_json::json;

fn date(text: &str) -> NaiveDate {
    NaiveDate::parse_from_str(text, "%Y-%m-%d").expect("a date")
}

/// The test chart installs, with its accounts named `code name`.
#[test]
fn test_the_chart_installs() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let sales = chart(&mut env, "a_sales");
    let row = read(
        &mut env,
        "account",
        sales,
        &["display_name", "account_type", "reconcile"],
    )?;
    assert_eq!(row["display_name"], json!("700000 Sales"));
    assert_eq!(row["account_type"], json!("income"));
    assert_eq!(row["reconcile"], json!(false));
    let receivable = chart(&mut env, "a_receivable");
    assert_eq!(
        read(&mut env, "account", receivable, &["reconcile"])?["reconcile"],
        json!(true)
    );
    Ok(())
}

/// A code is unique; a receivable account is always reconciled.
#[test]
fn test_account_rules() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let error = create(
        &mut env,
        "account",
        json!({"code": "700000", "name": "Again"}),
    )
    .expect_err("taken")
    .to_string();
    assert!(error.contains("700000"), "{error}");
    let other = create(
        &mut env,
        "account",
        json!({"code": "400100", "name": "Other customers", "account_type": "asset_receivable"}),
    )?;
    assert_eq!(
        read(&mut env, "account", other, &["reconcile"])?["reconcile"],
        json!(true)
    );
    let error = env
        .call_rpc(
            "account",
            "write",
            &json!({"ids": [other], "values": {"reconcile": false}}),
        )
        .expect_err("receivables are reconciled")
        .to_string();
    assert!(error.contains("reconciliation"), "{error}");
    Ok(())
}

/// A tax's distribution sends 100 % of it somewhere; a reverse charge sends it to two sides.
#[test]
fn test_tax_distributions() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let reverse = chart(&mut env, "tax_purchase_21_reverse");
    let tax: Tax<SingleId> = env.get_record(reverse.into());
    let spec = tax.spec(&mut env, TaxDocument::Invoice)?;
    let factors: Vec<String> = spec
        .repartitions
        .iter()
        .map(|r| r.factor.to_string())
        .collect();
    assert_eq!(factors, vec!["100", "-100"]);
    let vat_due = chart(&mut env, "a_vat_due");
    assert_eq!(spec.repartitions[1].account, Some(vat_due));

    let zero = chart(&mut env, "tax_sale_0");
    let tax: Tax<SingleId> = env.get_record(zero.into());
    let spec = tax.spec(&mut env, TaxDocument::Refund)?;
    assert_eq!(
        spec.repartitions.len(),
        1,
        "no distribution: all of it on the line's account"
    );
    assert_eq!(spec.repartitions[0].account, None);

    let error = create(
        &mut env,
        "account_tax",
        json!({"name": "Too much", "amount": "150"}),
    )
    .expect_err("over 100 %")
    .to_string();
    assert!(error.contains("100"), "{error}");
    let error = create(
        &mut env,
        "account_tax",
        json!({"name": "Half", "amount": "21", "repartitions": {"create": [
            {"document": "invoice", "factor": "50"}
        ]}}),
    )
    .expect_err("only half of it goes anywhere")
    .to_string();
    assert!(error.contains("add up to 100"), "{error}");
    Ok(())
}

/// A journal numbers its entries, and its credit notes apart.
#[test]
fn test_journals_number_their_entries() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let sale = chart(&mut env, "journal_sale");
    let journal: Journal<SingleId> = env.get_record(sale.into());
    let invoices = journal.numbering(&mut env, false)?;
    let refunds = journal.numbering(&mut env, true)?;
    assert_eq!(
        invoices.next(&mut env, date("2026-03-10"))?,
        "INV/2026/00001"
    );
    assert_eq!(
        refunds.next(&mut env, date("2026-03-10"))?,
        "RINV/2026/00001"
    );
    let error = create(
        &mut env,
        "account_journal",
        json!({"name": "Again", "code": "INV"}),
    )
    .expect_err("taken")
    .to_string();
    assert!(error.contains("INV"), "{error}");
    assert!(
        create(
            &mut env,
            "account_journal",
            json!({"name": "Too long", "code": "TOOLONGCODE"})
        )
        .is_err()
    );
    Ok(())
}

/// Terms split what is due by their installments; a term must end with its balance.
#[test]
fn test_payment_terms() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let advance = chart(&mut env, "term_advance");
    let term: PaymentTerm<SingleId> = env.get_record(advance.into());
    let due = term.compute(&mut env, d("1210"), date("2026-03-10"), d("0.01"))?;
    assert_eq!(
        due,
        vec![
            (date("2026-03-10"), d("363")),
            (date("2026-05-09"), d("847"))
        ]
    );
    let eom = chart(&mut env, "term_30_days_end_of_month");
    let term: PaymentTerm<SingleId> = env.get_record(eom.into());
    assert_eq!(
        term.compute(&mut env, d("100"), date("2026-01-15"), d("0.01"))?,
        vec![(date("2026-02-28"), d("100"))]
    );
    let error = create(
        &mut env,
        "account_payment_term",
        json!({"name": "Broken", "lines": {"create": [{"value": "percent", "value_amount": "50"}]}}),
    )
    .expect_err("no balance")
    .to_string();
    assert!(error.contains("balance"), "{error}");
    Ok(())
}

/// A fiscal position replaces the taxes it maps and keeps the others.
#[test]
fn test_fiscal_positions_map_taxes() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let position = chart(&mut env, "position_intra_eu");
    let position: FiscalPosition<SingleId> = env.get_record(position.into());
    let (s21, s0, s6) = (
        chart(&mut env, "tax_sale_21"),
        chart(&mut env, "tax_sale_0"),
        chart(&mut env, "tax_sale_6"),
    );
    assert_eq!(position.map_taxes(&mut env, &[s21, s6])?, vec![s0, s6]);
    let none: FiscalPosition<SingleId> = env.get_record(SingleId::empty());
    assert_eq!(none.map_taxes(&mut env, &[s21])?, vec![s21]);
    Ok(())
}

/// A bank account is kept as an IBAN, checked.
#[test]
fn test_bank_accounts_are_ibans() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let customer = partner(&mut env, "Customer")?;
    let bank = create(
        &mut env,
        "contact_bank",
        json!({"contact": customer, "iban": "be68 5390 0754 7034"}),
    )?;
    assert_eq!(
        read(&mut env, "contact_bank", bank, &["iban"])?["iban"],
        json!("BE68539007547034")
    );
    assert!(
        create(
            &mut env,
            "contact_bank",
            json!({"contact": customer, "iban": "BE69 5390 0754 7034"})
        )
        .is_err()
    );
    Ok(())
}

/// Every view of the plugin resolves, extensions of other plugins' forms included, and the
/// application has its menus.
#[test]
fn test_views_and_menus() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    for (model, kinds) in [
        ("account", &["list", "form", "search"][..]),
        ("account_tax", &["list", "form", "search"]),
        ("account_journal", &["list", "form"]),
        ("account_payment_term", &["list", "form"]),
        ("account_fiscal_position", &["list", "form"]),
        ("account_move", &["list", "form", "search"]),
        ("account_invoice_line", &["list", "form"]),
        ("account_move_line", &["list", "search"]),
        ("account_payment", &["list", "form", "search"]),
        ("account_bank_statement", &["list", "form"]),
        ("account_bank_statement_line", &["list", "form"]),
        ("account_trial_balance", &["list", "form"]),
        ("contact", &["form"]),
        ("product", &["form"]),
        ("company", &["form"]),
    ] {
        for kind in kinds {
            let arch = env.get_empty_record::<base::models::View<_>>().load(
                &mut env,
                model.to_string(),
                kind.to_string(),
            )?;
            assert!(
                arch.starts_with(&format!("<{kind}")),
                "{model} {kind}: {arch}"
            );
        }
    }
    let contact = env.get_empty_record::<base::models::View<_>>().load(
        &mut env,
        "contact".to_string(),
        "form".to_string(),
    )?;
    assert!(contact.contains("customer_payment_term"), "{contact}");
    let tree = env.call_rpc("menu", "tree", &json!({}))?;
    let invoicing = tree
        .as_array()
        .expect("menus")
        .iter()
        .find(|entry| entry["name"] == "Invoicing")
        .expect("the application");
    let sections: Vec<&str> = invoicing["children"]
        .as_array()
        .expect("sections")
        .iter()
        .filter_map(|entry| entry["name"].as_str())
        .collect();
    assert_eq!(
        sections,
        vec![
            "Customers",
            "Vendors",
            "Accounting",
            "Reporting",
            "Configuration"
        ]
    );
    Ok(())
}

/// A product made without taxes gets the company's default ones; given taxes, it keeps them.
#[test]
fn test_products_get_the_default_taxes() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let (sale, purchase, six) = (
        chart(&mut env, "tax_sale_21"),
        chart(&mut env, "tax_purchase_21"),
        chart(&mut env, "tax_sale_6"),
    );
    let desk = create(&mut env, "product", json!({"name": "Desk"}))?;
    let row = read(&mut env, "product", desk, &["taxes", "supplier_taxes"])?;
    assert_eq!(row["taxes"], json!([sale]));
    assert_eq!(row["supplier_taxes"], json!([purchase]));
    let book = create(
        &mut env,
        "product",
        json!({"name": "Book", "taxes": [six], "supplier_taxes": []}),
    )?;
    let row = read(&mut env, "product", book, &["taxes", "supplier_taxes"])?;
    assert_eq!(row["taxes"], json!([six]));
    assert_eq!(row["supplier_taxes"], json!([]));
    Ok(())
}
