use crate::models::timer::Timer;
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::model::ModelVerbs;
use erp::serde_json::{Value, json};
use erp::types::field::{IdMode, MultipleIds, Reference, SingleId, Utc};
use erp::types::model::MapOfFields;
use project::models::{BaseProjectProject, BaseProjectTask, Project, Task};

/// Where the time of a timer went, confirmed whenever it is stopped: the project, a task of it if
/// any, and how long — the time it ran, unless corrected. A record of it lives only while its
/// button runs.
#[derive(Model)]
#[erp(id = "timesheet_timer_stop", methods)]
#[allow(dead_code)]
pub struct TimerStop<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Time spent")]
    elapsed: Option<String>,
    #[erp(ondelete = "cascade", domain = r#"[["is_template", "=", false]]"#)]
    project: Reference<BaseProjectProject, SingleId>,
    #[erp(ondelete = "cascade")]
    task: Reference<BaseProjectTask, SingleId>,
    #[erp(label = "What was done")]
    name: Option<String>,
}

#[erp_methods]
impl TimerStop<MultipleIds> {
    /// The time the caller's timer has run, as `0:07:42`, and where it was said to go, if it was:
    /// its task, and the project of the task when it was given none.
    pub fn default_get(
        env: &mut Environment,
        fields: Vec<String>,
        sup: Super,
    ) -> Result<MapOfFields> {
        let mut defaults = sup.call_with(fields.clone(), env)?;
        let Some(timer) = Timer::<MultipleIds>::current(env)? else {
            return Ok(defaults);
        };
        let asked = |name: &str| fields.iter().any(|field| field == name);
        let sudo = &mut *env.sudo();
        if asked("elapsed") {
            let seconds = (Utc::now() - *timer.get_started(sudo)?)
                .num_seconds()
                .max(0);
            let text = format!(
                "{}:{:02}:{:02}",
                seconds / 3600,
                seconds / 60 % 60,
                seconds % 60
            );
            defaults.insert("elapsed", text);
        }
        if asked("project") {
            let mut project: Project<SingleId> = timer.get_project(sudo)?;
            let task: Task<SingleId> = timer.get_task(sudo)?;
            if project.is_empty() && !task.is_empty() {
                project = task.get_project(sudo)?;
            }
            defaults.insert_option("project", project.get_optional_id());
        }
        if asked("task") {
            let task: Task<SingleId> = timer.get_task(sudo)?;
            defaults.insert_option("task", task.get_optional_id());
        }
        if asked("name") {
            defaults.insert_option("name", timer.get_name(sudo)?.cloned());
        }
        Ok(defaults)
    }

    /// Log the timer's time on the task, else on the project; a task of another project than the
    /// one chosen is refused.
    #[erp(rpc)]
    pub fn action_log(&self, env: &mut Environment) -> Result<Value> {
        let mut answer = json!({ "running": false });
        for asked in self {
            asked.correct_time(env)?;
            let project: Project<SingleId> = asked.get_project(env)?;
            let task: Task<SingleId> = asked.get_task(env)?;
            if project.is_empty() && task.is_empty() {
                return Err("Choose the project the time went to".into());
            }
            if !project.is_empty()
                && !task.is_empty()
                && task
                    .get_project::<Project<SingleId>>(env)?
                    .get_optional_id()
                    != project.get_optional_id()
            {
                return Err("The task is not one of the project".into());
            }
            let description = asked.get_name(env)?.cloned();
            answer = Timer::<MultipleIds>::stop(
                env,
                project.get_optional_id(),
                task.get_optional_id(),
                description,
            )?;
        }
        self.delete(env)?;
        Ok(answer)
    }

    /// Keep the timer running, remembering the project, the task and what for: stopped again, it
    /// logs its time there without asking.
    #[erp(rpc)]
    pub fn action_keep(&self, env: &mut Environment) -> Result<Value> {
        let mut answer = json!({ "running": false });
        if let Some(timer) = Timer::<MultipleIds>::current(env)? {
            for asked in self {
                asked.correct_time(env)?;
                let project: Project<SingleId> = asked.get_project(env)?;
                let task: Task<SingleId> = asked.get_task(env)?;
                let description = asked.get_name(env)?.cloned();
                answer = timer.remember(env, &project, &task, description)?;
            }
        }
        self.delete(env)?;
        Ok(answer)
    }

    /// Forget the timer's time.
    #[erp(rpc)]
    pub fn action_discard(&self, env: &mut Environment) -> Result<Value> {
        let answer = Timer::<MultipleIds>::discard(env)?;
        self.delete(env)?;
        Ok(answer)
    }
}

impl TimerStop<SingleId> {
    /// The time spent, as corrected by the user, becomes what the timer has run.
    fn correct_time(&self, env: &mut Environment) -> Result<()> {
        let Some(text) = self.get_elapsed(env)?.cloned() else {
            return Ok(());
        };
        if let Some(timer) = Timer::<MultipleIds>::current(env)? {
            timer.set_elapsed(env, seconds_of(&text)?)?;
        }
        Ok(())
    }
}

/// A duration as written: `1:30` or `1:30:00` — hours, minutes, seconds — or hours as a number,
/// `1.5`.
fn seconds_of(text: &str) -> Result<i64> {
    let refused = || format!("The time spent is \"{text}\": write it as 1:30 or 1.5");
    let text = text.trim();
    if text.contains(':') {
        let parts: Vec<i64> = text
            .split(':')
            .map(|part| part.trim().parse::<i64>())
            .collect::<std::result::Result<_, _>>()
            .map_err(|_| refused())?;
        let (hours, minutes, seconds) = match parts[..] {
            [hours, minutes] => (hours, minutes, 0),
            [hours, minutes, seconds] => (hours, minutes, seconds),
            _ => return Err(refused().into()),
        };
        if hours < 0 || !(0..60).contains(&minutes) || !(0..60).contains(&seconds) {
            return Err(refused().into());
        }
        return Ok(hours * 3600 + minutes * 60 + seconds);
    }
    let hours: f64 = text.replace(',', ".").parse().map_err(|_| refused())?;
    if !hours.is_finite() || hours < 0.0 {
        return Err(refused().into());
    }
    Ok((hours * 3600.0).round() as i64)
}
