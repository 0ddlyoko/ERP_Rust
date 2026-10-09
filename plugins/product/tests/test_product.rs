//! Products: defaults, names, categories, the rules a product follows, who may change them,
//! the chatter, and the views.

use base::BasePlugin;
use base::models::View;
use erp::Result;
use erp::app::Application;
use erp::environment::Environment;
use erp::types::field::{IdMode, SingleId};
use erp::types::model::MapOfFields;
use erp_test_support::{admin_env, d, user_env, xml_id};
use mail::MailPlugin;
use product::ProductPlugin;
use product::models::{Product, ProductCategory, ProductType};
use serde_json::json;
use uom::UomPlugin;
use uom::models::Uom;
use web::WebPlugin;

fn new_app() -> Result<Application> {
    erp_test_support::app(
        || -> Vec<Box<dyn erp::plugin::Plugin>> {
            vec![
                Box::new(BasePlugin {}),
                Box::new(WebPlugin {}),
                Box::new(MailPlugin {}),
                Box::new(UomPlugin {}),
                Box::new(ProductPlugin {}),
            ]
        },
        &["product"],
    )
}

fn new_product(env: &mut Environment, values: serde_json::Value) -> Result<Product<SingleId>> {
    let ids = env.call_rpc("product", "create", &json!({ "values": values }))?;
    let id = ids[0].as_u64().expect("an id") as u32;
    Ok(env.get_record(id.into()))
}

/// Nothing but a name makes a product: goods of category All, counted and bought in units,
/// sold and purchased, at no price yet.
#[test]
fn test_a_name_is_enough() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let desk = new_product(&mut env, json!({"name": "Desk"}))?;
    assert!(matches!(
        *desk.get_product_type(&mut env)?,
        ProductType::Goods
    ));
    let category: ProductCategory<SingleId> = desk.get_category(&mut env)?;
    assert_eq!(category.get_id(), xml_id(&mut env, "product.category_all"));
    let uom: Uom<SingleId> = desk.get_uom(&mut env)?;
    let purchase: Uom<SingleId> = desk.get_purchase_uom(&mut env)?;
    assert_eq!(uom.get_id(), xml_id(&mut env, "uom.uom_unit"));
    assert_eq!(purchase.get_id(), uom.get_id());
    assert!(*desk.get_sale_ok(&mut env)? && *desk.get_purchase_ok(&mut env)?);
    assert_eq!(*desk.get_list_price(&mut env)?, d("0"));
    assert_eq!(*desk.get_standard_price(&mut env)?, d("0"));
    Ok(())
}

/// A product with a reference is shown and found as `[REF] Name`.
#[test]
fn test_the_reference_comes_first() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let chair = new_product(&mut env, json!({"name": "Chair", "default_code": "CH-01"}))?;
    assert_eq!(chair.get_display_name(&mut env)?, "[CH-01] Chair");
    let plain = new_product(&mut env, json!({"name": "Plain"}))?;
    assert_eq!(plain.get_display_name(&mut env)?, "Plain");
    chair.set_default_code(None::<String>, &mut env)?;
    assert_eq!(chair.get_display_name(&mut env)?, "Chair");

    chair.set_default_code(Some("CH-02".to_string()), &mut env)?;
    let found = env.call_rpc("product", "name_search", &json!({"text": "CH-02"}))?;
    assert_eq!(found, json!([[chair.get_id(), "[CH-02] Chair"]]));
    Ok(())
}

/// Categories are named from the top down and follow their parent's renaming; none is its own
/// ancestor.
#[test]
fn test_categories_nest() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let saleable: ProductCategory<SingleId> = env.named("product.category_saleable")?;
    assert_eq!(saleable.get_complete_name(&mut env)?, "All / Saleable");
    let mut values = MapOfFields::default();
    values.insert("name", "Office");
    values.insert("parent", saleable.get_id());
    let office = env
        .create_records("product_category", vec![values])?
        .get_ids_ref()[0];
    let office: ProductCategory<SingleId> = env.get_record(office.into());
    assert_eq!(
        office.get_complete_name(&mut env)?,
        "All / Saleable / Office"
    );
    let all: ProductCategory<SingleId> = env.named("product.category_all")?;
    all.set_name("Everything", &mut env)?;
    assert_eq!(
        office.get_complete_name(&mut env)?,
        "Everything / Saleable / Office"
    );

    let error = all
        .set_parent(&office, &mut env)
        .expect_err("a cycle")
        .to_string();
    assert!(error.contains("under itself"), "{error}");
    Ok(())
}

/// Prices may be negative; the purchase unit measures what the unit measures; a barcode is
/// unique.
#[test]
fn test_products_follow_their_rules() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let discount = new_product(
        &mut env,
        json!({"name": "Loyalty discount", "product_type": "service", "list_price": "-5"}),
    )?;
    assert_eq!(*discount.get_list_price(&mut env)?, d("-5"));
    let kg = xml_id(&mut env, "uom.uom_kg");
    let dozen = xml_id(&mut env, "uom.uom_dozen");
    let error = new_product(&mut env, json!({"name": "Flour", "purchase_uom": kg}))
        .map(|product| product.get_id())
        .expect_err("kg are no units")
        .to_string();
    assert!(error.contains("purchase unit"), "{error}");
    let eggs = new_product(&mut env, json!({"name": "Eggs", "purchase_uom": dozen}))?;
    let purchase: Uom<SingleId> = eggs.get_purchase_uom(&mut env)?;
    assert_eq!(purchase.get_id(), dozen);
    let flour = new_product(&mut env, json!({"name": "Flour", "uom": kg}))?;
    let purchase: Uom<SingleId> = flour.get_purchase_uom(&mut env)?;
    assert_eq!(purchase.get_id(), kg, "bought in the unit it is counted in");

    new_product(
        &mut env,
        json!({"name": "Scanned", "barcode": "5410000000001"}),
    )?;
    let error = new_product(
        &mut env,
        json!({"name": "Copy", "barcode": "5410000000001"}),
    )
    .map(|product| product.get_id())
    .expect_err("taken")
    .to_string();
    assert!(error.contains("5410000000001"), "{error}");
    let count = env.count(
        "product",
        &erp_search_code_gen::make_domain!([("name", "=", "Copy")]),
    )?;
    assert_eq!(count, 0, "refused products are not kept");
    Ok(())
}

/// A unit products are counted or bought in keeps its category; an unused one may move.
#[test]
fn test_a_used_unit_keeps_its_category() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let dozen = xml_id(&mut env, "uom.uom_dozen");
    let weight = xml_id(&mut env, "uom.category_weight");
    let unit_category = xml_id(&mut env, "uom.category_unit");
    new_product(&mut env, json!({"name": "Eggs", "purchase_uom": dozen}))?;
    let error = env
        .call_rpc(
            "uom",
            "write",
            &json!({"ids": [dozen], "values": {"category": weight, "uom_type": "bigger"}}),
        )
        .expect_err("eggs are bought by the dozen")
        .to_string();
    assert!(error.contains("bought in Dozens"), "{error}");
    let ton = xml_id(&mut env, "uom.uom_ton");
    env.call_rpc(
        "uom",
        "write",
        &json!({"ids": [ton], "values": {"category": unit_category}}),
    )?;
    Ok(())
}

/// A service is no goods, and a category used by a product cannot be deleted.
#[test]
fn test_services_and_deleting() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let services = xml_id(&mut env, "product.category_services");
    let support = new_product(
        &mut env,
        json!({"name": "Support", "product_type": "service", "category": services}),
    )?;
    assert!(matches!(
        *support.get_product_type(&mut env)?,
        ProductType::Service
    ));
    assert!(
        env.delete("product_category", &SingleId::from(services))
            .is_err()
    );
    assert_eq!(env.delete("product", &SingleId::from(support.get_id()))?, 1);
    assert_eq!(
        env.delete("product_category", &SingleId::from(services))?,
        1
    );
    Ok(())
}

/// Archived products are left out of searches until asked for, and come back.
#[test]
fn test_archiving() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let old = new_product(&mut env, json!({"name": "Old model"}))?;
    env.call_rpc("product", "archive", &json!({"ids": [old.get_id()]}))?;
    assert!(!*old.get_active(&mut env)?);
    env.call_rpc("product", "unarchive", &json!({"ids": [old.get_id()]}))?;
    assert!(*old.get_active(&mut env)?);
    Ok(())
}

/// A price change is noted in the product's thread, once the work is saved.
#[test]
fn test_price_changes_are_tracked() -> Result<()> {
    let app = new_app()?;
    let lamp = {
        let mut env = admin_env(&app)?;
        let lamp = new_product(&mut env, json!({"name": "Lamp", "list_price": "20"}))?.get_id();
        env.close()?;
        lamp
    };
    {
        let mut env = admin_env(&app)?;
        let product: Product<SingleId> = env.get_record(lamp.into());
        product.set_list_price(d("25"), &mut env)?;
        env.close()?;
    }
    let mut env = admin_env(&app)?;
    let thread = env.call_rpc(
        "message",
        "thread",
        &json!({"ids": [], "args": {"model": "product", "record": lamp}}),
    )?;
    let tracking = thread
        .as_array()
        .expect("a thread")
        .iter()
        .find(|message| message["kind"] == "tracking")
        .unwrap_or_else(|| panic!("a tracking message in {thread}"));
    assert_eq!(
        tracking["changes"],
        json!([{"field": "list_price", "label": "Sales price", "old": "20", "new": "25",
                "old_value": "20", "new_value": "25"}])
    );
    Ok(())
}

/// Employees create and change products and read categories; only administrators manage the
/// categories and delete products.
#[test]
fn test_access_rights() -> Result<()> {
    let app = new_app()?;
    let mut env = user_env(&app, "employee", &["base.group_user"])?;
    let mug = new_product(&mut env, json!({"name": "Mug"}))?;
    mug.set_list_price(d("7.5"), &mut env)?;
    assert!(
        env.delete("product", &SingleId::from(mug.get_id()))
            .is_err()
    );
    let mut values = MapOfFields::default();
    values.insert("name", "Mine");
    assert!(
        env.create_records("product_category", vec![values])
            .is_err()
    );
    Ok(())
}

/// Every view resolves against its model.
#[test]
fn test_views_load() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    for (model, kind) in [
        ("product", "list"),
        ("product", "form"),
        ("product", "search"),
        ("product_category", "list"),
        ("product_category", "form"),
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
    for action in [
        "product.action_products",
        "product.action_product_categories",
    ] {
        xml_id(&mut env, action);
    }
    Ok(())
}
