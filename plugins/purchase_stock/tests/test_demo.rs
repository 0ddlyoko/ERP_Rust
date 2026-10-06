//! Demo purchases: requests for quotation, and orders confirmed once receipts exist, each with its
//! receipt.

use account::AccountPlugin;
use account::testing::TestChartPlugin;
use base::BasePlugin;
use contacts::ContactsPlugin;
use currency::CurrencyPlugin;
use erp::Result;
use erp::environment::Environment;
use erp_test_support::{admin_env, xml_id};
use mail::MailPlugin;
use product::ProductPlugin;
use purchase::PurchasePlugin;
use purchase_stock::PurchaseStockPlugin;
use sequence::SequencePlugin;
use serde_json::{Value, json};
use stock::StockPlugin;
use uom::UomPlugin;
use web::WebPlugin;

fn read(env: &mut Environment, model: &str, id: u32, fields: &[&str]) -> Result<Value> {
    Ok(env.call_rpc(model, "read", &json!({"ids": [id], "fields": fields}))?[0].clone())
}

/// Demo data asked for, the purchases installed bring their orders: three confirmed, each with
/// its receipt, one sent to its vendor.
#[test]
fn test_demo_purchases_come_with_their_receipts() -> Result<()> {
    let mut app = erp_test_support::app(
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
            Box::new(PurchasePlugin {}),
            Box::new(StockPlugin {}),
            Box::new(PurchaseStockPlugin {}),
        ],
        "base",
    )?;
    {
        let mut env = admin_env(&app)?;
        let settings = xml_id(&mut env, "base.settings");
        env.call_rpc(
            "settings",
            "write",
            &json!({ "ids": [settings], "values": { "demo_data": true } }),
        )?;
        env.close()?;
    }
    app.load_plugin("purchase_stock")?;
    app.load_plugin("account_test_chart")?;

    let mut env = admin_env(&app)?;
    let confirmed = xml_id(&mut env, "purchase.demo_purchase_2");
    let order = read(
        &mut env,
        "purchase_order",
        confirmed,
        &["state", "pickings"],
    )?;
    assert_eq!(order["state"], "purchase");
    assert_eq!(order["pickings"].as_array().map(Vec::len), Some(1));
    let sent = xml_id(&mut env, "purchase.demo_purchase_4");
    assert_eq!(
        read(&mut env, "purchase_order", sent, &["state"])?["state"],
        "sent"
    );
    Ok(())
}
