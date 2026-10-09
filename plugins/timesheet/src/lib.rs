//! Timesheets: the hours spent on the tasks of projects, against the hours planned.

use erp::assets::{BundleContribution, ModuleFiles, StaticFiles};
use erp::model::ModelManager;
use erp::plugin::{Plugin, PluginInfo};

pub mod models;

include!(concat!(env!("OUT_DIR"), "/static_files.rs"));

pub struct TimesheetPlugin;

impl Plugin for TimesheetPlugin {
    fn name(&self) -> String {
        "timesheet".to_string()
    }

    fn info(&self) -> PluginInfo {
        PluginInfo {
            description: Some(
                "The hours spent on tasks, logged by each person, against the hours planned for \
                 the task and the project."
                    .to_string(),
            ),
            category: Some("Services".to_string()),
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            color: Some("#0f8b8d".to_string()),
            ..PluginInfo::default()
        }
    }

    fn init_models(&self, model_manager: &mut ModelManager) {
        model_manager.register_model::<models::Timesheet<_>>();
        model_manager.register_model::<models::TaskTimesheet<_>>();
        model_manager.register_model::<models::ProjectTimesheet<_>>();
        model_manager.register_model::<models::Timer<_>>();
        model_manager.register_model::<models::TimerStop<_>>();
    }

    fn data(&self) -> Vec<&'static str> {
        vec![
            include_str!("../data/access.xml"),
            include_str!("../views/timesheet_views.xml"),
            include_str!("../views/dashboard_views.xml"),
            include_str!("../views/menus.xml"),
        ]
    }

    fn demo(&self) -> Vec<&'static str> {
        vec![include_str!("../demo/timesheet.xml")]
    }

    fn static_files(&self) -> StaticFiles {
        STATIC_FILES
    }

    fn module_files(&self) -> ModuleFiles {
        MODULE_FILES
    }

    fn assets(&self) -> Vec<BundleContribution> {
        vec![BundleContribution::new(
            "web.assets_backend",
            &[
                "timesheet/static/src/**/*.css",
                "timesheet/static/src/**/*.js",
                "timesheet/static/src/**/*.xml",
            ],
        )]
    }

    fn get_depends(&self) -> Vec<String> {
        vec!["project".to_string()]
    }
}

code_gen::export_plugin!(TimesheetPlugin {});
