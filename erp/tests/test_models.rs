use base::BasePlugin;
use erp::app::Application;
use erp::model::{CommonModel, MapOfFields};
use std::error::Error;
use base::models::{Contact, Lang};
use erp::field::{FieldDepend, IdMode, Reference, SingleId};
use test_utilities::models::{BaseSaleOrder, SaleOrder, SaleOrderLine, SaleOrderState};
use test_utilities::TestLibPlugin;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[test]
fn test_models() -> Result<()> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    app.load_plugin("test_lib_plugin")?;

    let mut env = app.new_env()?;

    // Create a new SO
    let mut sale_order_map = MapOfFields::default();
    sale_order_map.insert("name", "0ddlyoko's SO");
    let sale_order = env.create_new_record_from_map::<SaleOrder<_>>(sale_order_map)?;
    assert_eq!(sale_order.get_name(&mut env)?, "0ddlyoko's SO");
    assert_eq!(*sale_order.get_state(&mut env)?, SaleOrderState::Draft);
    assert_eq!(*sale_order.get_total_price(&mut env)?, 0, "Total price should be 0, as there is no line");
    assert!(sale_order.get_lines::<SaleOrderLine<_>>(&mut env)?.get_id_mode().is_empty());

    // Create a new SO line
    let mut sale_order_line_map = MapOfFields::default();
    sale_order_line_map.insert::<&i32>("price", &100);
    sale_order_line_map.insert::<&i32>("amount", &200);
    sale_order_line_map.insert::<&Reference<BaseSaleOrder, SingleId>>("order", &sale_order.id.clone().into());
    let sale_order_line = env.create_new_record_from_map::<SaleOrderLine<_>>(sale_order_line_map)?;
    assert_eq!(*sale_order_line.get_price(&mut env)?, 100);
    assert_eq!(*sale_order_line.get_amount(&mut env)?, 200);
    assert_eq!(*sale_order_line.get_total_price(&mut env)?, 100 * 200);

    // Test if modifying it works
    sale_order_line.set_amount(20, &mut env)?;

    assert_eq!(*sale_order_line.get_price(&mut env)?, 100);
    assert_eq!(*sale_order_line.get_amount(&mut env)?, 20);
    assert_eq!(*sale_order_line.get_total_price(&mut env)?, 100 * 20);
    assert_eq!(sale_order.get_lines::<SaleOrderLine<_>>(&mut env)?.get_id_mode(), sale_order_line.get_id_mode());

    // Change the state
    sale_order.set_state(SaleOrderState::Paid, &mut env)?;
    assert_eq!(*sale_order.get_state(&mut env)?, SaleOrderState::Paid);
    Ok(())
}

#[test]
fn test_ref() -> Result<()> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    app.load_plugin("base")?;
    app.load_plugin("test_lib_plugin")?;

    let mut env = app.new_env()?;
    // Create a new lang
    let mut record = MapOfFields::default();
    record.insert("name", "French");
    record.insert("code", "fr_FR");
    let lang = env.create_new_record_from_map::<Lang<_>>(record)?;

    // Create a new contact
    let mut record = MapOfFields::default();
    record.insert("name", "0ddlyoko");
    record.insert("email", "0ddlyoko@test.com");
    record.insert("lang", lang.get_id());
    let contact = env.create_new_record_from_map::<Contact<_>>(record)?;
    assert_eq!(contact.get_name(&mut env)?, "0ddlyoko");
    assert_eq!(contact.get_email(&mut env)?.clone(), Some(&"0ddlyoko@test.com".to_string()));
    let contact_lang = contact.get_lang::<Lang<_>>(&mut env)?;
    assert!(contact_lang.is_some());
    let contact_lang = contact_lang.unwrap();
    assert_eq!(contact_lang.get_name(&mut env)?, "French");
    assert_eq!(contact_lang.get_code(&mut env)?, "fr_FR");

    Ok(())
}

#[test]
fn test_many2one_one2many() -> Result<()> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    app.load_plugin("test_lib_plugin")?;

    let mut env = app.new_env()?;

    // Create a new SO
    let mut sale_order_map = MapOfFields::default();
    sale_order_map.insert("name", "0ddlyoko's SO");
    let sale_order = env.create_new_record_from_map::<SaleOrder<_>>(sale_order_map)?;

    // Create a new SO line
    let mut sale_order_line_map = MapOfFields::default();
    sale_order_line_map.insert::<&Reference<BaseSaleOrder, SingleId>>("order", &sale_order.id.clone().into());
    let sale_order_line = env.create_new_record_from_map::<SaleOrderLine<_>>(sale_order_line_map)?;

    // Check if there is the link from a sale_order_line to a sale_order
    let sale_order_linked = sale_order_line.get_order::<SaleOrder<_>>(&mut env)?;
    assert!(sale_order_linked.is_some());
    assert_eq!(sale_order_linked.unwrap().id, sale_order.id);
    // Check if there is the opposite link
    let sale_order_line_linked = sale_order.get_lines::<SaleOrderLine<_>>(&mut env)?;
    assert_eq!(sale_order_line_linked.id, sale_order_line.id);

    Ok(())
}

#[test]
fn test_depends() -> Result<()> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    app.load_plugin("test_lib_plugin")?;

    let sale_order = app.model_manager.get_model("sale_order");
    let sale_order_line = app.model_manager.get_model("sale_order_line");

    // SO
    assert!(sale_order.get_internal_field("name").depends.is_empty());
    assert!(sale_order.get_internal_field("state").depends.is_empty());
    assert!(sale_order.get_internal_field("total_price").depends.is_empty());
    assert!(sale_order.get_internal_field("lines").depends.is_empty(), "O2M shouldn't have any dependencies");

    // SOL
    let so_line_order = &sale_order_line.get_internal_field("order").depends;
    assert_eq!(so_line_order.len(), 1);
    assert_eq!(so_line_order[0], vec![
        FieldDepend::CurrentFieldAnotherModel { target_model: "sale_order".to_string(), field_name: "order".to_string() },
        FieldDepend::SameModel { field_name: "total_price".to_string() }
    ]);
    let so_line_price = &sale_order_line.get_internal_field("price").depends;
    assert_eq!(so_line_price.len(), 1);
    assert_eq!(so_line_price[0], vec![
        FieldDepend::SameModel { field_name: "total_price".to_string() }
    ]);
    let so_line_amount = &sale_order_line.get_internal_field("amount").depends;
    assert_eq!(so_line_amount.len(), 1);
    assert_eq!(so_line_amount[0], vec![
        FieldDepend::SameModel { field_name: "total_price".to_string() }
    ]);
    let so_line_amount = &sale_order_line.get_internal_field("total_price").depends;
    assert_eq!(so_line_amount.len(), 1);
    assert_eq!(so_line_amount[0], vec![
        FieldDepend::CurrentFieldAnotherModel { target_model: "sale_order".to_string(), field_name: "order".to_string() },
        FieldDepend::SameModel { field_name: "total_price".to_string() }
    ]);

    Ok(())
}
