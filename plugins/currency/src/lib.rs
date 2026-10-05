//! Currencies: what amounts are counted in, their rates over time, and the one the company keeps
//! its books in.

use erp::Result;
use erp::environment::Environment;
use erp::model::ModelManager;
use erp::plugin::{Plugin, PluginInfo};
use erp::types::field::{MultipleIds, SingleId};
use erp_search::SearchType;

pub mod models;
pub mod money;

pub struct CurrencyPlugin;

impl Plugin for CurrencyPlugin {
    fn name(&self) -> String {
        "currency".to_string()
    }

    fn info(&self) -> PluginInfo {
        PluginInfo {
            description: Some(
                "Currencies, their exchange rates by date, and the company's currency.".to_string(),
            ),
            category: Some("Accounting".to_string()),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            ..PluginInfo::default()
        }
    }

    fn init_models(&self, model_manager: &mut ModelManager) {
        model_manager.register_model::<models::Currency<_>>();
        model_manager.register_model::<models::CurrencyRate<_>>();
        model_manager.register_model::<models::CompanyCurrency<_>>();
    }

    fn data(&self) -> Vec<&'static str> {
        vec![
            include_str!("../data/access.xml"),
            include_str!("../data/currencies.xml"),
            include_str!("../views/currency_views.xml"),
        ]
    }

    /// Companies without a currency keep their books in euros: `base` seeds the main company
    /// before this plugin exists, so its data cannot say so.
    fn post_init(&mut self, env: &mut Environment) -> Result<()> {
        let env = &mut *env.sudo();
        let euro: models::Currency<SingleId> = env.named("currency.currency_eur")?;
        let companies: models::CompanyCurrency<MultipleIds> = env.search(&SearchType::Nothing)?;
        for company in &companies {
            let currency: models::Currency<SingleId> = company.get_currency(env)?;
            if currency.is_empty() {
                company.set_currency(&euro, env)?;
            }
        }
        Ok(())
    }

    fn get_depends(&self) -> Vec<String> {
        vec!["base".to_string(), "web".to_string()]
    }
}

code_gen::export_plugin!(CurrencyPlugin {});
