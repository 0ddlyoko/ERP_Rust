//! Demo data: customers, products and orders shown once the database asks for them — by
//! every plugin installed then, and by each plugin installed afterwards — and loaded once.

use account::AccountPlugin;
use account::testing::TestChartPlugin;
use base::BasePlugin;
use contacts::ContactsPlugin;
use currency::CurrencyPlugin;
use erp::Result;
use erp::app::Application;
use erp::environment::Environment;
use erp::plugin::Plugin;
use erp_test_support::{admin_env, read, xml_id};
use mail::MailPlugin;
use product::ProductPlugin;
use sale::SalePlugin;
use sale_stock::SaleStockPlugin;
use sequence::SequencePlugin;
use serde_json::{Value, json};
use stock::StockPlugin;
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
        Box::new(StockPlugin {}),
        Box::new(SaleStockPlugin {}),
    ]
}

/// Ask for demo data, as an administrator does from the settings.
fn turn_demo(app: &Application, on: bool) -> Result<()> {
    let mut env = admin_env(app)?;
    let settings = xml_id(&mut env, "base.settings");
    env.call_rpc(
        "settings",
        "write",
        &json!({ "ids": [settings], "values": { "demo_data": on } }),
    )?;
    env.close()
}

fn count(env: &mut Environment, model: &str) -> Result<u64> {
    Ok(env
        .call_rpc(model, "count", &json!({}))?
        .as_u64()
        .unwrap_or_default())
}

/// The demo orders as they should stand: quotations, sent, confirmed with their delivery,
/// cancelled.
fn assert_demo_orders(env: &mut Environment) -> Result<()> {
    let state_of = |env: &mut Environment, name: &str| -> Result<Value> {
        let id = xml_id(env, name);
        read(env, "sale_order", id, &["state", "pickings"])
    };
    let confirmed = state_of(env, "sale.demo_order_1")?;
    assert_eq!(confirmed["state"], "sale");
    assert_eq!(confirmed["pickings"].as_array().map(Vec::len), Some(1));
    let services = state_of(env, "sale.demo_order_4")?;
    assert_eq!(services["state"], "sale");
    assert_eq!(state_of(env, "sale.demo_order_7")?["state"], "sent");
    assert_eq!(state_of(env, "sale.demo_order_10")?["state"], "cancel");
    let customer = xml_id(env, "base.demo_brasserie");
    assert_eq!(read(env, "contact", customer, &["city"])?["city"], "Liège");
    Ok(())
}

/// Turned on in a database already holding the plugins, demo data comes for each of them; turned
/// off and on again, it is not loaded twice.
#[test]
fn test_turning_demo_data_on_loads_that_of_each_plugin_once() -> Result<()> {
    let mut app = erp_test_support::app(plugins, &["sale_stock"])?;
    app.load_plugin("account_test_chart")?;
    {
        let mut env = admin_env(&app)?;
        assert_eq!(count(&mut env, "sale_order")?, 0);
    }

    turn_demo(&app, true)?;
    let mut env = admin_env(&app)?;
    assert_demo_orders(&mut env)?;
    let orders = count(&mut env, "sale_order")?;
    assert_eq!(orders, 10);
    let rows = env.call_rpc(
        "plugin",
        "read_matching",
        &json!({"domain": [["name", "in", ["base", "sale", "sale_stock"]]], "fields": ["demo_loaded"]}),
    )?;
    assert!(
        rows.as_array().is_some_and(
            |rows| rows.len() == 3 && rows.iter().all(|row| row["demo_loaded"] == true)
        ),
        "{rows}"
    );
    drop(env);

    turn_demo(&app, false)?;
    turn_demo(&app, true)?;
    let mut env = admin_env(&app)?;
    assert_eq!(count(&mut env, "sale_order")?, orders);
    Ok(())
}

/// Asked for before the plugins are installed, demo data comes with each as it installs: the
/// orders confirmed once deliveries exist have one.
#[test]
fn test_a_plugin_installed_after_demo_data_was_asked_for_brings_its_own() -> Result<()> {
    let mut app = erp_test_support::app(plugins, &["base"])?;
    turn_demo(&app, true)?;
    app.load_plugin("sale_stock")?;
    app.load_plugin("account_test_chart")?;
    let mut env = admin_env(&app)?;
    assert_demo_orders(&mut env)
}
