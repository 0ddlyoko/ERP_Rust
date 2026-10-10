use crate::models::timesheet::Timesheet;
use base::models::BaseUsers;
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::model::ModelVerbs;
use erp::serde_json::{Value, json};
use erp::types::field::{
    Decimal, IdMode, MultipleIds, Reference, SingleId, TimeDelta, Timestamp, Utc,
};
use erp::types::model::MapOfFields;
use erp_search_code_gen::make_domain;
use project::models::{BaseProjectProject, BaseProjectTask, Project, Task};

/// Time someone is spending right now, on a task, on a project, or on nothing said yet: one per
/// person, from when it was started until it is stopped and logged, or discarded.
#[derive(Model)]
#[erp(id = "timesheet_timer", methods)]
#[allow(dead_code)]
pub struct Timer<Mode: IdMode> {
    id: Mode,
    #[erp(required, ondelete = "cascade", index)]
    user: Reference<BaseUsers, SingleId>,
    #[erp(ondelete = "set_null")]
    project: Reference<BaseProjectProject, SingleId>,
    #[erp(ondelete = "set_null")]
    task: Reference<BaseProjectTask, SingleId>,
    started: Timestamp,
    #[erp(label = "Description")]
    name: Option<String>,
}

#[erp_methods]
impl Timer<MultipleIds> {
    /// The caller's timer, if one runs: when it started, on which task, and what for.
    #[erp(rpc)]
    pub fn timer_status(&self, env: &mut Environment) -> Result<Value> {
        let _ = self;
        let Some(timer) = Self::current(env)? else {
            return Ok(json!({ "running": false }));
        };
        timer.describe(env)
    }

    /// Start a timer for the caller, on a task or on none yet. One already running on another
    /// task is stopped and logged first; one on no task is given this one.
    #[erp(rpc)]
    pub fn timer_start(&self, env: &mut Environment, task: Option<u32>) -> Result<Value> {
        let _ = self;
        if let Some(running) = Self::current(env)? {
            let on: Task<SingleId> = running.get_task(&mut env.sudo())?;
            if on.get_optional_id() == task || task.is_none() {
                return running.describe(env);
            }
            if on.is_empty() {
                running.set_task_id(task, env)?;
                return running.describe(env);
            }
            running.log(env, None, None, None)?;
        }
        let Some(uid) = env.uid() else {
            return Err("Nobody to time".into());
        };
        let mut values = MapOfFields::default();
        values.insert("user", uid);
        values.insert("started", Utc::now());
        values.insert_option("task", task);
        let started: Timer<MultipleIds> = env.sudo().create_new_records_from_maps(vec![values])?;
        let timer: Timer<SingleId> = started.ensure_one()?;
        timer.describe(env)
    }

    /// Stop the caller's timer and log the time spent on its task — or, when it has none, on the
    /// task given, else on the project given. With neither, nothing is logged:
    /// `{"needs_project": true}` asks where the time went.
    #[erp(rpc)]
    pub fn timer_stop(
        &self,
        env: &mut Environment,
        project: Option<u32>,
        task: Option<u32>,
        description: Option<String>,
    ) -> Result<Value> {
        let _ = self;
        Self::stop(env, project, task, description)
    }

    /// Stop the caller's timer without logging anything.
    #[erp(rpc)]
    pub fn timer_discard(&self, env: &mut Environment) -> Result<Value> {
        let _ = self;
        Self::discard(env)
    }
}

impl Timer<MultipleIds> {
    /// What `timer_stop` does.
    pub(crate) fn stop(
        env: &mut Environment,
        project: Option<u32>,
        task: Option<u32>,
        description: Option<String>,
    ) -> Result<Value> {
        let Some(timer) = Self::current(env)? else {
            return Ok(json!({ "running": false }));
        };
        let on: Task<SingleId> = timer.get_task(&mut env.sudo())?;
        let within: Project<SingleId> = timer.get_project(&mut env.sudo())?;
        if on.is_empty() && within.is_empty() && task.is_none() && project.is_none() {
            return Ok(json!({ "running": true, "needs_project": true }));
        }
        let entry = timer.log(env, project, task, description)?;
        Ok(json!({ "running": false, "timesheet": entry }))
    }

    /// What `timer_discard` does.
    pub(crate) fn discard(env: &mut Environment) -> Result<Value> {
        if let Some(timer) = Self::current(env)? {
            Timer::<MultipleIds>::from_ids(vec![timer.get_id()], env).delete(&mut env.sudo())?;
        }
        Ok(json!({ "running": false }))
    }

    /// The caller's running timer.
    pub(crate) fn current(env: &mut Environment) -> Result<Option<Timer<SingleId>>> {
        let Some(uid) = env.uid() else {
            return Ok(None);
        };
        let found: Timer<MultipleIds> = env.sudo().search(&make_domain!([("user", "=", uid)]))?;
        Ok(found.into_iter().next())
    }
}

impl Timer<SingleId> {
    /// What a client shows of the timer.
    fn describe(&self, env: &mut Environment) -> Result<Value> {
        let env = &mut *env.sudo();
        let task: Task<SingleId> = self.get_task(env)?;
        let task = if task.is_empty() {
            Value::Null
        } else {
            let number = task.get_number(env)?.clone();
            json!([task.get_id(), format!("{number} {}", task.get_name(env)?)])
        };
        let project: Project<SingleId> = self.get_project(env)?;
        let project = if project.is_empty() {
            Value::Null
        } else {
            json!([project.get_id(), project.get_name(env)?])
        };
        Ok(json!({
            "running": true,
            "started": self.get_started(env)?.to_rfc3339(),
            "project": project,
            "task": task,
            "description": self.get_name(env)?,
        }))
    }

    /// Say how long the timer has run, by moving its start: what it logs follows.
    pub(crate) fn set_elapsed(&self, env: &mut Environment, seconds: i64) -> Result<()> {
        let started = Utc::now() - TimeDelta::seconds(seconds.max(0));
        self.set_started(started, &mut env.sudo())
    }

    /// Remember where the time goes and what for, to log it there once stopped.
    pub(crate) fn remember(
        &self,
        env: &mut Environment,
        project: &Project<SingleId>,
        task: &Task<SingleId>,
        description: Option<String>,
    ) -> Result<Value> {
        {
            let sudo = &mut *env.sudo();
            self.set_project((!project.is_empty()).then_some(project), sudo)?;
            self.set_task((!task.is_empty()).then_some(task), sudo)?;
            self.set_name(description, sudo)?;
        }
        self.describe(env)
    }

    /// Put the timer on a task.
    fn set_task_id(&self, task: Option<u32>, env: &mut Environment) -> Result<()> {
        let task: Option<Task<SingleId>> = task.map(|id| env.get_record(id.into()));
        self.set_task(task.as_ref(), &mut env.sudo())
    }

    /// Log the time since the timer started — whole minutes, one at least — on its task or the
    /// one given, else on its project or the one given, and remove the timer; the timesheet's id.
    fn log(
        &self,
        env: &mut Environment,
        project: Option<u32>,
        task: Option<u32>,
        description: Option<String>,
    ) -> Result<u32> {
        let started = *self.get_started(&mut env.sudo())?;
        let minutes = ((Utc::now() - started).num_seconds().max(0) + 59) / 60;
        let hours = Decimal::from(minutes.max(1)) / Decimal::from(60);
        let own: Task<SingleId> = self.get_task(&mut env.sudo())?;
        let mut values = MapOfFields::default();
        let within: Project<SingleId> = self.get_project(&mut env.sudo())?;
        match own.get_optional_id().or(task) {
            Some(task) => values.insert("task", task),
            None => values.insert(
                "project",
                within
                    .get_optional_id()
                    .or(project)
                    .ok_or("The time is spent on no project")?,
            ),
        }
        values.insert("hours", hours.round_dp(2));
        let description = description.or(self.get_name(&mut env.sudo())?.cloned());
        values.insert_option("name", description);
        let entry: Timesheet<MultipleIds> = env.create_new_records_from_maps(vec![values])?;
        Timer::<MultipleIds>::from_ids(vec![self.get_id()], env).delete(&mut env.sudo())?;
        Ok(entry.get_ids_ref()[0])
    }
}
