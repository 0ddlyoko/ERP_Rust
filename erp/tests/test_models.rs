use base::BasePlugin;
use base::models::{Contact, Lang};
use erp::app::Application;
use erp_types::field::{FieldDepend, Reference};
use erp_types::field::{IdMode, SingleId};
use erp_types::model::{CommonModel, MapOfFields};
use std::error::Error;
use test_utilities::TestLibPlugin;
use test_utilities::models::{BaseSaleOrder, SaleOrder, SaleOrderLine, SaleOrderState};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

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
    assert_eq!(
        *sale_order.get_total_price(&mut env)?,
        0,
        "Total price should be 0, as there is no line"
    );
    assert!(
        sale_order
            .get_lines::<SaleOrderLine<_>>(&mut env)?
            .get_id_mode()
            .is_empty()
    );

    // Create a new SO line
    let mut sale_order_line_map = MapOfFields::default();
    sale_order_line_map.insert::<&i32>("price", &100);
    sale_order_line_map.insert::<&i32>("amount", &200);
    sale_order_line_map
        .insert::<&Reference<BaseSaleOrder, SingleId>>("order", &sale_order.id.clone().into());
    let sale_order_line =
        env.create_new_record_from_map::<SaleOrderLine<_>>(sale_order_line_map)?;
    assert_eq!(*sale_order_line.get_price(&mut env)?, 100);
    assert_eq!(*sale_order_line.get_amount(&mut env)?, 200);
    assert_eq!(*sale_order_line.get_total_price(&mut env)?, 100 * 200);

    // Test if modifying it works
    sale_order_line.set_amount(20, &mut env)?;

    assert_eq!(*sale_order_line.get_price(&mut env)?, 100);
    assert_eq!(*sale_order_line.get_amount(&mut env)?, 20);
    assert_eq!(*sale_order_line.get_total_price(&mut env)?, 100 * 20);
    assert_eq!(
        sale_order
            .get_lines::<SaleOrderLine<_>>(&mut env)?
            .get_id_mode(),
        sale_order_line.get_id_mode()
    );

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

    let mut env = app.new_env_as_option(None)?;
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
    assert_eq!(
        contact.get_email(&mut env)?.clone(),
        Some(&"0ddlyoko@test.com".to_string())
    );
    let contact_lang = contact.get_lang::<Lang<_>>(&mut env)?;
    assert!(!contact_lang.is_empty());
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
    sale_order_line_map
        .insert::<&Reference<BaseSaleOrder, SingleId>>("order", &sale_order.id.clone().into());
    let sale_order_line =
        env.create_new_record_from_map::<SaleOrderLine<_>>(sale_order_line_map)?;

    // Check if there is the link from a sale_order_line to a sale_order
    let sale_order_linked = sale_order_line.get_order::<SaleOrder<_>>(&mut env)?;
    assert!(!sale_order_linked.is_empty());
    assert_eq!(sale_order_linked.id, sale_order.id);
    // Check if there is the opposite link
    let sale_order_line_linked = sale_order.get_lines::<SaleOrderLine<_>>(&mut env)?;
    assert_eq!(sale_order_line_linked.id, sale_order_line.id);

    Ok(())
}

/// Reverse dependencies, spelled out in full.
///
/// The set is compared exactly rather than by prefix: a spurious edge is as much a bug as a
/// missing one, since it makes the ORM recompute fields nothing changed for.
fn depends_of(
    model: &erp_internal_types::FinalInternalModel,
    field: &str,
) -> Vec<Vec<FieldDepend>> {
    let mut depends = model.get_internal_field(field).depends.clone();
    depends.sort_by_key(|chain| format!("{chain:?}"));
    depends
}

fn same(field: &str) -> FieldDepend {
    FieldDepend::SameModel {
        field_name: field.to_string(),
    }
}

fn through(model: &str, field: &str) -> FieldDepend {
    FieldDepend::CurrentFieldAnotherModel {
        target_model: model.to_string(),
        field_name: field.to_string(),
    }
}

fn hop(model: &str, field: &str) -> FieldDepend {
    FieldDepend::AnotherModel {
        target_model: model.to_string(),
        target_field: field.to_string(),
    }
}

fn sorted(mut chains: Vec<Vec<FieldDepend>>) -> Vec<Vec<FieldDepend>> {
    chains.sort_by_key(|chain| format!("{chain:?}"));
    chains
}

#[test]
fn test_depends() -> Result<()> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    app.load_plugin("test_lib_plugin")?;

    let sale_order = app.model_manager.get_model("sale_order");
    let sale_order_line = app.model_manager.get_model("sale_order_line");

    // SO
    assert!(depends_of(sale_order, "name").is_empty());
    assert!(depends_of(sale_order, "state").is_empty());
    assert!(depends_of(sale_order, "total_price").is_empty());
    assert!(
        depends_of(sale_order, "lines").is_empty(),
        "O2M shouldn't have any dependencies"
    );
    assert!(
        depends_of(sale_order, "tags").is_empty(),
        "a M2M carries its edges on the mirror side, and a write touches both"
    );

    // SOL
    assert_eq!(
        depends_of(sale_order_line, "order"),
        sorted(vec![
            vec![through("sale_order", "order"), same("total_price")],
            vec![same("order_tags")],
            vec![same("siblings_total")],
            vec![
                through("sale_order", "order"),
                hop("sale_order_line", "order"),
                same("siblings_total"),
            ],
        ]),
        "moving a line re-totals both orders, and re-reads what the new one carries"
    );
    assert_eq!(
        depends_of(sale_order_line, "price"),
        sorted(vec![
            vec![same("total_price")],
            vec![
                through("sale_order", "order"),
                hop("sale_order_line", "order"),
                same("siblings_total"),
            ],
        ])
    );
    assert_eq!(
        depends_of(sale_order_line, "amount"),
        vec![vec![same("total_price")]]
    );
    assert_eq!(
        depends_of(sale_order_line, "total_price"),
        vec![vec![through("sale_order", "order"), same("total_price")]]
    );

    Ok(())
}

/// A `depends` reaching through a many2many lands on the mirror field of the relation.
#[test]
fn test_depends_through_many2many() -> Result<()> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    app.load_plugin("test_lib_plugin")?;

    let tag = app.model_manager.get_model("tag");

    let two_hops = vec![through("invoice", "invoices"), same("tag_summary")];
    let three_hops = vec![
        through("sale_order", "orders"),
        hop("sale_order_line", "order"),
        same("order_tags"),
    ];

    assert_eq!(
        depends_of(tag, "name"),
        sorted(vec![two_hops, three_hops.clone()]),
        "renaming a tag must reach every model listing it, however far"
    );
    assert_eq!(
        depends_of(tag, "orders"),
        vec![three_hops],
        "so must linking or unlinking one"
    );
    Ok(())
}
