//! A purchase from end to end: request, order, receipt, vendor bill, payment — the stock, its
//! value at the price paid and the books checked at every step; partial receipts, returns to
//! the vendor and cancellations.

use account::AccountPlugin;
use account::testing::TestChartPlugin;
use base::BasePlugin;
use contacts::ContactsPlugin;
use currency::CurrencyPlugin;
use erp::Result;
use erp::app::Application;
use erp::environment::Environment;
use erp::types::field::Decimal;
use erp_test_support::{admin_env, d, xml_id};
use mail::MailPlugin;
use product::ProductPlugin;
use purchase::PurchasePlugin;
use purchase_stock::PurchaseStockPlugin;
use sequence::SequencePlugin;
use serde_json::{Value, json};
use stock::StockPlugin;
use uom::UomPlugin;
use web::WebPlugin;

fn new_app() -> Result<Application> {
    let mut app = erp_test_support::app(
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
            Box::new(StockPlugin {}),
            Box::new(PurchaseStockPlugin {}),
        ],
        "purchase_stock",
    )?;
    app.load_plugin("account_test_chart")?;
    Ok(app)
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

fn ids(value: &Value) -> Vec<u32> {
    value
        .as_array()
        .expect("ids")
        .iter()
        .map(|id| id.as_u64().expect("id") as u32)
        .collect()
}

fn on_hand(env: &mut Environment, product: u32) -> Result<(Decimal, Decimal)> {
    let row = read(env, "product", product, &["qty_available", "stock_value"])?;
    Ok((amount(&row["qty_available"]), amount(&row["stock_value"])))
}

fn order(env: &mut Environment, lines: Vec<Value>) -> Result<u32> {
    let vendor = create(
        env,
        "contact",
        json!({"name": "Vendor", "is_company": true}),
    )?;
    create(
        env,
        "purchase_order",
        json!({"partner": vendor, "date_order": "2026-03-02", "lines": {"create": lines}}),
    )
}

fn receipt_of(env: &mut Environment, order: u32) -> Result<u32> {
    let pickings = ids(&read(env, "purchase_order", order, &["pickings"])?["pickings"]);
    pickings.last().copied().ok_or_else(|| "a receipt".into())
}

/// Request, order, receipt valued at the price paid, bill, payment.
#[test]
fn test_request_to_payment() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let paper = create(
        &mut env,
        "product",
        json!({"name": "Paper", "standard_price": "5"}),
    )?;
    let advice = create(
        &mut env,
        "product",
        json!({"name": "Advice", "product_type": "service", "standard_price": "100", "purchase_method": "order"}),
    )?;
    assert_eq!(
        read(&mut env, "product", paper, &["purchase_method"])?["purchase_method"],
        json!("receive")
    );
    let order = order(
        &mut env,
        vec![
            json!({"product": paper, "product_qty": "20", "price_unit": "4", "discount": "10"}),
            json!({"product": advice, "product_qty": "1"}),
        ],
    )?;
    call(&mut env, "purchase_order", "action_rfq_send", &[order])?;
    call(&mut env, "purchase_order", "button_confirm", &[order])?;
    let row = read(
        &mut env,
        "purchase_order",
        order,
        &["receipt_status", "bill_status"],
    )?;
    assert_eq!(row["receipt_status"], json!("pending"));
    assert_eq!(
        row["bill_status"],
        json!("to_bill"),
        "the advice, billed as ordered"
    );
    let receipt = receipt_of(&mut env, order)?;
    let picking = read(
        &mut env,
        "stock_picking",
        receipt,
        &["state", "origin", "moves"],
    )?;
    assert_eq!(picking["state"], json!("assigned"));
    assert_eq!(picking["origin"], json!("P00001"));
    let moves = ids(&picking["moves"]);
    assert_eq!(moves.len(), 1, "the service is not received");
    assert_eq!(
        amount(&read(&mut env, "stock_move", moves[0], &["price_unit"])?["price_unit"]),
        d("3.6"),
        "4 less 10 %"
    );

    call(&mut env, "stock_picking", "button_validate", &[receipt])?;
    assert_eq!(on_hand(&mut env, paper)?, (d("20"), d("72")));
    assert_eq!(
        amount(&read(&mut env, "product", paper, &["standard_price"])?["standard_price"]),
        d("3.6")
    );
    let lines = ids(&read(&mut env, "purchase_order", order, &["lines"])?["lines"]);
    assert_eq!(
        amount(
            &read(&mut env, "purchase_order_line", lines[0], &["qty_received"])?["qty_received"]
        ),
        d("20")
    );
    assert_eq!(
        read(&mut env, "purchase_order", order, &["receipt_status"])?["receipt_status"],
        json!("full")
    );

    let bill = call(&mut env, "purchase_order", "action_create_bill", &[order])?["id"]
        .as_u64()
        .expect("a bill") as u32;
    let row = read(
        &mut env,
        "account_move",
        bill,
        &["amount_untaxed", "amount_total"],
    )?;
    assert_eq!(amount(&row["amount_untaxed"]), d("172"), "72 + 100");
    assert_eq!(amount(&row["amount_total"]), d("208.12"));
    env.call_rpc(
        "account_move",
        "write",
        &json!({"ids": [bill], "values": {"invoice_date": "2026-03-10"}}),
    )?;
    call(&mut env, "account_move", "action_post", &[bill])?;
    assert_eq!(
        read(&mut env, "purchase_order", order, &["bill_status"])?["bill_status"],
        json!("billed")
    );
    let payment = call(&mut env, "account_move", "action_register_payment", &[bill])?["id"]
        .as_u64()
        .expect("a payment") as u32;
    call(&mut env, "account_payment", "action_post", &[payment])?;
    assert_eq!(
        read(&mut env, "account_move", bill, &["payment_state"])?["payment_state"],
        json!("paid")
    );
    let payable = xml_id(&mut env, "account_test_chart.a_payable");
    let open = env.call_rpc(
        "account_move_line",
        "search",
        &json!({"domain": [["account", "=", payable], ["reconciled", "=", false]]}),
    )?;
    assert_eq!(open, json!([]), "every payable is matched");
    account::invariants::check_books(&mut env)
}

/// Bought by the dozen in dollars, received in units in euros.
#[test]
fn test_units_and_currencies_of_the_receipt() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let usd = xml_id(&mut env, "currency.currency_usd");
    create(
        &mut env,
        "currency_rate",
        json!({"currency": usd, "date": "2026-01-01", "rate": "1.25"}),
    )?;
    let dozen = xml_id(&mut env, "uom.uom_dozen");
    let egg = create(
        &mut env,
        "product",
        json!({"name": "Egg", "standard_price": "0.2", "purchase_uom": dozen}),
    )?;
    let vendor = create(
        &mut env,
        "contact",
        json!({"name": "US vendor", "is_company": true}),
    )?;
    let order = create(
        &mut env,
        "purchase_order",
        json!({"partner": vendor, "date_order": "2026-03-02",
        "currency": usd, "lines": {"create": [{"product": egg, "product_qty": "5", "price_unit": "3.75"}]}}),
    )?;
    call(&mut env, "purchase_order", "button_confirm", &[order])?;
    let receipt = receipt_of(&mut env, order)?;
    call(&mut env, "stock_picking", "button_validate", &[receipt])?;
    // 5 dozen at 3.75 dollars: 60 eggs worth 15 euros, 0.25 each.
    assert_eq!(on_hand(&mut env, egg)?, (d("60"), d("15")));
    Ok(())
}

/// A receipt done in part bills only what came; the rest comes in a back order.
#[test]
fn test_partial_receipt() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let ink = create(
        &mut env,
        "product",
        json!({"name": "Ink", "standard_price": "12"}),
    )?;
    let order = order(&mut env, vec![json!({"product": ink, "product_qty": "10"})])?;
    call(&mut env, "purchase_order", "button_confirm", &[order])?;
    let receipt = receipt_of(&mut env, order)?;
    let moves = read(&mut env, "stock_picking", receipt, &["moves"])?["moves"].clone();
    env.call_rpc(
        "stock_move",
        "write",
        &json!({"ids": moves, "values": {"quantity": "6"}}),
    )?;
    let backorder = call(&mut env, "stock_picking", "button_validate", &[receipt])?["id"]
        .as_u64()
        .expect("a back order") as u32;
    assert_eq!(
        read(&mut env, "purchase_order", order, &["receipt_status"])?["receipt_status"],
        json!("partial")
    );
    let bill = call(&mut env, "purchase_order", "action_create_bill", &[order])?["id"]
        .as_u64()
        .expect("a bill") as u32;
    assert_eq!(
        amount(&read(&mut env, "account_move", bill, &["amount_untaxed"])?["amount_untaxed"]),
        d("72")
    );
    assert_eq!(receipt_of(&mut env, order)?, backorder);
    call(&mut env, "stock_picking", "button_validate", &[backorder])?;
    assert_eq!(on_hand(&mut env, ink)?, (d("10"), d("120")));
    let second = call(&mut env, "purchase_order", "action_create_bill", &[order])?["id"]
        .as_u64()
        .expect("a bill") as u32;
    assert_eq!(
        amount(&read(&mut env, "account_move", second, &["amount_untaxed"])?["amount_untaxed"]),
        d("48")
    );
    Ok(())
}

/// Goods sent back to the vendor are no longer received: the order calls for a vendor credit
/// note.
#[test]
fn test_returns_to_the_vendor() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let ink = create(
        &mut env,
        "product",
        json!({"name": "Ink", "standard_price": "12"}),
    )?;
    let order = order(&mut env, vec![json!({"product": ink, "product_qty": "5"})])?;
    call(&mut env, "purchase_order", "button_confirm", &[order])?;
    let receipt = receipt_of(&mut env, order)?;
    call(&mut env, "stock_picking", "button_validate", &[receipt])?;
    let bill = call(&mut env, "purchase_order", "action_create_bill", &[order])?["id"]
        .as_u64()
        .expect("a bill") as u32;
    call(&mut env, "account_move", "action_post", &[bill])?;
    let returned = call(&mut env, "stock_picking", "action_return", &[receipt])?["id"]
        .as_u64()
        .expect("a return") as u32;
    let moves = read(&mut env, "stock_picking", returned, &["moves"])?["moves"].clone();
    env.call_rpc(
        "stock_move",
        "write",
        &json!({"ids": moves, "values": {"quantity": "2"}}),
    )?;
    call(&mut env, "stock_picking", "button_validate", &[returned])?;
    assert_eq!(on_hand(&mut env, ink)?, (d("3"), d("36")));
    let lines = ids(&read(&mut env, "purchase_order", order, &["lines"])?["lines"]);
    let row = read(
        &mut env,
        "purchase_order_line",
        lines[0],
        &["qty_received", "qty_to_bill"],
    )?;
    assert_eq!(
        (amount(&row["qty_received"]), amount(&row["qty_to_bill"])),
        (d("3"), d("-2"))
    );
    let answer = call(&mut env, "purchase_order", "action_create_bill", &[order])?;
    assert_eq!(answer["action"], json!("account.action_move_in_refund"));
    let refund = answer["id"].as_u64().expect("a credit note") as u32;
    assert_eq!(
        amount(&read(&mut env, "account_move", refund, &["amount_untaxed"])?["amount_untaxed"]),
        d("24")
    );
    call(&mut env, "account_move", "action_post", &[refund])?;
    account::invariants::check_books(&mut env)
}

/// Cancelling an order cancels its receipt not done; one received must go back first.
#[test]
fn test_cancelling_with_receipts() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let ink = create(
        &mut env,
        "product",
        json!({"name": "Ink", "standard_price": "12"}),
    )?;
    let first = order(&mut env, vec![json!({"product": ink, "product_qty": "5"})])?;
    call(&mut env, "purchase_order", "button_confirm", &[first])?;
    call(&mut env, "purchase_order", "button_cancel", &[first])?;
    let receipt = receipt_of(&mut env, first)?;
    assert_eq!(
        read(&mut env, "stock_picking", receipt, &["state"])?["state"],
        json!("cancel")
    );
    let second = order(&mut env, vec![json!({"product": ink, "product_qty": "1"})])?;
    call(&mut env, "purchase_order", "button_confirm", &[second])?;
    let receipt = receipt_of(&mut env, second)?;
    call(&mut env, "stock_picking", "button_validate", &[receipt])?;
    let error = call(&mut env, "purchase_order", "button_cancel", &[second])
        .expect_err("received")
        .to_string();
    assert!(error.contains("return the goods"), "{error}");
    assert_eq!(on_hand(&mut env, ink)?, (d("1"), d("12")));
    Ok(())
}

/// The order form shows its receipts.
#[test]
fn test_views() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let form = env.get_empty_record::<base::models::View<_>>().load(
        &mut env,
        "purchase_order".to_string(),
        "form".to_string(),
    )?;
    assert!(form.contains("name=\"pickings\""), "{form}");
    let list = env.get_empty_record::<base::models::View<_>>().load(
        &mut env,
        "purchase_order".to_string(),
        "list".to_string(),
    )?;
    assert!(list.contains("receipt_status"), "{list}");
    Ok(())
}
