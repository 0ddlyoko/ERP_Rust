//! A small chart for tests: the accounts, taxes, journals and terms a scenario needs, without
//! a localization. Registered by tests only, as a plugin of its own.

use crate::models::CompanyAccount;
use erp::Result;
use erp::data;
use erp::environment::Environment;
use erp::model::ModelManager;
use erp::plugin::Plugin;
use erp::types::field::SingleId;
use erp::types::model::MapOfFields;

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
        let mut values = MapOfFields::default();
        for (field, xml_id) in [
            ("account_receivable", "a_receivable"),
            ("account_payable", "a_payable"),
            ("account_income", "a_sales"),
            ("account_expense", "a_purchases"),
            ("account_exchange_gain", "a_exchange_gain"),
            ("account_exchange_loss", "a_exchange_loss"),
            ("journal_exchange", "journal_exchange"),
            ("sale_tax", "tax_sale_21"),
            ("purchase_tax", "tax_purchase_21"),
        ] {
            let id = data::resolve(env, &format!("account_test_chart.{xml_id}"))?
                .ok_or_else(|| format!("account_test_chart.{xml_id} is missing"))?;
            values.insert(field, id);
        }
        let company = CompanyAccount::<SingleId>::current(env)?;
        env.write("company", &SingleId::from(company.get_id()), values)
    }

    fn get_depends(&self) -> Vec<String> {
        vec!["account".to_string()]
    }
}
