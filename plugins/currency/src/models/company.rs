use crate::models::currency::BaseCurrency;
use code_gen::Model;
use erp::types::field::{IdMode, Reference, SingleId};

/// The company keeps its books in a currency: the one rates are relative to.
#[derive(Model)]
#[erp(id = "company")]
#[erp(derived_model = "base::models")]
#[allow(dead_code)]
pub struct CompanyCurrency<Mode: IdMode> {
    id: Mode,
    #[erp(ondelete = "restrict")]
    currency: Reference<BaseCurrency, SingleId>,
}
