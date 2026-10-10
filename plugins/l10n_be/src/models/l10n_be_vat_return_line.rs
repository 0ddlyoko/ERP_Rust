use crate::models::l10n_be_vat_return::BaseL10nBeVatReturn;
use code_gen::Model;
use erp::types::field::{Decimal, IdMode, Reference, SingleId};

/// One grid of a VAT return.
#[derive(Model)]
#[erp(id = "l10n_be_vat_return_line")]
#[allow(dead_code)]
pub struct L10nBeVatReturnLine<Mode: IdMode> {
    id: Mode,
    #[erp(required, ondelete = "cascade")]
    vat_return: Reference<BaseL10nBeVatReturn, SingleId>,
    grid: String,
    #[erp(label = "Description")]
    name: String,
    #[erp(default = 0.0)]
    amount: Decimal,
}
