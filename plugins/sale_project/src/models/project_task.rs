use code_gen::Model;
use erp::types::field::{IdMode, Reference, SingleId};
use sale::models::BaseSaleOrderLine;

/// The order line a task carries out.
#[derive(Model)]
#[erp(id = "project_task")]
#[erp(derived_model = "project::models")]
#[allow(dead_code)]
pub struct TaskSale<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Order line", ondelete = "set_null", index)]
    sale_line: Reference<BaseSaleOrderLine, SingleId>,
}
