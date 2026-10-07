use crate::models::project::{BaseProjectProject, Project};
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};
use erp_search::{OrderBy, SearchOptions};
use erp_search_code_gen::make_domain;

/// A column of a project's board: where its tasks stand, left to right. A closing one — `Done` —
/// marks its tasks finished; an unlocking one lets the tasks waiting for its own start; a folded
/// one shows narrow. Those not shown in the progress are left out of the steps a task's form
/// lists, unless the task is in one.
#[derive(Model)]
#[erp(id = "project_stage", methods)]
#[allow(dead_code)]
pub struct Stage<Mode: IdMode> {
    id: Mode,
    name: String,
    #[erp(default = 10)]
    sequence: i32,
    #[erp(required, ondelete = "cascade", index)]
    project: Reference<BaseProjectProject, SingleId>,
    #[erp(label = "Folded", default = false)]
    fold: bool,
    #[erp(label = "Closes the task", default = false)]
    is_closed: bool,
    #[erp(label = "Unlock tasks", default = false)]
    unlocks: bool,
    #[erp(label = "Shown in the progress", default = true)]
    in_progress: bool,
}

#[erp_methods]
impl Stage<SingleId> {
    /// The columns of a project's board, left to right.
    pub fn columns_of(
        env: &mut Environment,
        project: Project<SingleId>,
    ) -> Result<Vec<Stage<SingleId>>> {
        let Some(project) = project.get_optional_id() else {
            return Ok(Vec::new());
        };
        let found: Stage<MultipleIds> = env.sudo().search_with(
            &make_domain!([("project", "=", project)]),
            &SearchOptions::new()
                .order_by(OrderBy::asc("sequence"))
                .order_by(OrderBy::asc("id")),
        )?;
        Ok(found.into_iter().collect())
    }

    /// Whether a column, if one is given, is one of a project's.
    pub fn is_column_of(env: &mut Environment, stage: Option<u32>, project: u32) -> Result<bool> {
        let Some(stage) = stage.filter(|id| *id != 0) else {
            return Ok(false);
        };
        let stage: Stage<SingleId> = env.get_record(stage.into());
        let of: Project<SingleId> = stage.get_project(&mut env.sudo())?;
        Ok(of.get_optional_id() == Some(project))
    }

    /// The first column of a project's board, if it has any.
    pub fn first_of(env: &mut Environment, project: Project<SingleId>) -> Result<Stage<SingleId>> {
        let columns = Self::columns_of(env, project)?;
        Ok(columns
            .into_iter()
            .next()
            .unwrap_or_else(|| env.get_record(SingleId::empty())))
    }
}
