use base::BasePlugin;
use erp::app::Application;
use erp::model::{CommonModel, MapOfFields};
use std::error::Error;
use base::models::{Contact, Lang};
use erp::field::{IdMode, Reference, SingleId};
use test_utilities::models::{BaseSaleOrder, SaleOrder, SaleOrderLine, SaleOrderState};
use test_utilities::TestLibPlugin;

#[test]
fn test_models() -> Result<(), Box<dyn Error>> {
    let mut app = Application::new(test_utilities::default_config()?);
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    app.load_plugin("test_lib_plugin")?;

    let mut env = app.new_env();

    // Create a new SO
    let mut sale_order_map = MapOfFields::default();
    sale_order_map.insert("name", "0ddlyoko's SO");
    sale_order_map.insert::<&i32>("price", &100);
    sale_order_map.insert::<&i32>("amount", &200);
    let sale_order = env.create_new_record_from_map::<SaleOrder<_>>(&mut sale_order_map)?;
    assert_eq!(sale_order.get_name(&mut env)?, "0ddlyoko's SO");
    assert_eq!(*sale_order.get_state(&mut env)?, SaleOrderState::Draft);
    assert_eq!(*sale_order.get_total_price(&mut env)?, 0, "Total price should be 0, as there is no line");
    assert!(sale_order.get_lines::<SaleOrderLine<_>>(&mut env)?.get_id_mode().is_empty());

    // Create a new SO line
    let mut sale_order_line_map = MapOfFields::default();
    sale_order_line_map.insert::<&i32>("price", &100);
    sale_order_line_map.insert::<&i32>("amount", &200);
    sale_order_line_map.insert::<&Reference<BaseSaleOrder, SingleId>>("order", &sale_order.id.clone().into());
    let sale_order_line = env.create_new_record_from_map::<SaleOrderLine<_>>(&mut sale_order_line_map)?;
    assert_eq!(*sale_order_line.get_price(&mut env)?, 100);
    assert_eq!(*sale_order_line.get_amount(&mut env)?, 200);
    assert_eq!(*sale_order_line.get_total_price(&mut env)?, 100 * 200);

    // Test if modifying it works
    sale_order_line.set_amount(20, &mut env)?;

    assert_eq!(*sale_order_line.get_price(&mut env)?, 100);
    assert_eq!(*sale_order_line.get_amount(&mut env)?, 20);
    // TODO Later, this line should not fail, as it will automatically be recomputed
    // assert_eq!(*sale_order_line.get_total_price(&mut env)?, 100 * 20);
    // TODO Later, This should not fail if we add a link between sale_order & sale_order_line
    // assert_eq!(sale_order.get_lines::<SaleOrderLine<_>>(&mut env)?, vec![]);
    // TODO For now there is no link between fields, so the computed method is not recomputed.
    //  Fix that
    // assert_eq!(*sale_order.get_total_price(&mut env)?, 100 * 200);

    // But we can force the computation of total_price
    env.call_compute_method("sale_order_line", &sale_order_line.id, &["total_price".to_string()])?;
    // TODO Add depends, so that we can check if the computed field is automatically called
    //
    assert_eq!(*sale_order_line.get_total_price(&mut env)?, 100 * 20);
    // assert_eq!(*sale_order.get_total_price(&mut env)?, 100 * 20);

    // Change the state
    sale_order.set_state(SaleOrderState::Paid, &mut env)?;

    assert_eq!(*sale_order.get_state(&mut env)?, SaleOrderState::Paid);
    Ok(())
}

#[test]
fn test_ref() -> Result<(), Box<dyn Error>> {
    let mut app = Application::new(test_utilities::default_config()?);
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.load_plugin("test_lib_plugin")?;
    app.load_plugin("base")?;

    let mut env = app.new_env();
    // Create a new lang
    let mut record = MapOfFields::default();
    record.insert("name", "French");
    record.insert("code", "fr_FR");
    let lang = env.create_new_record_from_map::<Lang<_>>(&mut record)?;

    // Create a new contact
    let mut record = MapOfFields::default();
    record.insert("name", "0ddlyoko");
    record.insert("email", "0ddlyoko@test.com");
    record.insert("lang", &lang.get_id());
    let contact = env.create_new_record_from_map::<Contact<_>>(&mut record)?;
    assert_eq!(contact.get_id(), 2);
    assert_eq!(contact.get_name(&mut env)?, "0ddlyoko");
    assert_eq!(contact.get_email(&mut env)?.clone(), Some(&"0ddlyoko@test.com".to_string()));
    let contact_lang = contact.get_lang::<Lang<_>>(&mut env)?;
    assert!(contact_lang.is_some());
    let contact_lang = contact_lang.unwrap();
    assert_eq!(contact_lang.get_id(), 1);
    assert_eq!(contact_lang.get_name(&mut env)?, "French");
    assert_eq!(contact_lang.get_code(&mut env)?, "fr_FR");

    Ok(())
}

#[test]
fn test_many2one_one2many() -> Result<(), Box<dyn Error>> {
    let mut app = Application::new(test_utilities::default_config()?);
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    app.load_plugin("test_lib_plugin")?;

    let mut env = app.new_env();

    // Create a new SO
    let mut sale_order_map = MapOfFields::default();
    sale_order_map.insert("name", "0ddlyoko's SO");
    let sale_order = env.create_new_record_from_map::<SaleOrder<_>>(&mut sale_order_map)?;

    // Create a new SO line
    let mut sale_order_line_map = MapOfFields::default();
    sale_order_line_map.insert::<&Reference<BaseSaleOrder, SingleId>>("order", &sale_order.id.clone().into());
    let sale_order_line = env.create_new_record_from_map::<SaleOrderLine<_>>(&mut sale_order_line_map)?;

    // Check if there is the link from a sale_order_line to a sale_order
    let sale_order_linked = sale_order_line.get_order::<SaleOrder<_>>(&mut env)?;
    assert!(sale_order_linked.is_some());
    assert_eq!(sale_order_linked.unwrap().id, sale_order.id);
    // Check if there is the opposite link
    let sale_order_line_linked = sale_order.get_lines::<SaleOrderLine<_>>(&mut env)?;
    // TODO Once implemented, uncomment this line
    // assert_eq!(sale_order_line_linked.id, sale_order_line.id);


    Ok(())
}
