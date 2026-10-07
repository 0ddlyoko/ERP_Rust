//! Projects: their tasks on a board of columns, moved from one to the next as the work goes.

use erp::environment::Environment;
use erp::model::ModelManager;
use erp::plugin::{Plugin, PluginInfo};
use erp::types::field::MultipleIds;
use erp_search_code_gen::make_domain;

pub mod models;

pub struct ProjectPlugin;

impl Plugin for ProjectPlugin {
    fn name(&self) -> String {
        "project".to_string()
    }

    fn info(&self) -> PluginInfo {
        PluginInfo {
            description: Some(
                "Projects and their tasks on a board: columns of cards moved by hand, people \
                 assigned, deadlines, priorities and subtasks."
                    .to_string(),
            ),
            category: Some("Services".to_string()),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            color: Some("#0f8b8d".to_string()),
            ..PluginInfo::default()
        }
    }

    fn init_models(&self, model_manager: &mut ModelManager) {
        model_manager.register_model::<models::Project<_>>();
        model_manager.register_model::<models::Stage<_>>();
        model_manager.register_model::<models::Tag<_>>();
        model_manager.register_model::<models::Task<_>>();
        model_manager.register_model::<models::ChecklistItem<_>>();
    }

    fn data(&self) -> Vec<&'static str> {
        vec![
            include_str!("../data/groups.xml"),
            include_str!("../data/access.xml"),
            include_str!("../data/project_data.xml"),
            include_str!("../views/project_views.xml"),
            include_str!("../views/task_views.xml"),
            include_str!("../views/menus.xml"),
        ]
    }

    fn demo(&self) -> Vec<&'static str> {
        vec![include_str!("../demo/project.xml")]
    }

    /// Tasks saved with a status since gone — "to do", "done" — are in progress; those blocked or
    /// waiting see whether they are held up.
    fn post_init(&mut self, env: &mut Environment) -> erp::Result<()> {
        let env = &mut *env.sudo();
        let outdated: models::Task<MultipleIds> =
            env.search(&make_domain!([("status", "in", vec!["to_do", "done"])]))?;
        for task in &outdated {
            task.set_status(models::TaskStatus::InProgress, env)?;
        }
        let held: models::Task<MultipleIds> = env.search(&make_domain!([(
            "status",
            "in",
            vec!["blocked", "waiting"]
        )]))?;
        held.refresh_blocked(env)
    }

    fn get_depends(&self) -> Vec<String> {
        vec!["mail".to_string(), "sequence".to_string()]
    }
}

code_gen::export_plugin!(ProjectPlugin {});
