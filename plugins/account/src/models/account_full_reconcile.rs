use crate::models::account_move_line::BaseAccountMoveLine;
use crate::models::account_partial_reconcile::BaseAccountPartialReconcile;
use code_gen::Model;
use erp::types::field::{IdMode, MultipleIds, Reference};

/// Journal items fully settling each other, under one matching number.
#[derive(Model)]
#[erp(id = "account_full_reconcile")]
#[allow(dead_code)]
pub struct AccountFullReconcile<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Matching number")]
    name: String,
    #[erp(label = "Partial matchings", inverse = "full_reconcile")]
    partials: Reference<BaseAccountPartialReconcile, MultipleIds>,
    #[erp(label = "Matched items", inverse = "full_reconcile")]
    lines: Reference<BaseAccountMoveLine, MultipleIds>,
}
