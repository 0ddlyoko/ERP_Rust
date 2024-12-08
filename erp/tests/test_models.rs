use erp::app::Application;
use erp::model::MapOfFields;
use std::error::Error;
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
    record.insert("name", &"0ddlyoko's SO".to_string());
    record.insert("price", 100);
    record.insert("amount", 200);
    let mut record = env.create_new_record_from_map::<SaleOrder>(&mut record)?;
    assert_eq!(record.id, 1);
    assert_eq!(record.name, "0ddlyoko's SO");
    assert_eq!(record.price, 100);
    assert_eq!(record.amount, 200);
    assert_eq!(record.state, SaleOrderState::Draft);
    // Total price should be computed as we haven't passed it
    assert_eq!(record.total_price, 100 * 200);

    // Test if modifying it works
    record.amount = 20;
    env.save_record(&record);

    let record = env.get_record::<SaleOrder>(1)?;
    assert_eq!(record.id, 1);
    assert_eq!(record.name, "0ddlyoko's SO");
    assert_eq!(record.price, 100);
    assert_eq!(record.amount, 20);
    assert_eq!(record.state, SaleOrderState::Draft);
    // For now there is no link between fields, so the computed method is not recomputed
    assert_eq!(record.total_price, 100 * 200);

    // But we can force the computation of total_price
    env.call_compute_fields("sale_order", 1, &["total_price".to_string()])?;
    // TODO Add depends, so that we can check if the computed field is automatically called

    let mut record = env.get_record::<SaleOrder>(1)?;
    assert_eq!(record.id, 1);
    assert_eq!(record.name, "0ddlyoko's SO");
    assert_eq!(record.price, 100);
    assert_eq!(record.amount, 20);
    assert_eq!(record.state, SaleOrderState::Draft);
    assert_eq!(record.total_price, 100 * 20);

    // Changing the state
    record.state = SaleOrderState::Paid;
    env.save_record(&record);

    let record = env.get_record::<SaleOrder>(1)?;
    assert_eq!(record.state, SaleOrderState::Paid);
    Ok(())
}
