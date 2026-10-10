use crate::models::account::Account;
use crate::models::account_fiscal_position_account::{
    AccountFiscalPositionAccount, BaseAccountFiscalPositionAccount,
};
use crate::models::account_fiscal_position_tax::{
    AccountFiscalPositionTax, BaseAccountFiscalPositionTax,
};
use crate::models::account_tax::AccountTax;
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};

/// How taxes and accounts change for a kind of customer or supplier: an EU business with a VAT
/// number is invoiced intra-community, without VAT; one outside the EU, as an export.
#[derive(Model)]
#[erp(id = "account_fiscal_position", order = "sequence, id", methods)]
#[allow(dead_code)]
pub struct AccountFiscalPosition<Mode: IdMode> {
    id: Mode,
    name: String,
    #[erp(label = "Legal mention on invoices")]
    note: Option<String>,
    #[erp(label = "Tax mapping", inverse = "position", owned)]
    tax_mappings: Reference<BaseAccountFiscalPositionTax, MultipleIds>,
    #[erp(label = "Account mapping", inverse = "position", owned)]
    account_mappings: Reference<BaseAccountFiscalPositionAccount, MultipleIds>,
    #[erp(default = 10)]
    sequence: i32,
    #[erp(default = true)]
    active: bool,
}

#[erp_methods]
impl AccountFiscalPosition<SingleId> {
    /// `taxes` as they apply under this position: each mapped tax replaced, a tax mapped to
    /// none dropped, the others kept. No position keeps them all.
    pub fn map_taxes(&self, env: &mut Environment, taxes: Vec<u32>) -> Result<Vec<u32>> {
        if self.is_empty() {
            return Ok(taxes.to_vec());
        }
        let env = &mut *env.sudo();
        let mappings: AccountFiscalPositionTax<MultipleIds> = self.get_tax_mappings(env)?;
        let mut rows = Vec::new();
        for mapping in &mappings {
            let source: AccountTax<SingleId> = mapping.get_tax_src(env)?;
            let destination: AccountTax<SingleId> = mapping.get_tax_dest(env)?;
            rows.push((source.get_id(), destination.get_optional_id()));
        }
        let mut mapped = Vec::new();
        for tax in taxes {
            let replacements: Vec<Option<u32>> = rows
                .iter()
                .filter(|(source, _)| *source == tax)
                .map(|(_, destination)| *destination)
                .collect();
            if replacements.is_empty() {
                mapped.push(tax);
            } else {
                mapped.extend(replacements.into_iter().flatten());
            }
        }
        mapped.dedup();
        Ok(mapped)
    }

    /// `account` as it applies under this position.
    pub fn map_account(
        &self,
        env: &mut Environment,
        account: Account<SingleId>,
    ) -> Result<Account<SingleId>> {
        if self.is_empty() || account.is_empty() {
            return Ok(account);
        }
        let env = &mut *env.sudo();
        let mappings: AccountFiscalPositionAccount<MultipleIds> = self.get_account_mappings(env)?;
        for mapping in &mappings {
            let source: Account<SingleId> = mapping.get_account_src(env)?;
            if source.get_id() == account.get_id() {
                return mapping.get_account_dest(env);
            }
        }
        Ok(account)
    }
}
