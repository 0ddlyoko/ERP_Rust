use crate::models::account_payment_term::BaseAccountPaymentTerm;
use code_gen::{Model, selection};
use erp::types::field::{Decimal, IdMode, Reference, SingleId};

#[selection]
pub enum PaymentTermValue {
    #[selection(label = "Percent")]
    Percent,
    #[default]
    #[selection(label = "Balance")]
    Balance,
}

/// One installment of a payment term.
#[derive(Model)]
#[erp(id = "account_payment_term_line", order = "sequence, id")]
#[allow(dead_code)]
pub struct PaymentTermLine<Mode: IdMode> {
    id: Mode,
    #[erp(required, ondelete = "cascade")]
    term: Reference<BaseAccountPaymentTerm, SingleId>,
    #[erp(label = "Due")]
    value: PaymentTermValue,
    #[erp(
        label = "Value",
        default = 0.0,
        description = "The percentage, for a percent line"
    )]
    value_amount: Decimal,
    #[erp(label = "Days", default = 0)]
    days: i32,
    #[erp(label = "End of month")]
    end_of_month: bool,
    #[erp(label = "Days after end of month", default = 0)]
    days_after_end_of_month: i32,
    #[erp(default = 10)]
    sequence: i32,
}
