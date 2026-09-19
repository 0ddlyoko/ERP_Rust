use crate::models::{BaseTag, Tag};
use code_gen::{Model, erp_methods};
use erp::environment::Environment;
use erp::types::field::{Decimal, IdMode, MultipleIds, NaiveDate, Reference, Timestamp};
use std::error::Error;

/// Exercises the field types an ERP cannot do without: exact money, a due date and an audit stamp.
#[derive(Model)]
#[erp(id = "invoice", methods)]
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
    #[erp(relation = "invoice_tag_rel")]
    tags: Reference<BaseTag, MultipleIds>,
    /// Depends on a path that crosses the relation table.
    #[erp(compute = "compute_tag_summary", depends = ["tags.name"])]
    tag_summary: String,
}

#[erp_methods]
impl Invoice<MultipleIds> {
    #[erp(overridable)]
    pub fn compute_tag_summary(
        &self,
        env: &mut Environment,
        _parent: Super,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        for invoice in self {
            let tags: Tag<MultipleIds> = invoice.get_tags(env)?;
            let mut names: Vec<String> = tags.get_name(env)?.into_iter().cloned().collect();
            names.sort();
            invoice.set_tag_summary(names.join(","), env)?;
        }
        Ok(())
    }
}
