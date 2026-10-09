use crate::models::uom::BaseUom;
use code_gen::Model;
use erp::types::field::{IdMode, MultipleIds, Reference};

/// What a family of units measures — units, weight, length — and so which convert into which.
#[derive(Model)]
#[erp(id = "uom_category", order = "name, id")]
#[allow(dead_code)]
pub struct UomCategory<Mode: IdMode> {
    id: Mode,
    name: String,
    #[erp(label = "Units", inverse = "category")]
    uoms: Reference<BaseUom, MultipleIds>,
}
