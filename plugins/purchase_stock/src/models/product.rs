use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{IdMode, MultipleIds};
use erp::types::model::MapOfFields;

/// Goods are billed as received unless said otherwise.
#[derive(Model)]
#[erp(id = "product", methods)]
#[erp(derived_model = "product::models")]
#[allow(dead_code)]
pub struct ProductPurchaseStock<Mode: IdMode> {
    id: Mode,
}

#[erp_methods]
impl ProductPurchaseStock<MultipleIds> {
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
            if goods && !product.contains_key("purchase_method") {
                product.insert("purchase_method", "receive");
            }
        }
        sup.call_with(values, env)
    }
}
