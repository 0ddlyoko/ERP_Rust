//! Timesheets: time logged on tasks and projects, what it adds up to, and what is refused.

use base::BasePlugin;
use base::models::View;
use erp::Result;
use erp::app::Application;
use erp::environment::Environment;
use erp_test_support::{admin_env, create, read, xml_id};
use mail::MailPlugin;
use project::ProjectPlugin;
use sequence::SequencePlugin;
use serde_json::{Value, json};
use timesheet::TimesheetPlugin;
use web::WebPlugin;

fn new_app() -> Result<Application> {
    erp_test_support::app(
        || -> Vec<Box<dyn erp::plugin::Plugin>> {
            vec![
                Box::new(BasePlugin {}),
                Box::new(WebPlugin {}),
                Box::new(MailPlugin {}),
                Box::new(SequencePlugin {}),
                Box::new(ProjectPlugin {}),
                Box::new(TimesheetPlugin {}),
            ]
        },
        &["timesheet"],
    )
}

fn hours(value: &Value) -> f64 {
    value
        .as_str()
        .and_then(|text| text.parse().ok())
        .or_else(|| value.as_f64())
        .expect("hours")
}

/// Time logged on a task counts for it and its project, against the hours planned.
#[test]
fn test_time_adds_up() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let project = create(
        &mut env,
        "project_project",
        json!({"name": "Site", "planned_hours": "20"}),
    )?;
    let task = create(
        &mut env,
        "project_task",
        json!({"name": "Design", "project": project, "planned_hours": "8"}),
    )?;
    let entry = create(
        &mut env,
        "timesheet",
        json!({"task": task, "name": "Sketches", "hours": "2.5"}),
    )?;
    create(
        &mut env,
        "timesheet",
        json!({"task": task, "name": "Review", "hours": "1.5"}),
    )?;
    create(
        &mut env,
        "timesheet",
        json!({"project": project, "name": "Meeting", "hours": "1"}),
    )?;

    let row = read(&mut env, "timesheet", entry, &["project", "user", "date"])?;
    assert_eq!(row["project"], json!(project), "the project of the task");
    assert_eq!(row["user"], json!(env.uid()), "logged by whoever logs it");
    assert!(row["date"].is_string(), "today");

    let row = read(
        &mut env,
        "project_task",
        task,
        &["spent_hours", "remaining_hours", "progress"],
    )?;
    assert_eq!(
        (hours(&row["spent_hours"]), hours(&row["remaining_hours"])),
        (4.0, 4.0)
    );
    assert_eq!(row["progress"], json!(50));
    let row = read(
        &mut env,
        "project_project",
        project,
        &["spent_hours", "progress"],
    )?;
    assert_eq!(hours(&row["spent_hours"]), 5.0);
    assert_eq!(row["progress"], json!(25));
    Ok(())
}

/// Time is spent on a project, and never negative.
#[test]
fn test_what_is_refused() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let project = create(&mut env, "project_project", json!({"name": "Site"}))?;
    assert!(
        env.savepoint(|env| create(env, "timesheet", json!({"name": "Nowhere", "hours": "1"})))
            .is_err()
    );
    assert!(
        env.savepoint(|env| create(env, "timesheet", json!({"project": project, "hours": "-1"})))
            .is_err()
    );
    assert_eq!(
        env.count(
            "timesheet",
            &erp_search_code_gen::make_domain!([("id", "!=", 0)])
        )?,
        0
    );
    Ok(())
}

/// Every view of the plugin resolves, the project's and the task's with what it adds to them.
#[test]
fn test_views_load() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    for (model, kind) in [
        ("timesheet", "list"),
        ("timesheet", "form"),
        ("timesheet", "search"),
        ("project_task", "form"),
        ("project_task", "kanban"),
        ("project_task", "list"),
        ("project_project", "form"),
        ("project_project", "kanban"),
    ] {
        let arch = env.get_empty_record::<View<_>>().load(
            &mut env,
            model.to_string(),
            kind.to_string(),
        )?;
        assert!(
            arch.starts_with(&format!("<{kind}")),
            "{model} {kind}: {arch}"
        );
    }
    Ok(())
}

/// The demo logs time on the demo tasks.
#[test]
fn test_demo() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let settings = xml_id(&mut env, "base.settings");
    env.call_rpc(
        "settings",
        "write",
        &json!({ "ids": [settings], "values": { "demo_data": true } }),
    )?;
    let customers = xml_id(&mut env, "project.demo_task_customers");
    let row = read(&mut env, "project_task", customers, &["spent_hours"])?;
    assert_eq!(hours(&row["spent_hours"]), 7.5);
    Ok(())
}

fn timer(env: &mut Environment, method: &str, args: Value) -> Result<Value> {
    env.call_rpc("timesheet_timer", method, &json!({"ids": [], "args": args}))
}

/// A timer started on a task logs the time on it when stopped; started on none, stopping asks
/// for one first.
#[test]
fn test_timer_logs_time() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let project = create(&mut env, "project_project", json!({"name": "Site"}))?;
    let task = create(
        &mut env,
        "project_task",
        json!({"name": "Design", "project": project}),
    )?;
    assert_eq!(
        timer(&mut env, "timer_status", json!({}))?["running"],
        false
    );

    let started = timer(&mut env, "timer_start", json!({"task": task}))?;
    assert_eq!(started["task"][0], json!(task));
    let row = read(&mut env, "project_task", task, &["timer_running"])?;
    assert_eq!(row["timer_running"], true, "the task knows its timer runs");
    let stopped = timer(&mut env, "timer_stop", json!({"description": "Sketches"}))?;
    assert_eq!(stopped["running"], false);
    let entry = stopped["timesheet"].as_u64().expect("a timesheet") as u32;
    let row = read(&mut env, "timesheet", entry, &["task", "name", "hours"])?;
    assert_eq!(row["task"], json!(task));
    assert_eq!(row["name"], "Sketches");
    assert_eq!(hours(&row["hours"]), 0.02, "a minute at least");

    timer(&mut env, "timer_start", json!({"task": null}))?;
    let asked = timer(&mut env, "timer_stop", json!({}))?;
    assert_eq!(asked["needs_project"], true, "nowhere to log it yet");
    let stopped = timer(
        &mut env,
        "timer_stop",
        json!({"project": project, "task": task}),
    )?;
    assert!(stopped["timesheet"].is_u64(), "logged on the task chosen");

    timer(&mut env, "timer_start", json!({"task": null}))?;
    let stopped = timer(&mut env, "timer_stop", json!({"project": project}))?;
    let entry = stopped["timesheet"].as_u64().expect("a timesheet") as u32;
    let row = read(&mut env, "timesheet", entry, &["task", "project"])?;
    assert_eq!(row["task"], Value::Null, "on the project, no task");
    assert_eq!(row["project"], json!(project));

    timer(&mut env, "timer_start", json!({"task": null}))?;
    assert_eq!(
        timer(&mut env, "timer_discard", json!({}))?["running"],
        false
    );
    assert_eq!(
        timer(&mut env, "timer_status", json!({}))?["running"],
        false
    );
    Ok(())
}

/// Starting a timer on another task logs the one running; one on no task is given the task.
#[test]
fn test_timer_switches_task() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let project = create(&mut env, "project_project", json!({"name": "Site"}))?;
    let first = create(
        &mut env,
        "project_task",
        json!({"name": "A", "project": project}),
    )?;
    let second = create(
        &mut env,
        "project_task",
        json!({"name": "B", "project": project}),
    )?;

    timer(&mut env, "timer_start", json!({"task": null}))?;
    let given = timer(&mut env, "timer_start", json!({"task": first}))?;
    assert_eq!(
        given["task"][0],
        json!(first),
        "the running timer gets the task"
    );
    let switched = timer(&mut env, "timer_start", json!({"task": second}))?;
    assert_eq!(switched["task"][0], json!(second));
    let row = read(&mut env, "project_task", first, &["spent_hours"])?;
    assert!(
        hours(&row["spent_hours"]) > 0.0,
        "the first task's time was logged"
    );
    Ok(())
}

/// Stopped from its dialog, a timer on no task logs its time on the project or the task chosen;
/// the assistant leaves nothing behind.
#[test]
fn test_timer_stop_wizard() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let project = create(&mut env, "project_project", json!({"name": "Site"}))?;
    let other = create(&mut env, "project_project", json!({"name": "Other"}))?;
    let task = create(
        &mut env,
        "project_task",
        json!({"name": "A", "project": other}),
    )?;
    timer(&mut env, "timer_start", json!({"task": null}))?;

    let defaults = env.call_rpc(
        "timesheet_timer_stop",
        "default_get",
        &json!({"fields": ["elapsed"]}),
    )?;
    assert!(
        defaults["elapsed"]
            .as_str()
            .is_some_and(|text| text.starts_with("0:00:"))
    );

    let wrong = create(
        &mut env,
        "timesheet_timer_stop",
        json!({"project": project, "task": task}),
    )?;
    let refused = env.call_rpc(
        "timesheet_timer_stop",
        "action_log",
        &json!({"ids": [wrong]}),
    );
    assert!(refused.is_err(), "a task of another project");

    let asked = create(
        &mut env,
        "timesheet_timer_stop",
        json!({"project": project, "name": "Meeting"}),
    )?;
    let answer = env.call_rpc(
        "timesheet_timer_stop",
        "action_log",
        &json!({"ids": [asked]}),
    )?;
    let entry = answer["timesheet"].as_u64().expect("a timesheet") as u32;
    let row = read(&mut env, "timesheet", entry, &["project", "task", "name"])?;
    assert_eq!(
        (row["project"].clone(), row["task"].clone()),
        (json!(project), Value::Null)
    );
    assert_eq!(row["name"], "Meeting");
    let left = env.call_rpc(
        "timesheet_timer_stop",
        "search",
        &json!({"domain": [["id", "=", asked]]}),
    )?;
    assert_eq!(left, json!([]), "the assistant is gone");
    assert_eq!(
        timer(&mut env, "timer_status", json!({}))?["running"],
        false
    );
    Ok(())
}

/// Kept running from its dialog, a timer remembers the project and what for: the dialog opens
/// with them again, and stopped, the timer logs its time there without asking.
#[test]
fn test_timer_kept_running() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let project = create(&mut env, "project_project", json!({"name": "Site"}))?;
    timer(&mut env, "timer_start", json!({"task": null}))?;
    let kept = create(
        &mut env,
        "timesheet_timer_stop",
        json!({"project": project, "name": "Review"}),
    )?;
    let running = env.call_rpc(
        "timesheet_timer_stop",
        "action_keep",
        &json!({"ids": [kept]}),
    )?;
    assert_eq!(
        running["project"][0],
        json!(project),
        "the timer remembers the project"
    );
    let defaults = env.call_rpc(
        "timesheet_timer_stop",
        "default_get",
        &json!({"fields": ["project", "name"]}),
    )?;
    assert_eq!(defaults["project"], json!([project, "Site"]));
    assert_eq!(defaults["name"], json!("Review"));
    let stopped = timer(&mut env, "timer_stop", json!({}))?;
    let entry = stopped["timesheet"]
        .as_u64()
        .expect("logged without asking") as u32;
    let row = read(&mut env, "timesheet", entry, &["project", "name"])?;
    assert_eq!(row["project"], json!(project));
    assert_eq!(row["name"], json!("Review"));
    Ok(())
}

/// The time spent may be corrected before it is logged: as hours and minutes, or as hours; kept
/// running, the timer goes on from the time corrected.
#[test]
fn test_timer_time_corrected() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let project = create(&mut env, "project_project", json!({"name": "Site"}))?;
    for (written, expected) in [("1:30", 1.5), ("0:45:00", 0.75), ("2,25", 2.25)] {
        timer(&mut env, "timer_start", json!({"task": null}))?;
        let asked = create(
            &mut env,
            "timesheet_timer_stop",
            json!({"project": project, "elapsed": written}),
        )?;
        let answer = env.call_rpc(
            "timesheet_timer_stop",
            "action_log",
            &json!({"ids": [asked]}),
        )?;
        let entry = answer["timesheet"].as_u64().expect("a timesheet") as u32;
        let row = read(&mut env, "timesheet", entry, &["hours"])?;
        assert_eq!(hours(&row["hours"]), expected, "{written}");
    }

    timer(&mut env, "timer_start", json!({"task": null}))?;
    let refused = create(
        &mut env,
        "timesheet_timer_stop",
        json!({"project": project, "elapsed": "an hour"}),
    )?;
    let error = env.call_rpc(
        "timesheet_timer_stop",
        "action_log",
        &json!({"ids": [refused]}),
    );
    assert!(error.is_err(), "not a time");

    let kept = create(
        &mut env,
        "timesheet_timer_stop",
        json!({"project": project, "elapsed": "2:00"}),
    )?;
    env.call_rpc(
        "timesheet_timer_stop",
        "action_keep",
        &json!({"ids": [kept]}),
    )?;
    let stopped = timer(&mut env, "timer_stop", json!({}))?;
    let entry = stopped["timesheet"].as_u64().expect("a timesheet") as u32;
    let row = read(&mut env, "timesheet", entry, &["hours"])?;
    assert_eq!(hours(&row["hours"]), 2.0, "on from two hours");
    Ok(())
}
