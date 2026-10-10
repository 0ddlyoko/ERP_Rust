use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};

/// A family of products, within a parent family: `All / Saleable / Office furniture`.
#[derive(Model)]
#[erp(
    id = "product_category",
    order = "name, id",
    name_field = "complete_name",
    methods
)]
#[allow(dead_code)]
pub struct ProductCategory<Mode: IdMode> {
    id: Mode,
    name: String,
    #[erp(label = "Parent category", ondelete = "restrict")]
    parent: Reference<BaseProductCategory, SingleId>,
    #[erp(label = "Subcategories", inverse = "parent")]
    children: Reference<BaseProductCategory, MultipleIds>,
    #[erp(
        label = "Full name",
        compute = "compute_complete_name",
        depends = ["name", "parent.complete_name"],
        stored,
        index = "trigram",
    )]
    complete_name: String,
}

#[erp_methods]
impl ProductCategory<MultipleIds> {
    /// A category is never its own ancestor.
    pub fn check_no_cycle(&self, env: &mut Environment) -> Result<()> {
        for category in self {
            let mut seen = vec![category.get_id()];
            let mut parent: ProductCategory<SingleId> = category.get_parent(env)?;
            while !parent.is_empty() {
                if seen.contains(&parent.get_id()) {
                    let name = category.get_name(env)?.clone();
                    return Err(format!("The category {name} cannot be placed under itself").into());
                }
                seen.push(parent.get_id());
                parent = parent.get_parent(env)?;
            }
        }
        Ok(())
    }

    /// The names from the top family down, `All / Saleable`.
    pub fn compute_complete_name(&self, env: &mut Environment) -> Result<()> {
        for category in self {
            let name = category.get_name(env)?.clone();
            let parent: ProductCategory<SingleId> = category.get_parent(env)?;
            let complete = if parent.is_empty() {
                name
            } else {
                format!("{} / {name}", parent.get_complete_name(env)?)
            };
            category.set_complete_name(complete, env)?;
        }
        Ok(())
    }
}
