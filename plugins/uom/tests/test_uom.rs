//! Units of measure: the seeded units, conversions between them, the rules a unit follows, who
//! may change them, and their views.

use base::BasePlugin;
use base::models::View;
use erp::Result;
use erp::app::Application;
use erp::data;
use erp::environment::Environment;
use erp::types::field::{IdMode, SingleId};
use erp::types::model::MapOfFields;
use erp_test_support::{admin_env, d};
use serde_json::json;
use uom::UomPlugin;
use uom::conversion::Rounding;
use uom::models::Uom;
use web::WebPlugin;

fn new_app() -> Result<Application> {
    erp_test_support::app(
        || -> Vec<Box<dyn erp::plugin::Plugin>> {
            vec![
                Box::new(BasePlugin {}),
                Box::new(WebPlugin {}),
                Box::new(UomPlugin {}),
            ]
        },
        &["uom"],
    )
}

fn employee_env(app: &Application) -> Result<Environment<'_>> {
    erp_test_support::user_env(app, "employee", &["base.group_user"])
}

fn uom(env: &mut Environment, xml_id: &str) -> Result<Uom<SingleId>> {
    env.named(&format!("uom.{xml_id}"))
}

fn new_uom(
    env: &mut Environment,
    name: &str,
    category: &str,
    kind: &str,
    ratio: &str,
) -> Result<u32> {
    let category = data::resolve(env, &format!("uom.{category}"))?.expect("seeded");
    let mut values = MapOfFields::default();
    values.insert("name", name);
    values.insert("category", category);
    values.insert("uom_type", kind);
    values.insert("ratio", d(ratio));
    Ok(env.create_records("uom", vec![values])?.get_ids_ref()[0])
}

/// Quantities convert between units of a category, rounded to the precision of the target unit.
#[test]
fn test_seeded_units_convert() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let unit = uom(&mut env, "uom_unit")?;
    let dozen = uom(&mut env, "uom_dozen")?;
    let kg = uom(&mut env, "uom_kg")?;
    let gram = uom(&mut env, "uom_gram")?;
    let lb = uom(&mut env, "uom_lb")?;
    let ton = uom(&mut env, "uom_ton")?;
    let day = uom(&mut env, "uom_day")?;
    let hour = uom(&mut env, "uom_hour")?;

    assert_eq!(
        dozen.convert_to(&mut env, d("3"), unit.clone(), Rounding::HalfUp)?,
        d("36")
    );
    assert_eq!(
        unit.convert_to(&mut env, d("18"), dozen.clone(), Rounding::HalfUp)?,
        d("1.5")
    );
    assert_eq!(
        unit.convert_to(&mut env, d("13"), dozen.clone(), Rounding::Up)?,
        d("1.09")
    );
    assert_eq!(
        gram.convert_to(&mut env, d("1500"), kg.clone(), Rounding::HalfUp)?,
        d("1.5")
    );
    assert_eq!(
        kg.convert_to(&mut env, d("1.2345"), gram.clone(), Rounding::HalfUp)?,
        d("1235")
    );
    assert_eq!(
        lb.convert_to(&mut env, d("2"), kg.clone(), Rounding::HalfUp)?,
        d("0.907")
    );
    assert_eq!(
        ton.convert_to(&mut env, d("0.5"), kg.clone(), Rounding::HalfUp)?,
        d("500")
    );
    assert_eq!(
        day.convert_to(&mut env, d("2.5"), hour, Rounding::HalfUp)?,
        d("20")
    );
    assert_eq!(
        kg.convert_to(&mut env, d("0"), gram, Rounding::HalfUp)?,
        d("0")
    );
    assert_eq!(
        dozen.convert_to(&mut env, d("-1"), unit, Rounding::HalfUp)?,
        d("-12")
    );
    assert_eq!(
        kg.convert_to(&mut env, d("1.23456"), kg.clone(), Rounding::HalfUp)?,
        d("1.235"),
        "the same unit only rounds"
    );
    Ok(())
}

/// Prices convert the other way round, and are not rounded: a unit at 2 is a dozen at 24.
#[test]
fn test_prices_convert() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let unit = uom(&mut env, "uom_unit")?;
    let dozen = uom(&mut env, "uom_dozen")?;
    let kg = uom(&mut env, "uom_kg")?;
    let gram = uom(&mut env, "uom_gram")?;
    assert_eq!(
        unit.convert_price(&mut env, d("2"), dozen.clone())?,
        d("24")
    );
    assert_eq!(
        dozen.convert_price(&mut env, d("10"), unit.clone())?,
        d("10") / d("12")
    );
    assert_eq!(kg.convert_price(&mut env, d("12.5"), gram)?, d("0.0125"));
    assert_eq!(
        kg.convert_price(&mut env, d("1.23456"), kg.clone())?,
        d("1.23456")
    );
    let error = kg
        .convert_price(&mut env, d("1"), unit)
        .expect_err("not the same thing")
        .to_string();
    assert!(error.contains("kg") && error.contains("Units"), "{error}");
    Ok(())
}

/// Kilograms are no number of units.
#[test]
fn test_units_of_different_categories_do_not_convert() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let unit = uom(&mut env, "uom_unit")?;
    let kg = uom(&mut env, "uom_kg")?;
    let error = kg
        .convert_to(&mut env, d("1"), unit, Rounding::HalfUp)
        .expect_err("not the same thing")
        .to_string();
    assert!(error.contains("kg") && error.contains("Units"), "{error}");
    Ok(())
}

/// A unit's ratio fits its type; a precision is positive; a category has one reference unit.
#[test]
fn test_units_follow_their_rules() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    assert!(new_uom(&mut env, "Pack", "category_unit", "bigger", "6").is_ok());
    for (name, kind, ratio) in [
        ("Wrong reference", "reference", "2"),
        ("Wrong bigger", "bigger", "0.5"),
        ("Wrong smaller", "smaller", "3"),
        ("Zero smaller", "smaller", "0"),
    ] {
        let error = env
            .savepoint(|env| new_uom(env, name, "category_unit", kind, ratio))
            .expect_err(name)
            .to_string();
        assert!(error.contains(name), "{error}");
    }
    let count = env.count(
        "uom",
        &erp_search_code_gen::make_domain!([("name", "like", "Wrong")]),
    )?;
    assert_eq!(count, 0, "a refused unit is not kept");

    let mut values = MapOfFields::default();
    values.insert("rounding", d("0"));
    let dozen = uom(&mut env, "uom_dozen")?;
    assert!(
        env.savepoint(|env| env.write("uom", &SingleId::from(dozen.get_id()), values))
            .is_err()
    );
    assert_eq!(
        *dozen.get_rounding(&mut env)?,
        d("0.01"),
        "the write was undone"
    );
    Ok(())
}

/// A second active reference unit in a category is refused; an archived one is not counted.
#[test]
fn test_a_category_has_one_reference_unit() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let error = env
        .savepoint(|env| new_uom(env, "Piece", "category_unit", "reference", "1"))
        .expect_err("Units is the reference")
        .to_string();
    assert!(error.contains("reference units"), "{error}");

    let unit = uom(&mut env, "uom_unit")?;
    unit.set_active(false, &mut env)?;
    assert!(new_uom(&mut env, "Piece", "category_unit", "reference", "1").is_ok());
    Ok(())
}

/// Employees read the units; administrators manage them.
#[test]
fn test_access_rights() -> Result<()> {
    let app = new_app()?;
    let mut env = employee_env(&app)?;
    let found = env.call_rpc("uom", "search", &json!({"domain": [["name", "=", "kg"]]}))?;
    assert_eq!(found.as_array().map(Vec::len), Some(1));
    assert!(new_uom(&mut env, "Pack", "category_unit", "bigger", "6").is_err());
    let kg = uom(&mut env, "uom_kg")?;
    assert!(kg.set_name("Kilo", &mut env).is_err());
    drop(env);

    let mut env = admin_env(&app)?;
    assert!(new_uom(&mut env, "Pack", "category_unit", "bigger", "6").is_ok());
    Ok(())
}

/// A unit used nowhere may be deleted; a category holding units may not.
#[test]
fn test_deleting() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let pack = new_uom(&mut env, "Pack", "category_unit", "bigger", "6")?;
    assert_eq!(env.delete("uom", &SingleId::from(pack))?, 1);
    let category = data::resolve(&mut env, "uom.category_unit")?.expect("seeded");
    assert!(
        env.delete("uom_category", &SingleId::from(category))
            .is_err()
    );
    Ok(())
}

/// Every view of the plugin resolves against its model.
#[test]
fn test_views_load() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    for (model, kind) in [
        ("uom", "list"),
        ("uom", "form"),
        ("uom", "search"),
        ("uom_category", "list"),
        ("uom_category", "form"),
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
    for action in ["uom.action_uoms", "uom.action_uom_categories"] {
        assert!(data::resolve(&mut env, action)?.is_some(), "{action}");
    }
    Ok(())
}
