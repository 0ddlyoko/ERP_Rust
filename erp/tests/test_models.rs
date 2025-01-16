use base::BasePlugin;
use erp::app::Application;
use erp::model::MapOfFields;
use std::error::Error;
use base::models::{Contact, Lang};
use test_utilities::models::{SaleOrder, SaleOrderState};
use test_utilities::TestLibPlugin;

#[test]
fn test_models() -> Result<(), Box<dyn Error>> {
    let mut app = Application::new(test_utilities::default_config()?);
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    app.load_plugin("test_lib_plugin")?;

    let mut env = app.new_env();

    // Create a new SO
    let mut record = MapOfFields::default();
    record.insert("name", "0ddlyoko's SO");
    record.insert::<&i32>("price", &100);
    record.insert::<&i32>("amount", &200);
    let mut record = env.create_new_record_from_map::<SaleOrder>(&mut record)?;
    assert_eq!(record.id, 1);
    assert_eq!(record.get_name(&mut env)?, "0ddlyoko's SO");
    assert_eq!(*record.get_price(&mut env)?, 100);
    assert_eq!(*record.get_amount(&mut env)?, 200);
    assert_eq!(*record.get_state(&mut env)?, SaleOrderState::Draft);
    // Total price should be computed as we haven't passed it
    assert_eq!(*record.get_total_price(&mut env)?, 100 * 200);

    // Test if modifying it works
    record.set_amount(20, &mut env)?;

    let record = env.get_record::<SaleOrder>(1)?;
    assert_eq!(record.id, 1);
    assert_eq!(record.get_name(&mut env)?, "0ddlyoko's SO");
    assert_eq!(*record.get_price(&mut env)?, 100);
    assert_eq!(*record.get_amount(&mut env)?, 20);
    assert_eq!(*record.get_state(&mut env)?, SaleOrderState::Draft);
    // For now there is no link between fields, so the computed method is not recomputed
    assert_eq!(*record.get_total_price(&mut env)?, 100 * 200);

    // But we can force the computation of total_price
    env.call_compute_fields("sale_order", 1, &["total_price".to_string()])?;
    // TODO Add depends, so that we can check if the computed field is automatically called

    let record = env.get_record::<SaleOrder>(1)?;
    assert_eq!(record.id, 1);
    assert_eq!(record.get_name(&mut env)?, "0ddlyoko's SO");
    assert_eq!(*record.get_price(&mut env)?, 100);
    assert_eq!(*record.get_amount(&mut env)?, 20);
    assert_eq!(*record.get_state(&mut env)?, SaleOrderState::Draft);
    assert_eq!(*record.get_total_price(&mut env)?, 100 * 20);

    // Changing the state
    record.set_state(SaleOrderState::Paid, &mut env)?;

    let record = env.get_record::<SaleOrder>(1)?;
    assert_eq!(*record.get_state(&mut env)?, SaleOrderState::Paid);
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
    let lang = env.create_new_record_from_map::<Lang>(&mut record)?;

    // Create a new contact
    let mut record = MapOfFields::default();
    record.insert("name", "0ddlyoko");
    record.insert("email", "0ddlyoko@test.com");
    record.insert("lang", &lang.get_id());
    let contact = env.create_new_record_from_map::<Contact>(&mut record)?;
    assert_eq!(contact.get_id(), 2);
    assert_eq!(contact.get_name(&mut env)?, "0ddlyoko");
    assert_eq!(contact.get_email(&mut env)?.clone(), Some(&"0ddlyoko@test.com".to_string()));
    let contact_lang = contact.get_lang::<Lang>(&mut env)?;
    assert!(contact_lang.is_some());
    let contact_lang = contact_lang.unwrap();
    assert_eq!(contact_lang.get_id(), 1);
    assert_eq!(contact_lang.get_name(&mut env)?, "French");
    assert_eq!(contact_lang.get_code(&mut env)?, "fr_FR");

    Ok(())
}
