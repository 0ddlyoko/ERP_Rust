use code_gen::{Model, erp_methods, selection};
use currency::models::Currency;
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{Command, Decimal, FieldType, IdMode, MultipleIds, Reference, SingleId};
use erp::types::model::MapOfFields;
use erp_search_code_gen::make_domain;
use product::models::{Product, ProductType};
use purchase::models::{PurchaseOrder, PurchaseOrderLine, PurchaseState};
use stock::models::{BaseStockPicking, Picking, PickingState, PickingType, StockMove, Warehouse};
use uom::models::Uom;

#[selection]
pub enum ReceiptStatus {
    #[default]
    #[selection(label = "Nothing to receive")]
    No,
    #[selection(label = "To receive")]
    Pending,
    #[selection(label = "Partially received")]
    Partial,
    #[selection(label = "Received")]
    Full,
}

/// A purchase order's receipts.
#[derive(Model)]
#[erp(id = "purchase_order", methods)]
#[erp(derived_model = "purchase::models")]
#[allow(dead_code)]
pub struct PurchaseOrderStock<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Receipts", compute = "compute_pickings", depends = ["lines.moves.picking"])]
    pickings: Reference<BaseStockPicking, MultipleIds>,
    #[erp(
        label = "Receipt status",
        compute = "compute_receipt_status",
        depends = ["lines.qty_received", "state"],
        stored
    )]
    receipt_status: ReceiptStatus,
}

/// An order line of goods, the quantity to receive, and its product.
type Receivable = (PurchaseOrderLine<SingleId>, Decimal, Product<SingleId>);

/// What of the order's lines is goods to receive.
fn receivable(env: &mut Environment, order: &PurchaseOrder<SingleId>) -> Result<Vec<Receivable>> {
    let lines: PurchaseOrderLine<MultipleIds> = order.get_lines(env)?;
    let mut rows = Vec::new();
    for line in &lines {
        let product: Product<SingleId> = line.get_product(env)?;
        if product.is_empty()
            || matches!(
                *product.get_product_type(&mut env.sudo())?,
                ProductType::Service
            )
        {
            continue;
        }
        let quantity = *line.get_product_qty(env)?;
        if quantity > Decimal::ZERO {
            rows.push((line, quantity, product));
        }
    }
    Ok(rows)
}

/// What one unit of the line's product cost, in the company's currency and the product's unit:
/// the untaxed amount of the line, discount taken, over its quantity.
fn unit_cost(
    env: &mut Environment,
    order: &PurchaseOrder<SingleId>,
    line: &PurchaseOrderLine<SingleId>,
    product: &Product<SingleId>,
) -> Result<Decimal> {
    let quantity = *line.get_product_qty(env)?;
    if quantity.is_zero() {
        return Ok(Decimal::ZERO);
    }
    let mut subtotal = *line.get_price_subtotal(env)?;
    let currency = order.currency_or_company(env)?;
    let company = Currency::of_company(env)?;
    if currency.get_id() != company.get_id() {
        let date = *order.get_date_order(env)?;
        subtotal = currency.convert(env, subtotal, company, date)?;
    }
    let mut cost = subtotal / quantity;
    let env = &mut *env.sudo();
    let line_uom: Uom<SingleId> = line.get_uom(env)?;
    let product_uom: Uom<SingleId> = product.get_uom(env)?;
    if !line_uom.is_empty() && !product_uom.is_empty() {
        cost = line_uom.convert_price(env, cost, product_uom)?;
    }
    Ok(cost.round_dp(6))
}

#[erp_methods]
impl PurchaseOrderStock<MultipleIds> {
    /// The transfers receiving the order, or sending it back.
    pub fn compute_pickings(&self, env: &mut Environment) -> Result<()> {
        for order in self {
            let order_record: PurchaseOrder<SingleId> = order.as_model();
            let lines: PurchaseOrderLine<MultipleIds> = order_record.get_lines(env)?;
            let line_ids = lines.get_ids_ref().clone();
            let pickings: Picking<MultipleIds> = env.sudo_with(|env| {
                let moves: StockMove<MultipleIds> =
                    env.search(&make_domain!([("purchase_line", "in", line_ids)]))?;
                moves.get_picking(env)
            })?;
            order.set_pickings(&pickings, env)?;
        }
        Ok(())
    }

    /// Nothing, part or all of the goods ordered received.
    pub fn compute_receipt_status(&self, env: &mut Environment) -> Result<()> {
        for order in self {
            let order_record: PurchaseOrder<SingleId> = order.as_model();
            let status = if !order_record.is_state(env, PurchaseState::Purchase)? {
                ReceiptStatus::No
            } else {
                let rows = receivable(env, &order_record)?;
                if rows.is_empty() {
                    ReceiptStatus::No
                } else {
                    let mut any = false;
                    let mut all = true;
                    for (line, quantity, _) in &rows {
                        let received = *line.get_qty_received(env)?;
                        any |= received > Decimal::ZERO;
                        all &= received >= *quantity;
                    }
                    match (any, all) {
                        (_, true) => ReceiptStatus::Full,
                        (true, false) => ReceiptStatus::Partial,
                        _ => ReceiptStatus::Pending,
                    }
                }
            };
            order.set_receipt_status(status, env)?;
        }
        Ok(())
    }

    /// A confirmed order's goods are to be received in the main warehouse, in one receipt,
    /// valued at what they cost.
    pub fn on_confirmed(&self, env: &mut Environment, sup: Super) -> Result<()> {
        sup.call(env)?;
        for order in self {
            let order: PurchaseOrder<SingleId> = order.as_model();
            let rows = receivable(env, &order)?;
            if rows.is_empty() {
                continue;
            }
            let warehouse = Warehouse::main(env)?;
            if warehouse.is_empty() {
                return Err("No warehouse receives the order".into());
            }
            let receipt_type: PickingType<SingleId> = warehouse.get_in_type(&mut env.sudo())?;
            let partner: base::models::Contact<SingleId> = order.get_partner(env)?;
            let mut moves = Vec::new();
            for (line, quantity, product) in rows {
                let uom: Uom<SingleId> = line.get_uom(env)?;
                let mut values = MapOfFields::default();
                values.insert("name", line.get_name(env)?.cloned().unwrap_or_default());
                values.insert("product", product.get_id());
                values.insert("product_uom_qty", quantity);
                if let Some(uom) = uom.get_optional_id() {
                    values.insert("uom", uom);
                }
                values.insert("price_unit", unit_cost(env, &order, &line, &product)?);
                values.insert("purchase_line", line.get_id());
                moves.push(values);
            }
            let mut picking = MapOfFields::default();
            picking.insert("picking_type", receipt_type.get_id());
            picking.insert("partner", partner.get_id());
            picking.insert("origin", order.get_name(env)?.clone());
            picking.insert_option("scheduled_date", order.get_date_planned(env)?.copied());
            picking.insert_field_type("moves", FieldType::Commands(vec![Command::Create(moves)]));
            let env = &mut *env.sudo();
            let picking: Picking<SingleId> = env.create_new_record_from_map(picking)?;
            Picking::<MultipleIds>::from_ids(vec![picking.get_id()], env).action_confirm(env)?;
        }
        Ok(())
    }

    /// Cancelling an order cancels the receipts not done; one received must go back first.
    pub fn on_cancelled(&self, env: &mut Environment, sup: Super) -> Result<()> {
        for order in self {
            let pickings: Picking<MultipleIds> = order.get_pickings(env)?;
            let env = &mut *env.sudo();
            let mut open = Vec::new();
            for picking in &pickings {
                if picking.is_state(env, PickingState::Done)? {
                    return Err(format!(
                        "{} is received: return the goods before cancelling the order",
                        picking.get_name(env)?
                    )
                    .into());
                }
                if !picking.is_state(env, PickingState::Cancel)? {
                    open.push(picking.get_id());
                }
            }
            if !open.is_empty() {
                Picking::<MultipleIds>::from_ids(open, env).action_cancel(env)?;
            }
        }
        sup.call(env)
    }
}
