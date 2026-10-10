use crate::models::checklist::{BaseProjectChecklistItem, ChecklistItem};
use crate::models::project::{BaseProjectProject, Project};
use crate::models::stage::{BaseProjectStage, Stage};
use crate::models::tag::BaseProjectTag;
use base::models::{BaseContact, BaseUsers, Contact};
use code_gen::{Model, erp_methods, selection};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{Decimal, IdMode, MultipleIds, NaiveDate, Reference, SingleId, Utc};
use erp::types::model::MapOfFields;
use sequence::models::Sequence;
use std::collections::HashMap;

#[selection]
pub enum TaskStatus {
    #[default]
    #[selection(label = "In progress")]
    InProgress,
    #[selection(label = "Waiting")]
    Waiting,
    #[selection(label = "Ready")]
    Ready,
    #[selection(label = "Blocked")]
    Blocked,
}

#[selection]
pub enum Priority {
    #[default]
    #[selection(label = "Normal")]
    Normal,
    #[selection(label = "High")]
    High,
    #[selection(label = "Urgent")]
    Urgent,
}

/// A piece of work on a project's board: a card in one of its columns, numbered `T-0001`, given
/// to people, with a deadline, a priority, hours planned and subtasks.
#[derive(Model)]
#[erp(
    id = "project_task",
    contact_field = "customer",
    order = "sequence, id",
    methods
)]
#[allow(dead_code)]
pub struct Task<Mode: IdMode> {
    id: Mode,
    #[erp(label = "Title", tracking, index = "trigram")]
    name: String,
    #[erp(label = "Number", default = "/", index = "trigram")]
    number: String,
    #[erp(default = true)]
    active: bool,
    #[erp(required, ondelete = "cascade", tracking, index)]
    project: Reference<BaseProjectProject, SingleId>,
    #[erp(
        label = "Stage",
        ondelete = "set_null",
        tracking,
        index,
        domain = "[['project', '=', project]]"
    )]
    stage: Reference<BaseProjectStage, SingleId>,
    #[erp(default = 10)]
    sequence: i32,
    #[erp(label = "Assigned to", relation = "project_task_user_rel", tracking)]
    assignees: Reference<BaseUsers, MultipleIds>,
    #[erp(relation = "project_task_tag_rel")]
    tags: Reference<BaseProjectTag, MultipleIds>,
    #[erp(tracking)]
    priority: Priority,
    #[erp(label = "Deadline", tracking, index)]
    date_deadline: Option<NaiveDate>,
    #[erp(label = "Planned hours", default = 0.0, tracking)]
    planned_hours: Decimal,
    description: Option<String>,
    #[erp(
        label = "Parent task",
        ondelete = "cascade",
        index,
        domain = "[['project', '=', project], ['id', '!=', id]]"
    )]
    parent: Reference<BaseProjectTask, SingleId>,
    #[erp(label = "Subtasks", inverse = "parent")]
    children: Reference<BaseProjectTask, MultipleIds>,
    #[erp(compute = "compute_customer", depends = ["project", "project.customer"], stored)]
    customer: Reference<BaseContact, SingleId>,
    #[erp(label = "Done", compute = "compute_is_closed", depends = ["stage", "stage.is_closed"], stored, index)]
    is_closed: bool,
    #[erp(label = "Subtasks done", compute = "compute_subtasks", depends = ["children", "children.is_closed"])]
    subtasks_done: Option<String>,
    #[erp(
        label = "Waiting for",
        relation = "project_task_dependency_rel",
        relation_columns = "task_id,depends_on_id",
        domain = "[['project', '=', project], ['id', '!=', id]]"
    )]
    depends_on: Reference<BaseProjectTask, MultipleIds>,
    #[erp(
        label = "Blocks",
        relation = "project_task_dependency_rel",
        relation_columns = "depends_on_id,task_id"
    )]
    blocking: Reference<BaseProjectTask, MultipleIds>,
    #[erp(label = "Checklist", inverse = "task", owned)]
    checklist: Reference<BaseProjectChecklistItem, MultipleIds>,
    #[erp(label = "Checklist done", compute = "compute_checklist", depends = ["checklist", "checklist.done"])]
    checklist_done: Option<String>,
    #[erp(
        label = "On the board",
        compute = "compute_on_board",
        depends = ["parent", "project", "project.show_subtasks"],
        stored,
        index
    )]
    on_board: bool,
    #[erp(tracking, index)]
    status: TaskStatus,
}

#[erp_methods]
impl Task<SingleId> {
    /// Whether the task is held up: one of the tasks it waits for is in no column that unlocks
    /// it, or one of its subtasks is blocked.
    pub fn held_up(&self, env: &mut Environment) -> Result<bool> {
        Ok(self.waits(env)? || self.has_blocked_subtask(env)?)
    }

    /// Whether one of its subtasks is blocked, or has one that is.
    pub fn has_blocked_subtask(&self, env: &mut Environment) -> Result<bool> {
        let children: Task<MultipleIds> = self.get_children(env)?;
        for child in &children {
            if matches!(*child.get_status(env)?, TaskStatus::Blocked) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Whether one of the tasks this one waits for is in no column that unlocks it.
    pub fn waits(&self, env: &mut Environment) -> Result<bool> {
        let awaited: Task<MultipleIds> = self.get_depends_on(env)?;
        let env = &mut *env.sudo();
        for other in &awaited {
            let stage: Stage<SingleId> = other.get_stage(env)?;
            if stage.is_empty() || !*stage.get_unlocks(env)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

#[erp_methods]
impl Task<MultipleIds> {
    /// A task's column is one of its project's board — none without a project.
    pub fn check_stages(&self, env: &mut Environment) -> Result<()> {
        for task in self {
            let stage: Stage<SingleId> = task.get_stage(env)?;
            if stage.is_empty() {
                continue;
            }
            let project: Project<SingleId> = task.get_project(env)?;
            let board: Project<SingleId> = stage.get_project(&mut env.sudo())?;
            if board.get_optional_id() != project.get_optional_id() {
                let name = stage.get_name(&mut env.sudo())?.clone();
                return Err(format!("The column {name} is not one of the task's project").into());
            }
        }
        Ok(())
    }

    /// The customer of the project.
    pub fn compute_customer(&self, env: &mut Environment) -> Result<()> {
        for task in self {
            let project: Project<SingleId> = task.get_project(env)?;
            let customer: Contact<SingleId> = if project.is_empty() {
                env.get_record(SingleId::empty())
            } else {
                project.get_customer(&mut env.sudo())?
            };
            task.set_customer(&customer, env)?;
        }
        Ok(())
    }

    /// Done once in a closing column.
    pub fn compute_is_closed(&self, env: &mut Environment) -> Result<()> {
        for task in self {
            let stage: Stage<SingleId> = task.get_stage(env)?;
            let closed = !stage.is_empty() && *stage.get_is_closed(&mut env.sudo())?;
            task.set_is_closed(closed, env)?;
        }
        Ok(())
    }

    /// `2/5`: the steps of the checklist ticked, out of all of them; nothing without a checklist.
    pub fn compute_checklist(&self, env: &mut Environment) -> Result<()> {
        for task in self {
            let steps: ChecklistItem<MultipleIds> = task.get_checklist(env)?;
            let total = steps.get_ids_ref().len();
            let summary = if total == 0 {
                None
            } else {
                let mut done = 0;
                for step in &steps {
                    if *step.get_done(env)? {
                        done += 1;
                    }
                }
                Some(format!("{done}/{total}"))
            };
            task.set_checklist_done(summary, env)?;
        }
        Ok(())
    }

    /// A task shows on its project's board unless it is a subtask, which the project may show too.
    pub fn compute_on_board(&self, env: &mut Environment) -> Result<()> {
        for task in self {
            let parent: Task<SingleId> = task.get_parent(env)?;
            let shown = parent.is_empty() || {
                let project: Project<SingleId> = task.get_project(env)?;
                !project.is_empty() && *project.get_show_subtasks(&mut env.sudo())?
            };
            task.set_on_board(shown, env)?;
        }
        Ok(())
    }

    /// `3 / 5`: the subtasks done, out of all of them; nothing without subtasks.
    pub fn compute_subtasks(&self, env: &mut Environment) -> Result<()> {
        for task in self {
            let children: Task<MultipleIds> = task.get_children(env)?;
            let total = children.get_ids_ref().len();
            let summary = if total == 0 {
                None
            } else {
                let mut done = 0;
                for child in &children {
                    if *child.get_is_closed(env)? {
                        done += 1;
                    }
                }
                Some(format!("{done} / {total}"))
            };
            task.set_subtasks_done(summary, env)?;
        }
        Ok(())
    }

    /// A task is numbered when created, in the first column of its project unless given one of
    /// its project's columns; a subtask is of its parent's project unless said otherwise. The
    /// tasks created together are numbered at once, each project's first column found once.
    pub fn create(
        &self,
        env: &mut Environment,
        values: Vec<MapOfFields>,
        sup: Super,
    ) -> Result<MultipleIds> {
        let today = Utc::now().date_naive();
        let mut values = values;
        let mut first_columns: HashMap<u32, Option<u32>> = HashMap::new();
        for task in &mut values {
            if task.get_option::<&u32>("project").is_none_or(|id| *id == 0)
                && let Some(parent) = task.get_option::<&u32>("parent").copied()
            {
                let parent: Task<SingleId> = env.get_record(parent.into());
                let project: Project<SingleId> = parent.get_project(env)?;
                if let Some(project) = project.get_optional_id() {
                    task.insert("project", project);
                }
            }
            if let Some(project) = task.get_option::<&u32>("project").copied()
                && !Stage::is_column_of(env, task.get_option::<&u32>("stage").copied(), project)?
            {
                let first = match first_columns.get(&project) {
                    Some(first) => *first,
                    None => {
                        let record: Project<SingleId> = env.get_record(project.into());
                        let first = Stage::first_of(env, record)?.get_optional_id();
                        first_columns.insert(project, first);
                        first
                    }
                };
                task.insert_option("stage", first);
            }
        }
        let unnumbered: Vec<usize> = values
            .iter()
            .enumerate()
            .filter(|(_, task)| {
                task.get_option::<&String>("number")
                    .is_none_or(|number| number == "/")
            })
            .map(|(at, _)| at)
            .collect();
        let numbers =
            Sequence::next_many_by_code(env, "project.task".to_string(), today, unnumbered.len())?;
        for (at, number) in unnumbered.into_iter().zip(numbers) {
            values[at].insert("number", number);
        }
        let linked: Vec<bool> = values
            .iter()
            .map(|task| task.contains_key("depends_on") || task.contains_key("parent"))
            .collect();
        let created = sup.call_with(values, env)?;
        Task::<MultipleIds>::from_ids(created.get_ids_ref().clone(), env).check_stages(env)?;
        let linked: Vec<u32> = created
            .get_ids_ref()
            .iter()
            .zip(linked)
            .filter(|(_, linked)| *linked)
            .map(|(id, _)| *id)
            .collect();
        Task::<MultipleIds>::from_ids(linked, env).refresh_blocked(env)?;
        Ok(created)
    }

    /// A task moved to another project starts on that project's board, in its first column,
    /// unless a column of it is given too. A task moved to another column is in progress again,
    /// unless blocked; the tasks waiting for it, and those it is a subtask of, see whether they
    /// are still blocked.
    pub fn write(&self, env: &mut Environment, values: MapOfFields, sup: Super) -> Result<()> {
        let mut values = values;
        if values.get_option::<&u32>("stage").is_none_or(|id| *id == 0)
            && let Some(project) = values.get_option::<&u32>("project").copied()
        {
            let project: Project<SingleId> = env.get_record(project.into());
            let first = Stage::first_of(env, project)?;
            values.insert_option("stage", first.get_optional_id());
        }
        let moved = values.contains_key("stage");
        let relinked = values.contains_key("depends_on") || values.contains_key("parent");
        let restated = values.contains_key("status");
        let parents_before: Task<MultipleIds> = if relinked {
            self.get_parent(env)?
        } else {
            Task::from_ids(Vec::<u32>::new(), env)
        };
        let placed = moved || values.contains_key("project");
        sup.call_with(values, env)?;
        if placed {
            self.check_stages(env)?;
        }
        if moved {
            for task in self {
                if !matches!(
                    *task.get_status(env)?,
                    TaskStatus::Blocked | TaskStatus::InProgress
                ) {
                    task.set_status(TaskStatus::InProgress, env)?;
                }
            }
            let waiting: Task<MultipleIds> = self.get_blocking(env)?;
            waiting.refresh_blocked(env)?;
        }
        if relinked || restated {
            self.refresh_blocked(env)?;
        }
        if relinked {
            parents_before.refresh_blocked(env)?;
        }
        Ok(())
    }

    /// Move the tasks one column to the right on their board; those in the last one stay.
    #[erp(rpc)]
    pub fn action_next_stage(&self, env: &mut Environment) -> Result<bool> {
        for task in self {
            let stage: Stage<SingleId> = task.get_stage(env)?;
            let project: Project<SingleId> = task.get_project(env)?;
            let columns = Stage::columns_of(env, project)?;
            let at = columns
                .iter()
                .position(|column| column.get_optional_id() == stage.get_optional_id());
            if let Some(next) = at.and_then(|at| columns.get(at + 1)) {
                task.set_stage(next, env)?;
            }
        }
        Ok(true)
    }
}

impl Task<MultipleIds> {
    /// Blocked while held up, in progress again once no longer; up to the tasks they are subtasks
    /// of, whose subtasks changed.
    pub fn refresh_blocked(&self, env: &mut Environment) -> Result<()> {
        if self.get_ids_ref().is_empty() {
            return Ok(());
        }
        let mut changed = Vec::new();
        for task in self {
            let held_up = task.held_up(env)?;
            let blocked = matches!(*task.get_status(env)?, TaskStatus::Blocked);
            if held_up != blocked {
                let status = if held_up {
                    TaskStatus::Blocked
                } else {
                    TaskStatus::InProgress
                };
                task.set_status(status, env)?;
                changed.push(task.get_id());
            }
        }
        let changed = Task::<MultipleIds>::from_ids(changed, env);
        let parents: Task<MultipleIds> = changed.get_parent(env)?;
        parents.refresh_blocked(env)
    }
}
