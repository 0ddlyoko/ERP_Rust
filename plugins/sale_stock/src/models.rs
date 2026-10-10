use code_gen::{Model, erp_methods, selection};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{Decimal, FieldType, IdMode, MultipleIds, Reference, SingleId};
use erp::types::model::MapOfFields;
use erp_search_code_gen::make_domain;
use product::models::{Product, ProductType};
use sale::models::{BaseSaleOrderLine, SaleOrder, SaleOrderLine};
use stock::models::{
    BaseStockMove, BaseStockPicking, Location, MoveStatus, Picking, PickingState, PickingType,
    StockMove, Warehouse,
};

/// The order line a move delivers.
#[derive(Model)]
#[erp(id = "stock_move", methods)]
#[erp(derived_model = "stock::models")]
#[allow(dead_code)]
pub struct StockMoveSale<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Sales order line", ondelete = "set_null")]
    sale_line: Reference<BaseSaleOrderLine, SingleId>,
}

#[erp_methods]
impl StockMoveSale<MultipleIds> {
    /// A back order or a return still delivers the same order line.
    pub fn copy_values(&self, env: &mut Environment, sup: Super) -> Result<MapOfFields> {
        let mut values: MapOfFields = sup.call(env)?;
        if let Some(stock_move) = self.into_iter().next() {
            let line: SaleOrderLine<SingleId> = stock_move.get_sale_line(&mut env.sudo())?;
            if let Some(line) = line.get_optional_id() {
                values.insert("sale_line", line);
            }
        }
        Ok(values)
    }
}

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

/// The moves delivering an order line.
#[derive(Model)]
#[erp(id = "sale_order_line")]
#[erp(derived_model = "sale::models")]
#[allow(dead_code)]
pub struct SaleOrderLineStock<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Stock moves", inverse = "sale_line")]
    moves: Reference<BaseStockMove, MultipleIds>,
}

/// A sales order's deliveries.
#[derive(Model)]
#[erp(id = "sale_order", methods)]
#[erp(derived_model = "sale::models")]
#[allow(dead_code)]
pub struct SaleOrderStock<Mode: IdMode> {
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
impl SaleOrderStock<MultipleIds> {
    /// The transfers delivering the order, or bringing it back.
    pub fn compute_pickings(&self, env: &mut Environment) -> Result<()> {
        for order in self {
            let order_record: SaleOrder<SingleId> = order.as_model();
            let lines: SaleOrderLine<MultipleIds> = order_record.get_lines(env)?;
            let line_ids = lines.get_ids_ref().clone();
            let pickings: Picking<MultipleIds> = env.sudo_with(|env| {
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
            let warehouse = Warehouse::main(env)?;
            if warehouse.is_empty() {
                return Err("No warehouse delivers the order".into());
            }
            let delivery_type: PickingType<SingleId> = warehouse.get_out_type(&mut env.sudo())?;
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
            let picking: Picking<SingleId> = env.create_new_record_from_map(picking)?;
            Picking::<MultipleIds>::from_ids(vec![picking.get_id()], env).action_confirm(env)?;
        }
        Ok(())
    }

    /// Cancelling an order cancels the deliveries not done; one delivered must come back first.
    pub fn on_cancelled(&self, env: &mut Environment, sup: Super) -> Result<()> {
        for order in self {
            let pickings: Picking<MultipleIds> = order.get_pickings(env)?;
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
                Picking::<MultipleIds>::from_ids(open, env).action_cancel(env)?;
            }
        }
        Ok(sup.call(env)?)
    }
}

/// Deliveries done count on their orders' lines.
#[derive(Model)]
#[erp(id = "stock_picking", methods)]
#[erp(derived_model = "stock::models")]
#[allow(dead_code)]
pub struct PickingSale<Mode: IdMode> {
    id: Mode,
}

#[erp_methods]
impl PickingSale<MultipleIds> {
    /// Each order line delivered by these transfers counts what its moves took to customers,
    /// less what came back.
    pub fn on_done(&self, env: &mut Environment, sup: Super) -> Result<()> {
        sup.call(env)?;
        let env = &mut *env.sudo();
        let pickings: Picking<MultipleIds> = self.as_model();
        let moves: StockMove<MultipleIds> = pickings.get_moves(env)?;
        let moves: StockMoveSale<MultipleIds> = moves.as_model();
        let lines: SaleOrderLine<MultipleIds> = moves.get_sale_line(env)?;
        for line in &lines {
            let delivered = delivered_quantity(env, line.get_id())?;
            line.set_qty_delivered(delivered, env)?;
        }
        Ok(())
    }
}

/// What the done moves of an order line took to customers, less what they brought back, in the
/// line's unit.
fn delivered_quantity(env: &mut Environment, line: u32) -> Result<Decimal> {
    let moves: StockMove<MultipleIds> = env.search(&make_domain!([
        ("sale_line", "=", line),
        ("state", "=", MoveStatus::Done)
    ]))?;
    let mut delivered = Decimal::ZERO;
    for stock_move in &moves {
        let source: Location<SingleId> = stock_move.get_location(env)?;
        let destination: Location<SingleId> = stock_move.get_location_dest(env)?;
        let quantity = *stock_move.get_quantity(env)?;
        if source.is_internal(env)? && !destination.is_internal(env)? {
            delivered += quantity;
        } else if !source.is_internal(env)? && destination.is_internal(env)? {
            delivered -= quantity;
        }
    }
    Ok(delivered)
}

/// Goods are invoiced as delivered unless said otherwise.
#[derive(Model)]
#[erp(id = "product", methods)]
#[erp(derived_model = "product::models")]
#[allow(dead_code)]
pub struct ProductSaleStock<Mode: IdMode> {
    id: Mode,
}

#[erp_methods]
impl ProductSaleStock<MultipleIds> {
    pub fn create(
        &self,
        env: &mut Environment,
        values: Vec<MapOfFields>,
        sup: Super,
    ) -> Result<MultipleIds> {
        let mut values = values;
        for product in &mut values {
            let goods = product
                .get_option::<&String>("product_type")
                .is_none_or(|kind| kind == "goods");
            if goods && !product.contains_key("invoice_policy") {
                product.insert("invoice_policy", "delivery");
            }
        }
        Ok(sup.call_with(values, env)?)
    }
}
