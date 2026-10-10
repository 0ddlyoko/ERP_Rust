use crate::models::project_task::ProjectTaskSaleProject;
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{Decimal, IdMode, MultipleIds, SingleId};
use erp::types::model::MapOfFields;
use erp_search_code_gen::make_domain;
use product::models::Product;
use project::models::ProjectTask;
use sale::models::{InvoicePolicy, ProductSale, SaleOrderLine};
use timesheet::models::Timesheet;

/// Time logged on a task counts as delivered on the order line it carries out.
#[derive(Model)]
#[erp(id = "timesheet", methods)]
#[erp(derived_model = "timesheet::models")]
#[allow(dead_code)]
pub struct TimesheetSaleProject<Mode: IdMode> {
    id: Mode,
}

#[erp_methods]
impl TimesheetSaleProject<MultipleIds> {
    pub fn create(
        &self,
        env: &mut Environment,
        values: Vec<MapOfFields>,
        sup: Super,
    ) -> Result<MultipleIds> {
        let ids = sup.call_with(values, env)?;
        let lines = lines_of(env, ids.get_ids_ref())?;
        refresh_delivered(env, lines)?;
        Ok(ids)
    }

    pub fn write(&self, env: &mut Environment, values: MapOfFields, sup: Super) -> Result<()> {
        let before = lines_of(env, self.get_ids_ref())?;
        sup.call_with(values, env)?;
        let after = lines_of(env, self.get_ids_ref())?;
        refresh_delivered(env, before + after)
    }

    pub fn delete(&self, env: &mut Environment, sup: Super) -> Result<u32> {
        let lines = lines_of(env, self.get_ids_ref())?;
        let deleted = sup.call(env)?;
        refresh_delivered(env, lines)?;
        Ok(deleted)
    }
}

/// The order lines carried out by the tasks these entries are logged on.
fn lines_of(env: &mut Environment, entries: &[u32]) -> Result<SaleOrderLine<MultipleIds>> {
    let env = &mut *env.sudo();
    let entries = Timesheet::<MultipleIds>::from_ids(entries.to_vec(), env);
    let tasks: ProjectTask<MultipleIds> = entries.get_task(env)?;
    let tasks: ProjectTaskSaleProject<MultipleIds> = tasks.as_model();
    tasks.get_sale_line(env)
}

/// What is delivered of a service invoiced as delivered: the hours logged on its tasks.
fn refresh_delivered(env: &mut Environment, lines: SaleOrderLine<MultipleIds>) -> Result<()> {
    let env = &mut *env.sudo();
    for line in &lines {
        let product: Product<SingleId> = line.get_product(env)?;
        let Some(product) = product.get_optional_id() else {
            continue;
        };
        let product: ProductSale<SingleId> = env.get_record(product.into());
        if !matches!(*product.get_invoice_policy(env)?, InvoicePolicy::Delivery) {
            continue;
        }
        let entries: Timesheet<MultipleIds> =
            env.search(&make_domain!([("task.sale_line", "=", line.get_id())]))?;
        let mut hours = Decimal::ZERO;
        for entry in &entries {
            hours += *entry.get_hours(env)?;
        }
        line.set_qty_delivered(hours, env)?;
    }
    Ok(())
}
