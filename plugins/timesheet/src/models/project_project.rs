use crate::models::project_task::percent;
use crate::models::timesheet::{BaseTimesheet, Timesheet};
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{Decimal, IdMode, MultipleIds, Reference, SingleId};
use project::models::Project;

/// The time logged on a project, all its tasks together.
#[derive(Model)]
#[erp(id = "project_project", methods)]
#[erp(derived_model = "project::models")]
#[allow(dead_code)]
pub struct ProjectTimesheet<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Timesheets", inverse = "project")]
    timesheets: Reference<BaseTimesheet, MultipleIds>,
    #[erp(label = "Hours spent", compute = "compute_hours", depends = ["timesheets", "timesheets.hours", "planned_hours"])]
    spent_hours: Decimal,
    #[erp(label = "Hours left", compute = "compute_hours", depends = ["timesheets", "timesheets.hours", "planned_hours"])]
    remaining_hours: Decimal,
    #[erp(label = "Budget spent (%)", compute = "compute_hours", depends = ["timesheets", "timesheets.hours", "planned_hours"])]
    progress: i32,
}

#[erp_methods]
impl ProjectTimesheet<MultipleIds> {
    /// The hours logged on the project, what is left of those planned, and how much of them is
    /// spent.
    pub fn compute_hours(&self, env: &mut Environment) -> Result<()> {
        for project in self {
            let entries: Timesheet<MultipleIds> = project.get_timesheets(env)?;
            let spent: Decimal = entries.sum(env, |entry, env| Ok(*entry.get_hours(env)?))?;
            let own: Project<SingleId> = project.as_model();
            let planned = *own.get_planned_hours(env)?;
            project.set_spent_hours(spent, env)?;
            project.set_remaining_hours(planned - spent, env)?;
            project.set_progress(percent(spent, planned), env)?;
        }
        Ok(())
    }
}
