use crate::models::product_category::BaseProductCategory;
use code_gen::{Model, erp_methods, selection};
use erp::Result;
use erp::data;
use erp::environment::Environment;
use erp::types::field::{Decimal, IdMode, MultipleIds, Reference, SingleId};
use erp::types::model::MapOfFields;
use erp_search_code_gen::make_domain;
use uom::models::{BaseUom, Uom, UomCategory};

#[selection]
pub enum ProductType {
    #[default]
    #[selection(label = "Goods")]
    Goods,
    #[selection(label = "Service")]
    Service,
    #[selection(label = "Consumable")]
    Consumable,
}

/// Something the business sells, buys or keeps: goods counted in a unit, or a service.
#[derive(Model)]
#[erp(
    id = "product",
    order = "name, id",
    name_field = "display_name",
    methods
)]
#[allow(dead_code)]
pub struct Product<Mode: IdMode> {
    id: Mode,
    #[erp(tracking, index = "trigram")]
    name: String,
    #[erp(label = "Internal reference", tracking, index)]
    default_code: Option<String>,
    #[erp(index)]
    barcode: Option<String>,
    #[erp(
        label = "Name",
        compute = "compute_display_name",
        depends = ["name", "default_code"],
        stored,
        index = "trigram",
    )]
    display_name: String,
    #[erp(label = "Product type", tracking)]
    product_type: ProductType,
    #[erp(required, ondelete = "restrict", tracking)]
    category: Reference<BaseProductCategory, SingleId>,
    #[erp(label = "Sales price", default = 0.0, tracking)]
    list_price: Decimal,
    #[erp(
        label = "Cost",
        default = 0.0,
        tracking,
        description = "What one unit costs the company, in its currency"
    )]
    standard_price: Decimal,
    #[erp(label = "Unit of measure", required, ondelete = "restrict", tracking)]
    uom: Reference<BaseUom, SingleId>,
    #[erp(label = "Purchase unit", required, ondelete = "restrict")]
    purchase_uom: Reference<BaseUom, SingleId>,
    #[erp(label = "Can be sold", default = true)]
    sale_ok: bool,
    #[erp(label = "Can be purchased", default = true)]
    purchase_ok: bool,
    #[erp(label = "Sales description")]
    description_sale: Option<String>,
    #[erp(label = "Purchase description")]
    description_purchase: Option<String>,
    #[erp(label = "Internal notes")]
    description: Option<String>,
    #[erp(label = "Weight (kg)", default = 0.0)]
    weight: Decimal,
    #[erp(label = "Volume (m³)", default = 0.0)]
    volume: Decimal,
    #[erp(default = true, tracking)]
    active: bool,
}

#[erp_methods]
impl Product<MultipleIds> {
    /// The purchase unit measures what the unit does; a barcode belongs to one product.
    ///
    /// Prices may be negative: a discount or a deposit given back is a product too.
    #[erp(check = ["uom", "purchase_uom", "barcode"])]
    pub fn check_product(&self, env: &mut Environment) -> Result<()> {
        for product in self {
            let name = product.get_name(env)?.clone();
            let uom: Uom<SingleId> = product.get_uom(env)?;
            let purchase_uom: Uom<SingleId> = product.get_purchase_uom(env)?;
            let uom_category: UomCategory<SingleId> = uom.get_category(env)?;
            let purchase_category: UomCategory<SingleId> = purchase_uom.get_category(env)?;
            if uom_category.get_id() != purchase_category.get_id() {
                return Err(format!(
                    "The purchase unit of {name} must measure the same thing as its unit"
                )
                .into());
            }
            if let Some(barcode) = product.get_barcode(env)?.cloned() {
                let env = &mut *env.sudo();
                let same = env.count(
                    "product",
                    &make_domain!([("barcode", "=", barcode.clone())]),
                )?;
                if same > 1 {
                    return Err(format!(
                        "The barcode {barcode} is already used by another product"
                    )
                    .into());
                }
            }
        }
        Ok(())
    }

    /// `[REF] Name` when the product has a reference, its name otherwise.
    pub fn compute_display_name(&self, env: &mut Environment) -> Result<()> {
        for product in self {
            let name = product.get_name(env)?.clone();
            let display = match product.get_default_code(env)? {
                Some(code) if !code.trim().is_empty() => format!("[{}] {name}", code.trim()),
                _ => name,
            };
            product.set_display_name(display, env)?;
        }
        Ok(())
    }

    /// What a product is created with when nothing is said: the category `All`, counted in
    /// units, and bought in the unit it is counted in.
    pub fn create(
        &self,
        env: &mut Environment,
        values: Vec<MapOfFields>,
        sup: Super,
    ) -> Result<MultipleIds> {
        let mut values = values;
        let all = data::resolve(env, "product.category_all")?;
        let unit = data::resolve(env, "uom.uom_unit")?;
        for product in &mut values {
            let missing = |product: &MapOfFields, field: &str| {
                product.get_option::<&u32>(field).is_none_or(|id| *id == 0)
            };
            if missing(product, "category")
                && let Some(all) = all
            {
                product.insert("category", all);
            }
            if missing(product, "uom")
                && let Some(unit) = unit
            {
                product.insert("uom", unit);
            }
            if missing(product, "purchase_uom")
                && let Some(uom) = product.get_option::<&u32>("uom").copied()
            {
                product.insert("purchase_uom", uom);
            }
        }
        Ok(sup.call_with(values, env)?)
    }

    /// Archive the products: they are no longer offered, and stay where they are used.
    #[erp(rpc)]
    pub fn archive(&self, env: &mut Environment) -> Result<bool> {
        self.set_active(false, env)?;
        Ok(true)
    }

    /// Bring archived products back.
    #[erp(rpc)]
    pub fn unarchive(&self, env: &mut Environment) -> Result<bool> {
        self.set_active(true, env)?;
        Ok(true)
    }
}
