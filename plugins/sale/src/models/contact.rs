use crate::models::product_pricelist::BaseProductPricelist;
use code_gen::Model;
use erp::types::field::{IdMode, Reference, SingleId};

/// The prices a customer is offered.
#[derive(Model)]
#[erp(id = "contact")]
#[erp(derived_model = "base::models")]
#[allow(dead_code)]
pub struct ContactSale<Mode: IdMode> {
    id: Mode,
    #[erp(ondelete = "set_null")]
    pricelist: Reference<BaseProductPricelist, SingleId>,
}
