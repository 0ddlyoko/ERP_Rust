use crate::models::account::BaseAccount;
use crate::models::account_tax::{BaseAccountTax, Tax};
use crate::models::company::CompanyAccount;
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{FieldType, IdMode, MultipleIds, Reference, SingleId};
use erp::types::model::MapOfFields;

/// The taxes a product is sold and bought with, and where its sales and purchases are recorded
/// when not where its category says.
#[derive(Model)]
#[erp(id = "product", methods)]
#[erp(derived_model = "product::models")]
#[allow(dead_code)]
pub struct ProductAccount<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Customer taxes", relation = "product_sale_tax_rel")]
    taxes: Reference<BaseAccountTax, MultipleIds>,
    #[erp(label = "Vendor taxes", relation = "product_purchase_tax_rel")]
    supplier_taxes: Reference<BaseAccountTax, MultipleIds>,
    #[erp(label = "Income account", ondelete = "restrict")]
    income_account: Reference<BaseAccount, SingleId>,
    #[erp(label = "Expense account", ondelete = "restrict")]
    expense_account: Reference<BaseAccount, SingleId>,
}

#[erp_methods]
impl ProductAccount<MultipleIds> {
    /// A product created without taxes is sold and bought with the company's default ones.
    pub fn create(
        &self,
        env: &mut Environment,
        values: Vec<MapOfFields>,
        sup: Super,
    ) -> Result<MultipleIds> {
        let (sale, purchase) = {
            let env = &mut *env.sudo();
            let company = CompanyAccount::current(env)?;
            if company.is_empty() {
                (None, None)
            } else {
                let sale: Tax<SingleId> = company.get_sale_tax(env)?;
                let purchase: Tax<SingleId> = company.get_purchase_tax(env)?;
                (sale.get_optional_id(), purchase.get_optional_id())
            }
        };
        let mut values = values;
        for product in &mut values {
            if !product.contains_key("taxes")
                && let Some(tax) = sale
            {
                product.insert("taxes", FieldType::Refs(vec![tax]));
            }
            if !product.contains_key("supplier_taxes")
                && let Some(tax) = purchase
            {
                product.insert("supplier_taxes", FieldType::Refs(vec![tax]));
            }
        }
        sup.call_with(values, env)
    }
}
