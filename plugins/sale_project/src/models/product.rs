use code_gen::{Model, selection};
use erp::types::field::{IdMode, Reference, SingleId};
use project::models::BaseProjectProject;

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
