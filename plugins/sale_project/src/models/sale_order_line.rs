use code_gen::Model;
use erp::types::field::{IdMode, MultipleIds, Reference};
use project::models::BaseProjectTask;

/// The tasks carrying out an order line.
#[derive(Model)]
#[erp(id = "sale_order_line")]
#[erp(derived_model = "sale::models")]
#[allow(dead_code)]
pub struct SaleOrderLineSaleProject<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Tasks", inverse = "sale_line")]
    tasks: Reference<BaseProjectTask, MultipleIds>,
}
