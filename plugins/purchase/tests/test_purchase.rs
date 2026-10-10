//! Purchases: requests priced by the vendor's prices, confirmed into orders, billed as ordered
//! or as received, paid and matched; cancelled at every step; rights; views.

use account::AccountPlugin;
use account::testing::TestChartPlugin;
use base::BasePlugin;
use contacts::ContactsPlugin;
use currency::CurrencyPlugin;
use erp::Result;
use erp::app::Application;
use erp::environment::Environment;
use erp::types::field::Decimal;
use erp_test_support::{admin_env, d, user_env, xml_id};
use mail::MailPlugin;
use product::ProductPlugin;
use purchase::PurchasePlugin;
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
                Box::new(TestChartPlugin {}),
                Box::new(PurchasePlugin {}),
            ]
        },
        &["purchase", "account_test_chart"],
    )
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

fn vendor(env: &mut Environment) -> Result<u32> {
    create(
        env,
        "contact",
        json!({"name": "Vendor", "is_company": true}),
    )
}

fn request(env: &mut Environment, partner: u32, lines: Vec<Value>) -> Result<u32> {
    create(
        env,
        "purchase_order",
        json!({"partner": partner, "date_order": "2026-03-02", "lines": {"create": lines}}),
    )
}

fn lines(env: &mut Environment, order: u32) -> Result<Vec<u32>> {
    let lines = read(env, "purchase_order", order, &["lines"])?["lines"].clone();
    Ok(lines
        .as_array()
        .expect("lines")
        .iter()
        .map(|id| id.as_u64().expect("id") as u32)
        .collect())
}

/// A request is numbered and priced: at the vendor's price for the quantity, with its code and
/// lead time; without one, at the product's cost in the purchase unit.
#[test]
fn test_a_request_priced_by_the_vendor() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let partner = vendor(&mut env)?;
    let paper = create(
        &mut env,
        "product",
        json!({"name": "Paper", "standard_price": "5"}),
    )?;
    create(
        &mut env,
        "product_supplierinfo",
        json!({"partner": partner, "product": paper, "price": "4.50", "product_code": "PAP-A4", "delay": 3}),
    )?;
    create(
        &mut env,
        "product_supplierinfo",
        json!({"partner": partner, "product": paper, "price": "4", "min_quantity": "100", "delay": 3}),
    )?;
    let order = request(
        &mut env,
        partner,
        vec![json!({"product": paper, "product_qty": "10"})],
    )?;
    assert_eq!(
        read(&mut env, "purchase_order", order, &["name"])?["name"],
        json!("P00001")
    );
    let line = lines(&mut env, order)?[0];
    let row = read(
        &mut env,
        "purchase_order_line",
        line,
        &["name", "price_unit", "date_planned", "taxes"],
    )?;
    assert_eq!(row["name"], json!("[PAP-A4] Paper"));
    assert_eq!(amount(&row["price_unit"]), d("4.5"));
    assert_eq!(row["date_planned"], json!("2026-03-05"));
    assert_eq!(
        row["taxes"],
        json!([xml_id(&mut env, "account_test_chart.tax_purchase_21")])
    );
    env.call_rpc(
        "purchase_order_line",
        "write",
        &json!({"ids": [line], "values": {"product_qty": "150"}}),
    )?;
    assert_eq!(
        amount(&read(&mut env, "purchase_order_line", line, &["price_unit"])?["price_unit"]),
        d("4")
    );
    let row = read(
        &mut env,
        "purchase_order",
        order,
        &["amount_untaxed", "amount_tax", "amount_total"],
    )?;
    assert_eq!(
        (
            amount(&row["amount_untaxed"]),
            amount(&row["amount_tax"]),
            amount(&row["amount_total"])
        ),
        (d("600"), d("126"), d("726"))
    );

    let dozen = xml_id(&mut env, "uom.uom_dozen");
    let eggs = create(
        &mut env,
        "product",
        json!({"name": "Eggs", "standard_price": "0.25", "purchase_uom": dozen}),
    )?;
    let order = request(
        &mut env,
        partner,
        vec![json!({"product": eggs, "product_qty": "2"})],
    )?;
    let line = lines(&mut env, order)?[0];
    let row = read(
        &mut env,
        "purchase_order_line",
        line,
        &["price_unit", "uom"],
    )?;
    assert_eq!(amount(&row["price_unit"]), d("3"), "a dozen at 0.25 each");
    assert_eq!(row["uom"], json!(dozen));
    Ok(())
}

/// Request, confirmation, bill, payment, matching: the whole purchase, the books balanced.
#[test]
fn test_request_to_payment() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let partner = vendor(&mut env)?;
    let paper = create(
        &mut env,
        "product",
        json!({"name": "Paper", "standard_price": "5"}),
    )?;
    let order = request(
        &mut env,
        partner,
        vec![json!({"product": paper, "product_qty": "20"})],
    )?;
    env.call_rpc(
        "purchase_order",
        "write",
        &json!({"ids": [order], "values": {"partner_ref": "Q-889"}}),
    )?;
    call(&mut env, "purchase_order", "action_rfq_send", &[order])?;
    call(&mut env, "purchase_order", "button_confirm", &[order])?;
    let row = read(&mut env, "purchase_order", order, &["state", "bill_status"])?;
    assert_eq!(
        (row["state"].clone(), row["bill_status"].clone()),
        (json!("purchase"), json!("to_bill"))
    );
    let answer = call(&mut env, "purchase_order", "action_create_bill", &[order])?;
    assert_eq!(answer["action"], json!("account.action_move_in_invoice"));
    let bill = answer["id"].as_u64().expect("a bill") as u32;
    let row = read(
        &mut env,
        "account_move",
        bill,
        &["amount_total", "reference", "move_type"],
    )?;
    assert_eq!(amount(&row["amount_total"]), d("121"));
    assert_eq!(row["reference"], json!("Q-889"));
    assert_eq!(row["move_type"], json!("in_invoice"));
    assert_eq!(
        read(&mut env, "purchase_order", order, &["bill_status"])?["bill_status"],
        json!("billed")
    );
    call(&mut env, "account_move", "action_post", &[bill])?;
    let payment = call(&mut env, "account_move", "action_register_payment", &[bill])?["id"]
        .as_u64()
        .expect("a payment") as u32;
    call(&mut env, "account_payment", "action_post", &[payment])?;
    assert_eq!(
        read(&mut env, "account_move", bill, &["payment_state"])?["payment_state"],
        json!("paid")
    );
    account::invariants::check_books(&mut env)
}

/// A product billed as received is billed as it comes; a vendor credit note counts against it.
#[test]
fn test_billing_what_is_received() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let partner = vendor(&mut env)?;
    let ink = create(
        &mut env,
        "product",
        json!({"name": "Ink", "standard_price": "12", "purchase_method": "receive"}),
    )?;
    let order = request(
        &mut env,
        partner,
        vec![json!({"product": ink, "product_qty": "10"})],
    )?;
    call(&mut env, "purchase_order", "button_confirm", &[order])?;
    assert!(
        call(&mut env, "purchase_order", "action_create_bill", &[order]).is_err(),
        "nothing received"
    );
    let line = lines(&mut env, order)?[0];
    env.call_rpc(
        "purchase_order_line",
        "write",
        &json!({"ids": [line], "values": {"qty_received": "4"}}),
    )?;
    let bill = call(&mut env, "purchase_order", "action_create_bill", &[order])?["id"]
        .as_u64()
        .expect("a bill") as u32;
    assert_eq!(
        amount(&read(&mut env, "account_move", bill, &["amount_untaxed"])?["amount_untaxed"]),
        d("48")
    );
    call(&mut env, "account_move", "action_post", &[bill])?;
    let refund = call(&mut env, "account_move", "action_reverse", &[bill])?["id"]
        .as_u64()
        .expect("a credit note") as u32;
    call(&mut env, "account_move", "action_post", &[refund])?;
    let row = read(
        &mut env,
        "purchase_order_line",
        line,
        &["qty_billed", "qty_to_bill"],
    )?;
    assert_eq!(
        (amount(&row["qty_billed"]), amount(&row["qty_to_bill"])),
        (d("0"), d("4"))
    );
    account::invariants::check_books(&mut env)
}

/// Cancelled at each step: a request; an order and its draft bill; not an order whose bill is
/// posted; set back to request, it is confirmed again.
#[test]
fn test_cancelling_at_each_step() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let partner = vendor(&mut env)?;
    let paper = create(
        &mut env,
        "product",
        json!({"name": "Paper", "standard_price": "5"}),
    )?;
    let first = request(&mut env, partner, vec![json!({"product": paper})])?;
    call(&mut env, "purchase_order", "button_cancel", &[first])?;
    assert!(call(&mut env, "purchase_order", "button_confirm", &[first]).is_err());
    call(&mut env, "purchase_order", "button_draft", &[first])?;
    call(&mut env, "purchase_order", "button_confirm", &[first])?;
    let second = request(&mut env, partner, vec![json!({"product": paper})])?;
    call(&mut env, "purchase_order", "button_confirm", &[second])?;
    let draft = call(&mut env, "purchase_order", "action_create_bill", &[second])?["id"]
        .as_u64()
        .expect("a bill") as u32;
    call(&mut env, "purchase_order", "button_cancel", &[second])?;
    assert_eq!(
        read(&mut env, "account_move", draft, &["state"])?["state"],
        json!("cancel")
    );
    let bill = call(&mut env, "purchase_order", "action_create_bill", &[first])?["id"]
        .as_u64()
        .expect("a bill") as u32;
    call(&mut env, "account_move", "action_post", &[bill])?;
    let error = call(&mut env, "purchase_order", "button_cancel", &[first])
        .expect_err("billed")
        .to_string();
    assert!(error.contains("credit the bill"), "{error}");
    let other = vendor(&mut env)?;
    let error = env
        .call_rpc(
            "purchase_order",
            "write",
            &json!({"ids": [first], "values": {"partner": other}}),
        )
        .expect_err("locked")
        .to_string();
    assert!(error.contains("partner"), "{error}");
    account::invariants::check_books(&mut env)
}

/// Buyers request, confirm and bill; other employees see no orders.
#[test]
fn test_access_rights() -> Result<()> {
    let app = new_app()?;
    let (partner, paper) = {
        let mut env = admin_env(&app)?;
        let ids = (
            vendor(&mut env)?,
            create(
                &mut env,
                "product",
                json!({"name": "Paper", "standard_price": "5"}),
            )?,
        );
        env.close()?;
        ids
    };
    {
        let mut env = user_env(&app, "employee", &["base.group_user"])?;
        assert!(request(&mut env, partner, vec![json!({"product": paper})]).is_err());
    }
    let mut env = user_env(
        &app,
        "buyer",
        &["base.group_user", "purchase.group_purchase_user"],
    )?;
    let order = request(&mut env, partner, vec![json!({"product": paper})])?;
    call(&mut env, "purchase_order", "button_confirm", &[order])?;
    call(&mut env, "purchase_order", "action_create_bill", &[order])?;
    Ok(())
}

/// The views resolve, and the application has its menus.
#[test]
fn test_views_and_menus() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    for (model, kinds) in [
        ("purchase_order", &["list", "form", "search"][..]),
        ("purchase_order_line", &["list", "form"]),
        ("product_supplierinfo", &["list"]),
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
    let menu = tree
        .as_array()
        .expect("menus")
        .iter()
        .find(|entry| entry["name"] == "Purchase")
        .expect("the application");
    let sections: Vec<&str> = menu["children"]
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
            "To bill",
            "Products",
            "Configuration"
        ]
    );
    Ok(())
}

/// A product says how much of it confirmed orders bought, and which: requests count for nothing.
#[test]
fn test_a_product_tells_what_was_purchased() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let partner = vendor(&mut env)?;
    let paper = create(
        &mut env,
        "product",
        json!({"name": "Paper", "standard_price": "5"}),
    )?;
    let confirmed = request(
        &mut env,
        partner,
        vec![json!({"product": paper, "product_qty": "20"})],
    )?;
    let _requested = request(
        &mut env,
        partner,
        vec![json!({"product": paper, "product_qty": "4"})],
    )?;
    call(&mut env, "purchase_order", "button_confirm", &[confirmed])?;
    let row = read(
        &mut env,
        "product",
        paper,
        &["purchased_qty", "purchase_orders"],
    )?;
    assert_eq!(amount(&row["purchased_qty"]), d("20"));
    assert_eq!(row["purchase_orders"], json!([confirmed]));
    Ok(())
}
