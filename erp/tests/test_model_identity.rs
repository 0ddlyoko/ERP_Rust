use erp::app::Application;
use erp::types::model::CommonModel;
use erp_search_code_gen::make_domain;
use erp_types::field::{Decimal, IdMode, MultipleIds, NaiveDate, SingleId};
use erp_types::model::MapOfFields;
use std::collections::HashMap;
use std::error::Error;
use std::str::FromStr;
use test_utilities::models::{Invoice, MeterReading};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

fn new_app() -> Application {
    let mut app = Application::new_test();
    app.model_manager.register_model::<Invoice<_>>();
    app.model_manager.register_model::<MeterReading<_>>();
    app.model_manager.post_register();
    app
}

/// `id` is the identity: it is what the registry keys on and what `_get_model_name` returns.
#[test]
fn test_identity_is_the_id_not_the_table() {
    let app = new_app();

    assert_eq!(MeterReading::<SingleId>::_get_model_name(), "meter_reading");
    assert!(app.model_manager.is_valid_model("meter_reading"));
    assert!(
        !app.model_manager.is_valid_model("legacy_meter_data"),
        "the physical table must not be addressable as a model"
    );
}

/// `table_name` is carried as metadata, ready for the SQL backend.
#[test]
fn test_table_name_is_recorded() {
    let app = new_app();

    let model = app.model_manager.get_model("meter_reading");
    assert_eq!(model.name, "meter_reading");
    assert_eq!(model.table_name, "legacy_meter_data");
}

/// A model that declares no `table_name` stores under its own identity.
#[test]
fn test_table_name_defaults_to_the_id() {
    let app = new_app();

    let model = app.model_manager.get_model("invoice");
    assert_eq!(model.name, "invoice");
    assert_eq!(model.table_name, "invoice");
}

/// An unknown name is reported, not fatal.
#[test]
fn test_unknown_model_is_an_error() {
    let app = new_app();

    assert!(app.model_manager.try_get_model("meter_reading").is_ok());
    assert!(
        app.model_manager
            .try_get_model("legacy_meter_data")
            .is_err(),
        "the table name is not a model name"
    );
}

/// The whole record lifecycle still works when the two names differ.
#[test]
fn test_records_round_trip_under_an_aliased_table() -> Result<()> {
    let app = new_app();
    let read_on = NaiveDate::from_str("2026-02-11")?;

    let mut env = app.new_env()?;
    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("reference", "C-1024");
    map.insert("value", Decimal::from_str("1234.56")?);
    map.insert("read_on", read_on);
    let reading: MeterReading<SingleId> = env.create_new_record_from_map(map)?;
    assert_eq!(*reading.get_reference(&mut env)?, "C-1024".to_string());
    env.close()?;

    let mut env = app.new_env()?;
    let found: MeterReading<MultipleIds> =
        env.search(&make_domain!([("reference", "=", "C-1024")]))?;
    assert_eq!(found.id.get_ids_ref().len(), 1);
    assert_eq!(
        found.get_value(&mut env)?,
        vec![&Decimal::from_str("1234.56")?]
    );
    Ok(())
}
