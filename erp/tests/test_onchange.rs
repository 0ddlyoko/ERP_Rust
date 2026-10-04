//! What a form shows before it is saved: the fields computed from the values the user changed,
//! worked out on virtual records — nothing created, written or saved.

use erp::app::Application;
use erp::jsonrpc;
use serde_json::{Value, json};
use std::error::Error;
use test_utilities::models::{SaleOrder, SaleOrderLine, Tag};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

mod gauges {
    use code_gen::{Model, erp_methods};
    use erp::environment::Environment;
    use erp::types::field::{IdMode, MultipleIds};
    use std::error::Error;

    /// Two computed fields: one that fails for a zero, one that never does.
    #[derive(Model)]
    #[erp(id = "gauge", methods)]
    #[allow(dead_code)]
    pub struct Gauge<Mode: IdMode> {
        pub id: Mode,
        #[erp(default = 1)]
        value: i32,
        #[erp(compute = "compute_inverse", depends = ["value"])]
        inverse: i32,
        #[erp(compute = "compute_double", depends = ["value"])]
        double: i32,
    }

    #[erp_methods]
    impl Gauge<MultipleIds> {
        pub fn compute_inverse(
            &self,
            env: &mut Environment,
        ) -> Result<(), Box<dyn Error + Send + Sync>> {
            for gauge in self {
                let value = *gauge.get_value(env)?;
                if value == 0 {
                    return Err("there is no inverse of zero".into());
                }
                gauge.set_inverse(100 / value, env)?;
            }
            Ok(())
        }

        pub fn compute_double(
            &self,
            env: &mut Environment,
        ) -> Result<(), Box<dyn Error + Send + Sync>> {
            for gauge in self {
                let value = *gauge.get_value(env)?;
                gauge.set_double(value * 2, env)?;
            }
            Ok(())
        }
    }
}

fn new_app() -> Application {
    let mut app = Application::new_test();
    app.model_manager.register_model::<gauges::Gauge<_>>();
    app.model_manager.register_model::<SaleOrder<_>>();
    app.model_manager.register_model::<SaleOrderLine<_>>();
    app.model_manager.register_model::<Tag<_>>();
    app.model_manager.post_register();
    app
}

/// One request, in its own unit of work, as the server answers it.
fn call(app: &Application, method: &str, params: Value) -> Value {
    let body = json!({"jsonrpc": "2.0", "method": method, "params": params, "id": 1}).to_string();
    let answer = jsonrpc::handle(app, None, &body).expect("a request with an id is owed an answer");
    assert!(answer.get("error").is_none(), "{method} failed: {answer}");
    answer["result"].clone()
}

/// An order of two lines: 10 × 2 and 5 × 1, so 25 in all. Returns its id and its lines' ids.
fn order(app: &Application) -> (u64, u64, u64) {
    let ids = call(
        app,
        "sale_order.create",
        json!({"values": {"name": "SO1", "lines": {"create": [
            {"price": 10, "amount": 2},
            {"price": 5, "amount": 1},
        ]}}}),
    );
    let order = ids[0].as_u64().unwrap();
    let read = call(
        app,
        "sale_order.read",
        json!({"ids": [order], "fields": ["lines", "total_price"]}),
    );
    assert_eq!(read[0]["total_price"], 25);
    let lines = &read[0]["lines"];
    (
        order,
        lines[0].as_u64().unwrap(),
        lines[1].as_u64().unwrap(),
    )
}

fn updated<'a>(answer: &'a Value, field: &str, id: u64) -> Option<&'a Value> {
    answer["lines"][field]["updated"]
        .as_array()?
        .iter()
        .find(|line| line["id"] == id)
        .map(|line| &line["values"])
}

fn created<'a>(answer: &'a Value, field: &str, draft: u64) -> Option<&'a Value> {
    answer["lines"][field]["created"]
        .as_array()?
        .iter()
        .find(|line| line["draft"] == draft)
        .map(|line| &line["values"])
}

/// Changing a line computes it again, and the order's total from it.
#[test]
fn test_a_changed_line_computes_the_total() -> Result<()> {
    let app = new_app();
    let (order, first, second) = order(&app);
    let answer = call(
        &app,
        "sale_order.onchange",
        json!({"id": order, "values": {"lines": {"update": [{"id": first, "amount": 3}]}}}),
    );
    assert_eq!(answer["values"]["total_price"], 35);
    assert_eq!(updated(&answer, "lines", first).unwrap()["total_price"], 30);
    assert!(
        updated(&answer, "lines", second).is_none_or(|values| values.get("total_price").is_none()),
        "a line nothing changed is not computed again: {answer}"
    );
    Ok(())
}

/// Lines added and removed count, the added ones named by their draft number.
#[test]
fn test_added_and_removed_lines() -> Result<()> {
    let app = new_app();
    let (order, _, second) = order(&app);
    let answer = call(
        &app,
        "sale_order.onchange",
        json!({"id": order, "values": {"lines": {
            "unlink": [second],
            "create": [{"draft": 7, "price": 4, "amount": 5}, {"draft": 8, "price": 1, "amount": 1}],
        }}}),
    );
    assert_eq!(answer["values"]["total_price"], 20 + 20 + 1);
    assert_eq!(created(&answer, "lines", 7).unwrap()["total_price"], 20);
    assert_eq!(created(&answer, "lines", 8).unwrap()["total_price"], 1);
    Ok(())
}

/// A field depending on the order's lines is computed again on every line, sent or not.
#[test]
fn test_a_line_not_sent_is_computed_again_when_it_depends_on_one_that_was() -> Result<()> {
    let app = new_app();
    let (order, first, second) = order(&app);
    let answer = call(
        &app,
        "sale_order.onchange",
        json!({"id": order, "values": {"lines": {"update": [{"id": first, "price": 12}]}}}),
    );
    assert_eq!(
        updated(&answer, "lines", second).unwrap()["siblings_total"],
        12 + 5
    );
    Ok(())
}

/// A value nothing depends on computes nothing.
#[test]
fn test_only_what_depends_on_a_change_is_computed() -> Result<()> {
    let app = new_app();
    let (order, _, _) = order(&app);
    let answer = call(
        &app,
        "sale_order.onchange",
        json!({"id": order, "values": {"name": "Renamed"}}),
    );
    assert_eq!(answer["values"], json!({}));
    Ok(())
}

/// A new record is worked out from its defaults and what the form holds, required fields left
/// empty included.
#[test]
fn test_a_new_record() -> Result<()> {
    let app = new_app();
    let answer = call(
        &app,
        "sale_order.onchange",
        json!({"values": {"name": "", "lines": {"create": [{"draft": 1, "price": 3, "amount": 2}]}}}),
    );
    assert_eq!(answer["values"]["total_price"], 6);
    assert_eq!(created(&answer, "lines", 1).unwrap()["total_price"], 6);
    Ok(())
}

/// Nothing an onchange does is saved.
#[test]
fn test_nothing_is_saved() -> Result<()> {
    let app = new_app();
    let (order, first, _) = order(&app);
    call(
        &app,
        "sale_order.onchange",
        json!({"id": order, "values": {"name": "Renamed", "lines": {
            "update": [{"id": first, "amount": 9}],
            "create": [{"draft": 1, "price": 1, "amount": 1}],
        }}}),
    );
    call(
        &app,
        "sale_order.onchange",
        json!({"values": {"name": "Never", "lines": {"create": [{"draft": 1}]}}}),
    );
    let read = call(
        &app,
        "sale_order.read",
        json!({"ids": [order], "fields": ["name", "lines", "total_price"]}),
    );
    assert_eq!(read[0]["name"], "SO1");
    assert_eq!(read[0]["total_price"], 25);
    assert_eq!(read[0]["lines"].as_array().map(Vec::len), Some(2));
    assert_eq!(call(&app, "sale_order.count", json!({"domain": []})), 1);
    assert_eq!(
        call(&app, "sale_order_line.count", json!({"domain": []})),
        2
    );
    Ok(())
}

/// A field whose computation fails is named with why, and the others are still computed.
#[test]
fn test_a_field_that_cannot_be_computed_is_named() -> Result<()> {
    let app = new_app();
    let answer = call(&app, "gauge.onchange", json!({"values": {"value": 0}}));
    assert_eq!(answer["values"]["double"], 0, "{answer}");
    assert!(answer["values"].get("inverse").is_none());
    assert_eq!(answer["errors"][0]["field"], "inverse");
    assert_eq!(
        answer["errors"][0]["message"],
        "there is no inverse of zero"
    );

    let answer = call(&app, "gauge.onchange", json!({"values": {"value": 4}}));
    assert_eq!(answer["values"]["inverse"], 25);
    assert_eq!(answer["errors"], json!([]));
    Ok(())
}
