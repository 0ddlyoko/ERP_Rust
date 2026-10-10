use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{IdMode, MultipleIds};
use erp::types::model::MapOfFields;

/// Goods are invoiced as delivered unless said otherwise.
#[derive(Model)]
#[erp(id = "product", methods)]
#[erp(derived_model = "product::models")]
#[allow(dead_code)]
pub struct ProductSaleStock<Mode: IdMode> {
    id: Mode,
}

#[erp_methods]
impl ProductSaleStock<MultipleIds> {
    pub fn create(
        &self,
        env: &mut Environment,
        values: Vec<MapOfFields>,
        sup: Super,
    ) -> Result<MultipleIds> {
        let mut values = values;
        for product in &mut values {
            let goods = product
                .get_option::<&String>("product_type")
                .is_none_or(|kind| kind == "goods");
            if goods && !product.contains_key("invoice_policy") {
                product.insert("invoice_policy", "delivery");
            }
        }
        sup.call_with(values, env)
    }
}
