use code_gen::Model;
use erp::types::field::{Decimal, IdMode, NaiveDate, Timestamp};

/// Exercises the field types an ERP cannot do without: exact money, a due date and an audit stamp.
#[derive(Model)]
#[erp(table_name = "invoice")]
#[allow(dead_code)]
pub struct Invoice<Mode: IdMode> {
    pub id: Mode,
    #[erp(default = "draft")]
    name: String,
    #[erp(default = 0.00)]
    amount_untaxed: Decimal,
    #[erp(default = 0.21)]
    tax_rate: Decimal,
    due_date: NaiveDate,
    created_at: Timestamp,
    signed_on: Option<NaiveDate>,
}
