//! Sales: quotations priced by pricelists, confirmed into orders, invoiced as ordered or as
//! delivered, paid and matched; cancelled at every step; who may do it; the views.

use account::AccountPlugin;
use account::testing::TestChartPlugin;
use base::BasePlugin;
use base::models::View;
use contacts::ContactsPlugin;
use currency::CurrencyPlugin;
use erp::Result;
use erp::app::Application;
use erp::environment::Environment;
use erp::plugin::Plugin;
use erp::types::field::Decimal;
use erp_test_support::{admin_env, d, user_env, xml_id};
use mail::MailPlugin;
use product::ProductPlugin;
use sale::SalePlugin;
use sequence::SequencePlugin;
use serde_json::{Value, json};
use uom::UomPlugin;
use web::WebPlugin;

fn plugins() -> Vec<Box<dyn Plugin>> {
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
        Box::new(SalePlugin {}),
    ]
}

fn new_app() -> Result<Application> {
    erp_test_support::app(plugins, &["sale", "account_test_chart"])
}

fn create(env: &mut Environment, model: &str, values: Value) -> Result<u32> {
    let ids = env.call_rpc(model, "create", &json!({ "values": values }))?;
    Ok(ids[0].as_u64().ok_or("an id")? as u32)
}

fn call(env: &mut Environment, model: &str, method: &str, ids: &[u32]) -> Result<Value> {
    env.call_rpc(model, method, &json!({ "ids": ids }))
}

fn read(env: &mut Environment, model: &str, id: u32, fields: &[&str]) -> Result<Value> {
    Ok(env.call_rpc(model, "read", &json!({"ids": [id], "fields": fields}))?[0].clone())
}

fn amount(value: &Value) -> Decimal {
    d(value.as_str().unwrap_or(&value.to_string()))
}

fn chart(env: &mut Environment, name: &str) -> u32 {
    xml_id(env, &format!("account_test_chart.{name}"))
}

fn customer(env: &mut Environment) -> Result<u32> {
    create(
        env,
        "contact",
        json!({"name": "Customer", "is_company": true}),
    )
}

fn desk(env: &mut Environment) -> Result<u32> {
    create(
        env,
        "product",
        json!({"name": "Desk", "default_code": "D1", "list_price": "250", "standard_price": "120"}),
    )
}

fn quotation(env: &mut Environment, partner: u32, lines: Vec<Value>) -> Result<u32> {
    create(
        env,
        "sale_order",
        json!({"partner": partner, "date_order": "2026-03-02", "lines": {"create": lines}}),
    )
}

fn line_ids(env: &mut Environment, order: u32) -> Result<Vec<u32>> {
    let lines = read(env, "sale_order", order, &["lines"])?["lines"].clone();
    Ok(lines
        .as_array()
        .expect("lines")
        .iter()
        .map(|id| id.as_u64().expect("id") as u32)
        .collect())
}

/// A quotation is numbered, dated, valid a month, its lines priced and taxed from the product.
#[test]
fn test_a_quotation() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let partner = customer(&mut env)?;
    let desk = desk(&mut env)?;
    let order = quotation(
        &mut env,
        partner,
        vec![json!({"product": desk, "product_uom_qty": "2"})],
    )?;
    let row = read(
        &mut env,
        "sale_order",
        order,
        &[
            "name",
            "state",
            "validity_date",
            "amount_untaxed",
            "amount_tax",
            "amount_total",
            "currency",
            "invoice_status",
        ],
    )?;
    assert_eq!(row["name"], json!("S00001"));
    assert_eq!(row["state"], json!("draft"));
    assert_eq!(row["validity_date"], json!("2026-04-01"));
    assert_eq!(amount(&row["amount_untaxed"]), d("500"));
    assert_eq!(amount(&row["amount_tax"]), d("105"));
    assert_eq!(amount(&row["amount_total"]), d("605"));
    assert_eq!(
        row["currency"],
        json!(xml_id(&mut env, "currency.currency_eur"))
    );
    assert_eq!(row["invoice_status"], json!("no"));
    let line = line_ids(&mut env, order)?[0];
    let row = read(
        &mut env,
        "sale_order_line",
        line,
        &["name", "price_unit", "taxes", "uom"],
    )?;
    assert_eq!(row["name"], json!("[D1] Desk"));
    assert_eq!(amount(&row["price_unit"]), d("250"));
    assert_eq!(row["taxes"], json!([chart(&mut env, "tax_sale_21")]));
    let second = quotation(&mut env, partner, vec![])?;
    assert_eq!(
        read(&mut env, "sale_order", second, &["name"])?["name"],
        json!("S00002")
    );
    Ok(())
}

/// A line sold by the dozen is priced twelve units; a pricelist in dollars prices in dollars,
/// at the rate of the order's date, to the cent.
#[test]
fn test_a_price_in_another_unit_and_currency() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let partner = customer(&mut env)?;
    let pen = create(
        &mut env,
        "product",
        json!({"name": "Pen", "list_price": "9.99"}),
    )?;
    let dozen = xml_id(&mut env, "uom.uom_dozen");
    let by_the_dozen = json!({"product": pen, "product_uom_qty": "1", "uom": dozen});
    let order = quotation(&mut env, partner, vec![by_the_dozen.clone()])?;
    let line = line_ids(&mut env, order)?[0];
    let row = read(&mut env, "sale_order_line", line, &["price_unit"])?;
    assert_eq!(amount(&row["price_unit"]), d("119.88"));

    let usd = xml_id(&mut env, "currency.currency_usd");
    create(
        &mut env,
        "currency_rate",
        json!({"currency": usd, "date": "2026-01-01", "rate": "1.0837"}),
    )?;
    let dollars = create(
        &mut env,
        "product_pricelist",
        json!({"name": "Dollars", "currency": usd}),
    )?;
    let order = create(
        &mut env,
        "sale_order",
        json!({"partner": partner, "date_order": "2026-03-02", "pricelist": dollars,
               "lines": {"create": [by_the_dozen]}}),
    )?;
    let line = line_ids(&mut env, order)?[0];
    let row = read(&mut env, "sale_order_line", line, &["price_unit"])?;
    assert_eq!(amount(&row["price_unit"]), d("129.91"));
    Ok(())
}

/// A customer's pricelist prices the lines: by category, by quantity, by product; the price
/// follows the quantity, a price typed by hand stays, a discount lowers the line.
#[test]
fn test_pricelists_and_discounts() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let all = xml_id(&mut env, "product.category_all");
    let desk = desk(&mut env)?;
    let pricelist = create(
        &mut env,
        "product_pricelist",
        json!({"name": "Resellers", "items": {"create": [
            {"applied_on": "category", "category": all, "compute_price": "discount", "percent_price": "10"},
            {"applied_on": "product", "product": desk, "min_quantity": "10", "compute_price": "fixed", "fixed_price": "200"}
        ]}}),
    )?;
    let partner = create(
        &mut env,
        "contact",
        json!({"name": "Reseller", "is_company": true, "pricelist": pricelist}),
    )?;
    let order = quotation(
        &mut env,
        partner,
        vec![json!({"product": desk, "product_uom_qty": "1"})],
    )?;
    assert_eq!(
        read(&mut env, "sale_order", order, &["pricelist"])?["pricelist"],
        json!(pricelist)
    );
    let line = line_ids(&mut env, order)?[0];
    assert_eq!(
        amount(&read(&mut env, "sale_order_line", line, &["price_unit"])?["price_unit"]),
        d("225")
    );
    env.call_rpc(
        "sale_order_line",
        "write",
        &json!({"ids": [line], "values": {"product_uom_qty": "12"}}),
    )?;
    let row = read(
        &mut env,
        "sale_order_line",
        line,
        &["price_unit", "price_subtotal"],
    )?;
    assert_eq!(amount(&row["price_unit"]), d("200"), "the quantity break");
    assert_eq!(amount(&row["price_subtotal"]), d("2400"));
    env.call_rpc(
        "sale_order_line",
        "write",
        &json!({"ids": [line], "values": {"price_unit": "190", "discount": "5"}}),
    )?;
    let row = read(
        &mut env,
        "sale_order_line",
        line,
        &["price_unit", "price_subtotal"],
    )?;
    assert_eq!(amount(&row["price_unit"]), d("190"), "typed by hand");
    assert_eq!(
        amount(&row["price_subtotal"]),
        d("2166"),
        "12 × 190 less 5 %"
    );
    let error = create(
        &mut env,
        "product_pricelist_item",
        json!({"pricelist": pricelist, "compute_price": "discount", "percent_price": "120"}),
    )
    .expect_err("over 100 %")
    .to_string();
    assert!(error.contains("100"), "{error}");
    let error = create(
        &mut env,
        "sale_order_line",
        json!({"order": order, "name": "Minus", "product_uom_qty": "-1"}),
    )
    .expect_err("negative")
    .to_string();
    assert!(error.contains("negative"), "{error}");
    Ok(())
}

/// Quotation, confirmation, invoice, payment, matching: the whole sale, the books balanced.
#[test]
fn test_quotation_to_payment() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let partner = customer(&mut env)?;
    let desk = desk(&mut env)?;
    let order = quotation(
        &mut env,
        partner,
        vec![json!({"product": desk, "product_uom_qty": "3"})],
    )?;
    call(&mut env, "sale_order", "action_quotation_send", &[order])?;
    assert_eq!(
        read(&mut env, "sale_order", order, &["state"])?["state"],
        json!("sent")
    );
    call(&mut env, "sale_order", "action_confirm", &[order])?;
    let row = read(&mut env, "sale_order", order, &["state", "invoice_status"])?;
    assert_eq!(
        (row["state"].clone(), row["invoice_status"].clone()),
        (json!("sale"), json!("to_invoice"))
    );

    let answer = call(&mut env, "sale_order", "action_create_invoice", &[order])?;
    assert_eq!(answer["action"], json!("account.action_move_out_invoice"));
    let invoice = answer["id"].as_u64().expect("an invoice") as u32;
    let row = read(
        &mut env,
        "account_move",
        invoice,
        &["partner", "amount_total", "reference", "state"],
    )?;
    assert_eq!(row["partner"], json!(partner));
    assert_eq!(amount(&row["amount_total"]), d("907.5"));
    assert_eq!(row["reference"], json!("S00001"));
    let line = line_ids(&mut env, order)?[0];
    let row = read(
        &mut env,
        "sale_order_line",
        line,
        &["qty_invoiced", "qty_to_invoice", "invoice_status"],
    )?;
    assert_eq!(
        (amount(&row["qty_invoiced"]), amount(&row["qty_to_invoice"])),
        (d("3"), d("0"))
    );
    assert_eq!(row["invoice_status"], json!("invoiced"));
    assert_eq!(
        read(
            &mut env,
            "sale_order",
            order,
            &["invoice_status", "invoices"]
        )?,
        json!({"id": order, "invoice_status": "invoiced", "invoices": [invoice]})
    );
    assert!(
        call(&mut env, "sale_order", "action_create_invoice", &[order]).is_err(),
        "nothing left to invoice"
    );

    call(&mut env, "account_move", "action_post", &[invoice])?;
    let answer = call(
        &mut env,
        "account_move",
        "action_register_payment",
        &[invoice],
    )?;
    let payment = answer["id"].as_u64().expect("a payment") as u32;
    call(&mut env, "account_payment", "action_post", &[payment])?;
    let row = read(
        &mut env,
        "account_move",
        invoice,
        &["payment_state", "amount_residual"],
    )?;
    assert_eq!(row["payment_state"], json!("paid"));
    assert_eq!(amount(&row["amount_residual"]), d("0"));
    account::invariants::check_books(&mut env)
}

/// A product invoiced as delivered is invoiced as it is delivered, in as many invoices.
#[test]
fn test_invoicing_what_is_delivered() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let partner = customer(&mut env)?;
    let chair = create(
        &mut env,
        "product",
        json!({"name": "Chair", "list_price": "40", "invoice_policy": "delivery"}),
    )?;
    let order = quotation(
        &mut env,
        partner,
        vec![json!({"product": chair, "product_uom_qty": "5"})],
    )?;
    call(&mut env, "sale_order", "action_confirm", &[order])?;
    assert_eq!(
        read(&mut env, "sale_order", order, &["invoice_status"])?["invoice_status"],
        json!("no")
    );
    assert!(
        call(&mut env, "sale_order", "action_create_invoice", &[order]).is_err(),
        "nothing delivered"
    );
    let line = line_ids(&mut env, order)?[0];
    env.call_rpc(
        "sale_order_line",
        "write",
        &json!({"ids": [line], "values": {"qty_delivered": "2"}}),
    )?;
    let first = call(&mut env, "sale_order", "action_create_invoice", &[order])?["id"]
        .as_u64()
        .expect("an invoice") as u32;
    assert_eq!(
        amount(&read(&mut env, "account_move", first, &["amount_untaxed"])?["amount_untaxed"]),
        d("80")
    );
    env.call_rpc(
        "sale_order_line",
        "write",
        &json!({"ids": [line], "values": {"qty_delivered": "5"}}),
    )?;
    assert_eq!(
        amount(&read(&mut env, "sale_order_line", line, &["qty_to_invoice"])?["qty_to_invoice"]),
        d("3")
    );
    let second = call(&mut env, "sale_order", "action_create_invoice", &[order])?["id"]
        .as_u64()
        .expect("an invoice") as u32;
    assert_eq!(
        amount(&read(&mut env, "account_move", second, &["amount_untaxed"])?["amount_untaxed"]),
        d("120")
    );
    assert_eq!(
        read(&mut env, "sale_order", order, &["invoice_status"])?["invoice_status"],
        json!("invoiced")
    );
    Ok(())
}

/// A credit note of an order's invoice counts against what was invoiced: the order is to
/// invoice again; a cancelled invoice counts for nothing.
#[test]
fn test_credit_notes_count_against_the_order() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let partner = customer(&mut env)?;
    let desk = desk(&mut env)?;
    let order = quotation(
        &mut env,
        partner,
        vec![json!({"product": desk, "product_uom_qty": "2"})],
    )?;
    call(&mut env, "sale_order", "action_confirm", &[order])?;
    let invoice = call(&mut env, "sale_order", "action_create_invoice", &[order])?["id"]
        .as_u64()
        .expect("an invoice") as u32;
    call(&mut env, "account_move", "action_post", &[invoice])?;
    let refund = call(&mut env, "account_move", "action_reverse", &[invoice])?["id"]
        .as_u64()
        .expect("a credit note") as u32;
    call(&mut env, "account_move", "action_post", &[refund])?;
    let line = line_ids(&mut env, order)?[0];
    let row = read(
        &mut env,
        "sale_order_line",
        line,
        &["qty_invoiced", "qty_to_invoice"],
    )?;
    assert_eq!(
        (amount(&row["qty_invoiced"]), amount(&row["qty_to_invoice"])),
        (d("0"), d("2"))
    );
    assert_eq!(
        read(&mut env, "sale_order", order, &["invoice_status"])?["invoice_status"],
        json!("to_invoice")
    );
    account::invariants::check_books(&mut env)
}

/// Cancelled at each step: a quotation; an order and its draft invoice; not an order whose
/// invoice is posted; set back to quotation, it is confirmed again.
#[test]
fn test_cancelling_at_each_step() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let partner = customer(&mut env)?;
    let desk = desk(&mut env)?;
    let first = quotation(&mut env, partner, vec![json!({"product": desk})])?;
    call(&mut env, "sale_order", "action_cancel", &[first])?;
    assert_eq!(
        read(&mut env, "sale_order", first, &["state"])?["state"],
        json!("cancel")
    );
    assert!(
        call(&mut env, "sale_order", "action_confirm", &[first]).is_err(),
        "cancelled"
    );
    let line = line_ids(&mut env, first)?[0];
    assert!(
        env.call_rpc(
            "sale_order_line",
            "write",
            &json!({"ids": [line], "values": {"product_uom_qty": "4"}})
        )
        .is_err()
    );
    call(&mut env, "sale_order", "action_draft", &[first])?;
    call(&mut env, "sale_order", "action_confirm", &[first])?;

    let invoice = call(&mut env, "sale_order", "action_create_invoice", &[first])?["id"]
        .as_u64()
        .expect("an invoice") as u32;
    let second = quotation(&mut env, partner, vec![json!({"product": desk})])?;
    call(&mut env, "sale_order", "action_confirm", &[second])?;
    let draft = call(&mut env, "sale_order", "action_create_invoice", &[second])?["id"]
        .as_u64()
        .expect("an invoice") as u32;
    call(&mut env, "sale_order", "action_cancel", &[second])?;
    assert_eq!(
        read(&mut env, "account_move", draft, &["state"])?["state"],
        json!("cancel"),
        "its draft invoice too"
    );

    call(&mut env, "account_move", "action_post", &[invoice])?;
    let error = call(&mut env, "sale_order", "action_cancel", &[first])
        .expect_err("invoiced")
        .to_string();
    assert!(error.contains("credit the invoice"), "{error}");
    let error = env
        .call_rpc("sale_order", "delete", &json!({"ids": [first]}))
        .expect_err("confirmed")
        .to_string();
    assert!(error.contains("cancel"), "{error}");
    account::invariants::check_books(&mut env)
}

/// Confirmed, an order keeps its customer and prices' basis.
#[test]
fn test_what_a_confirmed_order_keeps() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let partner = customer(&mut env)?;
    let other = create(&mut env, "contact", json!({"name": "Other"}))?;
    let desk = desk(&mut env)?;
    let order = quotation(&mut env, partner, vec![json!({"product": desk})])?;
    let empty = quotation(&mut env, partner, vec![])?;
    assert!(
        call(&mut env, "sale_order", "action_confirm", &[empty]).is_err(),
        "no line"
    );
    call(&mut env, "sale_order", "action_confirm", &[order])?;
    let error = env
        .call_rpc(
            "sale_order",
            "write",
            &json!({"ids": [order], "values": {"partner": other}}),
        )
        .expect_err("locked")
        .to_string();
    assert!(error.contains("partner"), "{error}");
    env.call_rpc(
        "sale_order",
        "write",
        &json!({"ids": [order], "values": {"note": "Delivered at the back door"}}),
    )?;
    create(
        &mut env,
        "sale_order_line",
        json!({"order": order, "product": desk, "product_uom_qty": "1"}),
    )?;
    assert_eq!(
        amount(&read(&mut env, "sale_order", order, &["amount_untaxed"])?["amount_untaxed"]),
        d("500"),
        "a line added"
    );
    Ok(())
}

/// The customer's fiscal position maps the taxes of the order's lines and of its invoice.
#[test]
fn test_a_fiscal_position() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let position = chart(&mut env, "position_intra_eu");
    let partner = create(
        &mut env,
        "contact",
        json!({"name": "EU", "is_company": true, "fiscal_position": position}),
    )?;
    let desk = desk(&mut env)?;
    let order = quotation(&mut env, partner, vec![json!({"product": desk})])?;
    assert_eq!(
        amount(&read(&mut env, "sale_order", order, &["amount_tax"])?["amount_tax"]),
        d("0")
    );
    call(&mut env, "sale_order", "action_confirm", &[order])?;
    let invoice = call(&mut env, "sale_order", "action_create_invoice", &[order])?["id"]
        .as_u64()
        .expect("an invoice") as u32;
    let row = read(
        &mut env,
        "account_move",
        invoice,
        &["fiscal_position", "amount_tax"],
    )?;
    assert_eq!(row["fiscal_position"], json!(position));
    assert_eq!(amount(&row["amount_tax"]), d("0"));
    Ok(())
}

/// Salespeople quote, confirm and invoice; other employees see no orders.
#[test]
fn test_access_rights() -> Result<()> {
    let app = new_app()?;
    let (partner, desk) = {
        let mut env = admin_env(&app)?;
        let ids = (customer(&mut env)?, desk(&mut env)?);
        env.close()?;
        ids
    };
    {
        let mut env = user_env(&app, "employee", &["base.group_user"])?;
        assert!(quotation(&mut env, partner, vec![json!({"product": desk})]).is_err());
    }
    let mut env = user_env(
        &app,
        "seller",
        &["base.group_user", "sale.group_sale_salesman"],
    )?;
    let order = quotation(&mut env, partner, vec![json!({"product": desk})])?;
    call(&mut env, "sale_order", "action_confirm", &[order])?;
    call(&mut env, "sale_order", "action_create_invoice", &[order])?;
    assert!(
        create(&mut env, "product_pricelist", json!({"name": "Mine"})).is_err(),
        "pricelists are the manager's"
    );
    Ok(())
}

/// The views resolve, and the application has its menus.
#[test]
fn test_views_and_menus() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    for (model, kinds) in [
        ("sale_order", &["list", "form", "search"][..]),
        ("sale_order_line", &["list", "form"]),
        ("product_pricelist", &["list", "form"]),
        ("product_pricelist_item", &["list", "form"]),
        ("contact", &["form"]),
        ("product", &["form"]),
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
    let tree = env.call_rpc("menu", "tree", &json!({}))?;
    let sales = tree
        .as_array()
        .expect("menus")
        .iter()
        .find(|entry| entry["name"] == "Sales")
        .expect("the application");
    let sections: Vec<&str> = sales["children"]
        .as_array()
        .expect("sections")
        .iter()
        .filter_map(|entry| entry["name"].as_str())
        .collect();
    assert_eq!(
        sections,
        vec![
            "Dashboard",
            "Orders",
            "To invoice",
            "Products",
            "Configuration"
        ]
    );
    Ok(())
}

/// The application loaded again, as once more plugins are installed: the plugins sales builds
/// on load before it, while its view of a contact, showing the pricelist, is already there.
#[test]
fn test_loading_again() -> Result<()> {
    let app = new_app()?;
    let mut next = app.successor();
    for plugin in plugins() {
        next.register_plugin(plugin)?;
    }
    next.load()?;
    let mut env = admin_env(&next)?;
    let arch = env.get_empty_record::<View<_>>().load(
        &mut env,
        "contact".to_string(),
        "form".to_string(),
    )?;
    assert!(arch.contains("pricelist"), "{arch}");
    Ok(())
}

/// A record created from an action starts with what the action declares, not with what its domain
/// filters on: a new customer invoice is one, a new order opened from the confirmed ones a quotation.
#[test]
fn test_a_new_record_starts_with_its_action_defaults() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let load = |env: &mut Environment, xml_id: &str| {
        env.call_rpc(
            "action",
            "load",
            &json!({ "ids": [], "args": { "xml_id": xml_id } }),
        )
    };
    let invoices = load(&mut env, "account.action_move_out_invoice")?;
    assert_eq!(invoices["defaults"], json!({"move_type": "out_invoice"}));
    let orders = load(&mut env, "sale.action_orders")?;
    assert_eq!(orders["domain"], json!([["state", "=", "sale"]]));
    assert_eq!(orders["defaults"], json!({}));
    Ok(())
}

/// A new quotation starts dated today and made by whoever opens it: what a form shows before
/// saving, and what a quotation created without them gets.
#[test]
fn test_a_new_quotation_starts_today() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let today = erp::types::field::Utc::now().date_naive();
    let defaults = env.call_rpc(
        "sale_order",
        "default_get",
        &json!({"fields": ["date_order", "user", "state"]}),
    )?;
    assert_eq!(defaults["date_order"], json!(today.to_string()));
    assert_eq!(defaults["user"][0], json!(env.uid()));
    assert_eq!(defaults["state"], json!("draft"));
    Ok(())
}

/// A product says how much of it confirmed orders sold, and which: quotations count for nothing.
#[test]
fn test_a_product_tells_what_was_sold() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let partner = customer(&mut env)?;
    let desk = desk(&mut env)?;
    let confirmed = quotation(
        &mut env,
        partner,
        vec![
            json!({"product": desk, "product_uom_qty": "3"}),
            json!({"product": desk, "product_uom_qty": "2"}),
        ],
    )?;
    let _quoted = quotation(
        &mut env,
        partner,
        vec![json!({"product": desk, "product_uom_qty": "7"})],
    )?;
    call(&mut env, "sale_order", "action_confirm", &[confirmed])?;
    let row = read(&mut env, "product", desk, &["sold_qty", "sale_orders"])?;
    assert_eq!(amount(&row["sold_qty"]), d("5"));
    assert_eq!(row["sale_orders"], json!([confirmed]));
    Ok(())
}
