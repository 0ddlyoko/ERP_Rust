use crate::models::location::BaseStockLocation;
use crate::models::warehouse::BaseStockWarehouse;
use code_gen::{Model, selection};
use erp::types::field::{IdMode, Reference, SingleId};
use sequence::models::BaseSequence;

#[selection]
pub enum PickingKind {
    #[selection(label = "Receipt")]
    Incoming,
    #[selection(label = "Delivery")]
    Outgoing,
    #[default]
    #[selection(label = "Internal transfer")]
    Internal,
}

/// A kind of transfer of a warehouse: its receipts, its deliveries, its internal transfers —
/// where they go from and to, and how they are numbered.
#[derive(Model)]
#[erp(id = "stock_picking_type")]
#[allow(dead_code)]
pub struct PickingType<Mode: IdMode> {
    id: Mode,
    name: String,
    #[erp(label = "Type of operation")]
    code: PickingKind,
    #[erp(ondelete = "cascade")]
    warehouse: Reference<BaseStockWarehouse, SingleId>,
    #[erp(label = "Numbering", ondelete = "restrict")]
    sequence: Reference<BaseSequence, SingleId>,
    #[erp(label = "Default source location", ondelete = "restrict")]
    default_location_src: Reference<BaseStockLocation, SingleId>,
    #[erp(label = "Default destination location", ondelete = "restrict")]
    default_location_dest: Reference<BaseStockLocation, SingleId>,
    #[erp(default = true)]
    active: bool,
}
