use crate::models::product::{ProductSaleProject, ServiceTracking};
use crate::models::project_task::ProjectTaskSaleProject;
use base::models::Contact;
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{Decimal, IdMode, MultipleIds, Reference, SingleId};
use erp::types::model::MapOfFields;
use erp_search_code_gen::make_domain;
use product::models::Product;
use project::models::{BaseProjectProject, BaseProjectTask, ProjectProject, ProjectTask};
use sale::models::{SaleOrder, SaleOrderLine};

/// The projects and tasks an order became.
#[derive(Model)]
#[erp(id = "sale_order", methods)]
#[erp(derived_model = "sale::models")]
#[allow(dead_code)]
pub struct SaleOrderSaleProject<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Projects", compute = "compute_work", depends = ["lines.tasks.project"])]
    projects: Reference<BaseProjectProject, MultipleIds>,
    #[erp(label = "Tasks", compute = "compute_work", depends = ["lines.tasks"])]
    tasks: Reference<BaseProjectTask, MultipleIds>,
}

#[erp_methods]
impl SaleOrderSaleProject<MultipleIds> {
    /// The tasks of the order's lines, and the projects holding them or made for the order.
    pub fn compute_work(&self, env: &mut Environment) -> Result<()> {
        for order in self {
            let id = order.get_id();
            let (tasks, projects) = {
                let sudo = &mut *env.sudo();
                let tasks: ProjectTask<MultipleIds> =
                    sudo.search(&make_domain!([("sale_line.order", "=", id)]))?;
                let sold: ProjectProject<MultipleIds> =
                    sudo.search(&make_domain!([("sale_order", "=", id)]))?;
                let worked_on: ProjectProject<MultipleIds> = tasks.get_project(sudo)?;
                (tasks, sold | worked_on)
            };
            order.set_projects(&projects, env)?;
            order.set_tasks(&tasks, env)?;
        }
        Ok(())
    }

    /// A confirmed order's services become work: a task in the project their product names, or
    /// in a project made for the order, one task per line.
    pub fn on_confirmed(&self, env: &mut Environment, sup: Super) -> Result<()> {
        sup.call(env)?;
        for order in self {
            let order: SaleOrder<SingleId> = order.as_model();
            let lines: SaleOrderLine<MultipleIds> = order.get_lines(env)?;
            let mut own_project: Option<u32> = None;
            for line in &lines {
                let product: Product<SingleId> = line.get_product(env)?;
                let Some(product) = product.get_optional_id() else {
                    continue;
                };
                let product: ProductSaleProject<SingleId> = env.sudo().get_record(product.into());
                let tracking = *product.get_service_tracking(&mut env.sudo())?;
                let named: ProjectProject<SingleId> =
                    product.get_service_project(&mut env.sudo())?;
                let template: ProjectProject<SingleId> =
                    product.get_service_template(&mut env.sudo())?;
                let project = match tracking {
                    ServiceTracking::No => continue,
                    ServiceTracking::Task if !named.is_empty() => named.get_id(),
                    _ => match own_project {
                        Some(project) => project,
                        None => {
                            let project = project_for(env, &order, &template)?;
                            own_project = Some(project);
                            project
                        }
                    },
                };
                task_for(env, &order, &line, project)?;
            }
            if let Some(project) = own_project {
                sell_hours(env, project)?;
            }
        }
        Ok(())
    }
}

/// A project made for an order: named after it and its customer, for that customer, started from
/// the template of the first product asking for a project, if it names one.
fn project_for(
    env: &mut Environment,
    order: &SaleOrder<SingleId>,
    template: &ProjectProject<SingleId>,
) -> Result<u32> {
    let customer: Contact<SingleId> = order.get_partner(env)?;
    let number = order.get_name(env)?.clone();
    let name = format!("{number} — {}", customer.get_name(env)?);
    let mut values = MapOfFields::default();
    values.insert("name", name);
    values.insert_option("customer", customer.get_optional_id());
    values.insert("sale_order", order.get_id());
    values.insert_option("template", template.get_optional_id());
    let project: ProjectProject<MultipleIds> =
        env.sudo().create_new_records_from_maps(vec![values])?;
    Ok(project.get_ids_ref()[0])
}

/// The budget of a project made for an order: the hours its tasks sell.
fn sell_hours(env: &mut Environment, project: u32) -> Result<()> {
    let env = &mut *env.sudo();
    let tasks: ProjectTask<MultipleIds> = env.search(&make_domain!([
        ("project", "=", project),
        ("sale_line", "!=", false)
    ]))?;
    let sold: Decimal = tasks.sum(env, |task, env| {
        let task: ProjectTaskSaleProject<SingleId> = task.as_model();
        let line: SaleOrderLine<SingleId> = task.get_sale_line(env)?;
        Ok(*line.get_product_uom_qty(env)?)
    })?;
    let project: ProjectProject<SingleId> = env.get_record(project.into());
    project.set_planned_hours(sold, env)
}

/// A task carrying out an order line, planned for as many hours as the line sells.
fn task_for(
    env: &mut Environment,
    order: &SaleOrder<SingleId>,
    line: &SaleOrderLine<SingleId>,
    project: u32,
) -> Result<()> {
    let label = line.get_name(env)?.cloned().unwrap_or_default();
    let mut values = MapOfFields::default();
    values.insert("name", format!("{}: {label}", order.get_name(env)?));
    values.insert("project", project);
    values.insert("sale_line", line.get_id());
    values.insert("planned_hours", *line.get_product_uom_qty(env)?);
    values.insert("description", label);
    let _: ProjectTask<MultipleIds> = env.sudo().create_new_records_from_maps(vec![values])?;
    Ok(())
}
