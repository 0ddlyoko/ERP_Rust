//! Computed fields that may also be set by hand: worked out when what they depend on changes,
//! and keeping what is written to them — a line's price filled from its product, then haggled.

use erp::app::Application;
use erp::jsonrpc;
use erp_types::field::{Decimal, SingleId};
use erp_types::model::MapOfFields;
use serde_json::{Value, json};
use std::error::Error;
use std::str::FromStr;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

mod lines {
    use code_gen::{Model, erp_methods};
    use erp::environment::Environment;
    use erp::types::field::{Decimal, IdMode, MultipleIds};
    use std::error::Error;

    /// A price worked out from a list price, and a total from the price.
    #[derive(Model)]
    #[erp(id = "priced_line", methods)]
    #[allow(dead_code)]
    pub struct PricedLine<Mode: IdMode> {
        pub id: Mode,
        #[erp(default = 0.0)]
        list_price: Decimal,
        #[erp(default = 1)]
        quantity: i32,
        #[erp(compute = "compute_price", depends = ["list_price"], stored, editable)]
        price: Decimal,
        #[erp(compute = "compute_total", depends = ["price", "quantity"], stored)]
        total: Decimal,
    }

    #[erp_methods]
    impl PricedLine<MultipleIds> {
        pub fn compute_price(
            &self,
            env: &mut Environment,
        ) -> Result<(), Box<dyn Error + Send + Sync>> {
            for line in self {
                let price = *line.get_list_price(env)?;
                line.set_price(price, env)?;
            }
            Ok(())
        }

        pub fn compute_total(
            &self,
            env: &mut Environment,
        ) -> Result<(), Box<dyn Error + Send + Sync>> {
            for line in self {
                let total = *line.get_price(env)? * Decimal::from(*line.get_quantity(env)?);
                line.set_total(total, env)?;
            }
            Ok(())
        }
    }
}

use lines::PricedLine;

fn new_app() -> Application {
    let mut app = Application::new_test();
    app.model_manager.register_model::<PricedLine<_>>();
    app.model_manager.post_register();
    app
}

fn d(value: &str) -> Decimal {
    Decimal::from_str(value).expect("a decimal")
}

fn values(pairs: &[(&str, &str)]) -> MapOfFields {
    let mut values = MapOfFields::default();
    for (field, value) in pairs {
        values.insert(field, d(value));
    }
    values
}

/// Created without a price, a line takes its list price; created with one, it keeps it.
#[test]
fn test_created_with_or_without_the_computed_value() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let computed: PricedLine<SingleId> =
        env.create_new_record_from_map(values(&[("list_price", "10")]))?;
    assert_eq!(*computed.get_price(&mut env)?, d("10"));
    assert_eq!(*computed.get_total(&mut env)?, d("10"));

    let haggled: PricedLine<SingleId> =
        env.create_new_record_from_map(values(&[("list_price", "10"), ("price", "8.5")]))?;
    assert_eq!(
        *haggled.get_price(&mut env)?,
        d("8.5"),
        "what was given is kept"
    );
    assert_eq!(*haggled.get_total(&mut env)?, d("8.5"), "and counted");
    env.close()?;

    let mut env = app.new_env()?;
    let haggled: PricedLine<SingleId> = env.get_record(haggled.get_id().into());
    assert_eq!(*haggled.get_price(&mut env)?, d("8.5"), "and saved");
    Ok(())
}

/// A price written by hand stays until the list price changes; written together with the list
/// price, the hand-written one wins.
#[test]
fn test_written_by_hand_until_what_it_depends_on_changes() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let line: PricedLine<SingleId> =
        env.create_new_record_from_map(values(&[("list_price", "10")]))?;
    let ids = SingleId::from(line.get_id());

    env.write("priced_line", &ids, values(&[("price", "7")]))?;
    assert_eq!(*line.get_price(&mut env)?, d("7"));
    assert_eq!(*line.get_total(&mut env)?, d("7"));

    env.write("priced_line", &ids, values(&[("list_price", "12")]))?;
    assert_eq!(
        *line.get_price(&mut env)?,
        d("12"),
        "the list price changed: worked out again"
    );

    env.write(
        "priced_line",
        &ids,
        values(&[("list_price", "15"), ("price", "14")]),
    )?;
    assert_eq!(
        *line.get_price(&mut env)?,
        d("14"),
        "written with it, the hand wins"
    );
    assert_eq!(*line.get_total(&mut env)?, d("14"));
    Ok(())
}

/// Clients are told they may edit it, unlike a field that is only computed.
#[test]
fn test_clients_may_edit_it() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let fields = env.call_rpc("priced_line", "fields_get", &json!({}))?;
    assert_eq!(fields["price"]["readonly"], json!(false), "{fields}");
    assert_eq!(fields["total"]["readonly"], json!(true), "{fields}");
    Ok(())
}

fn call(app: &Application, method: &str, params: Value) -> Value {
    let body = json!({"jsonrpc": "2.0", "method": method, "params": params, "id": 1}).to_string();
    let answer = jsonrpc::handle(app, None, &body).expect("an answer");
    assert!(answer.get("error").is_none(), "{method} failed: {answer}");
    answer["result"].clone()
}

/// In a form, changing the list price fills the price in; changing the price counts it as it is.
#[test]
fn test_a_form_fills_it_in_and_lets_it_be_changed() -> Result<()> {
    let app = new_app();
    let created = call(
        &app,
        "priced_line.create",
        json!({"values": {"list_price": "10"}}),
    );
    let id = created[0].clone();
    let answer = call(
        &app,
        "priced_line.onchange",
        json!({"id": id, "values": {"list_price": "20"}}),
    );
    assert_eq!(answer["values"]["price"], json!("20"), "{answer}");
    let answer = call(
        &app,
        "priced_line.onchange",
        json!({"id": id, "values": {"price": "18", "quantity": 2}}),
    );
    assert_eq!(answer["values"]["total"], json!("36"), "{answer}");
    assert!(
        answer["values"]
            .get("price")
            .is_none_or(|price| price == "18"),
        "the price typed stays: {answer}"
    );
    Ok(())
}
