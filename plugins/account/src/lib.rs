//! Accounting: the chart of accounts, taxes, journals, invoices and bills, payments, bank
//! statements and matching — the books of the company, double-entry.

use erp::model::ModelManager;
use erp::plugin::{Plugin, PluginInfo};

pub mod invariants;
pub mod matching;
pub mod models;
pub mod payment_terms;
pub mod tax_engine;
pub mod testing;

pub struct AccountPlugin;

impl Plugin for AccountPlugin {
    fn name(&self) -> String {
        "account".to_string()
    }

    fn info(&self) -> PluginInfo {
        PluginInfo {
            description: Some(
                "Invoicing and accounting: invoices, bills, payments, bank statements, taxes and \
                 the general ledger."
                    .to_string(),
            ),
            category: Some("Accounting".to_string()),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            color: Some("#2160d6".to_string()),
            ..PluginInfo::default()
        }
    }

    fn init_models(&self, model_manager: &mut ModelManager) {
        model_manager.register_model::<models::Account<_>>();
        model_manager.register_model::<models::AccountTaxTag<_>>();
        model_manager.register_model::<models::AccountTax<_>>();
        model_manager.register_model::<models::AccountTaxRepartition<_>>();
        model_manager.register_model::<models::AccountJournal<_>>();
        model_manager.register_model::<models::AccountPaymentTerm<_>>();
        model_manager.register_model::<models::AccountPaymentTermLine<_>>();
        model_manager.register_model::<models::AccountFiscalPosition<_>>();
        model_manager.register_model::<models::AccountFiscalPositionTax<_>>();
        model_manager.register_model::<models::AccountFiscalPositionAccount<_>>();
        model_manager.register_model::<models::CompanyAccount<_>>();
        model_manager.register_model::<models::ContactAccount<_>>();
        model_manager.register_model::<models::ContactBank<_>>();
        model_manager.register_model::<models::ProductAccount<_>>();
        model_manager.register_model::<models::ProductCategoryAccount<_>>();
        model_manager.register_model::<models::AccountMove<_>>();
        model_manager.register_model::<models::AccountInvoiceLine<_>>();
        model_manager.register_model::<models::AccountMoveLine<_>>();
        model_manager.register_model::<models::AccountPartialReconcile<_>>();
        model_manager.register_model::<models::AccountFullReconcile<_>>();
        model_manager.register_model::<models::AccountPayment<_>>();
        model_manager.register_model::<models::AccountBankStatement<_>>();
        model_manager.register_model::<models::AccountBankStatementLine<_>>();
        model_manager.register_model::<models::AccountTrialBalance<_>>();
        model_manager.register_model::<models::AccountTrialBalanceLine<_>>();
    }

    fn data(&self) -> Vec<&'static str> {
        vec![
            include_str!("../data/groups.xml"),
            include_str!("../data/access.xml"),
            include_str!("../data/mail_data.xml"),
            include_str!("../views/account_views.xml"),
            include_str!("../views/tax_views.xml"),
            include_str!("../views/journal_views.xml"),
            include_str!("../views/move_views.xml"),
            include_str!("../views/payment_views.xml"),
            include_str!("../views/bank_statement_views.xml"),
            include_str!("../views/report_views.xml"),
            include_str!("../views/partner_product_views.xml"),
            include_str!("../views/dashboard_views.xml"),
            include_str!("../views/menus.xml"),
        ]
    }

    fn get_depends(&self) -> Vec<String> {
        vec![
            "base".to_string(),
            "web".to_string(),
            "mail".to_string(),
            "contacts".to_string(),
            "currency".to_string(),
            "sequence".to_string(),
            "product".to_string(),
        ]
    }
}

code_gen::export_plugin!(AccountPlugin {});
