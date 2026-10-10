use crate::payment_terms::{self, Installment, InstallmentValue};
use code_gen::{Model, erp_methods, selection};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{Decimal, IdMode, MultipleIds, NaiveDate, Reference, SingleId};

#[selection]
pub enum PaymentTermValue {
    #[selection(label = "Percent")]
    Percent,
    #[default]
    #[selection(label = "Balance")]
    Balance,
}

/// When an invoice is to be paid: `30 days end of month`, or 30 % now and the rest in 60 days.
#[derive(Model)]
#[erp(id = "account_payment_term", order = "name, id", methods)]
#[allow(dead_code)]
pub struct PaymentTerm<Mode: IdMode> {
    id: Mode,
    name: String,
    #[erp(label = "Description on invoices")]
    note: Option<String>,
    #[erp(label = "Installments", inverse = "term", owned)]
    lines: Reference<BaseAccountPaymentTermLine, MultipleIds>,
    #[erp(default = true)]
    active: bool,
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

#[erp_methods]
impl PaymentTerm<SingleId> {
    /// The installments of the term, in order.
    pub fn installments(&self, env: &mut Environment) -> Result<Vec<Installment>> {
        if self.is_empty() {
            return Ok(Vec::new());
        }
        let env = &mut *env.sudo();
        let lines: PaymentTermLine<MultipleIds> = self.get_lines(env)?;
        let mut rows = Vec::new();
        for line in &lines {
            rows.push((
                *line.get_sequence(env)?,
                line.get_id(),
                Installment {
                    value: match *line.get_value(env)? {
                        PaymentTermValue::Percent => InstallmentValue::Percent,
                        _ => InstallmentValue::Balance,
                    },
                    value_amount: *line.get_value_amount(env)?,
                    days: *line.get_days(env)?,
                    end_of_month: *line.get_end_of_month(env)?,
                    days_after_end_of_month: *line.get_days_after_end_of_month(env)?,
                },
            ));
        }
        rows.sort_by_key(|(sequence, id, _)| (*sequence, *id));
        Ok(rows
            .into_iter()
            .map(|(_, _, installment)| installment)
            .collect())
    }

    /// `total` split into what falls due when, for an invoice of `date`.
    pub fn compute(
        &self,
        env: &mut Environment,
        total: Decimal,
        date: NaiveDate,
        rounding: Decimal,
    ) -> Result<Vec<(NaiveDate, Decimal)>> {
        let installments = self.installments(env)?;
        Ok(payment_terms::installments(
            total,
            date,
            &installments,
            rounding,
        )?)
    }
}

#[erp_methods]
impl PaymentTerm<MultipleIds> {
    /// A term with installments ends with its balance; one without falls due at once.
    #[erp(check)]
    pub fn check_terms(&self, env: &mut Environment) -> Result<()> {
        for term in self {
            let installments = term.installments(env)?;
            if !installments.is_empty() {
                payment_terms::check(&installments).map_err(|error| {
                    format!(
                        "{}: {error}",
                        term.get_name(env).map(String::as_str).unwrap_or("")
                    )
                })?;
            }
        }
        Ok(())
    }
}
