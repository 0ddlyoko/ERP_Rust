use crate::models::account_fiscal_position::BaseAccountFiscalPosition;
use crate::models::account_tax::BaseAccountTax;
use code_gen::Model;
use erp::types::field::{IdMode, Reference, SingleId};

/// A tax replaced by another — or by none — under a fiscal position.
#[derive(Model)]
#[erp(id = "account_fiscal_position_tax")]
#[allow(dead_code)]
pub struct FiscalPositionTax<Mode: IdMode> {
    id: Mode,
    #[erp(required, ondelete = "cascade")]
    position: Reference<BaseAccountFiscalPosition, SingleId>,
    #[erp(label = "Tax on product", required, ondelete = "cascade")]
    tax_src: Reference<BaseAccountTax, SingleId>,
    #[erp(label = "Tax to apply instead", ondelete = "cascade")]
    tax_dest: Reference<BaseAccountTax, SingleId>,
}
