use code_gen::Model;
use erp::types::field::{IdMode, Reference, SingleId};
use sale::models::BaseSaleOrder;

/// The order a project was made for.
#[derive(Model)]
#[erp(id = "project_project")]
#[erp(derived_model = "project::models")]
#[allow(dead_code)]
pub struct ProjectSale<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Sales order", ondelete = "set_null", index)]
    sale_order: Reference<BaseSaleOrder, SingleId>,
}
