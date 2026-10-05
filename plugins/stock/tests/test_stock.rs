//! Inventory: receipts, deliveries, transfers, counts and returns — the quantities on hand and
//! the value of the stock checked at every step, by average cost, FIFO and standard price.

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
use sequence::SequencePlugin;
use serde_json::{Value, json};
use stock::StockPlugin;
use uom::UomPlugin;
use web::WebPlugin;

fn new_app() -> Result<Application> {
    erp_test_support::app(
        vec![
            Box::new(BasePlugin {}),
            Box::new(WebPlugin {}),
            Box::new(MailPlugin {}),
            Box::new(ContactsPlugin {}),
            Box::new(UomPlugin {}),
            Box::new(CurrencyPlugin {}),
            Box::new(SequencePlugin {}),
            Box::new(ProductPlugin {}),
            Box::new(StockPlugin {}),
        ],
        "stock",
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

fn warehouse_field(env: &mut Environment, field: &str) -> Result<u32> {
    let warehouse = xml_id(env, "stock.warehouse_main");
    Ok(read(env, "stock_warehouse", warehouse, &[field])?[field]
        .as_u64()
        .expect("set") as u32)
}

/// A product of a category valued by `method`, at `cost`.
fn goods(env: &mut Environment, name: &str, method: &str, cost: &str) -> Result<u32> {
    let category = create(
        env,
        "product_category",
        json!({"name": format!("{name} goods"), "cost_method": method}),
    )?;
    create(
        env,
        "product",
        json!({"name": name, "standard_price": cost, "category": category}),
    )
}

/// A transfer of the warehouse's `kind` (in_type, out_type, int_type), one move per line.
fn transfer(env: &mut Environment, kind: &str, lines: &[(u32, &str, &str)]) -> Result<u32> {
    let picking_type = warehouse_field(env, kind)?;
    let moves: Vec<Value> = lines
        .iter()
        .map(|(product, quantity, price)| json!({"name": "Move", "product": product, "product_uom_qty": quantity, "price_unit": price}))
        .collect();
    create(
        env,
        "stock_picking",
        json!({"picking_type": picking_type, "moves": {"create": moves}}),
    )
}

fn transfer_and_validate(
    env: &mut Environment,
    kind: &str,
    lines: &[(u32, &str, &str)],
) -> Result<u32> {
    let picking = transfer(env, kind, lines)?;
    validate(env, picking)?;
    Ok(picking)
}

fn validate(env: &mut Environment, picking: u32) -> Result<Value> {
    call(env, "stock_picking", "action_confirm", &[picking])?;
    call(env, "stock_picking", "button_validate", &[picking])
}

/// On hand and stock value of a product.
fn stock(env: &mut Environment, product: u32) -> Result<(Decimal, Decimal)> {
    let row = read(env, "product", product, &["qty_available", "stock_value"])?;
    Ok((amount(&row["qty_available"]), amount(&row["stock_value"])))
}

fn cost(env: &mut Environment, product: u32) -> Result<Decimal> {
    Ok(amount(
        &read(env, "product", product, &["standard_price"])?["standard_price"],
    ))
}

/// The main warehouse comes with its stock location and its numbered operations.
#[test]
fn test_the_warehouse() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let stock_location = warehouse_field(&mut env, "lot_stock")?;
    assert_eq!(
        read(
            &mut env,
            "stock_location",
            stock_location,
            &["complete_name"]
        )?["complete_name"],
        json!("WH/Stock")
    );
    let paper = goods(&mut env, "Paper", "average", "1")?;
    let receipt = transfer(&mut env, "in_type", &[(paper, "1", "1")])?;
    let delivery = transfer(&mut env, "out_type", &[(paper, "1", "0")])?;
    let internal = transfer(&mut env, "int_type", &[(paper, "1", "0")])?;
    assert_eq!(
        read(&mut env, "stock_picking", receipt, &["name"])?["name"],
        json!("WH/IN/00001")
    );
    assert_eq!(
        read(&mut env, "stock_picking", delivery, &["name"])?["name"],
        json!("WH/OUT/00001")
    );
    assert_eq!(
        read(&mut env, "stock_picking", internal, &["name"])?["name"],
        json!("WH/INT/00001")
    );
    let row = read(
        &mut env,
        "stock_picking",
        receipt,
        &["location", "location_dest", "state"],
    )?;
    assert_eq!(row["location_dest"], json!(stock_location));
    assert_eq!(
        row["location"],
        json!(xml_id(&mut env, "stock.location_suppliers"))
    );
    assert_eq!(row["state"], json!("draft"));
    Ok(())
}

/// Receipts at their cost raise the stock and its average cost; a delivery takes out at it.
#[test]
fn test_average_cost() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let ink = goods(&mut env, "Ink", "average", "0")?;
    let first = transfer(&mut env, "in_type", &[(ink, "10", "4")])?;
    validate(&mut env, first)?;
    assert_eq!(stock(&mut env, ink)?, (d("10"), d("40")));
    assert_eq!(cost(&mut env, ink)?, d("4"));
    let second = transfer(&mut env, "in_type", &[(ink, "10", "6")])?;
    validate(&mut env, second)?;
    assert_eq!(stock(&mut env, ink)?, (d("20"), d("100")));
    assert_eq!(cost(&mut env, ink)?, d("5"));
    let delivery = transfer(&mut env, "out_type", &[(ink, "5", "0")])?;
    call(&mut env, "stock_picking", "action_confirm", &[delivery])?;
    assert_eq!(
        read(&mut env, "stock_picking", delivery, &["state"])?["state"],
        json!("assigned")
    );
    assert_eq!(
        amount(&read(&mut env, "product", ink, &["free_qty"])?["free_qty"]),
        d("15"),
        "5 promised"
    );
    call(&mut env, "stock_picking", "button_validate", &[delivery])?;
    assert_eq!(stock(&mut env, ink)?, (d("15"), d("75")));
    let moves = read(&mut env, "stock_picking", delivery, &["moves", "state"])?;
    assert_eq!(moves["state"], json!("done"));
    let out_move = moves["moves"][0].as_u64().expect("a move") as u32;
    assert_eq!(
        amount(&read(&mut env, "stock_move", out_move, &["value"])?["value"]),
        d("-25")
    );
    let rest = transfer(&mut env, "out_type", &[(ink, "15", "0")])?;
    validate(&mut env, rest)?;
    assert_eq!(
        stock(&mut env, ink)?,
        (d("0"), d("0")),
        "nothing left, worth nothing"
    );
    Ok(())
}

/// FIFO takes out the oldest units at what they cost.
#[test]
fn test_fifo() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let tea = goods(&mut env, "Tea", "fifo", "0")?;
    transfer_and_validate(&mut env, "in_type", &[(tea, "10", "4")])?;
    transfer_and_validate(&mut env, "in_type", &[(tea, "10", "6")])?;
    let delivery = transfer(&mut env, "out_type", &[(tea, "15", "0")])?;
    validate(&mut env, delivery)?;
    assert_eq!(
        stock(&mut env, tea)?,
        (d("5"), d("30")),
        "10 at 4 and 5 at 6 gone"
    );
    assert_eq!(cost(&mut env, tea)?, d("6"), "what is left cost 6");
    Ok(())
}

/// At standard price, what was paid does not matter.
#[test]
fn test_standard_price() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let bolt = goods(&mut env, "Bolt", "standard", "0.50")?;
    transfer_and_validate(&mut env, "in_type", &[(bolt, "100", "0.70")])?;
    assert_eq!(stock(&mut env, bolt)?, (d("100"), d("50")));
    transfer_and_validate(&mut env, "out_type", &[(bolt, "30", "0")])?;
    assert_eq!(stock(&mut env, bolt)?, (d("70"), d("35")));
    assert_eq!(cost(&mut env, bolt)?, d("0.5"));
    Ok(())
}

/// What is not there cannot leave; what is, leaves, and the rest waits in a back order.
#[test]
fn test_shortages_and_back_orders() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let lamp = goods(&mut env, "Lamp", "average", "10")?;
    transfer_and_validate(&mut env, "in_type", &[(lamp, "4", "10")])?;
    let delivery = transfer(&mut env, "out_type", &[(lamp, "10", "0")])?;
    call(&mut env, "stock_picking", "action_confirm", &[delivery])?;
    assert_eq!(
        read(&mut env, "stock_picking", delivery, &["state"])?["state"],
        json!("confirmed"),
        "waiting: 4 of 10"
    );
    let error = call(&mut env, "stock_picking", "button_validate", &[delivery])
        .expect_err("short")
        .to_string();
    assert!(error.contains("Only 4"), "{error}");
    assert_eq!(stock(&mut env, lamp)?, (d("4"), d("40")), "nothing moved");
    let moves = read(&mut env, "stock_picking", delivery, &["moves"])?["moves"].clone();
    env.call_rpc(
        "stock_move",
        "write",
        &json!({"ids": moves, "values": {"quantity": "4"}}),
    )?;
    let answer = call(&mut env, "stock_picking", "button_validate", &[delivery])?;
    let backorder = answer["id"].as_u64().expect("a back order") as u32;
    let row = read(
        &mut env,
        "stock_picking",
        backorder,
        &["backorder", "state", "moves"],
    )?;
    assert_eq!(row["backorder"], json!(delivery));
    assert_eq!(row["state"], json!("confirmed"));
    let rest = row["moves"][0].as_u64().expect("a move") as u32;
    assert_eq!(
        amount(&read(&mut env, "stock_move", rest, &["product_uom_qty"])?["product_uom_qty"]),
        d("6")
    );
    assert_eq!(stock(&mut env, lamp)?, (d("0"), d("0")));
    transfer_and_validate(&mut env, "in_type", &[(lamp, "6", "10")])?;
    call(&mut env, "stock_picking", "action_assign", &[backorder])?;
    assert_eq!(
        read(&mut env, "stock_picking", backorder, &["state"])?["state"],
        json!("assigned")
    );
    call(&mut env, "stock_picking", "button_validate", &[backorder])?;
    assert_eq!(stock(&mut env, lamp)?, (d("0"), d("0")));
    Ok(())
}

/// An internal transfer moves stock between shelves without changing its value.
#[test]
fn test_internal_transfers() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let chair = goods(&mut env, "Chair", "average", "20")?;
    transfer_and_validate(&mut env, "in_type", &[(chair, "5", "20")])?;
    let stock_location = warehouse_field(&mut env, "lot_stock")?;
    let shelf = create(
        &mut env,
        "stock_location",
        json!({"name": "Shelf 1", "parent": stock_location}),
    )?;
    assert_eq!(
        read(&mut env, "stock_location", shelf, &["complete_name"])?["complete_name"],
        json!("WH/Stock/Shelf 1")
    );
    let picking_type = warehouse_field(&mut env, "int_type")?;
    let internal = create(
        &mut env,
        "stock_picking",
        json!({"picking_type": picking_type, "location_dest": shelf,
        "moves": {"create": [{"name": "To the shelf", "product": chair, "product_uom_qty": "2"}]}}),
    )?;
    validate(&mut env, internal)?;
    assert_eq!(
        stock(&mut env, chair)?,
        (d("5"), d("100")),
        "the same stock, worth the same"
    );
    let quants = env.call_rpc(
        "stock_quant",
        "search",
        &json!({"domain": [["product", "=", chair], ["location", "=", shelf]]}),
    )?;

    let quant = quants[0].as_u64().expect("a quant") as u32;
    assert_eq!(
        amount(&read(&mut env, "stock_quant", quant, &["quantity"])?["quantity"]),
        d("2")
    );
    Ok(())
}

/// Counting adjusts the stock and its value both ways; a negative count is refused.
#[test]
fn test_physical_inventory() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let pen = goods(&mut env, "Pen", "average", "2")?;
    transfer_and_validate(&mut env, "in_type", &[(pen, "10", "2")])?;
    let quants = env.call_rpc(
        "stock_quant",
        "search",
        &json!({"domain": [["product", "=", pen]]}),
    )?;
    let quant = quants[0].as_u64().expect("a quant") as u32;
    env.call_rpc(
        "stock_quant",
        "write",
        &json!({"ids": [quant], "values": {"inventory_quantity": "7"}}),
    )?;
    assert_eq!(
        amount(&read(&mut env, "stock_quant", quant, &["inventory_diff"])?["inventory_diff"]),
        d("-3")
    );
    call(&mut env, "stock_quant", "action_apply_inventory", &[quant])?;
    assert_eq!(stock(&mut env, pen)?, (d("7"), d("14")));
    assert_eq!(
        read(&mut env, "stock_quant", quant, &["inventory_quantity"])?["inventory_quantity"],
        json!(null)
    );
    env.call_rpc(
        "stock_quant",
        "write",
        &json!({"ids": [quant], "values": {"inventory_quantity": "12"}}),
    )?;
    call(&mut env, "stock_quant", "action_apply_inventory", &[quant])?;
    assert_eq!(
        stock(&mut env, pen)?,
        (d("12"), d("24")),
        "found at the average cost"
    );
    env.call_rpc(
        "stock_quant",
        "write",
        &json!({"ids": [quant], "values": {"inventory_quantity": "-1"}}),
    )?;
    assert!(call(&mut env, "stock_quant", "action_apply_inventory", &[quant]).is_err());
    Ok(())
}

/// A delivery returned comes back into stock at the value it left at.
#[test]
fn test_returns() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let mug = goods(&mut env, "Mug", "fifo", "0")?;
    transfer_and_validate(&mut env, "in_type", &[(mug, "5", "3")])?;
    let delivery = transfer(&mut env, "out_type", &[(mug, "2", "0")])?;
    validate(&mut env, delivery)?;
    assert_eq!(stock(&mut env, mug)?, (d("3"), d("9")));
    let answer = call(&mut env, "stock_picking", "action_return", &[delivery])?;
    let returned = answer["id"].as_u64().expect("a return") as u32;
    let row = read(
        &mut env,
        "stock_picking",
        returned,
        &["returned_picking", "origin", "location_dest", "name"],
    )?;
    assert_eq!(row["returned_picking"], json!(delivery));
    assert_eq!(row["origin"], json!("Return of WH/OUT/00001"));
    assert_eq!(
        row["location_dest"],
        json!(warehouse_field(&mut env, "lot_stock")?)
    );
    call(&mut env, "stock_picking", "button_validate", &[returned])?;
    assert_eq!(stock(&mut env, mug)?, (d("5"), d("15")));
    let undone = transfer(&mut env, "out_type", &[(mug, "1", "0")])?;
    assert!(
        call(&mut env, "stock_picking", "action_return", &[undone]).is_err(),
        "nothing done to return"
    );
    Ok(())
}

/// A cancelled transfer releases what it promised; a done one cannot be cancelled nor changed.
#[test]
fn test_cancelling() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let book = goods(&mut env, "Book", "average", "8")?;
    transfer_and_validate(&mut env, "in_type", &[(book, "3", "8")])?;
    let delivery = transfer(&mut env, "out_type", &[(book, "3", "0")])?;
    call(&mut env, "stock_picking", "action_confirm", &[delivery])?;
    assert_eq!(
        amount(&read(&mut env, "product", book, &["free_qty"])?["free_qty"]),
        d("0")
    );
    call(&mut env, "stock_picking", "action_cancel", &[delivery])?;
    assert_eq!(
        amount(&read(&mut env, "product", book, &["free_qty"])?["free_qty"]),
        d("3"),
        "released"
    );
    let done = transfer(&mut env, "out_type", &[(book, "1", "0")])?;
    validate(&mut env, done)?;
    assert!(call(&mut env, "stock_picking", "action_cancel", &[done]).is_err());
    let moves = read(&mut env, "stock_picking", done, &["moves"])?["moves"].clone();
    assert!(
        env.call_rpc(
            "stock_move",
            "write",
            &json!({"ids": moves, "values": {"quantity": "5"}})
        )
        .is_err()
    );
    assert!(
        env.call_rpc("stock_picking", "delete", &json!({"ids": [done]}))
            .is_err()
    );
    let (source, destination) = (
        warehouse_field(&mut env, "lot_stock")?,
        xml_id(&mut env, "stock.location_customers"),
    );
    assert!(
        create(
            &mut env,
            "stock_move",
            json!({"name": "Minus", "product": book, "product_uom_qty": "-1",
        "location": source, "location_dest": destination})
        )
        .is_err()
    );
    Ok(())
}

/// Received by the dozen, the stock counts units.
#[test]
fn test_units_of_measure() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let egg = goods(&mut env, "Egg", "average", "0.2")?;
    let dozen = xml_id(&mut env, "uom.uom_dozen");
    let picking_type = warehouse_field(&mut env, "in_type")?;
    let receipt = create(
        &mut env,
        "stock_picking",
        json!({"picking_type": picking_type, "moves": {"create": [
        {"name": "Eggs", "product": egg, "product_uom_qty": "3", "uom": dozen, "price_unit": "0.25"}]}}),
    )?;
    validate(&mut env, receipt)?;
    assert_eq!(stock(&mut env, egg)?.0, d("36"));
    Ok(())
}

/// Inventory users move goods; other employees only see the locations.
#[test]
fn test_access_rights() -> Result<()> {
    let app = new_app()?;
    let paper = {
        let mut env = admin_env(&app)?;
        let paper = goods(&mut env, "Paper", "average", "1")?;
        env.close()?;
        paper
    };
    {
        let mut env = user_env(&app, "employee", &["base.group_user"])?;
        assert!(transfer(&mut env, "in_type", &[(paper, "1", "1")]).is_err());
    }
    let mut env = user_env(
        &app,
        "keeper",
        &["base.group_user", "stock.group_stock_user"],
    )?;
    let receipt = transfer(&mut env, "in_type", &[(paper, "1", "1")])?;
    validate(&mut env, receipt)?;
    assert!(
        create(
            &mut env,
            "stock_warehouse",
            json!({"name": "Mine", "code": "MINE"})
        )
        .is_err()
    );
    Ok(())
}

/// The views resolve, and the application has its menus.
#[test]
fn test_views_and_menus() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    for (model, kinds) in [
        ("stock_picking", &["list", "form", "search"][..]),
        ("stock_move", &["list", "form", "search"]),
        ("stock_quant", &["list", "form", "search"]),
        ("stock_valuation_layer", &["list"]),
        ("stock_warehouse", &["list", "form"]),
        ("stock_location", &["list", "form"]),
        ("stock_picking_type", &["list"]),
        ("product", &["form"]),
        ("product_category", &["form"]),
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
    let receipts = env.call_rpc(
        "stock_picking",
        "search",
        &json!({"domain": [["picking_type.code", "=", "incoming"]]}),
    )?;
    assert_eq!(
        receipts,
        json!([]),
        "the receipts action's domain is searchable"
    );
    let tree = env.call_rpc("menu", "tree", &json!({}))?;
    let menu = tree
        .as_array()
        .expect("menus")
        .iter()
        .find(|entry| entry["name"] == "Inventory")
        .expect("the application");
    let sections: Vec<&str> = menu["children"]
        .as_array()
        .expect("sections")
        .iter()
        .filter_map(|entry| entry["name"].as_str())
        .collect();
    assert_eq!(
        sections,
        vec!["Operations", "Stock", "Reporting", "Configuration"]
    );
    Ok(())
}
