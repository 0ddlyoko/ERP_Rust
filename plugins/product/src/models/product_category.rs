use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};
use erp::types::model::MapOfFields;

/// A family of products, within a parent family: `All / Saleable / Office furniture`.
#[derive(Model)]
#[erp(id = "product_category", name_field = "complete_name", methods)]
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
        stored
    )]
    complete_name: String,
}

#[erp_methods]
impl ProductCategory<MultipleIds> {
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

    pub fn write(&self, env: &mut Environment, values: MapOfFields, sup: Super) -> Result<()> {
        let renames = values.contains_key("name") || values.contains_key("parent");
        env.savepoint(|env| {
            sup.call_with(values, env)?;
            self.check_no_cycle(env)?;
            if renames {
                self.rename_descendants(env)?;
            }
            Ok(())
        })
    }

    /// Name the subcategories again, a level at a time from the top: the ORM recomputes the
    /// children of a renamed category, not their own children.
    pub fn rename_descendants(&self, env: &mut Environment) -> Result<()> {
        let mut level: Vec<u32> = self.get_ids_ref().clone();
        while !level.is_empty() {
            let mut next = Vec::new();
            for id in level {
                let category: ProductCategory<SingleId> = env.get_record(id.into());
                let children: ProductCategory<MultipleIds> = category.get_children(env)?;
                next.extend(children.get_ids_ref().iter().copied());
            }
            ProductCategory::<MultipleIds>::from_ids(next.clone(), env)
                .compute_complete_name(env)?;
            level = next;
        }
        Ok(())
    }

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
}
