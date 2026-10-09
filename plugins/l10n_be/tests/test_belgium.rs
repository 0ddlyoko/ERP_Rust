//! Belgian accounting: the PCMN chart installed as the company's, VAT at Belgian rates reported
//! in the grids of the return, structured communications matched by the bank, VAT numbers.

use account::AccountPlugin;
use base::BasePlugin;
use contacts::ContactsPlugin;
use currency::CurrencyPlugin;
use erp::Result;
use erp::app::Application;
use erp::environment::Environment;
use erp::types::field::{Decimal, NaiveDate};
use erp_test_support::{admin_env, d, xml_id};
use l10n_be::L10nBePlugin;
use l10n_be::models::grid_amounts;
use mail::MailPlugin;
use product::ProductPlugin;
use sequence::SequencePlugin;
use serde_json::{Value, json};
use uom::UomPlugin;
use web::WebPlugin;

fn new_app() -> Result<Application> {
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
                Box::new(L10nBePlugin {}),
            ]
        },
        &["l10n_be"],
    )
}

fn be(env: &mut Environment, name: &str) -> u32 {
    xml_id(env, &format!("l10n_be.{name}"))
}

fn create(env: &mut Environment, model: &str, values: Value) -> Result<u32> {
    let ids = env.call_rpc(model, "create", &json!({ "values": values }))?;
    Ok(ids[0].as_u64().ok_or("an id")? as u32)
}

fn read(env: &mut Environment, model: &str, id: u32, field: &str) -> Result<Value> {
    Ok(env.call_rpc(model, "read", &json!({"ids": [id], "fields": [field]}))?[0][field].clone())
}

fn date(text: &str) -> NaiveDate {
    NaiveDate::parse_from_str(text, "%Y-%m-%d").expect("a date")
}

/// A posted document of `move_type` dated `on`, one line per `(price, tax)`.
fn document(
    env: &mut Environment,
    move_type: &str,
    partner: u32,
    on: &str,
    lines: &[(&str, &str)],
) -> Result<u32> {
    let lines: Vec<Value> = lines
        .iter()
        .map(|(price, tax)| {
            let tax = be(env, tax);
            json!({"name": "Line", "price_unit": price, "taxes": [tax]})
        })
        .collect();
    let id = create(
        env,
        "account_move",
        json!({"move_type": move_type, "partner": partner,
        "invoice_date": on, "invoice_lines": {"create": lines}}),
    )?;
    env.call_rpc("account_move", "action_post", &json!({"ids": [id]}))?;
    Ok(id)
}

/// Installed, the chart is the company's: its customers, suppliers, sales and VAT accounts.
#[test]
fn test_the_chart_is_the_companys() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let company = account::models::CompanyAccount::current(&mut env)?;
    let receivable: account::models::Account<erp::types::field::SingleId> =
        company.get_account_receivable(&mut env)?;
    assert_eq!(receivable.get_code(&mut env)?, "400000");
    let tax: account::models::Tax<erp::types::field::SingleId> = company.get_sale_tax(&mut env)?;
    assert_eq!(*tax.get_amount(&mut env)?, d("21"));
    let count = env.count("account", &erp_search::SearchType::Nothing)?;
    assert!(count >= 70, "the PCMN has its accounts: {count}");
    Ok(())
}

/// Every kind of operation lands in its grid; the return settles to 71, what is due less what
/// is deductible.
#[test]
fn test_the_vat_return() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let customer = create(
        &mut env,
        "contact",
        json!({"name": "Client", "is_company": true}),
    )?;
    let vendor = create(
        &mut env,
        "contact",
        json!({"name": "Fournisseur", "is_company": true}),
    )?;
    let on = "2026-02-10";
    document(
        &mut env,
        "out_invoice",
        customer,
        on,
        &[
            ("1000", "tax_sale_21"),
            ("100", "tax_sale_6"),
            ("200", "tax_sale_12"),
        ],
    )?;
    document(
        &mut env,
        "out_invoice",
        customer,
        on,
        &[
            ("500", "tax_sale_intracom_goods"),
            ("300", "tax_sale_export"),
            ("250", "tax_sale_cocontractant"),
            ("150", "tax_sale_intracom_services"),
        ],
    )?;
    document(
        &mut env,
        "out_refund",
        customer,
        on,
        &[("100", "tax_sale_21")],
    )?;
    document(
        &mut env,
        "in_invoice",
        vendor,
        on,
        &[
            ("200", "tax_purchase_21_goods"),
            ("100", "tax_purchase_21_services"),
            ("1000", "tax_purchase_21_investments"),
        ],
    )?;
    document(
        &mut env,
        "in_invoice",
        vendor,
        on,
        &[("1000", "tax_purchase_21_cocontractant")],
    )?;
    document(
        &mut env,
        "in_invoice",
        vendor,
        on,
        &[
            ("400", "tax_purchase_21_intracom_goods"),
            ("300", "tax_purchase_21_intracom_services"),
        ],
    )?;
    document(
        &mut env,
        "in_refund",
        vendor,
        on,
        &[("50", "tax_purchase_21_goods")],
    )?;
    document(
        &mut env,
        "in_refund",
        vendor,
        on,
        &[("100", "tax_purchase_21_cocontractant")],
    )?;
    document(
        &mut env,
        "out_invoice",
        customer,
        "2026-03-05",
        &[("999", "tax_sale_21")],
    )?;

    let grids = grid_amounts(&mut env, date("2026-02-01"), date("2026-02-28"))?;
    let expected: [(&str, &str); 25] = [
        ("00", "0"),
        ("01", "100"),
        ("02", "200"),
        ("03", "1000"),
        ("44", "150"),
        ("45", "250"),
        ("46", "500"),
        ("47", "300"),
        ("48", "0"),
        ("49", "100"),
        ("81", "200"),
        ("82", "100"),
        ("83", "1000"),
        ("84", "0"),
        ("85", "150"),
        ("86", "400"),
        ("87", "1000"),
        ("88", "300"),
        // 210 + 6 + 24 on sales; 84 + 63 on 86 and 88; 210 on 87.
        ("54", "240"),
        ("55", "147"),
        ("56", "210"),
        // 42 + 21 + 210 + 210 + 84 + 63 deducted; 10.50 + 21 to give back; 21 to recover.
        ("59", "630"),
        ("63", "31.5"),
        ("64", "42"),
        ("71", "0"),
    ];
    for (grid, amount) in expected {
        assert_eq!(
            grids.get(grid).copied().unwrap_or_default(),
            d(amount),
            "grid {grid}: {grids:?}"
        );
    }
    // Due 240 + 147 + 210 + 31.50 = 628.50; deductible 630 + 42 = 672: 43.50 due by the State.
    assert_eq!(grids["72"], d("43.5"));
    l10n_be::invariants::check_return(&mut env, date("2026-02-01"), date("2026-02-28"))?;
    l10n_be::invariants::check_books(&mut env)
}

/// The return of a month, computed, shows its grids and the Intervat file.
#[test]
fn test_filing_a_return() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let company = base::models::Company::current(&mut env)?;
    let contact: base::models::Contact<erp::types::field::SingleId> =
        company.get_contact(&mut env)?;
    env.call_rpc(
        "contact",
        "write",
        &json!({"ids": [contact.get_id()], "values": {"vat": "BE 0477.472.701"}}),
    )?;
    let customer = create(
        &mut env,
        "contact",
        json!({"name": "Client", "is_company": true}),
    )?;
    document(
        &mut env,
        "out_invoice",
        customer,
        "2026-04-15",
        &[("1000", "tax_sale_21")],
    )?;
    let vat_return = create(
        &mut env,
        "l10n_be_vat_return",
        json!({"name": "2026-04", "date_from": "2026-04-01", "date_to": "2026-04-30"}),
    )?;
    env.call_rpc(
        "l10n_be_vat_return",
        "action_compute",
        &json!({"ids": [vat_return]}),
    )?;
    let due = read(&mut env, "l10n_be_vat_return", vat_return, "amount_due")?;
    assert_eq!(due, json!("210"));
    let xml = read(&mut env, "l10n_be_vat_return", vat_return, "xml")?;
    let xml = xml.as_str().expect("the file").to_string();
    let document = roxmltree::Document::parse(&xml).expect("well-formed XML");
    let amounts: Vec<(String, String)> = document
        .descendants()
        .filter(|node| node.has_tag_name("Amount"))
        .map(|node| {
            (
                node.attribute("GridNumber").unwrap_or_default().to_string(),
                node.text().unwrap_or_default().to_string(),
            )
        })
        .collect();
    assert_eq!(
        amounts,
        vec![
            ("3".to_string(), "1000.00".to_string()),
            ("54".to_string(), "210.00".to_string()),
            ("71".to_string(), "210.00".to_string())
        ]
    );
    let month = document
        .descendants()
        .find(|node| node.has_tag_name("Month"))
        .and_then(|node| node.text());
    assert_eq!(month, Some("4"));
    let vat = document
        .descendants()
        .find(|node| node.has_tag_name("VATNumber"))
        .and_then(|node| node.text());
    assert_eq!(vat, Some("0477472701"));
    let lines = read(&mut env, "l10n_be_vat_return", vat_return, "lines")?;
    assert_eq!(
        lines.as_array().map(Vec::len),
        Some(29),
        "every grid of the form"
    );
    Ok(())
}

/// A customer invoice is paid with a structured communication, which the bank recognizes.
#[test]
fn test_structured_communications() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let customer = create(
        &mut env,
        "contact",
        json!({"name": "Client", "is_company": true}),
    )?;
    let invoice = document(
        &mut env,
        "out_invoice",
        customer,
        "2026-02-10",
        &[("100", "tax_sale_21")],
    )?;
    let reference = read(&mut env, "account_move", invoice, "payment_reference")?;
    let reference = reference.as_str().expect("a reference").to_string();
    assert_eq!(
        reference,
        l10n_be::structured::structured_communication(u64::from(invoice))
    );
    let bill = document(
        &mut env,
        "in_invoice",
        customer,
        "2026-02-10",
        &[("100", "tax_purchase_21_goods")],
    )?;
    assert_eq!(
        read(&mut env, "account_move", bill, "payment_reference")?,
        json!("BILL/2026/00001")
    );

    let bank = be(&mut env, "journal_bank");
    let digits: String = reference.chars().filter(|c| c.is_ascii_digit()).collect();
    let statement = create(
        &mut env,
        "account_bank_statement",
        json!({"name": "02", "journal": bank, "date": "2026-02-28",
        "balance_start": "0", "balance_end_real": "121", "lines": {"create": [
            {"date": "2026-02-20", "payment_ref": digits, "amount": "121"}]}}),
    )?;
    env.call_rpc(
        "account_bank_statement",
        "action_validate",
        &json!({"ids": [statement]}),
    )?;
    assert_eq!(
        read(&mut env, "account_move", invoice, "payment_state")?,
        json!("paid")
    );
    l10n_be::invariants::check_books(&mut env)
}

/// A Belgian VAT number is checked and written the one way; a foreign one is kept as given.
#[test]
fn test_vat_numbers() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let belgian = create(
        &mut env,
        "contact",
        json!({"name": "SA", "vat": "be 0477.472.701"}),
    )?;
    assert_eq!(
        read(&mut env, "contact", belgian, "vat")?,
        json!("BE0477472701")
    );
    let error = create(
        &mut env,
        "contact",
        json!({"name": "Typo", "vat": "BE0477472702"}),
    )
    .expect_err("wrong")
    .to_string();
    assert!(error.contains("check digits"), "{error}");
    let french = create(
        &mut env,
        "contact",
        json!({"name": "SARL", "vat": "FR40303265045"}),
    )?;
    assert_eq!(
        read(&mut env, "contact", french, "vat")?,
        json!("FR40303265045")
    );
    Ok(())
}

/// An EU business customer is invoiced without VAT, in grid 46, with the legal mention.
#[test]
fn test_intra_community_supplies() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let position = be(&mut env, "position_intra_eu");
    let customer = create(
        &mut env,
        "contact",
        json!({"name": "GmbH", "is_company": true, "fiscal_position": position}),
    )?;
    let desk = create(
        &mut env,
        "product",
        json!({"name": "Desk", "list_price": "400"}),
    )?;
    let invoice = create(
        &mut env,
        "account_move",
        json!({"move_type": "out_invoice", "partner": customer,
        "invoice_date": "2026-05-04", "invoice_lines": {"create": [{"product": desk, "quantity": "2"}]}}),
    )?;
    assert_eq!(
        read(&mut env, "account_move", invoice, "amount_total")?,
        json!("800")
    );
    env.call_rpc("account_move", "action_post", &json!({"ids": [invoice]}))?;
    let grids = grid_amounts(&mut env, date("2026-05-01"), date("2026-05-31"))?;
    assert_eq!(grids["46"], d("800"));
    assert_eq!(grids["03"], Decimal::ZERO);
    l10n_be::invariants::check_books(&mut env)
}

/// Belgian VAT numbers are checked, those of the demo contacts too: turning demo data on in a
/// Belgian database loads them.
#[test]
fn test_demo_contacts_have_belgian_vat_numbers() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let settings = xml_id(&mut env, "base.settings");
    env.call_rpc(
        "settings",
        "write",
        &json!({ "ids": [settings], "values": { "demo_data": true } }),
    )?;
    let brasserie = xml_id(&mut env, "base.demo_brasserie");
    let read = env.call_rpc(
        "contact",
        "read",
        &json!({ "ids": [brasserie], "fields": ["vat"] }),
    )?;
    assert_eq!(read[0]["vat"], "BE0712345630");
    Ok(())
}
