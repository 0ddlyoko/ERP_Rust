//! Sales and projects: a service sold becomes work to do — a task, or a project of its own — and
//! the time spent on it is what is delivered, and invoiced.

use erp::model::ModelManager;
use erp::plugin::{Plugin, PluginInfo};

pub mod models;

pub struct SaleProjectPlugin;

impl Plugin for SaleProjectPlugin {
    fn name(&self) -> String {
        "sale_project".to_string()
    }

    fn info(&self) -> PluginInfo {
        PluginInfo {
            description: Some(
                "A confirmed order's services become tasks, or a project of their own; the time \
                 spent on them is what is delivered and invoiced."
                    .to_string(),
            ),
            category: Some("Sales".to_string()),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            ..PluginInfo::default()
        }
    }

    fn init_models(&self, model_manager: &mut ModelManager) {
        model_manager.register_model::<models::ProductProject<_>>();
        model_manager.register_model::<models::ProjectSale<_>>();
        model_manager.register_model::<models::TaskSale<_>>();
        model_manager.register_model::<models::SaleOrderLineProject<_>>();
        model_manager.register_model::<models::SaleOrderProject<_>>();
        model_manager.register_model::<models::TimesheetSale<_>>();
    }

    fn data(&self) -> Vec<&'static str> {
        vec![include_str!("../views/sale_project_views.xml")]
    }

    fn demo(&self) -> Vec<&'static str> {
        vec![include_str!("../demo/sale_project.xml")]
    }

    fn get_depends(&self) -> Vec<String> {
        vec!["sale".to_string(), "timesheet".to_string()]
    }

    fn auto_install(&self) -> bool {
        true
    }
}

code_gen::export_plugin!(SaleProjectPlugin {});
