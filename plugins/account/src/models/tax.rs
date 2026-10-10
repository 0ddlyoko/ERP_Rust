use crate::models::account::{Account, BaseAccount};
use crate::tax_engine::{Repartition, TaxKind, TaxSpec};
use code_gen::{Model, erp_methods, selection};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::Selection;
use erp::types::field::{Decimal, IdMode, MultipleIds, Reference, SingleId};

#[selection]
pub enum TaxUse {
    #[default]
    #[selection(label = "Sales")]
    Sale,
    #[selection(label = "Purchases")]
    Purchase,
    #[selection(label = "None")]
    None,
}

#[selection]
pub enum TaxAmountType {
    #[default]
    #[selection(label = "Percentage of price")]
    Percent,
    #[selection(label = "Fixed per unit")]
    Fixed,
}

#[selection]
pub enum TaxDocument {
    #[default]
    #[selection(label = "Invoices")]
    Invoice,
    #[selection(label = "Credit notes")]
    Refund,
}

/// A grid of the tax return an amount is reported in: `03`, counted with the sign the entry's
/// balance has on that side, so that grids hold positive amounts.
#[derive(Model)]
#[erp(id = "account_tax_tag", order = "name, id")]
#[allow(dead_code)]
pub struct TaxTag<Mode: IdMode> {
    id: Mode,
    name: String,
    #[erp(description = "The grid of the tax return, e.g. 03")]
    grid: String,
    #[erp(
        default = 1,
        description = "1 when debits add to the grid, -1 when credits do"
    )]
    sign: i32,
    #[erp(default = true)]
    active: bool,
}

/// A tax: 21 % VAT on sales, with where its amount goes on invoices and on credit notes.
#[derive(Model)]
#[erp(id = "account_tax", order = "sequence, id", methods)]
#[allow(dead_code)]
pub struct Tax<Mode: IdMode> {
    id: Mode,
    name: String,
    #[erp(label = "Label on documents")]
    description: Option<String>,
    #[erp(label = "Tax scope")]
    type_tax_use: TaxUse,
    #[erp(label = "Computation")]
    amount_type: TaxAmountType,
    #[erp(default = 0.0, description = "21 for 21 %, or the amount per unit")]
    amount: Decimal,
    #[erp(label = "Included in price")]
    price_include: bool,
    #[erp(default = 10)]
    sequence: i32,
    #[erp(label = "Distribution", inverse = "tax", owned)]
    repartitions: Reference<BaseAccountTaxRepartition, MultipleIds>,
    #[erp(
        label = "Invoice base grids",
        relation = "account_tax_invoice_base_tag_rel"
    )]
    invoice_base_tags: Reference<BaseAccountTaxTag, MultipleIds>,
    #[erp(
        label = "Credit note base grids",
        relation = "account_tax_refund_base_tag_rel"
    )]
    refund_base_tags: Reference<BaseAccountTaxTag, MultipleIds>,
    #[erp(default = true)]
    active: bool,
}

/// Where a share of a tax goes, on invoices or on credit notes.
#[derive(Model)]
#[erp(id = "account_tax_repartition", order = "sequence, id")]
#[allow(dead_code)]
pub struct TaxRepartition<Mode: IdMode> {
    id: Mode,
    #[erp(required, ondelete = "cascade")]
    tax: Reference<BaseAccountTax, SingleId>,
    #[erp(label = "Applies to")]
    document: TaxDocument,
    #[erp(
        label = "%",
        default = 100.0,
        description = "The share of the tax: 100, or -100 for the VAT owed by a reverse charge"
    )]
    factor: Decimal,
    #[erp(ondelete = "restrict")]
    account: Reference<BaseAccount, SingleId>,
    #[erp(label = "Tax grids", relation = "account_tax_repartition_tag_rel")]
    tags: Reference<BaseAccountTaxTag, MultipleIds>,
    #[erp(default = 10)]
    sequence: i32,
}

#[erp_methods]
impl Tax<SingleId> {
    /// The tax as the engine computes it, for invoices or for credit notes. Without any
    /// distribution for that document, all of it goes to the account of the line it is on.
    ///
    /// As sudo: whoever invoices may compute the taxes, without managing them.
    pub fn spec(&self, env: &mut Environment, document: TaxDocument) -> Result<TaxSpec> {
        let env = &mut *env.sudo();
        let kind = match *self.get_amount_type(env)? {
            TaxAmountType::Fixed => TaxKind::Fixed,
            _ => TaxKind::Percent,
        };
        let base_tags: TaxTag<MultipleIds> = match document {
            TaxDocument::Refund => self.get_refund_base_tags(env)?,
            _ => self.get_invoice_base_tags(env)?,
        };
        let mut lines: Vec<(i32, u32, Repartition)> = Vec::new();
        let repartitions: TaxRepartition<MultipleIds> = self.get_repartitions(env)?;
        for repartition in &repartitions {
            if repartition.get_document(env)?.key() != document.key() {
                continue;
            }
            let account: Account<SingleId> = repartition.get_account(env)?;
            let tags: TaxTag<MultipleIds> = repartition.get_tags(env)?;
            lines.push((
                *repartition.get_sequence(env)?,
                repartition.get_id(),
                Repartition {
                    factor: *repartition.get_factor(env)?,
                    account: account.get_optional_id(),
                    tags: tags.get_ids_ref().clone(),
                },
            ));
        }
        lines.sort_by_key(|(sequence, id, _)| (*sequence, *id));
        let repartitions = if lines.is_empty() {
            vec![Repartition {
                factor: Decimal::ONE_HUNDRED,
                account: None,
                tags: Vec::new(),
            }]
        } else {
            lines
                .into_iter()
                .map(|(_, _, repartition)| repartition)
                .collect()
        };
        Ok(TaxSpec {
            id: self.get_id(),
            kind,
            amount: *self.get_amount(env)?,
            price_include: *self.get_price_include(env)?,
            base_tags: base_tags.get_ids_ref().clone(),
            repartitions,
        })
    }
}

#[erp_methods]
impl Tax<MultipleIds> {
    /// A percentage is between -100 and 100; the shares of a document that add to the tax are
    /// 100 % in all, so the tax is collected once.
    pub fn check_taxes(&self, env: &mut Environment) -> Result<()> {
        for tax in self {
            let name = tax.get_name(env)?.clone();
            let amount = *tax.get_amount(env)?;
            if matches!(*tax.get_amount_type(env)?, TaxAmountType::Percent)
                && (amount < -Decimal::ONE_HUNDRED || amount > Decimal::ONE_HUNDRED)
            {
                return Err(format!("The rate of {name} must be between -100 and 100 %").into());
            }
            for document in [TaxDocument::Invoice, TaxDocument::Refund] {
                let spec = tax.spec(env, document)?;
                let positive: Decimal = spec
                    .repartitions
                    .iter()
                    .map(|repartition| repartition.factor)
                    .filter(|factor| *factor > Decimal::ZERO)
                    .sum();
                if positive != Decimal::ONE_HUNDRED {
                    return Err(format!(
                        "The distribution of {name} on {} must add up to 100 %, not {positive} %",
                        match document {
                            TaxDocument::Refund => "credit notes",
                            _ => "invoices",
                        }
                    )
                    .into());
                }
            }
        }
        Ok(())
    }
}
