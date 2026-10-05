//! A sale from end to end: quotation, confirmation, delivery, invoice, payment, matching — the
//! stock, its value and the books checked at every step; partial deliveries, returns and
//! cancellations.

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
use sale::SalePlugin;
use sale_stock::SaleStockPlugin;
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
            Box::new(SalePlugin {}),
            Box::new(StockPlugin {}),
            Box::new(SaleStockPlugin {}),
        ],
        "sale_stock",
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

/// A desk in stock: 10 received at 120 each.
fn stocked_desk(env: &mut Environment) -> Result<u32> {
    let desk = create(
        env,
        "product",
        json!({"name": "Desk", "list_price": "250", "standard_price": "120"}),
    )?;
    let warehouse = xml_id(env, "stock.warehouse_main");
    let receipts = read(env, "stock_warehouse", warehouse, &["in_type"])?["in_type"]
        .as_u64()
        .expect("receipts") as u32;
    let receipt = create(
        env,
        "stock_picking",
        json!({"picking_type": receipts, "moves": {"create": [
        {"name": "Desks", "product": desk, "product_uom_qty": "10", "price_unit": "120"}]}}),
    )?;
    call(env, "stock_picking", "button_validate", &[receipt])?;
    Ok(desk)
}

fn on_hand(env: &mut Environment, product: u32) -> Result<(Decimal, Decimal)> {
    let row = read(env, "product", product, &["qty_available", "stock_value"])?;
    Ok((amount(&row["qty_available"]), amount(&row["stock_value"])))
}

fn order(env: &mut Environment, lines: Vec<Value>) -> Result<u32> {
    let customer = create(
        env,
        "contact",
        json!({"name": "Customer", "is_company": true}),
    )?;
    create(
        env,
        "sale_order",
        json!({"partner": customer, "date_order": "2026-03-02", "lines": {"create": lines}}),
    )
}

/// Quotation, confirmation, delivery, invoice, payment, matching.
#[test]
fn test_quotation_to_matching() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let desk = stocked_desk(&mut env)?;
    let support = create(
        &mut env,
        "product",
        json!({"name": "Support", "product_type": "service", "list_price": "50", "invoice_policy": "order"}),
    )?;
    assert_eq!(on_hand(&mut env, desk)?, (d("10"), d("1200")));
    assert_eq!(
        read(&mut env, "product", desk, &["invoice_policy"])?["invoice_policy"],
        json!("delivery")
    );
    let order = order(
        &mut env,
        vec![
            json!({"product": desk, "product_uom_qty": "3"}),
            json!({"product": support, "product_uom_qty": "2"}),
        ],
    )?;
    assert_eq!(
        read(&mut env, "sale_order", order, &["pickings"])?["pickings"],
        json!([]),
        "a quotation delivers nothing"
    );

    call(&mut env, "sale_order", "action_confirm", &[order])?;
    let row = read(
        &mut env,
        "sale_order",
        order,
        &["pickings", "delivery_status", "invoice_status"],
    )?;
    let pickings = ids(&row["pickings"]);
    assert_eq!(pickings.len(), 1, "one delivery");
    assert_eq!(row["delivery_status"], json!("pending"));
    assert_eq!(
        row["invoice_status"],
        json!("to_invoice"),
        "the service, invoiced as ordered"
    );
    let delivery = pickings[0];
    let picking = read(
        &mut env,
        "stock_picking",
        delivery,
        &["state", "origin", "moves", "name"],
    )?;
    assert_eq!(picking["state"], json!("assigned"));
    assert_eq!(picking["origin"], json!("S00001"));
    assert_eq!(
        ids(&picking["moves"]).len(),
        1,
        "the service is not delivered"
    );
    assert_eq!(
        amount(&read(&mut env, "product", desk, &["free_qty"])?["free_qty"]),
        d("7"),
        "3 promised"
    );

    call(&mut env, "stock_picking", "button_validate", &[delivery])?;
    assert_eq!(
        on_hand(&mut env, desk)?,
        (d("7"), d("840")),
        "3 desks out at 120"
    );
    let row = read(
        &mut env,
        "sale_order",
        order,
        &["delivery_status", "invoice_status"],
    )?;
    assert_eq!(row["delivery_status"], json!("full"));
    assert_eq!(row["invoice_status"], json!("to_invoice"));
    let lines = ids(&read(&mut env, "sale_order", order, &["lines"])?["lines"]);
    assert_eq!(
        amount(&read(&mut env, "sale_order_line", lines[0], &["qty_delivered"])?["qty_delivered"]),
        d("3")
    );

    let invoice = call(&mut env, "sale_order", "action_create_invoice", &[order])?["id"]
        .as_u64()
        .expect("an invoice") as u32;
    let row = read(
        &mut env,
        "account_move",
        invoice,
        &["amount_untaxed", "amount_total"],
    )?;
    assert_eq!(amount(&row["amount_untaxed"]), d("850"), "3 × 250 + 2 × 50");
    assert_eq!(amount(&row["amount_total"]), d("1028.5"));
    call(&mut env, "account_move", "action_post", &[invoice])?;
    assert_eq!(
        read(&mut env, "sale_order", order, &["invoice_status"])?["invoice_status"],
        json!("invoiced")
    );
    let payment = call(
        &mut env,
        "account_move",
        "action_register_payment",
        &[invoice],
    )?["id"]
        .as_u64()
        .expect("a payment") as u32;
    call(&mut env, "account_payment", "action_post", &[payment])?;
    let row = read(
        &mut env,
        "account_move",
        invoice,
        &["payment_state", "amount_residual"],
    )?;
    assert_eq!(row["payment_state"], json!("paid"));
    assert_eq!(amount(&row["amount_residual"]), d("0"));
    let receivable = xml_id(&mut env, "account_test_chart.a_receivable");
    let open = env.call_rpc(
        "account_move_line",
        "search",
        &json!({"domain": [["account", "=", receivable], ["reconciled", "=", false]]}),
    )?;
    assert_eq!(open, json!([]), "every receivable is matched");
    account::invariants::check_books(&mut env)
}

/// A delivery done in part invoices only what left; the back order delivers the rest.
#[test]
fn test_partial_delivery() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let desk = stocked_desk(&mut env)?;
    let order = order(
        &mut env,
        vec![json!({"product": desk, "product_uom_qty": "5"})],
    )?;
    call(&mut env, "sale_order", "action_confirm", &[order])?;
    let delivery = ids(&read(&mut env, "sale_order", order, &["pickings"])?["pickings"])[0];
    let moves = read(&mut env, "stock_picking", delivery, &["moves"])?["moves"].clone();
    env.call_rpc(
        "stock_move",
        "write",
        &json!({"ids": moves, "values": {"quantity": "2"}}),
    )?;
    let backorder = call(&mut env, "stock_picking", "button_validate", &[delivery])?["id"]
        .as_u64()
        .expect("a back order") as u32;
    assert_eq!(
        read(&mut env, "sale_order", order, &["delivery_status"])?["delivery_status"],
        json!("partial")
    );
    let first = call(&mut env, "sale_order", "action_create_invoice", &[order])?["id"]
        .as_u64()
        .expect("an invoice") as u32;
    assert_eq!(
        amount(&read(&mut env, "account_move", first, &["amount_untaxed"])?["amount_untaxed"]),
        d("500")
    );
    let pickings = ids(&read(&mut env, "sale_order", order, &["pickings"])?["pickings"]);
    assert!(
        pickings.contains(&backorder),
        "the back order is the order's too"
    );
    call(&mut env, "stock_picking", "button_validate", &[backorder])?;
    let second = call(&mut env, "sale_order", "action_create_invoice", &[order])?["id"]
        .as_u64()
        .expect("an invoice") as u32;
    assert_eq!(
        amount(&read(&mut env, "account_move", second, &["amount_untaxed"])?["amount_untaxed"]),
        d("750")
    );
    assert_eq!(on_hand(&mut env, desk)?, (d("5"), d("600")));
    Ok(())
}

/// Goods returned are no longer delivered: the order is to credit; a credit note settles it.
#[test]
fn test_returns_and_credit_notes() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let desk = stocked_desk(&mut env)?;
    let order = order(
        &mut env,
        vec![json!({"product": desk, "product_uom_qty": "2"})],
    )?;
    call(&mut env, "sale_order", "action_confirm", &[order])?;
    let delivery = ids(&read(&mut env, "sale_order", order, &["pickings"])?["pickings"])[0];
    call(&mut env, "stock_picking", "button_validate", &[delivery])?;
    let invoice = call(&mut env, "sale_order", "action_create_invoice", &[order])?["id"]
        .as_u64()
        .expect("an invoice") as u32;
    call(&mut env, "account_move", "action_post", &[invoice])?;
    let returned = call(&mut env, "stock_picking", "action_return", &[delivery])?["id"]
        .as_u64()
        .expect("a return") as u32;
    call(&mut env, "stock_picking", "button_validate", &[returned])?;
    assert_eq!(
        on_hand(&mut env, desk)?,
        (d("10"), d("1200")),
        "back at what they left at"
    );
    let lines = ids(&read(&mut env, "sale_order", order, &["lines"])?["lines"]);
    let row = read(
        &mut env,
        "sale_order_line",
        lines[0],
        &["qty_delivered", "qty_to_invoice"],
    )?;
    assert_eq!(
        (
            amount(&row["qty_delivered"]),
            amount(&row["qty_to_invoice"])
        ),
        (d("0"), d("-2"))
    );
    let answer = call(&mut env, "sale_order", "action_create_invoice", &[order])?;
    assert_eq!(
        answer["action"],
        json!("account.action_move_out_refund"),
        "a credit note"
    );
    let refund = answer["id"].as_u64().expect("a credit note") as u32;
    assert_eq!(
        amount(&read(&mut env, "account_move", refund, &["amount_total"])?["amount_total"]),
        d("605")
    );
    call(&mut env, "account_move", "action_post", &[refund])?;
    assert_eq!(
        amount(
            &read(&mut env, "sale_order_line", lines[0], &["qty_to_invoice"])?["qty_to_invoice"]
        ),
        d("0")
    );
    account::invariants::check_books(&mut env)
}

/// Cancelling an order cancels its delivery not done, releasing the goods; one delivered must
/// come back first.
#[test]
fn test_cancelling_with_deliveries() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let desk = stocked_desk(&mut env)?;
    let first = order(
        &mut env,
        vec![json!({"product": desk, "product_uom_qty": "4"})],
    )?;
    call(&mut env, "sale_order", "action_confirm", &[first])?;
    assert_eq!(
        amount(&read(&mut env, "product", desk, &["free_qty"])?["free_qty"]),
        d("6")
    );
    call(&mut env, "sale_order", "action_cancel", &[first])?;
    let delivery = ids(&read(&mut env, "sale_order", first, &["pickings"])?["pickings"])[0];
    assert_eq!(
        read(&mut env, "stock_picking", delivery, &["state"])?["state"],
        json!("cancel")
    );
    assert_eq!(
        amount(&read(&mut env, "product", desk, &["free_qty"])?["free_qty"]),
        d("10")
    );
    let second = order(
        &mut env,
        vec![json!({"product": desk, "product_uom_qty": "1"})],
    )?;
    call(&mut env, "sale_order", "action_confirm", &[second])?;
    let delivery = ids(&read(&mut env, "sale_order", second, &["pickings"])?["pickings"])[0];
    call(&mut env, "stock_picking", "button_validate", &[delivery])?;
    let error = call(&mut env, "sale_order", "action_cancel", &[second])
        .expect_err("delivered")
        .to_string();
    assert!(error.contains("return the goods"), "{error}");
    Ok(())
}

/// The order form shows its deliveries.
#[test]
fn test_views() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let form = env.get_empty_record::<base::models::View<_>>().load(
        &mut env,
        "sale_order".to_string(),
        "form".to_string(),
    )?;
    assert!(form.contains("name=\"pickings\""), "{form}");
    let list = env.get_empty_record::<base::models::View<_>>().load(
        &mut env,
        "sale_order".to_string(),
        "list".to_string(),
    )?;
    assert!(list.contains("delivery_status"), "{list}");
    Ok(())
}
