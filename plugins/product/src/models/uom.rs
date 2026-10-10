use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{IdMode, MultipleIds, SingleId};
use erp_search_code_gen::make_domain;
use uom::models::Uom;

/// A unit products are counted or bought in keeps measuring what it measures.
#[derive(Model)]
#[erp(id = "uom", methods)]
#[erp(derived_model = "uom::models")]
#[allow(dead_code)]
pub struct UomProduct<Mode: IdMode> {
    id: Mode,
}

#[erp_methods]
impl UomProduct<MultipleIds> {
    /// A unit products use cannot move to another category: their quantities and prices would
    /// suddenly count something else.
    #[erp(on = ["category"])]
    pub fn check_not_used(&self, env: &mut Environment) -> Result<()> {
        let env = &mut *env.sudo();
        for uom in self {
            let id = uom.get_id();
            let used = env.count(
                "product",
                &make_domain!(["|", ("uom", "=", id), ("purchase_uom", "=", id)]),
            )?;
            if used > 0 {
                let uom: Uom<SingleId> = env.get_record(id.into());
                return Err(format!(
                    "Products are counted or bought in {}: it cannot measure something else",
                    uom.get_name(env)?
                )
                .into());
            }
        }
        Ok(())
    }
}
