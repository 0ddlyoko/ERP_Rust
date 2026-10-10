use crate::models::stock_location::{BaseStockLocation, LocationUsage, StockLocation};
use crate::models::stock_picking_type::{BaseStockPickingType, StockPickingType};
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};
use erp::types::model::MapOfFields;

/// A warehouse: its stock location, and its receipts, deliveries and internal transfers.
#[derive(Model)]
#[erp(id = "stock_warehouse", order = "name, id", methods)]
#[allow(dead_code)]
pub struct StockWarehouse<Mode: IdMode> {
    id: Mode,
    name: String,
    #[erp(
        label = "Short name",
        description = "Starts the names of its locations and transfers, e.g. WH",
        index
    )]
    code: String,
    #[erp(label = "View location", ondelete = "restrict")]
    view_location: Reference<BaseStockLocation, SingleId>,
    #[erp(label = "Stock location", ondelete = "restrict")]
    lot_stock: Reference<BaseStockLocation, SingleId>,
    #[erp(label = "Receipts", ondelete = "restrict")]
    in_type: Reference<BaseStockPickingType, SingleId>,
    #[erp(label = "Delivery orders", ondelete = "restrict")]
    out_type: Reference<BaseStockPickingType, SingleId>,
    #[erp(label = "Internal transfers", ondelete = "restrict")]
    int_type: Reference<BaseStockPickingType, SingleId>,
    #[erp(default = true)]
    active: bool,
}

#[erp_methods]
impl StockWarehouse<SingleId> {
    /// The first warehouse, where goods are received and delivered unless said otherwise.
    pub fn main(env: &mut Environment) -> Result<StockWarehouse<SingleId>> {
        let env = &mut *env.sudo();
        let found: StockWarehouse<MultipleIds> = env.search_with(
            &erp_search_code_gen::make_domain!([("active", "=", true)]),
            &erp_search::SearchOptions::new()
                .order_by(erp_search::OrderBy::asc("id"))
                .with_limit(1),
        )?;
        Ok(found
            .into_iter()
            .next()
            .unwrap_or_else(|| env.get_record(SingleId::empty())))
    }
}

/// The location where goods of a kind come from or go to: the vendors, the customers, the
/// inventory adjustments.
pub fn partner_location(
    env: &mut Environment,
    usage: LocationUsage,
) -> Result<StockLocation<SingleId>> {
    let xml_id = match usage {
        LocationUsage::Supplier => "stock.location_suppliers",
        LocationUsage::Customer => "stock.location_customers",
        _ => "stock.location_inventory",
    };
    env.sudo().named(xml_id)
}

#[erp_methods]
impl StockWarehouse<MultipleIds> {
    /// A warehouse comes with its locations — `WH`, `WH/Stock` — and its kinds of transfers,
    /// each numbered on its own: `WH/IN/00001`, `WH/OUT/00001`, `WH/INT/00001`.
    pub fn create(
        &self,
        env: &mut Environment,
        values: Vec<MapOfFields>,
        sup: Super,
    ) -> Result<MultipleIds> {
        env.savepoint(|env| {
            let ids: MultipleIds = sup.call_with(values, env)?;
            for warehouse in StockWarehouse::<MultipleIds>::from_ids(ids.clone(), env) {
                let code = warehouse.get_code(env)?.trim().to_uppercase();
                if code.is_empty() || code.len() > 5 {
                    return Err(format!(
                        "A warehouse's short name has 1 to 5 characters, not \"{code}\""
                    )
                    .into());
                }
                let env = &mut *env.sudo();
                let mut view = MapOfFields::default();
                view.insert("name", code.clone());
                view.insert("usage", LocationUsage::View);
                let view: StockLocation<SingleId> = env.create_new_record_from_map(view)?;
                let mut stock = MapOfFields::default();
                stock.insert("name", "Stock");
                stock.insert("parent", view.get_id());
                let stock: StockLocation<SingleId> = env.create_new_record_from_map(stock)?;
                let suppliers = partner_location(env, LocationUsage::Supplier)?;
                let customers = partner_location(env, LocationUsage::Customer)?;
                let mut types = Vec::new();
                for (name, kind, prefix, source, destination) in [
                    (
                        "Receipts",
                        "incoming",
                        "IN",
                        suppliers.get_id(),
                        stock.get_id(),
                    ),
                    (
                        "Delivery orders",
                        "outgoing",
                        "OUT",
                        stock.get_id(),
                        customers.get_id(),
                    ),
                    (
                        "Internal transfers",
                        "internal",
                        "INT",
                        stock.get_id(),
                        stock.get_id(),
                    ),
                ] {
                    let mut numbering = MapOfFields::default();
                    numbering.insert("name", format!("{code} {name}"));
                    numbering.insert("code", format!("stock.picking.{code}.{prefix}"));
                    numbering.insert("prefix", format!("{code}/{prefix}/"));
                    numbering.insert("padding", 5);
                    numbering.insert("reset", "never");
                    let numbering = env
                        .create_records("sequence", vec![numbering])?
                        .get_ids_ref()[0];
                    let mut picking_type = MapOfFields::default();
                    picking_type.insert("name", name);
                    picking_type.insert("code", kind);
                    picking_type.insert("warehouse", warehouse.get_id());
                    picking_type.insert("sequence", numbering);
                    picking_type.insert("default_location_src", source);
                    picking_type.insert("default_location_dest", destination);
                    let picking_type: StockPickingType<SingleId> =
                        env.create_new_record_from_map(picking_type)?;
                    types.push(picking_type);
                }
                warehouse.set_view_location(&view, env)?;
                warehouse.set_lot_stock(&stock, env)?;
                warehouse.set_in_type(&types[0], env)?;
                warehouse.set_out_type(&types[1], env)?;
                warehouse.set_int_type(&types[2], env)?;
            }
            Ok(ids)
        })
    }
}
