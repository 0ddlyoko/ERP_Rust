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
        model_manager.register_model::<models::TaxTag<_>>();
        model_manager.register_model::<models::Tax<_>>();
        model_manager.register_model::<models::TaxRepartition<_>>();
        model_manager.register_model::<models::Journal<_>>();
        model_manager.register_model::<models::PaymentTerm<_>>();
        model_manager.register_model::<models::PaymentTermLine<_>>();
        model_manager.register_model::<models::FiscalPosition<_>>();
        model_manager.register_model::<models::FiscalPositionTax<_>>();
        model_manager.register_model::<models::FiscalPositionAccount<_>>();
        model_manager.register_model::<models::CompanyAccount<_>>();
        model_manager.register_model::<models::ContactAccount<_>>();
        model_manager.register_model::<models::ContactBank<_>>();
        model_manager.register_model::<models::ProductAccount<_>>();
        model_manager.register_model::<models::ProductCategoryAccount<_>>();
        model_manager.register_model::<models::Move<_>>();
        model_manager.register_model::<models::InvoiceLine<_>>();
        model_manager.register_model::<models::MoveLine<_>>();
        model_manager.register_model::<models::PartialReconcile<_>>();
        model_manager.register_model::<models::FullReconcile<_>>();
        model_manager.register_model::<models::Payment<_>>();
        model_manager.register_model::<models::BankStatement<_>>();
        model_manager.register_model::<models::BankStatementLine<_>>();
        model_manager.register_model::<models::TrialBalance<_>>();
        model_manager.register_model::<models::TrialBalanceLine<_>>();
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
