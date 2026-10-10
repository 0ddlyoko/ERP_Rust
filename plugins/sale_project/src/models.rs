use base::models::Contact;
use code_gen::{Model, erp_methods, selection};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{Decimal, IdMode, MultipleIds, Reference, SingleId};
use erp::types::model::MapOfFields;
use erp_search_code_gen::make_domain;
use product::models::Product;
use project::models::{BaseProjectProject, BaseProjectTask, Project, Task};
use sale::models::{
    BaseSaleOrder, BaseSaleOrderLine, InvoicePolicy, ProductSale, SaleOrder, SaleOrderLine,
};
use timesheet::models::Timesheet;

#[selection]
pub enum ServiceTracking {
    #[default]
    #[selection(label = "Nothing")]
    No,
    #[selection(label = "A task in a project")]
    Task,
    #[selection(label = "A project of its own, a task per line")]
    Project,
}

/// What selling a service creates: nothing, a task in the project the product names, or a
/// project for the order — started from the template the product names, if it does.
#[derive(Model)]
#[erp(id = "product")]
#[erp(derived_model = "product::models")]
#[allow(dead_code)]
pub struct ProductProject<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Creates on confirmation")]
    service_tracking: ServiceTracking,
    #[erp(label = "Project of its tasks", ondelete = "set_null")]
    service_project: Reference<BaseProjectProject, SingleId>,
    #[erp(
        label = "Project template",
        ondelete = "set_null",
        domain = r#"[["is_template", "=", true]]"#
    )]
    service_template: Reference<BaseProjectProject, SingleId>,
}

/// The order a project was made for.
#[derive(Model)]
#[erp(id = "project_project")]
#[erp(derived_model = "project::models")]
#[allow(dead_code)]
pub struct ProjectSale<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Sales order", ondelete = "set_null", index)]
    sale_order: Reference<BaseSaleOrder, SingleId>,
}

/// The order line a task carries out.
#[derive(Model)]
#[erp(id = "project_task")]
#[erp(derived_model = "project::models")]
#[allow(dead_code)]
pub struct TaskSale<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Order line", ondelete = "set_null", index)]
    sale_line: Reference<BaseSaleOrderLine, SingleId>,
}

/// The tasks carrying out an order line.
#[derive(Model)]
#[erp(id = "sale_order_line")]
#[erp(derived_model = "sale::models")]
#[allow(dead_code)]
pub struct SaleOrderLineProject<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Tasks", inverse = "sale_line")]
    tasks: Reference<BaseProjectTask, MultipleIds>,
}

/// The projects and tasks an order became.
#[derive(Model)]
#[erp(id = "sale_order", methods)]
#[erp(derived_model = "sale::models")]
#[allow(dead_code)]
pub struct SaleOrderProject<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Projects", compute = "compute_work", depends = ["lines.tasks.project"])]
    projects: Reference<BaseProjectProject, MultipleIds>,
    #[erp(label = "Tasks", compute = "compute_work", depends = ["lines.tasks"])]
    tasks: Reference<BaseProjectTask, MultipleIds>,
}

#[erp_methods]
impl SaleOrderProject<MultipleIds> {
    /// The tasks of the order's lines, and the projects holding them or made for the order.
    pub fn compute_work(&self, env: &mut Environment) -> Result<()> {
        for order in self {
            let id = order.get_id();
            let (tasks, projects) = {
                let sudo = &mut *env.sudo();
                let tasks: Task<MultipleIds> =
                    sudo.search(&make_domain!([("sale_line.order", "=", id)]))?;
                let sold: Project<MultipleIds> =
                    sudo.search(&make_domain!([("sale_order", "=", id)]))?;
                let worked_on: Project<MultipleIds> = tasks.get_project(sudo)?;
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
            let order: SaleOrder<SingleId> = env.get_record(order.get_id().into());
            let lines: SaleOrderLine<MultipleIds> = order.get_lines(env)?;
            let mut own_project: Option<u32> = None;
            for line in &lines {
                let product: Product<SingleId> = line.get_product(env)?;
                let Some(product) = product.get_optional_id() else {
                    continue;
                };
                let product: ProductProject<SingleId> = env.sudo().get_record(product.into());
                let tracking = *product.get_service_tracking(&mut env.sudo())?;
                let named: Project<SingleId> = product.get_service_project(&mut env.sudo())?;
                let template: Project<SingleId> = product.get_service_template(&mut env.sudo())?;
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
    template: &Project<SingleId>,
) -> Result<u32> {
    let customer: Contact<SingleId> = order.get_partner(env)?;
    let number = order.get_name(env)?.clone();
    let name = format!("{number} — {}", customer.get_name(env)?);
    let mut values = MapOfFields::default();
    values.insert("name", name);
    values.insert_option("customer", customer.get_optional_id());
    values.insert("sale_order", order.get_id());
    values.insert_option("template", template.get_optional_id());
    let project: Project<MultipleIds> = env.sudo().create_new_records_from_maps(vec![values])?;
    Ok(project.get_ids_ref()[0])
}

/// The budget of a project made for an order: the hours its tasks sell.
fn sell_hours(env: &mut Environment, project: u32) -> Result<()> {
    let env = &mut *env.sudo();
    let tasks: Task<MultipleIds> = env.search(&make_domain!([
        ("project", "=", project),
        ("sale_line", "!=", false)
    ]))?;
    let mut sold = Decimal::ZERO;
    for task in &tasks {
        let task: TaskSale<SingleId> = env.get_record(task.get_id().into());
        let line: SaleOrderLine<SingleId> = task.get_sale_line(env)?;
        sold += *line.get_product_uom_qty(env)?;
    }
    let project: Project<SingleId> = env.get_record(project.into());
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
    let _: Task<MultipleIds> = env.sudo().create_new_records_from_maps(vec![values])?;
    Ok(())
}

/// Time logged on a task counts as delivered on the order line it carries out.
#[derive(Model)]
#[erp(id = "timesheet", methods)]
#[erp(derived_model = "timesheet::models")]
#[allow(dead_code)]
pub struct TimesheetSale<Mode: IdMode> {
    id: Mode,
}

#[erp_methods]
impl TimesheetSale<MultipleIds> {
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
    let tasks: Task<MultipleIds> = entries.get_task(env)?;
    let tasks: TaskSale<MultipleIds> = TaskSale::from_ids(tasks.get_ids(), env);
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
