use crate::models::checklist::ChecklistItem;
use crate::models::stage::{BaseProjectStage, Stage};
use crate::models::tag::Tag;
use crate::models::task::{BaseProjectTask, Task};
use base::models::{BaseContact, BaseUsers};
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::FieldType;
use erp::types::field::{Decimal, IdMode, MultipleIds, NaiveDate, Reference, SingleId};
use erp::types::model::MapOfFields;
use std::collections::HashMap;

/// Work done for a customer or for the company: its tasks, on a board of columns of its own.
/// A template is a project to start others from: its columns and tasks are copied into a
/// project created from it.
#[derive(Model)]
#[erp(id = "project_project", order = "name, id", methods)]
#[allow(dead_code)]
pub struct Project<Mode: IdMode> {
    id: Mode,
    #[erp(tracking, index = "trigram")]
    name: String,
    #[erp(default = true)]
    active: bool,
    #[erp(ondelete = "restrict", tracking)]
    customer: Reference<BaseContact, SingleId>,
    #[erp(label = "Project manager", ondelete = "set_null", tracking)]
    manager: Reference<BaseUsers, SingleId>,
    #[erp(label = "Deadline", tracking)]
    date_deadline: Option<NaiveDate>,
    #[erp(label = "Planned hours", default = 0.0, tracking)]
    planned_hours: Decimal,
    #[erp(label = "Show subtasks on the board", default = false)]
    show_subtasks: bool,
    #[erp(label = "Template", default = false, index)]
    is_template: bool,
    #[erp(
        label = "Started from",
        ondelete = "set_null",
        domain = r#"[["is_template", "=", true]]"#
    )]
    template: Reference<BaseProjectProject, SingleId>,
    description: Option<String>,
    #[erp(label = "Columns", inverse = "project", owned)]
    stages: Reference<BaseProjectStage, MultipleIds>,
    #[erp(inverse = "project")]
    tasks: Reference<BaseProjectTask, MultipleIds>,
    #[erp(label = "Tasks", compute = "compute_task_counts", depends = ["tasks", "tasks.is_closed"])]
    task_count: i32,
    #[erp(label = "Tasks done", compute = "compute_task_counts", depends = ["tasks", "tasks.is_closed"])]
    closed_task_count: i32,
}

#[erp_methods]
impl Project<MultipleIds> {
    /// How many tasks a project holds, and how many of them are done.
    pub fn compute_task_counts(&self, env: &mut Environment) -> Result<()> {
        for project in self {
            let tasks: Task<MultipleIds> = project.get_tasks(env)?;
            let mut closed = 0;
            for task in &tasks {
                if *task.get_is_closed(env)? {
                    closed += 1;
                }
            }
            project.set_task_count(i32::try_from(tasks.get_ids_ref().len())?, env)?;
            project.set_closed_task_count(closed, env)?;
        }
        Ok(())
    }

    /// A project is managed by whoever creates it.
    pub fn default_get(
        env: &mut Environment,
        fields: Vec<String>,
        sup: Super,
    ) -> Result<MapOfFields> {
        let mut defaults = sup.call_with(fields.clone(), env)?;
        if fields.iter().any(|field| field == "manager")
            && let Some(uid) = env.uid()
        {
            defaults.insert("manager", uid);
        }
        Ok(defaults)
    }

    /// A project started from a template gets its columns and tasks; one created without has
    /// none until its people add them.
    pub fn create(
        &self,
        env: &mut Environment,
        values: Vec<MapOfFields>,
        sup: Super,
    ) -> Result<MultipleIds> {
        let created = sup.call_with(values, env)?;
        let projects: Project<MultipleIds> = Project::from_ids(created.get_ids_ref().clone(), env);
        for project in &projects {
            let template: Project<SingleId> = project.get_template(env)?;
            let stages: Stage<MultipleIds> = project.get_stages(env)?;
            if !template.is_empty() && stages.get_ids_ref().is_empty() {
                let project: Project<SingleId> = env.get_record(project.get_id().into());
                project.copy_board(env, &template)?;
            }
        }
        Ok(created)
    }
}

impl Project<SingleId> {
    /// Copy a template's board into the project: its columns, then its tasks — in the matching
    /// columns, with their tags, steps, subtasks and dependencies — and the hours planned.
    fn copy_board(&self, env: &mut Environment, template: &Project<SingleId>) -> Result<()> {
        let columns: Stage<MultipleIds> = template.get_stages(env)?;
        let mut values = Vec::new();
        for column in &columns {
            let mut copy = MapOfFields::default();
            copy.insert("name", column.get_name(env)?.clone());
            copy.insert("sequence", *column.get_sequence(env)?);
            copy.insert("fold", *column.get_fold(env)?);
            copy.insert("is_closed", *column.get_is_closed(env)?);
            copy.insert("unlocks", *column.get_unlocks(env)?);
            copy.insert("project", self.get_id());
            values.push(copy);
        }
        let copies: Stage<MultipleIds> = env.create_new_records_from_maps(values)?;
        let column_of: HashMap<u32, u32> = columns
            .get_ids_ref()
            .iter()
            .copied()
            .zip(copies.get_ids_ref().iter().copied())
            .collect();

        let tasks: Task<MultipleIds> = template.get_tasks(env)?;
        let mut values = Vec::new();
        for task in &tasks {
            let mut copy = MapOfFields::default();
            copy.insert("name", task.get_name(env)?.clone());
            copy.insert("project", self.get_id());
            copy.insert("sequence", *task.get_sequence(env)?);
            copy.insert("priority", *task.get_priority(env)?);
            copy.insert("planned_hours", *task.get_planned_hours(env)?);
            copy.insert_option("description", task.get_description(env)?.cloned());
            let tags: Tag<MultipleIds> = task.get_tags(env)?;
            copy.insert("tags", FieldType::Refs(tags.get_ids_ref().clone()));
            let stage: Stage<SingleId> = task.get_stage(env)?;
            copy.insert_option(
                "stage",
                stage
                    .get_optional_id()
                    .and_then(|stage| column_of.get(&stage).copied()),
            );
            values.push(copy);
        }
        let copies: Task<MultipleIds> = env.create_new_records_from_maps(values)?;
        let task_of: HashMap<u32, u32> = tasks
            .get_ids_ref()
            .iter()
            .copied()
            .zip(copies.get_ids_ref().iter().copied())
            .collect();

        let mut steps = Vec::new();
        for task in &tasks {
            let copy: Task<SingleId> = env.get_record(task_of[&task.get_id()].into());
            let parent: Task<SingleId> = task.get_parent(env)?;
            if let Some(parent) = parent
                .get_optional_id()
                .and_then(|parent| task_of.get(&parent))
            {
                let parent: Task<SingleId> = env.get_record((*parent).into());
                copy.set_parent(&parent, env)?;
            }
            let awaited: Task<MultipleIds> = task.get_depends_on(env)?;
            let awaited: Vec<u32> = awaited
                .get_ids_ref()
                .iter()
                .filter_map(|other| task_of.get(other).copied())
                .collect();
            if !awaited.is_empty() {
                copy.set_depends_on(&Task::<MultipleIds>::from_ids(awaited, env), env)?;
            }
            let checklist: ChecklistItem<MultipleIds> = task.get_checklist(env)?;
            for step in &checklist {
                let mut values = MapOfFields::default();
                values.insert("task", copy.get_id());
                values.insert("sequence", *step.get_sequence(env)?);
                values.insert("name", step.get_name(env)?.clone());
                steps.push(values);
            }
        }
        let _: ChecklistItem<MultipleIds> = env.create_new_records_from_maps(steps)?;
        let planned = *template.get_planned_hours(env)?;
        if planned > Decimal::ZERO && self.get_planned_hours(env)?.is_zero() {
            self.set_planned_hours(planned, env)?;
        }
        Ok(())
    }
}
