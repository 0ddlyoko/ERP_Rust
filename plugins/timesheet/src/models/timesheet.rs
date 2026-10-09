use base::models::BaseUsers;
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{Decimal, IdMode, MultipleIds, NaiveDate, Reference, SingleId, Utc};
use erp::types::model::MapOfFields;
use project::models::{BaseProjectProject, BaseProjectTask, Project, Task};

/// Time someone spent on a task, or on a project with no task in particular, on a day.
#[derive(Model)]
#[erp(id = "timesheet", order = "date desc, id desc", methods)]
#[allow(dead_code)]
pub struct Timesheet<Mode: IdMode> {
    id: Mode,
    #[erp(index)]
    date: NaiveDate,
    #[erp(label = "Employee", ondelete = "restrict", index)]
    user: Reference<BaseUsers, SingleId>,
    #[erp(
        ondelete = "cascade",
        index,
        domain = "project ? [['project', '=', project]] : []"
    )]
    task: Reference<BaseProjectTask, SingleId>,
    #[erp(ondelete = "cascade", index)]
    project: Reference<BaseProjectProject, SingleId>,
    #[erp(label = "Description")]
    name: Option<String>,
    #[erp(default = 0.0)]
    hours: Decimal,
}

#[erp_methods]
impl Timesheet<MultipleIds> {
    /// Time is spent on a project, and counted in hours that are not negative.
    pub fn check_entries(&self, env: &mut Environment) -> Result<()> {
        for entry in self {
            let project: Project<SingleId> = entry.get_project(env)?;
            if project.is_empty() {
                return Err("Time is spent on a project: give the task or the project".into());
            }
            if *entry.get_hours(env)? < Decimal::ZERO {
                return Err("The hours spent cannot be negative".into());
            }
        }
        Ok(())
    }

    /// Time is logged today, by whoever logs it.
    pub fn default_get(
        env: &mut Environment,
        fields: Vec<String>,
        sup: Super,
    ) -> Result<MapOfFields> {
        let mut defaults = sup.call_with(fields.clone(), env)?;
        if fields.iter().any(|field| field == "date") {
            defaults.insert("date", Utc::now().date_naive());
        }
        if fields.iter().any(|field| field == "user")
            && let Some(uid) = env.uid()
        {
            defaults.insert("user", uid);
        }
        Ok(defaults)
    }

    /// Time logged on a task is logged on its project.
    pub fn create(
        &self,
        env: &mut Environment,
        values: Vec<MapOfFields>,
        sup: Super,
    ) -> Result<MultipleIds> {
        let mut values = values;
        for entry in &mut values {
            project_of_task(env, entry)?;
        }
        env.savepoint(|env| {
            let ids: MultipleIds = sup.call_with(values, env)?;
            Timesheet::<MultipleIds>::from_ids(ids.get_ids_ref().clone(), env)
                .check_entries(env)?;
            Ok(ids)
        })
    }

    pub fn write(&self, env: &mut Environment, values: MapOfFields, sup: Super) -> Result<()> {
        let mut values = values;
        project_of_task(env, &mut values)?;
        env.savepoint(|env| {
            sup.call_with(values, env)?;
            self.check_entries(env)
        })
    }
}

/// Set the project of the task the values name, if they name one.
fn project_of_task(env: &mut Environment, values: &mut MapOfFields) -> Result<()> {
    let Some(task) = values
        .get_option::<&u32>("task")
        .copied()
        .filter(|id| *id != 0)
    else {
        return Ok(());
    };
    let task: Task<SingleId> = env.get_record(task.into());
    let project: Project<SingleId> = task.get_project(&mut env.sudo())?;
    values.insert_option("project", project.get_optional_id());
    Ok(())
}
