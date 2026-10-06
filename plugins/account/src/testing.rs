//! A small chart for tests: the accounts, taxes, journals and terms a scenario needs, without
//! a localization. Registered by tests only, as a plugin of its own.

use crate::models::{Account, CompanyAccount, Journal, Tax};
use erp::Result;
use erp::environment::Environment;
use erp::model::ModelManager;
use erp::plugin::Plugin;
use erp::types::field::SingleId;

pub struct TestChartPlugin;

impl Plugin for TestChartPlugin {
    fn name(&self) -> String {
        "account_test_chart".to_string()
    }

    fn init_models(&self, _model_manager: &mut ModelManager) {}

    fn data(&self) -> Vec<&'static str> {
        vec![include_str!("../data/test_chart.xml")]
    }

    /// The company records customers, suppliers and exchange differences on this chart, and
    /// gives new products its 21 % taxes.
    fn post_init(&mut self, env: &mut Environment) -> Result<()> {
        let env = &mut *env.sudo();
        let company = CompanyAccount::<SingleId>::current(env)?;
        let account = |env: &mut Environment, xml_id: &str| -> Result<Account<SingleId>> {
            env.named(&format!("account_test_chart.{xml_id}"))
        };
        let receivable = account(env, "a_receivable")?;
        company.set_account_receivable(&receivable, env)?;
        let payable = account(env, "a_payable")?;
        company.set_account_payable(&payable, env)?;
        let income = account(env, "a_sales")?;
        company.set_account_income(&income, env)?;
        let expense = account(env, "a_purchases")?;
        company.set_account_expense(&expense, env)?;
        let gain = account(env, "a_exchange_gain")?;
        company.set_account_exchange_gain(&gain, env)?;
        let loss = account(env, "a_exchange_loss")?;
        company.set_account_exchange_loss(&loss, env)?;
        let journal: Journal<SingleId> = env.named("account_test_chart.journal_exchange")?;
        company.set_journal_exchange(&journal, env)?;
        let sale_tax: Tax<SingleId> = env.named("account_test_chart.tax_sale_21")?;
        company.set_sale_tax(&sale_tax, env)?;
        let purchase_tax: Tax<SingleId> = env.named("account_test_chart.tax_purchase_21")?;
        company.set_purchase_tax(&purchase_tax, env)
    }

    fn get_depends(&self) -> Vec<String> {
        vec!["account".to_string()]
    }
}
