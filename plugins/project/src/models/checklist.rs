use crate::models::task::BaseProjectTask;
use code_gen::Model;
use erp::types::field::{IdMode, Reference, SingleId};

/// A step of a task, ticked once done: lighter than a subtask, with no one assigned nor time.
#[derive(Model)]
#[erp(id = "project_checklist_item")]
#[allow(dead_code)]
pub struct ChecklistItem<Mode: IdMode> {
    id: Mode,
    #[erp(required, ondelete = "cascade", index)]
    task: Reference<BaseProjectTask, SingleId>,
    #[erp(default = 10)]
    sequence: i32,
    #[erp(label = "Step")]
    name: String,
    #[erp(default = false)]
    done: bool,
}
