//! Belgium: the minimum standard chart (PCMN), Belgian VAT with the grids of the periodic
//! return, structured communications, and VAT numbers checked.

use account::models::{Account, CompanyAccount, Journal, Tax};
use erp::Result;
use erp::environment::Environment;
use erp::model::{Model, ModelManager};
use erp::plugin::{Plugin, PluginInfo};
use erp::types::field::{IdMode, SingleId};

pub mod invariants;
pub mod models;
pub mod structured;

pub struct L10nBePlugin;

/// The chart's record `xml_id`, for a default the company does not have yet.
fn unless_set<M: Model<SingleId>>(
    env: &mut Environment,
    current: M,
    xml_id: &str,
) -> Result<Option<M>> {
    if !current.get_id_mode().is_empty() {
        return Ok(None);
    }
    env.named(&format!("l10n_be.{xml_id}")).map(Some)
}

impl Plugin for L10nBePlugin {
    fn name(&self) -> String {
        "l10n_be".to_string()
    }

    fn info(&self) -> PluginInfo {
        PluginInfo {
            description: Some(
                "Belgian accounting: PCMN chart, VAT 21/12/6/0 % and reverse charge, the periodic \
                 VAT return, structured communications."
                    .to_string(),
            ),
            category: Some("Accounting".to_string()),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            ..PluginInfo::default()
        }
    }

    fn init_models(&self, model_manager: &mut ModelManager) {
        model_manager.register_model::<models::ContactBe<_>>();
        model_manager.register_model::<models::MoveBe<_>>();
        model_manager.register_model::<models::VatReturn<_>>();
        model_manager.register_model::<models::VatReturnLine<_>>();
    }

    fn data(&self) -> Vec<&'static str> {
        vec![
            include_str!("../data/chart.xml"),
            include_str!("../data/access.xml"),
            include_str!("../views/vat_return_views.xml"),
        ]
    }

    /// The company keeps its books on this chart: each default it does not have yet is set.
    fn post_init(&mut self, env: &mut Environment) -> Result<()> {
        let env = &mut *env.sudo();
        let company = CompanyAccount::<SingleId>::current(env)?;
        if company.is_empty() {
            return Ok(());
        }
        let current = company.get_account_receivable(env)?;
        if let Some(account) = unless_set::<Account<_>>(env, current, "a400000")? {
            company.set_account_receivable(&account, env)?;
        }
        let current = company.get_account_payable(env)?;
        if let Some(account) = unless_set::<Account<_>>(env, current, "a440000")? {
            company.set_account_payable(&account, env)?;
        }
        let current = company.get_account_income(env)?;
        if let Some(account) = unless_set::<Account<_>>(env, current, "a700000")? {
            company.set_account_income(&account, env)?;
        }
        let current = company.get_account_expense(env)?;
        if let Some(account) = unless_set::<Account<_>>(env, current, "a604000")? {
            company.set_account_expense(&account, env)?;
        }
        let current = company.get_account_exchange_gain(env)?;
        if let Some(account) = unless_set::<Account<_>>(env, current, "a754000")? {
            company.set_account_exchange_gain(&account, env)?;
        }
        let current = company.get_account_exchange_loss(env)?;
        if let Some(account) = unless_set::<Account<_>>(env, current, "a654000")? {
            company.set_account_exchange_loss(&account, env)?;
        }
        let current = company.get_journal_exchange(env)?;
        if let Some(journal) = unless_set::<Journal<_>>(env, current, "journal_exchange")? {
            company.set_journal_exchange(&journal, env)?;
        }
        let current = company.get_sale_tax(env)?;
        if let Some(tax) = unless_set::<Tax<_>>(env, current, "tax_sale_21")? {
            company.set_sale_tax(&tax, env)?;
        }
        let current = company.get_purchase_tax(env)?;
        if let Some(tax) = unless_set::<Tax<_>>(env, current, "tax_purchase_21_goods")? {
            company.set_purchase_tax(&tax, env)?;
        }
        Ok(())
    }

    fn get_depends(&self) -> Vec<String> {
        vec!["account".to_string()]
    }
}

code_gen::export_plugin!(L10nBePlugin {});
