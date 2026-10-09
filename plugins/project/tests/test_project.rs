//! Projects and tasks: the board a project starts with, how tasks move on it, subtasks, what a
//! user sees as theirs, who may change what, the views and the demo.

use base::BasePlugin;
use base::models::View;
use erp::Result;
use erp::app::Application;
use erp::environment::Environment;
use erp_test_support::{admin_env, user_env, xml_id};
use mail::MailPlugin;
use project::ProjectPlugin;
use sequence::SequencePlugin;
use serde_json::{Value, json};
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
            ]
        },
        &["project"],
    )
}

fn create(env: &mut Environment, model: &str, values: Value) -> Result<u32> {
    let ids = env.call_rpc(model, "create", &json!({ "values": values }))?;
    Ok(ids[0].as_u64().expect("an id") as u32)
}

fn read(env: &mut Environment, model: &str, id: u32, fields: &[&str]) -> Result<Value> {
    Ok(env.call_rpc(model, "read", &json!({"ids": [id], "fields": fields}))?[0].clone())
}

/// The columns of a project, left to right, as `(id, name)`.
/// A project with the columns a board usually has: to do, in progress, review, and done — which
/// closes its tasks and unlocks those waiting for them.
fn new_project(env: &mut Environment, values: Value) -> Result<u32> {
    let project = create(env, "project_project", values)?;
    for (at, name) in ["To do", "In progress", "Review", "Done"]
        .into_iter()
        .enumerate()
    {
        let closes = name == "Done";
        create(
            env,
            "project_stage",
            json!({"name": name, "project": project, "sequence": (at + 1) * 10,
                   "is_closed": closes, "unlocks": closes}),
        )?;
    }
    Ok(project)
}

fn columns(env: &mut Environment, project: u32) -> Result<Vec<(u32, String)>> {
    let rows = env.call_rpc(
        "project_stage",
        "read_matching",
        &json!({"domain": [["project", "=", project]], "fields": ["name"], "order": ["sequence", "id"]}),
    )?;
    Ok(rows
        .as_array()
        .expect("rows")
        .iter()
        .map(|row| {
            (
                row["id"].as_u64().expect("id") as u32,
                row["name"].as_str().unwrap_or("").to_string(),
            )
        })
        .collect())
}

/// A project started from no template has no columns; a task starts in the first column its
/// people add, numbered, and walks to the last, where it is done.
#[test]
fn test_a_task_walks_the_board() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let bare = create(&mut env, "project_project", json!({"name": "Bare"}))?;
    assert!(
        columns(&mut env, bare)?.is_empty(),
        "no column made up for it"
    );
    let loose = create(
        &mut env,
        "project_task",
        json!({"name": "Anywhere", "project": bare}),
    )?;
    assert_eq!(
        read(&mut env, "project_task", loose, &["stage"])?["stage"],
        Value::Null
    );

    let project = new_project(&mut env, json!({"name": "Launch"}))?;
    let board = columns(&mut env, project)?;
    let names: Vec<&str> = board.iter().map(|(_, name)| name.as_str()).collect();
    assert_eq!(names, ["To do", "In progress", "Review", "Done"]);
    assert_eq!(
        read(&mut env, "project_project", project, &["manager"])?["manager"],
        json!(env.uid())
    );

    let task = create(
        &mut env,
        "project_task",
        json!({"name": "Write the plan", "project": project}),
    )?;
    let row = read(
        &mut env,
        "project_task",
        task,
        &["number", "stage", "is_closed"],
    )?;
    assert_eq!(
        row["number"],
        json!("T-0002"),
        "after the one of the bare project"
    );
    assert_eq!(row["stage"], json!(board[0].0));
    assert_eq!(row["is_closed"], json!(false));

    for _ in 0..5 {
        env.call_rpc("project_task", "action_next_stage", &json!({"ids": [task]}))?;
    }
    let row = read(&mut env, "project_task", task, &["stage", "is_closed"])?;
    assert_eq!(row["stage"], json!(board[3].0), "the last column keeps it");
    assert_eq!(row["is_closed"], json!(true));
    let counts = read(
        &mut env,
        "project_project",
        project,
        &["task_count", "closed_task_count"],
    )?;
    assert_eq!(
        (
            counts["task_count"].clone(),
            counts["closed_task_count"].clone()
        ),
        (json!(1), json!(1))
    );
    Ok(())
}

/// Dropping a card on another column is writing its column; a task moved to another project
/// starts on that project's board.
#[test]
fn test_moving_tasks() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let first = new_project(&mut env, json!({"name": "First"}))?;
    let second = new_project(&mut env, json!({"name": "Second"}))?;
    let task = create(
        &mut env,
        "project_task",
        json!({"name": "Card", "project": first}),
    )?;
    let review = columns(&mut env, first)?[2].0;
    env.call_rpc(
        "project_task",
        "write",
        &json!({"ids": [task], "values": {"stage": review, "sequence": 3}}),
    )?;
    assert_eq!(
        read(&mut env, "project_task", task, &["stage"])?["stage"],
        json!(review)
    );

    let elsewhere = create(
        &mut env,
        "project_task",
        json!({"name": "Lost", "project": second, "stage": review}),
    )?;
    let second_to_do = columns(&mut env, second)?[0].0;
    assert_eq!(
        read(&mut env, "project_task", elsewhere, &["stage"])?["stage"],
        json!(second_to_do),
        "a column of another project is not one of its own"
    );

    env.call_rpc(
        "project_task",
        "write",
        &json!({"ids": [task], "values": {"project": second}}),
    )?;
    let to_do = columns(&mut env, second)?[0].0;
    assert_eq!(
        read(&mut env, "project_task", task, &["stage"])?["stage"],
        json!(to_do)
    );
    Ok(())
}

/// A subtask is of its parent's project, and the parent counts those done.
#[test]
fn test_subtasks() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let project = new_project(&mut env, json!({"name": "Move"}))?;
    let parent = create(
        &mut env,
        "project_task",
        json!({"name": "Pack", "project": project}),
    )?;
    let books = create(
        &mut env,
        "project_task",
        json!({"name": "Books", "parent": parent}),
    )?;
    create(
        &mut env,
        "project_task",
        json!({"name": "Dishes", "parent": parent}),
    )?;
    assert_eq!(
        read(&mut env, "project_task", books, &["project"])?["project"],
        json!(project)
    );
    assert_eq!(
        read(&mut env, "project_task", parent, &["subtasks_done"])?["subtasks_done"],
        json!("0 / 2")
    );

    let done = columns(&mut env, project)?[3].0;
    env.call_rpc(
        "project_task",
        "write",
        &json!({"ids": [books], "values": {"stage": done}}),
    )?;
    assert_eq!(
        read(&mut env, "project_task", parent, &["subtasks_done"])?["subtasks_done"],
        json!("1 / 2")
    );
    let top = env.call_rpc(
        "project_task",
        "search",
        &json!({"domain": [["project", "=", project], ["parent", "=", false]]}),
    )?;
    assert_eq!(top, json!([parent]), "false is no parent");
    Ok(())
}

/// A domain naming `$uid` finds what is the caller's, whoever they are.
#[test]
fn test_my_tasks() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let project = new_project(&mut env, json!({"name": "Shared"}))?;
    let admin = env.uid().expect("admin");
    let mine = create(
        &mut env,
        "project_task",
        json!({"name": "Mine", "project": project, "assignees": [admin]}),
    )?;
    create(
        &mut env,
        "project_task",
        json!({"name": "Nobody's", "project": project}),
    )?;
    let found = env.call_rpc(
        "project_task",
        "search",
        &json!({"domain": [["assignees", "in", ["$uid"]]]}),
    )?;
    assert_eq!(found, json!([mine]));
    Ok(())
}

/// Anyone of the projects group works on tasks; only managers change projects and their columns:
/// add one, rename or move it.
#[test]
fn test_access_rights() -> Result<()> {
    let app = new_app()?;
    let project = {
        let mut env = admin_env(&app)?;
        let project = new_project(&mut env, json!({"name": "Rights"}))?;
        env.close()?;
        project
    };
    let mut env = user_env(
        &app,
        "worker",
        &["base.group_user", "project.group_project_user"],
    )?;
    let task = create(
        &mut env,
        "project_task",
        json!({"name": "Do it", "project": project}),
    )?;
    env.call_rpc("project_task", "action_next_stage", &json!({"ids": [task]}))?;
    assert!(create(&mut env, "project_project", json!({"name": "Mine"})).is_err());
    assert!(
        create(
            &mut env,
            "project_stage",
            json!({"name": "Blocked", "project": project})
        )
        .is_err(),
        "only managers add columns"
    );
    let to_do = columns(&mut env, project)?[0].0;
    assert!(
        env.call_rpc(
            "project_stage",
            "write",
            &json!({"ids": [to_do], "values": {"name": "Later", "sequence": 99}})
        )
        .is_err(),
        "only managers rename and move columns"
    );
    assert!(
        env.call_rpc(
            "project_project",
            "write",
            &json!({"ids": [project], "values": {"name": "Renamed"}})
        )
        .is_err()
    );
    Ok(())
}

/// Every view of the plugin resolves against its model.
#[test]
fn test_views_load() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    for (model, kind) in [
        ("project_project", "kanban"),
        ("project_project", "list"),
        ("project_project", "form"),
        ("project_project", "search"),
        ("project_task", "kanban"),
        ("project_task", "list"),
        ("project_task", "form"),
        ("project_task", "search"),
        ("project_stage", "list"),
        ("project_tag", "list"),
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

/// The demo fills the boards: tasks spread over the columns, subtasks under their parent.
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
    let kickoff = xml_id(&mut env, "project.demo_task_kickoff");
    let row = read(&mut env, "project_task", kickoff, &["stage", "is_closed"])?;
    assert_eq!(row["is_closed"], json!(true), "{row}");
    let customers = xml_id(&mut env, "project.demo_task_customers");
    let row = read(&mut env, "project_task", customers, &["subtasks_done"])?;
    assert_eq!(row["subtasks_done"], json!("1 / 2"));
    Ok(())
}

/// Deleting a column keeps its tasks: they are in no column until given one.
#[test]
fn test_deleting_a_column_keeps_its_tasks() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let project = new_project(&mut env, json!({"name": "Cleanup"}))?;
    let review = columns(&mut env, project)?[2].0;
    let task = create(
        &mut env,
        "project_task",
        json!({"name": "Check", "project": project, "stage": review}),
    )?;
    env.call_rpc("project_stage", "delete", &json!({"ids": [review]}))?;
    let row = read(&mut env, "project_task", task, &["stage", "is_closed"])?;
    assert_eq!(row["stage"], Value::Null, "{row}");
    assert_eq!(row["is_closed"], json!(false));
    assert_eq!(columns(&mut env, project)?.len(), 3);
    Ok(())
}

/// A task waiting for another is blocked until that one reaches a column that unlocks it; it is
/// not refused a column for being blocked.
#[test]
fn test_dependencies() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let project = new_project(&mut env, json!({"name": "Chain"}))?;
    let board = columns(&mut env, project)?;
    let first = create(
        &mut env,
        "project_task",
        json!({"name": "Foundations", "project": project}),
    )?;
    let second = create(
        &mut env,
        "project_task",
        json!({"name": "Walls", "project": project, "depends_on": [first]}),
    )?;
    let status = |env: &mut Environment, task: u32| -> Result<Value> {
        Ok(read(env, "project_task", task, &["status"])?["status"].clone())
    };
    assert_eq!(status(&mut env, first)?, json!("in_progress"));
    assert_eq!(status(&mut env, second)?, json!("blocked"));
    assert_eq!(
        read(&mut env, "project_task", first, &["blocking"])?["blocking"],
        json!([second]),
        "the other side of the dependency"
    );

    env.call_rpc(
        "project_task",
        "write",
        &json!({"ids": [first], "values": {"stage": board[1].0}}),
    )?;
    assert_eq!(status(&mut env, first)?, json!("in_progress"));
    assert_eq!(
        status(&mut env, second)?,
        json!("blocked"),
        "in progress unlocks nothing"
    );

    env.call_rpc(
        "project_task",
        "write",
        &json!({"ids": [second], "values": {"stage": board[3].0}}),
    )?;
    assert_eq!(
        status(&mut env, second)?,
        json!("blocked"),
        "a blocked task may still be closed, and stays blocked"
    );

    env.call_rpc(
        "project_task",
        "write",
        &json!({"ids": [second], "values": {"stage": board[1].0}}),
    )?;
    env.call_rpc(
        "project_task",
        "write",
        &json!({"ids": [first], "values": {"stage": board[3].0}}),
    )?;
    assert_eq!(status(&mut env, first)?, json!("in_progress"));
    assert_eq!(
        status(&mut env, second)?,
        json!("in_progress"),
        "done unlocks the waiting task"
    );
    Ok(())
}

/// A checklist counts its steps ticked; a subtask is on its project's board only when the
/// project asks for subtasks there.
#[test]
fn test_checklist_and_subtasks_on_the_board() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let project = new_project(&mut env, json!({"name": "Fair"}))?;
    let task = create(
        &mut env,
        "project_task",
        json!({"name": "Stand", "project": project, "checklist": {"create": [
            {"name": "Book the space", "done": true},
            {"name": "Print the posters"},
            {"name": "Bring the coffee"},
        ]}}),
    )?;
    assert_eq!(
        read(&mut env, "project_task", task, &["checklist_done"])?["checklist_done"],
        json!("1/3")
    );
    let sub = create(
        &mut env,
        "project_task",
        json!({"name": "Posters", "parent": task}),
    )?;
    let on_board = |env: &mut Environment, id: u32| -> Result<Value> {
        Ok(read(env, "project_task", id, &["on_board"])?["on_board"].clone())
    };
    assert_eq!(
        (on_board(&mut env, task)?, on_board(&mut env, sub)?),
        (json!(true), json!(false))
    );
    env.call_rpc(
        "project_project",
        "write",
        &json!({"ids": [project], "values": {"show_subtasks": true}}),
    )?;
    assert_eq!(on_board(&mut env, sub)?, json!(true));
    Ok(())
}

/// A project started from a template gets its columns and its tasks — in the same columns, with
/// their steps, subtasks and dependencies — and the hours it plans; the template stays as it was.
#[test]
fn test_a_project_from_a_template() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let template = new_project(
        &mut env,
        json!({"name": "Website", "is_template": true, "planned_hours": "40"}),
    )?;
    let template_columns = columns(&mut env, template)?;
    let brief = create(
        &mut env,
        "project_task",
        json!({"name": "Brief", "project": template, "stage": template_columns[1].0}),
    )?;
    let design = create(
        &mut env,
        "project_task",
        json!({"name": "Design", "project": template, "depends_on": [brief]}),
    )?;
    create(
        &mut env,
        "project_task",
        json!({"name": "Mock-ups", "project": template, "parent": design}),
    )?;
    create(
        &mut env,
        "project_checklist_item",
        json!({"name": "Fonts", "task": design}),
    )?;

    let project = create(
        &mut env,
        "project_project",
        json!({"name": "Bakery website", "template": template}),
    )?;
    let names: Vec<String> = columns(&mut env, project)?
        .into_iter()
        .map(|(_, name)| name)
        .collect();
    let template_names: Vec<String> = template_columns.into_iter().map(|(_, name)| name).collect();
    assert_eq!(names, template_names);
    let row = read(
        &mut env,
        "project_project",
        project,
        &["planned_hours", "task_count"],
    )?;
    assert_eq!(row["task_count"], json!(3));
    assert_eq!(
        row["planned_hours"]
            .as_str()
            .map(|hours| hours.parse::<f64>().ok()),
        Some(Some(40.0))
    );

    let found = env.call_rpc(
        "project_task",
        "search",
        &json!({"domain": [["project", "=", project], ["name", "=", "Design"]]}),
    )?;
    let copy = found[0].as_u64().expect("a copy") as u32;
    let row = read(
        &mut env,
        "project_task",
        copy,
        &["depends_on", "children", "checklist_done"],
    )?;
    assert_eq!(row["children"].as_array().map(Vec::len), Some(1));
    assert_eq!(row["checklist_done"], json!("0/1"));
    let awaited = row["depends_on"][0].as_u64().expect("a dependency") as u32;
    let awaited = read(
        &mut env,
        "project_task",
        awaited,
        &["name", "project", "stage"],
    )?;
    assert_eq!(
        (awaited["name"].clone(), awaited["project"].clone()),
        (json!("Brief"), json!(project))
    );
    assert_eq!(
        read(&mut env, "project_task", brief, &["project"])?["project"],
        json!(template)
    );
    Ok(())
}

/// A task is in progress, waiting or ready as its people say, back in progress once moved to
/// another column; blocked by itself while a task it waits for is not done, or one of its
/// subtasks is blocked — up to the top — and no one else may say it is.
#[test]
fn test_status() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let project = new_project(&mut env, json!({"name": "Site"}))?;
    let board = columns(&mut env, project)?;
    let parent = create(
        &mut env,
        "project_task",
        json!({"name": "Launch", "project": project}),
    )?;
    let child = create(
        &mut env,
        "project_task",
        json!({"name": "Domain", "project": project, "parent": parent}),
    )?;
    let other = create(
        &mut env,
        "project_task",
        json!({"name": "Contract", "project": project}),
    )?;
    let status = |env: &mut Environment, task: u32| -> Result<Value> {
        Ok(read(env, "project_task", task, &["status"])?["status"].clone())
    };
    let write = |env: &mut Environment, task: u32, values: Value| -> Result<Value> {
        env.call_rpc(
            "project_task",
            "write",
            &json!({"ids": [task], "values": values}),
        )
    };

    write(&mut env, parent, json!({"status": "ready"}))?;
    assert_eq!(status(&mut env, parent)?, json!("ready"));
    write(&mut env, parent, json!({"stage": board[1].0}))?;
    assert_eq!(
        status(&mut env, parent)?,
        json!("in_progress"),
        "moved, in progress again"
    );
    write(&mut env, parent, json!({"status": "waiting"}))?;
    assert_eq!(status(&mut env, parent)?, json!("waiting"));

    write(&mut env, other, json!({"status": "blocked"}))?;
    assert_eq!(
        status(&mut env, other)?,
        json!("in_progress"),
        "blocked is not for people to say"
    );

    write(&mut env, child, json!({"depends_on": [other]}))?;
    assert_eq!(
        status(&mut env, child)?,
        json!("blocked"),
        "it waits for an open task"
    );
    assert_eq!(
        status(&mut env, parent)?,
        json!("blocked"),
        "its subtask is blocked"
    );
    write(&mut env, child, json!({"stage": board[2].0}))?;
    assert_eq!(
        status(&mut env, child)?,
        json!("blocked"),
        "moved, still blocked"
    );

    write(&mut env, other, json!({"stage": board[3].0}))?;
    assert_eq!(
        status(&mut env, child)?,
        json!("in_progress"),
        "what it waited for is done"
    );
    assert_eq!(status(&mut env, parent)?, json!("in_progress"));
    Ok(())
}

/// A task taken out of its project's list goes: it cannot be without its project, which deletes
/// its tasks with it.
#[test]
fn test_a_task_taken_out_of_its_project_goes() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let project = new_project(&mut env, json!({"name": "Site"}))?;
    let kept = create(
        &mut env,
        "project_task",
        json!({"name": "Kept", "project": project}),
    )?;
    let gone = create(
        &mut env,
        "project_task",
        json!({"name": "Gone", "project": project}),
    )?;
    env.call_rpc(
        "project_project",
        "write",
        &json!({"ids": [project], "values": {"tasks": {"unlink": [gone]}}}),
    )?;
    let left = env.call_rpc(
        "project_task",
        "search",
        &json!({"domain": [["id", "in", [kept, gone]]]}),
    )?;
    assert_eq!(left, json!([kept]));
    Ok(())
}

/// A task's column is one of its project's: another project's is refused, and the task of a
/// project moved to another starts on that one's board.
#[test]
fn test_a_task_stays_on_its_project_board() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let mine = new_project(&mut env, json!({"name": "Mine"}))?;
    let other = new_project(&mut env, json!({"name": "Other"}))?;
    let task = create(
        &mut env,
        "project_task",
        json!({"name": "Here", "project": mine}),
    )?;
    let elsewhere = columns(&mut env, other)?[1].0;
    let refused = env.call_rpc(
        "project_task",
        "write",
        &json!({"ids": [task], "values": {"stage": elsewhere}}),
    );
    assert!(refused.is_err(), "a column of another project");
    env.call_rpc(
        "project_task",
        "write",
        &json!({"ids": [task], "values": {"project": other}}),
    )?;
    let row = read(&mut env, "project_task", task, &["stage"])?;
    assert_eq!(
        row["stage"],
        json!(columns(&mut env, other)?[0].0),
        "the new board's first"
    );
    Ok(())
}

/// A task's stage offers its project's columns, as the field itself says — whatever view shows
/// it: an expression of the record the client works out.
#[test]
fn test_the_stage_offers_its_project_columns() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let fields = env.call_rpc("project_task", "fields_get", &json!({}))?;
    assert_eq!(
        fields["stage"]["domain"],
        json!("[['project', '=', project]]")
    );
    Ok(())
}
