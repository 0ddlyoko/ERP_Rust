//! Perpetual valuation: every receipt, delivery, return and count booked on the category's
//! stock accounts, the stock account always worth what the stock is.

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
use sequence::SequencePlugin;
use serde_json::{Value, json};
use stock::StockPlugin;
use stock_account::StockAccountPlugin;
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
                Box::new(StockPlugin {}),
                Box::new(StockAccountPlugin {}),
            ]
        },
        &["stock_account", "account_test_chart"],
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

struct Accounts {
    stock: u32,
    input: u32,
    output: u32,
}

/// A product valued on the stock accounts, by average cost.
fn valued_product(env: &mut Environment) -> Result<(u32, Accounts)> {
    let stock = xml_id(env, "account_test_chart.a_stock");
    let input = create(
        env,
        "account",
        json!({"code": "449100", "name": "Goods received not billed", "account_type": "liability_current"}),
    )?;
    let output = create(
        env,
        "account",
        json!({"code": "609400", "name": "Cost of goods sold", "account_type": "expense"}),
    )?;
    let category = create(
        env,
        "product_category",
        json!({"name": "Valued", "cost_method": "average",
        "stock_valuation_account": stock, "stock_input_account": input, "stock_output_account": output}),
    )?;
    let product = create(
        env,
        "product",
        json!({"name": "Desk", "standard_price": "100", "category": category}),
    )?;
    Ok((
        product,
        Accounts {
            stock,
            input,
            output,
        },
    ))
}

/// A validated transfer of the warehouse's `kind` with one move.
fn transfer(
    env: &mut Environment,
    kind: &str,
    product: u32,
    quantity: &str,
    price: &str,
) -> Result<u32> {
    let warehouse = xml_id(env, "stock.warehouse_main");
    let picking_type = read(env, "stock_warehouse", warehouse, &[kind])?[kind]
        .as_u64()
        .expect("a type") as u32;
    let picking = create(
        env,
        "stock_picking",
        json!({"picking_type": picking_type, "moves": {"create": [
        {"name": "Desks", "product": product, "product_uom_qty": quantity, "price_unit": price}]}}),
    )?;
    call(env, "stock_picking", "button_validate", &[picking])?;
    Ok(picking)
}

/// The balance of an account over its posted items.
fn balance(env: &mut Environment, account: u32) -> Result<Decimal> {
    let rows = env.call_rpc("account_move_line", "read_matching", &json!({
        "domain": [["account", "=", account], ["parent_state", "=", "posted"]], "fields": ["balance"]}))?;
    Ok(rows
        .as_array()
        .expect("rows")
        .iter()
        .map(|row| amount(&row["balance"]))
        .sum())
}

fn stock_value(env: &mut Environment, product: u32) -> Result<Decimal> {
    Ok(amount(
        &read(env, "product", product, &["stock_value"])?["stock_value"],
    ))
}

/// A receipt debits the stock and credits the goods received; a delivery debits their cost.
/// The stock account is always worth what the stock is.
#[test]
fn test_receipts_and_deliveries_are_booked() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let (desk, accounts) = valued_product(&mut env)?;
    let receipt = transfer(&mut env, "in_type", desk, "10", "120")?;
    let moves = read(&mut env, "stock_picking", receipt, &["moves"])?["moves"].clone();
    let entry = read(
        &mut env,
        "stock_move",
        moves[0].as_u64().expect("a move") as u32,
        &["account_move"],
    )?["account_move"]
        .as_u64()
        .expect("its entry") as u32;
    let row = read(
        &mut env,
        "account_move",
        entry,
        &["state", "journal", "amount_total"],
    )?;
    assert_eq!(row["state"], json!("posted"));
    assert_eq!(
        row["journal"],
        json!(xml_id(&mut env, "stock_account.journal_stock"))
    );
    assert_eq!(amount(&row["amount_total"]), d("1200"));
    assert_eq!(balance(&mut env, accounts.stock)?, d("1200"));
    assert_eq!(balance(&mut env, accounts.input)?, d("-1200"));

    transfer(&mut env, "out_type", desk, "4", "0")?;
    assert_eq!(balance(&mut env, accounts.stock)?, d("720"));
    assert_eq!(
        balance(&mut env, accounts.output)?,
        d("480"),
        "4 desks at 120"
    );
    assert_eq!(
        balance(&mut env, accounts.stock)?,
        stock_value(&mut env, desk)?
    );
    account::invariants::check_books(&mut env)
}

/// A customer's return takes back the cost of what was sold; a return to the vendor takes back
/// the goods received.
#[test]
fn test_returns_are_booked() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let (desk, accounts) = valued_product(&mut env)?;
    let receipt = transfer(&mut env, "in_type", desk, "10", "120")?;
    let delivery = transfer(&mut env, "out_type", desk, "2", "0")?;
    let back = call(&mut env, "stock_picking", "action_return", &[delivery])?["id"]
        .as_u64()
        .expect("a return") as u32;
    call(&mut env, "stock_picking", "button_validate", &[back])?;
    assert_eq!(balance(&mut env, accounts.output)?, d("0"));
    let to_vendor = call(&mut env, "stock_picking", "action_return", &[receipt])?["id"]
        .as_u64()
        .expect("a return") as u32;
    let moves = read(&mut env, "stock_picking", to_vendor, &["moves"])?["moves"].clone();
    env.call_rpc(
        "stock_move",
        "write",
        &json!({"ids": moves, "values": {"quantity": "3"}}),
    )?;
    call(&mut env, "stock_picking", "button_validate", &[to_vendor])?;
    assert_eq!(
        balance(&mut env, accounts.input)?,
        d("-840"),
        "7 of 10 received kept"
    );
    assert_eq!(balance(&mut env, accounts.stock)?, d("840"));
    assert_eq!(
        balance(&mut env, accounts.stock)?,
        stock_value(&mut env, desk)?
    );
    account::invariants::check_books(&mut env)
}

/// Counting less than the stock books a loss, counting more a gain.
#[test]
fn test_inventory_differences_are_booked() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let (desk, accounts) = valued_product(&mut env)?;
    transfer(&mut env, "in_type", desk, "10", "120")?;
    let quant = env.call_rpc(
        "stock_quant",
        "search",
        &json!({"domain": [["product", "=", desk]]}),
    )?[0]
        .as_u64()
        .expect("a quant") as u32;
    env.call_rpc(
        "stock_quant",
        "write",
        &json!({"ids": [quant], "values": {"inventory_quantity": "8"}}),
    )?;
    call(&mut env, "stock_quant", "action_apply_inventory", &[quant])?;
    assert_eq!(balance(&mut env, accounts.output)?, d("240"));
    env.call_rpc(
        "stock_quant",
        "write",
        &json!({"ids": [quant], "values": {"inventory_quantity": "9"}}),
    )?;
    call(&mut env, "stock_quant", "action_apply_inventory", &[quant])?;
    assert_eq!(balance(&mut env, accounts.output)?, d("120"));
    assert_eq!(balance(&mut env, accounts.stock)?, d("1080"));
    assert_eq!(
        balance(&mut env, accounts.stock)?,
        stock_value(&mut env, desk)?
    );
    account::invariants::check_books(&mut env)
}

/// A category not valued in accounting books nothing; one valued without its input and output
/// accounts refuses the move, which does not happen.
#[test]
fn test_categories_not_valued() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let plain = create(
        &mut env,
        "product",
        json!({"name": "Pen", "standard_price": "1"}),
    )?;
    let receipt = transfer(&mut env, "in_type", plain, "5", "1")?;
    let moves = read(&mut env, "stock_picking", receipt, &["moves"])?["moves"].clone();
    let entry = read(
        &mut env,
        "stock_move",
        moves[0].as_u64().expect("a move") as u32,
        &["account_move"],
    )?["account_move"]
        .clone();
    assert_eq!(entry, json!(null));
    let stock = xml_id(&mut env, "account_test_chart.a_stock");
    let half = create(
        &mut env,
        "product_category",
        json!({"name": "Half", "stock_valuation_account": stock}),
    )?;
    let lamp = create(
        &mut env,
        "product",
        json!({"name": "Lamp", "standard_price": "10", "category": half}),
    )?;
    let error = transfer(&mut env, "in_type", lamp, "1", "10")
        .expect_err("half set up")
        .to_string();
    assert!(error.contains("input and output"), "{error}");
    assert_eq!(
        amount(&read(&mut env, "product", lamp, &["qty_available"])?["qty_available"]),
        d("0")
    );
    Ok(())
}

/// The category form shows its stock accounts.
#[test]
fn test_views() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let form = env.get_empty_record::<base::models::View<_>>().load(
        &mut env,
        "product_category".to_string(),
        "form".to_string(),
    )?;
    assert!(form.contains("stock_valuation_account"), "{form}");
    Ok(())
}
