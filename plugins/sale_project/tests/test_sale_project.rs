//! Selling services: the work an order becomes, and the time spent on it delivered and invoiced.

use account::AccountPlugin;
use account::testing::TestChartPlugin;
use base::BasePlugin;
use base::models::View;
use contacts::ContactsPlugin;
use currency::CurrencyPlugin;
use erp::Result;
use erp::app::Application;
use erp::environment::Environment;
use erp::plugin::Plugin;
use erp_test_support::{admin_env, create, read, xml_id};
use mail::MailPlugin;
use product::ProductPlugin;
use project::ProjectPlugin;
use sale::SalePlugin;
use sale_project::SaleProjectPlugin;
use sequence::SequencePlugin;
use serde_json::{Value, json};
use timesheet::TimesheetPlugin;
use uom::UomPlugin;
use web::WebPlugin;

fn plugins() -> Vec<Box<dyn Plugin>> {
    vec![
        Box::new(BasePlugin {}),
        Box::new(WebPlugin {}),
        Box::new(MailPlugin {}),
        Box::new(ContactsPlugin {}),
        Box::new(UomPlugin {}),
        Box::new(CurrencyPlugin {}),
        Box::new(SequencePlugin {}),
        Box::new(ProductPlugin {}),
        Box::new(AccountPlugin {}),
        Box::new(TestChartPlugin {}),
        Box::new(SalePlugin {}),
        Box::new(ProjectPlugin {}),
        Box::new(TimesheetPlugin {}),
        Box::new(SaleProjectPlugin {}),
    ]
}

fn new_app() -> Result<Application> {
    erp_test_support::app(plugins, &["sale_project", "account_test_chart"])
}

fn number(value: &Value) -> f64 {
    value
        .as_str()
        .and_then(|text| text.parse().ok())
        .or_else(|| value.as_f64())
        .expect("a number")
}

fn service(env: &mut Environment, tracking: &str, project: Option<u32>) -> Result<u32> {
    let hour = xml_id(env, "uom.uom_hour");
    create(
        env,
        "product",
        json!({
            "name": "Consulting", "product_type": "service", "list_price": "100", "uom": hour,
            "invoice_policy": "delivery", "service_tracking": tracking, "service_project": project,
        }),
    )
}

fn order(env: &mut Environment, product: u32, lines: &[(&str, &str)]) -> Result<u32> {
    let customer = create(
        env,
        "contact",
        json!({"name": "Customer", "is_company": true}),
    )?;
    let lines: Vec<Value> = lines
        .iter()
        .map(|(name, quantity)| json!({"product": product, "name": name, "product_uom_qty": quantity}))
        .collect();
    let order = create(
        env,
        "sale_order",
        json!({"partner": customer, "lines": {"create": lines}}),
    )?;
    env.call_rpc("sale_order", "action_confirm", &json!({"ids": [order]}))?;
    Ok(order)
}

/// A service sold with a project of its own makes one for the order, with a task per line,
/// planned for the hours sold; the hours logged on a task are what its line delivered.
#[test]
fn test_an_order_becomes_a_project() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let consulting = service(&mut env, "project", None)?;
    let order = order(
        &mut env,
        consulting,
        &[("Analysis", "10"), ("Workshops", "4")],
    )?;
    let row = read(
        &mut env,
        "sale_order",
        order,
        &["name", "projects", "tasks", "lines"],
    )?;
    let projects = row["projects"].as_array().expect("projects").clone();
    let tasks = row["tasks"].as_array().expect("tasks").clone();
    assert_eq!((projects.len(), tasks.len()), (1, 2), "{row}");

    let project = projects[0].as_u64().expect("id") as u32;
    let name = format!("{} — Customer", row["name"].as_str().expect("a name"));
    let row_project = read(
        &mut env,
        "project_project",
        project,
        &["name", "sale_order"],
    )?;
    assert_eq!(row_project["name"], json!(name));
    assert_eq!(row_project["sale_order"], json!(order));

    let task = tasks[0].as_u64().expect("id") as u32;
    let task_row = read(
        &mut env,
        "project_task",
        task,
        &["planned_hours", "sale_line", "project"],
    )?;
    assert_eq!(number(&task_row["planned_hours"]), 10.0);
    let line = task_row["sale_line"].as_u64().expect("a line") as u32;

    create(
        &mut env,
        "timesheet",
        json!({"task": task, "name": "Interviews", "hours": "3"}),
    )?;
    let entry = create(
        &mut env,
        "timesheet",
        json!({"task": task, "name": "Report", "hours": "2"}),
    )?;
    let line_row = read(
        &mut env,
        "sale_order_line",
        line,
        &["qty_delivered", "qty_to_invoice"],
    )?;
    assert_eq!(number(&line_row["qty_delivered"]), 5.0);
    assert_eq!(number(&line_row["qty_to_invoice"]), 5.0);

    env.call_rpc("timesheet", "delete", &json!({"ids": [entry]}))?;
    let line_row = read(&mut env, "sale_order_line", line, &["qty_delivered"])?;
    assert_eq!(
        number(&line_row["qty_delivered"]),
        3.0,
        "time deleted is no longer delivered"
    );
    Ok(())
}

/// A service sold as a task in a given project adds its task there, and makes no project.
#[test]
fn test_a_task_in_a_given_project() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let support = create(&mut env, "project_project", json!({"name": "Support"}))?;
    let hotline = service(&mut env, "task", Some(support))?;
    let order = order(&mut env, hotline, &[("Hotline", "5")])?;
    let row = read(&mut env, "sale_order", order, &["projects", "tasks"])?;
    assert_eq!(row["projects"], json!([support]));
    assert_eq!(row["tasks"].as_array().map(Vec::len), Some(1));
    Ok(())
}

/// Goods, and services that track nothing, make no work.
#[test]
fn test_nothing_to_track() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let plain = service(&mut env, "no", None)?;
    let order = order(&mut env, plain, &[("Advice", "1")])?;
    let row = read(&mut env, "sale_order", order, &["projects", "tasks"])?;
    assert_eq!(
        (row["projects"].clone(), row["tasks"].clone()),
        (json!([]), json!([]))
    );
    Ok(())
}

/// The views of the order, the product, the project and the task resolve with what is added.
#[test]
fn test_views_load() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    for (model, kind) in [
        ("sale_order", "form"),
        ("product", "form"),
        ("project_project", "form"),
        ("project_task", "form"),
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

/// A service naming a project template makes the order's project from it: its columns and tasks
/// with the tasks of the lines, and as many hours planned as sold.
#[test]
fn test_an_order_starts_from_a_template() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let template = create(
        &mut env,
        "project_project",
        json!({"name": "Website", "is_template": true}),
    )?;
    for (name, sequence) in [("Design", 10), ("Build", 20)] {
        create(
            &mut env,
            "project_stage",
            json!({"name": name, "project": template, "sequence": sequence}),
        )?;
    }
    create(
        &mut env,
        "project_task",
        json!({"name": "Gather the brief", "project": template}),
    )?;
    let consulting = service(&mut env, "project", None)?;
    env.call_rpc(
        "product",
        "write",
        &json!({"ids": [consulting], "values": {"service_template": template}}),
    )?;
    let order = order(&mut env, consulting, &[("Design", "12"), ("Build", "20")])?;
    let row = read(&mut env, "sale_order", order, &["projects"])?;
    let project = row["projects"][0].as_u64().expect("a project") as u32;
    let row = read(
        &mut env,
        "project_project",
        project,
        &["template", "planned_hours", "task_count", "stages"],
    )?;
    assert_eq!(row["template"], json!(template));
    assert_eq!(number(&row["planned_hours"]), 32.0, "the hours sold");
    assert_eq!(
        row["task_count"],
        json!(3),
        "the template's task and the lines'"
    );
    assert_eq!(
        row["stages"].as_array().map(Vec::len),
        Some(2),
        "the template's columns"
    );
    Ok(())
}

/// Installed where demo data is wanted, the plugin loads its own and that of what it brings.
#[test]
fn test_installed_with_demo_data() -> Result<()> {
    let mut app = erp_test_support::app(plugins, &["sale"])?;
    app.load_plugin("account_test_chart")?;
    let mut env = admin_env(&app)?;
    let settings = xml_id(&mut env, "base.settings");
    env.call_rpc(
        "settings",
        "write",
        &json!({ "ids": [settings], "values": { "demo_data": true } }),
    )?;
    drop(env);
    app.load_plugin("sale_project")?;
    Ok(())
}
