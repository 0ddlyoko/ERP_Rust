use code_gen::{Model, erp_methods, selection};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};
use erp::types::model::MapOfFields;

#[selection]
pub enum LocationUsage {
    #[selection(label = "View")]
    View,
    #[default]
    #[selection(label = "Internal")]
    Internal,
    #[selection(label = "Vendor")]
    Supplier,
    #[selection(label = "Customer")]
    Customer,
    #[selection(label = "Inventory loss")]
    Inventory,
    #[selection(label = "Transit")]
    Transit,
}

/// A place goods are: a shelf of a warehouse, or where they come from and go to — vendors,
/// customers, inventory adjustments.
#[derive(Model)]
#[erp(id = "stock_location", name_field = "complete_name", methods)]
#[allow(dead_code)]
pub struct Location<Mode: IdMode> {
    id: Mode,
    name: String,
    #[erp(label = "Parent location", ondelete = "restrict")]
    parent: Reference<BaseStockLocation, SingleId>,
    #[erp(label = "Sublocations", inverse = "parent")]
    children: Reference<BaseStockLocation, MultipleIds>,
    #[erp(
        label = "Full name",
        compute = "compute_complete_name",
        depends = ["name", "parent.complete_name"],
        stored,
        index = "trigram",
    )]
    complete_name: String,
    #[erp(label = "Type")]
    usage: LocationUsage,
    #[erp(default = true)]
    active: bool,
}

impl Location<SingleId> {
    /// Whether goods here are the company's stock.
    pub fn is_internal(&self, env: &mut Environment) -> Result<bool> {
        if self.is_empty() {
            return Ok(false);
        }
        let env = &mut *env.sudo();
        Ok(matches!(*self.get_usage(env)?, LocationUsage::Internal))
    }

    /// The location and every one under it.
    pub fn and_below(&self, env: &mut Environment) -> Result<Vec<u32>> {
        let env = &mut *env.sudo();
        let mut found = vec![self.get_id()];
        let mut index = 0;
        while index < found.len() {
            let location: Location<SingleId> = env.get_record(found[index].into());
            let children: Location<MultipleIds> = location.get_children(env)?;
            for child in children.get_ids_ref() {
                if !found.contains(child) {
                    found.push(*child);
                }
            }
            index += 1;
        }
        Ok(found)
    }
}

#[erp_methods]
impl Location<MultipleIds> {
    /// `WH/Stock/Shelf 1`: the names from the top down.
    pub fn compute_complete_name(&self, env: &mut Environment) -> Result<()> {
        for location in self {
            let name = location.get_name(env)?.clone();
            let parent: Location<SingleId> = location.get_parent(env)?;
            let complete = if parent.is_empty() {
                name
            } else {
                format!("{}/{name}", parent.get_complete_name(env)?)
            };
            location.set_complete_name(complete, env)?;
        }
        Ok(())
    }

    pub fn write(&self, env: &mut Environment, values: MapOfFields, sup: Super) -> Result<()> {
        env.savepoint(|env| {
            sup.call_with(values, env)?;
            for location in self {
                let mut seen = vec![location.get_id()];
                let mut parent: Location<SingleId> = location.get_parent(env)?;
                while !parent.is_empty() {
                    if seen.contains(&parent.get_id()) {
                        let name = location.get_name(env)?.clone();
                        return Err(
                            format!("The location {name} cannot be placed under itself").into()
                        );
                    }
                    seen.push(parent.get_id());
                    parent = parent.get_parent(env)?;
                }
            }
            Ok(())
        })
    }
}
