use code_gen::{Model, erp_methods, selection};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{Decimal, FieldType, IdMode, MultipleIds, Reference, SingleId};
use erp::types::model::MapOfFields;
use erp_search_code_gen::make_domain;
use product::models::{Product, ProductType};
use sale::models::{SaleOrder, SaleOrderLine};
use stock::models::{
    BaseStockPicking, PickingState, StockMove, StockPicking, StockPickingType, StockWarehouse,
};

#[selection]
pub enum DeliveryStatus {
    #[default]
    #[selection(label = "Nothing to deliver")]
    No,
    #[selection(label = "To deliver")]
    Pending,
    #[selection(label = "Partially delivered")]
    Partial,
    #[selection(label = "Delivered")]
    Full,
}

/// A sales order's deliveries.
#[derive(Model)]
#[erp(id = "sale_order", methods)]
#[erp(derived_model = "sale::models")]
#[allow(dead_code)]
pub struct SaleOrderSaleStock<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Deliveries", compute = "compute_pickings", depends = ["lines.moves.picking"])]
    pickings: Reference<BaseStockPicking, MultipleIds>,
    #[erp(label = "Delivery status", compute = "compute_delivery_status", depends = ["lines.qty_delivered", "state"], stored)]
    delivery_status: DeliveryStatus,
}

/// An order line of goods, the quantity to deliver, and its product.
type Deliverable = (SaleOrderLine<SingleId>, Decimal, Product<SingleId>);

/// What of the order's lines is goods to deliver.
fn deliverable(env: &mut Environment, order: &SaleOrder<SingleId>) -> Result<Vec<Deliverable>> {
    let lines: SaleOrderLine<MultipleIds> = order.get_lines(env)?;
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
        let quantity = *line.get_product_uom_qty(env)?;
        if quantity > Decimal::ZERO {
            rows.push((line, quantity, product));
        }
    }
    Ok(rows)
}

#[erp_methods]
impl SaleOrderSaleStock<MultipleIds> {
    /// The transfers delivering the order, or bringing it back.
    pub fn compute_pickings(&self, env: &mut Environment) -> Result<()> {
        for order in self {
            let order_record: SaleOrder<SingleId> = order.as_model();
            let lines: SaleOrderLine<MultipleIds> = order_record.get_lines(env)?;
            let line_ids = lines.get_ids_ref().clone();
            let pickings: StockPicking<MultipleIds> = env.sudo_with(|env| {
                let moves: StockMove<MultipleIds> =
                    env.search(&make_domain!([("sale_line", "in", line_ids)]))?;
                moves.get_picking(env)
            })?;
            order.set_pickings(&pickings, env)?;
        }
        Ok(())
    }

    /// Nothing, part or all of the goods ordered delivered.
    pub fn compute_delivery_status(&self, env: &mut Environment) -> Result<()> {
        for order in self {
            let order_record: SaleOrder<SingleId> = order.as_model();
            let status = if !order_record.is_state(env, sale::models::SaleState::Sale)? {
                DeliveryStatus::No
            } else {
                let rows = deliverable(env, &order_record)?;
                if rows.is_empty() {
                    DeliveryStatus::No
                } else {
                    let mut any = false;
                    let mut all = true;
                    for (line, quantity, _) in &rows {
                        let delivered = *line.get_qty_delivered(env)?;
                        any |= delivered > Decimal::ZERO;
                        all &= delivered >= *quantity;
                    }
                    match (any, all) {
                        (_, true) => DeliveryStatus::Full,
                        (true, false) => DeliveryStatus::Partial,
                        _ => DeliveryStatus::Pending,
                    }
                }
            };
            order.set_delivery_status(status, env)?;
        }
        Ok(())
    }

    /// A confirmed order's goods are to be delivered from the main warehouse, in one delivery.
    pub fn on_confirmed(&self, env: &mut Environment, sup: Super) -> Result<()> {
        sup.call(env)?;
        for order in self {
            let order: SaleOrder<SingleId> = order.as_model();
            let rows = deliverable(env, &order)?;
            if rows.is_empty() {
                continue;
            }
            let warehouse = StockWarehouse::main(env)?;
            if warehouse.is_empty() {
                return Err("No warehouse delivers the order".into());
            }
            let delivery_type: StockPickingType<SingleId> =
                warehouse.get_out_type(&mut env.sudo())?;
            let partner: base::models::Contact<SingleId> = order.get_partner(env)?;
            let mut moves = Vec::new();
            for (line, quantity, product) in rows {
                let uom: uom::models::Uom<SingleId> = line.get_uom(env)?;
                let mut values = MapOfFields::default();
                values.insert("name", line.get_name(env)?.cloned().unwrap_or_default());
                values.insert("product", product.get_id());
                values.insert("product_uom_qty", quantity);
                if let Some(uom) = uom.get_optional_id() {
                    values.insert("uom", uom);
                }
                values.insert("sale_line", line.get_id());
                moves.push(values);
            }
            let mut picking = MapOfFields::default();
            picking.insert("picking_type", delivery_type.get_id());
            picking.insert("partner", partner.get_id());
            picking.insert("origin", order.get_name(env)?.clone());
            picking.insert_field_type(
                "moves",
                FieldType::Commands(vec![erp::types::field::Command::Create(moves)]),
            );
            let env = &mut *env.sudo();
            let picking: StockPicking<SingleId> = env.create_new_record_from_map(picking)?;
            StockPicking::<MultipleIds>::from_ids(vec![picking.get_id()], env)
                .action_confirm(env)?;
        }
        Ok(())
    }

    /// Cancelling an order cancels the deliveries not done; one delivered must come back first.
    pub fn on_cancelled(&self, env: &mut Environment, sup: Super) -> Result<()> {
        for order in self {
            let pickings: StockPicking<MultipleIds> = order.get_pickings(env)?;
            let env = &mut *env.sudo();
            let mut open = Vec::new();
            for picking in &pickings {
                if picking.is_state(env, PickingState::Done)? {
                    return Err(format!(
                        "{} is delivered: return the goods before cancelling the order",
                        picking.get_name(env)?
                    )
                    .into());
                }
                if !picking.is_state(env, PickingState::Cancel)? {
                    open.push(picking.get_id());
                }
            }
            if !open.is_empty() {
                StockPicking::<MultipleIds>::from_ids(open, env).action_cancel(env)?;
            }
        }
        sup.call(env)
    }
}
