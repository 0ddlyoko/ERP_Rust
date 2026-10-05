//! Belgium: the minimum standard chart (PCMN), Belgian VAT with the grids of the periodic
//! return, structured communications, and VAT numbers checked.

use account::models::CompanyAccount;
use erp::Result;
use erp::data;
use erp::environment::Environment;
use erp::model::ModelManager;
use erp::plugin::{Plugin, PluginInfo};
use erp::types::field::SingleId;
use erp::types::model::MapOfFields;

pub mod invariants;
pub mod models;
pub mod structured;

pub struct L10nBePlugin;

/// The company's defaults this chart sets when the company has none: field and record.
const COMPANY_DEFAULTS: [(&str, &str); 9] = [
    ("account_receivable", "a400000"),
    ("account_payable", "a440000"),
    ("account_income", "a700000"),
    ("account_expense", "a604000"),
    ("account_exchange_gain", "a754000"),
    ("account_exchange_loss", "a654000"),
    ("journal_exchange", "journal_exchange"),
    ("sale_tax", "tax_sale_21"),
    ("purchase_tax", "tax_purchase_21_goods"),
];

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
        let current = env.read(
            "company",
            &SingleId::from(company.get_id()),
            &COMPANY_DEFAULTS.map(|(field, _)| field),
        )?;
        let current = current.into_iter().next().unwrap_or_default();
        let mut values = MapOfFields::default();
        for (field, xml_id) in COMPANY_DEFAULTS {
            let set = current.get_option::<&u32>(field).is_some_and(|id| *id != 0);
            if !set && let Some(id) = data::resolve(env, &format!("l10n_be.{xml_id}"))? {
                values.insert(field, id);
            }
        }
        if values.fields.is_empty() {
            return Ok(());
        }
        env.write("company", &SingleId::from(company.get_id()), values)
    }

    fn get_depends(&self) -> Vec<String> {
        vec!["account".to_string()]
    }
}

code_gen::export_plugin!(L10nBePlugin {});
