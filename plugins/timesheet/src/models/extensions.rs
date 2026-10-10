use crate::models::timer::Timer;
use crate::models::timesheet::{BaseTimesheet, Timesheet};
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{Decimal, IdMode, MultipleIds, Reference, SingleId};
use erp_search_code_gen::make_domain;
use project::models::{Project, Task};

/// The time logged on a task, against the hours planned for it.
#[derive(Model)]
#[erp(id = "project_task", methods)]
#[erp(derived_model = "project::models")]
#[allow(dead_code)]
pub struct TaskTimesheet<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Timesheets", inverse = "task")]
    timesheets: Reference<BaseTimesheet, MultipleIds>,
    #[erp(label = "Hours spent", compute = "compute_hours", depends = ["timesheets", "timesheets.hours", "planned_hours"], stored)]
    spent_hours: Decimal,
    #[erp(label = "Hours left", compute = "compute_hours", depends = ["timesheets", "timesheets.hours", "planned_hours"], stored)]
    remaining_hours: Decimal,
    #[erp(label = "Progress (%)", compute = "compute_hours", depends = ["timesheets", "timesheets.hours", "planned_hours"], stored)]
    progress: i32,
    #[erp(label = "My timer runs", compute = "compute_timer_running")]
    timer_running: bool,
}

#[erp_methods]
impl TaskTimesheet<MultipleIds> {
    /// Whether the caller's timer runs on the task.
    pub fn compute_timer_running(&self, env: &mut Environment) -> Result<()> {
        let running: Vec<u32> = match env.uid() {
            Some(uid) => {
                let timers: Timer<MultipleIds> =
                    env.sudo().search(&make_domain!([("user", "=", uid)]))?;
                let on: Task<MultipleIds> = timers.get_task(&mut env.sudo())?;
                on.get_ids()
            }
            None => Vec::new(),
        };
        for task in self {
            task.set_timer_running(running.contains(&task.get_id()), env)?;
        }
        Ok(())
    }

    /// The hours logged, what is left of those planned, and how much of them is spent.
    pub fn compute_hours(&self, env: &mut Environment) -> Result<()> {
        for task in self {
            let entries: Timesheet<MultipleIds> = task.get_timesheets(env)?;
            let mut spent = Decimal::ZERO;
            for entry in &entries {
                spent += *entry.get_hours(env)?;
            }
            let own: Task<SingleId> = env.get_record(task.get_id().into());
            let planned = *own.get_planned_hours(env)?;
            task.set_spent_hours(spent, env)?;
            task.set_remaining_hours(planned - spent, env)?;
            task.set_progress(percent(spent, planned), env)?;
        }
        Ok(())
    }
}

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
            let mut spent = Decimal::ZERO;
            for entry in &entries {
                spent += *entry.get_hours(env)?;
            }
            let own: Project<SingleId> = env.get_record(project.get_id().into());
            let planned = *own.get_planned_hours(env)?;
            project.set_spent_hours(spent, env)?;
            project.set_remaining_hours(planned - spent, env)?;
            project.set_progress(percent(spent, planned), env)?;
        }
        Ok(())
    }
}

/// `spent` as a whole percentage of `planned`; none planned is none spent of it.
fn percent(spent: Decimal, planned: Decimal) -> i32 {
    if planned <= Decimal::ZERO {
        return 0;
    }
    i32::try_from((spent * Decimal::ONE_HUNDRED / planned).round().mantissa()).unwrap_or(i32::MAX)
}
