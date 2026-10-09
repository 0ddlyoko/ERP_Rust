use base::BasePlugin;
use erp::app::Application;
use erp::data;
use erp_search_code_gen::make_domain;
use erp_types::field::{Decimal, IdMode, MultipleIds, SingleId};
use erp_types::model::MapOfFields;
use std::collections::HashMap;
use std::error::Error;
use std::str::FromStr;
use test_utilities::models::{SaleOrder, SaleOrderLine};
use test_utilities::{SeedPlugin, TestLibPlugin};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// `model_data` lives in `base`, so both plugins are needed.
fn new_app() -> Result<Application> {
    Application::new_test_installed(
        || -> Vec<Box<dyn erp::plugin::Plugin>> {
            vec![
                Box::new(BasePlugin {}),
                Box::new(TestLibPlugin {}),
                Box::new(SeedPlugin {}),
            ]
        },
        &["seed_plugin"],
    )
}

/// Records declared in a data file exist once the plugin is loaded.
#[test]
fn test_records_are_created_from_the_file() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    let orders: SaleOrder<MultipleIds> =
        env.search(&make_domain!([("name", "=", "Seeded order")]))?;
    assert_eq!(orders.id.get_ids_ref().len(), 1);
    Ok(())
}

/// A field is read according to its declared type, not as text.
#[test]
fn test_values_are_parsed_by_kind() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    let invoices: SaleOrder<MultipleIds> =
        env.search(&make_domain!([("name", "=", "Seeded order")]))?;
    assert!(!invoices.id.is_empty());

    let ids = env.search_ids("invoice", &make_domain!([("name", "=", "Untouchable")]))?;
    let rows = env.read(
        "invoice",
        &MultipleIds::from(ids),
        &["amount_untaxed", "due_date"],
    )?;
    assert_eq!(
        rows[0].get::<&Decimal>("amount_untaxed"),
        &Decimal::from_str("1234.56")?,
        "a decimal must be parsed as a decimal, exactly"
    );
    assert!(
        rows[0]
            .get_option::<&erp_types::field::NaiveDate>("due_date")
            .is_some()
    );
    Ok(())
}

/// `ref=` resolves an external identifier to the record it designates.
#[test]
fn test_references_are_resolved() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    let order_id =
        data::resolve(&mut env, "seed_plugin.main_order")?.expect("the order must be registered");
    let lines: SaleOrderLine<MultipleIds> = env.search(&make_domain!([("price", "=", 7)]))?;
    assert_eq!(lines.id.get_ids_ref().len(), 1);

    let rows = env.read("sale_order_line", &lines.id, &["order"])?;
    assert_eq!(
        rows[0].get::<&u32>("order"),
        &order_id,
        "the line must point at the order the reference named"
    );
    Ok(())
}

/// The registry maps the external identifier to a technical id.
#[test]
fn test_external_ids_are_registered() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    assert!(data::resolve(&mut env, "seed_plugin.main_order")?.is_some());
    assert!(data::resolve(&mut env, "seed_plugin.first_line")?.is_some());
    assert!(
        data::resolve(&mut env, "seed_plugin.never_declared")?.is_none(),
        "an unknown identifier resolves to nothing rather than failing"
    );
    Ok(())
}

/// Loading the same file twice must not duplicate anything, and must not move the ids.
#[test]
fn test_loading_twice_is_idempotent() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    let before = data::resolve(&mut env, "seed_plugin.main_order")?.unwrap();
    assert_eq!(
        env.count("sale_order", &make_domain!([("name", "=", "Seeded order")]))?,
        1
    );

    data::load(
        &mut env,
        "seed_plugin",
        include_str!("../test_utilities/data/orders.xml"),
    )?;

    assert_eq!(
        env.count("sale_order", &make_domain!([("name", "=", "Seeded order")]))?,
        1,
        "a second load must not create a second record"
    );
    assert_eq!(
        data::resolve(&mut env, "seed_plugin.main_order")?,
        Some(before),
        "the technical id must not move"
    );
    Ok(())
}

/// A second load updates the record, unless it is protected.
#[test]
fn test_reload_updates_unless_protected() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    // Stand in for a user edit.
    let order_id = data::resolve(&mut env, "seed_plugin.main_order")?.unwrap();
    let mut edit = MapOfFields::new(HashMap::new());
    edit.insert("name", "Edited by hand");
    env.write("sale_order", &SingleId::from(order_id), edit)?;

    let invoice_id = data::resolve(&mut env, "seed_plugin.reference_invoice")?.unwrap();
    let mut edit = MapOfFields::new(HashMap::new());
    edit.insert("name", "Edited too");
    env.write("invoice", &SingleId::from(invoice_id), edit)?;

    data::load(
        &mut env,
        "seed_plugin",
        include_str!("../test_utilities/data/orders.xml"),
    )?;

    let rows = env.read("sale_order", &SingleId::from(order_id), &["name"])?;
    assert_eq!(
        rows[0].get::<&String>("name"),
        &"Seeded order".to_string(),
        "an unprotected record is brought back in line"
    );

    let rows = env.read("invoice", &SingleId::from(invoice_id), &["name"])?;
    assert_eq!(
        rows[0].get::<&String>("name"),
        &"Edited too".to_string(),
        "noupdate must leave the record to its owner"
    );
    Ok(())
}

/// A reference to something that was never declared is reported, and names it.
#[test]
fn test_unknown_reference_is_reported() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    let err = data::load(
        &mut env,
        "seed_plugin",
        r#"<erp>
            <record id="orphan" model="sale_order_line">
                <field name="order" ref="nowhere.at_all"/>
            </record>
        </erp>"#,
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("nowhere.at_all"),
        "the error must name the missing reference, got: {err}"
    );
    Ok(())
}

/// Malformed XML is reported rather than silently skipped.
#[test]
fn test_malformed_xml_is_reported() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    let err = data::load(&mut env, "seed_plugin", "<erp><record>").unwrap_err();
    assert!(err.to_string().contains("seed_plugin"), "got: {err}");
    Ok(())
}

/// Computed fields still run on records that came from a file.
#[test]
fn test_computed_fields_run_on_loaded_records() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    let order_id = data::resolve(&mut env, "seed_plugin.main_order")?.unwrap();
    let order: SaleOrder<SingleId> = env.get_record(order_id.into());
    assert_eq!(
        *order.get_total_price(&mut env)?,
        42,
        "7 * 6 from the seeded line"
    );
    Ok(())
}

/// The invoice model is untouched by the sale order file beyond what it declares.
#[test]
fn test_only_declared_records_are_created() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    assert_eq!(env.count("invoice", &make_domain!([]))?, 1);
    assert_eq!(env.count("sale_order", &make_domain!([]))?, 1);
    assert_eq!(env.count("sale_order_line", &make_domain!([]))?, 1);
    Ok(())
}

/// A field is named by its own tag.
#[test]
fn test_short_form_names_the_field() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    data::load(
        &mut env,
        "seed_plugin",
        r#"<erp>
            <record id="short" model="invoice">
                <name>Short form</name>
                <amount_untaxed>12.50</amount_untaxed>
            </record>
        </erp>"#,
    )?;

    let id = data::resolve(&mut env, "seed_plugin.short")?.expect("record must exist");
    let rows = env.read("invoice", &SingleId::from(id), &["name", "amount_untaxed"])?;
    assert_eq!(rows[0].get::<&String>("name"), &"Short form".to_string());
    assert_eq!(
        rows[0].get::<&Decimal>("amount_untaxed"),
        &Decimal::from_str("12.50")?
    );
    Ok(())
}

/// Both spellings may appear in the same record.
#[test]
fn test_both_forms_coexist() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    data::load(
        &mut env,
        "seed_plugin",
        r#"<erp>
            <record id="mixed" model="invoice">
                <name>Mixed</name>
                <field name="amount_untaxed">99.99</field>
            </record>
        </erp>"#,
    )?;

    let id = data::resolve(&mut env, "seed_plugin.mixed")?.unwrap();
    let rows = env.read("invoice", &SingleId::from(id), &["name", "amount_untaxed"])?;
    assert_eq!(rows[0].get::<&String>("name"), &"Mixed".to_string());
    assert_eq!(
        rows[0].get::<&Decimal>("amount_untaxed"),
        &Decimal::from_str("99.99")?
    );
    Ok(())
}

/// A reference works the same way in the short form.
#[test]
fn test_short_form_carries_a_reference() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    let order = data::resolve(&mut env, "seed_plugin.main_order")?.unwrap();
    data::load(
        &mut env,
        "seed_plugin",
        r#"<erp>
            <record id="extra_line" model="sale_order_line">
                <price>3</price>
                <order ref="main_order"/>
            </record>
        </erp>"#,
    )?;

    let id = data::resolve(&mut env, "seed_plugin.extra_line")?.unwrap();
    let rows = env.read("sale_order_line", &SingleId::from(id), &["order"])?;
    assert_eq!(rows[0].get::<&u32>("order"), &order);
    Ok(())
}

/// A tag that names no field is refused, and says which one.
#[test]
fn test_unknown_tag_is_refused() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    let err = data::load(
        &mut env,
        "seed_plugin",
        r#"<erp>
            <record id="bogus" model="invoice">
                <not_a_field>x</not_a_field>
            </record>
        </erp>"#,
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("not_a_field"),
        "the error must name the offending tag, got: {err}"
    );
    Ok(())
}

/// Comments and whitespace between fields are ignored.
#[test]
fn test_comments_are_not_fields() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    data::load(
        &mut env,
        "seed_plugin",
        r#"<erp>
            <record id="commented" model="invoice">
                <!-- this is not a field -->
                <name>Commented</name>
            </record>
        </erp>"#,
    )?;

    let id = data::resolve(&mut env, "seed_plugin.commented")?.unwrap();
    let rows = env.read("invoice", &SingleId::from(id), &["name"])?;
    assert_eq!(rows[0].get::<&String>("name"), &"Commented".to_string());
    Ok(())
}

/// `<field>` without a `name` attribute is the short form for the field called `field`.
///
/// This is the one place the two spellings meet, and it is what makes every field name
/// writable — including the ones that collide with the structural elements.
#[test]
fn test_field_tag_without_name_is_a_field_called_field() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    data::load(
        &mut env,
        "seed_plugin",
        r#"<erp>
            <record id="edge" model="tag">
                <name>Edge</name>
                <field>short form</field>
                <field name="field">overwritten by the long form</field>
            </record>
        </erp>"#,
    )?;

    let id = data::resolve(&mut env, "seed_plugin.edge")?.expect("record must exist");
    let rows = env.read("tag", &SingleId::from(id), &["name", "field"])?;
    assert_eq!(rows[0].get::<&String>("name"), &"Edge".to_string());
    assert_eq!(
        rows[0].get::<&String>("field"),
        &"overwritten by the long form".to_string(),
        "both spellings reach the same field"
    );
    Ok(())
}

/// The short form alone reaches it too.
#[test]
fn test_field_tag_alone_reaches_the_field() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    data::load(
        &mut env,
        "seed_plugin",
        r#"<erp>
            <record id="edge_short" model="tag">
                <field name="name">edge</field>
                <field>only the short form</field>
            </record>
        </erp>"#,
    )?;

    let id = data::resolve(&mut env, "seed_plugin.edge_short")?.unwrap();
    let rows = env.read("tag", &SingleId::from(id), &["field"])?;
    assert_eq!(
        rows[0].get::<&String>("field"),
        &"only the short form".to_string()
    );
    Ok(())
}

/// And the long form still names any field, including one it does not share a tag with.
#[test]
fn test_long_form_still_names_any_field() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    data::load(
        &mut env,
        "seed_plugin",
        r#"<erp>
            <record id="edge_long" model="tag">
                <field name="name">Named through the long form</field>
            </record>
        </erp>"#,
    )?;

    let id = data::resolve(&mut env, "seed_plugin.edge_long")?.unwrap();
    let rows = env.read("tag", &SingleId::from(id), &["name"])?;
    assert_eq!(
        rows[0].get::<&String>("name"),
        &"Named through the long form".to_string()
    );
    Ok(())
}

/// A record is named by its own tag, exactly as a field is.
#[test]
fn test_record_is_named_by_its_tag() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    data::load(
        &mut env,
        "seed_plugin",
        r#"<erp>
            <tag id="short_tag">
                <name>Named by its tag</name>
            </tag>
        </erp>"#,
    )?;

    let id = data::resolve(&mut env, "seed_plugin.short_tag")?.expect("record must exist");
    let rows = env.read("tag", &SingleId::from(id), &["name"])?;
    assert_eq!(
        rows[0].get::<&String>("name"),
        &"Named by its tag".to_string()
    );
    Ok(())
}

/// `<record>` without a `model` attribute is the short form for the model called `record`.
///
/// The mirror of `<field>` without a `name`: the one place the two spellings meet, and what
/// keeps every model name writable.
#[test]
fn test_record_tag_without_model_is_a_model_called_record() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    data::load(
        &mut env,
        "seed_plugin",
        r#"<erp>
            <record id="edge_record">
                <name>The model called record</name>
            </record>
        </erp>"#,
    )?;

    let id = data::resolve(&mut env, "seed_plugin.edge_record")?.expect("record must exist");
    let rows = env.read("record", &SingleId::from(id), &["name"])?;
    assert_eq!(
        rows[0].get::<&String>("name"),
        &"The model called record".to_string()
    );
    Ok(())
}

/// The long form still names any model, including one it does not share a tag with.
#[test]
fn test_long_form_still_names_any_model() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    data::load(
        &mut env,
        "seed_plugin",
        r#"<erp>
            <record id="long_tag" model="tag">
                <name>Named through the long form</name>
            </record>
        </erp>"#,
    )?;

    let id = data::resolve(&mut env, "seed_plugin.long_tag")?.unwrap();
    let rows = env.read("tag", &SingleId::from(id), &["name"])?;
    assert_eq!(
        rows[0].get::<&String>("name"),
        &"Named through the long form".to_string()
    );
    Ok(())
}

/// Both spellings designate the same record when they carry the same external identifier.
#[test]
fn test_both_spellings_reach_the_same_record() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    data::load(
        &mut env,
        "seed_plugin",
        r#"<erp>
            <tag id="same">
                <name>First</name>
            </tag>
            <record id="same" model="tag">
                <name>Second</name>
            </record>
        </erp>"#,
    )?;

    let ids = env.search_ids(
        "tag",
        &make_domain!([("name", "in", vec!["First", "Second"])]),
    )?;
    assert_eq!(ids.len(), 1, "the second element must update, not create");

    let id = data::resolve(&mut env, "seed_plugin.same")?.unwrap();
    let rows = env.read("tag", &SingleId::from(id), &["name"])?;
    assert_eq!(rows[0].get::<&String>("name"), &"Second".to_string());
    Ok(())
}

/// A tag naming no model at all is refused rather than silently skipped.
#[test]
fn test_unknown_model_tag_is_an_error() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;

    let err = data::load(
        &mut env,
        "seed_plugin",
        r#"<erp>
            <not_a_model id="oops">
                <name>Nothing declares this</name>
            </not_a_model>
        </erp>"#,
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("not_a_model"),
        "the error must name the offending tag, got: {err}"
    );
    Ok(())
}

/// A field holding elements, as a template's markup does, gets that content as it was written.
#[test]
fn test_a_field_can_hold_markup() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;
    data::load(
        &mut env,
        "seed_plugin",
        r#"<erp>
            <tag id="markup"><name><b class="x">bold</b> &amp; <i>more</i></name></tag>
            <tag id="plain"><name>just text</name></tag>
        </erp>"#,
    )?;
    let markup = data::resolve(&mut env, "seed_plugin.markup")?.expect("loaded");
    let rows = env.read("tag", &SingleId::from(markup), &["name"])?;
    assert_eq!(
        rows[0].get::<&String>("name"),
        r#"<b class="x">bold</b> &amp; <i>more</i>"#
    );
    let plain = data::resolve(&mut env, "seed_plugin.plain")?.expect("loaded");
    let rows = env.read("tag", &SingleId::from(plain), &["name"])?;
    assert_eq!(rows[0].get::<&String>("name"), "just text");
    Ok(())
}

/// A field may be given as an attribute of the record, a relational one as a reference.
#[test]
fn test_attributes_are_fields() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;
    let order = data::resolve(&mut env, "seed_plugin.main_order")?.unwrap();
    data::load(
        &mut env,
        "seed_plugin",
        r#"<erp>
            <sale_order_line id="attribute_line" price="4" order="main_order">
                <amount>2</amount>
            </sale_order_line>
        </erp>"#,
    )?;
    let id = data::resolve(&mut env, "seed_plugin.attribute_line")?.unwrap();
    let rows = env.read(
        "sale_order_line",
        &SingleId::from(id),
        &["price", "amount", "order"],
    )?;
    assert_eq!(rows[0].get::<&i32>("price"), &4);
    assert_eq!(rows[0].get::<&i32>("amount"), &2);
    assert_eq!(rows[0].get::<&u32>("order"), &order);
    Ok(())
}

/// For a model with a body field, what the record element holds is that field, not more fields.
#[test]
fn test_a_body_field_takes_the_content_of_the_record() -> Result<()> {
    let mut app = new_app()?;
    app.model_manager.set_data_body("tag", "name");
    let mut env = app.new_env_as_option(None)?;
    data::load(
        &mut env,
        "seed_plugin",
        r#"<erp>
            <tag id="bodied" field="beside">
                <b>bold</b> and <i>more</i>
            </tag>
        </erp>"#,
    )?;
    let id = data::resolve(&mut env, "seed_plugin.bodied")?.unwrap();
    let rows = env.read("tag", &SingleId::from(id), &["name", "field"])?;
    assert_eq!(
        rows[0].get::<&String>("name"),
        "<b>bold</b> and <i>more</i>"
    );
    assert_eq!(rows[0].get::<&String>("field"), "beside");
    Ok(())
}

/// For a model nesting records, one written inside another is its child, at any depth; the other
/// elements are still fields.
#[test]
fn test_records_nest_as_children() -> Result<()> {
    let mut app = new_app()?;
    app.model_manager.set_data_children("contact", "parent");
    let mut env = app.new_env_as_option(None)?;
    data::load(
        &mut env,
        "seed_plugin",
        r#"<erp>
            <contact id="acme" name="Acme">
                <contact id="alice" name="Alice">
                    <contact id="team" name="Team"/>
                </contact>
                <email>hello@acme.example</email>
            </contact>
        </erp>"#,
    )?;
    let id = |env: &mut erp::environment::Environment, name: &str| {
        data::resolve(env, &format!("seed_plugin.{name}"))
            .expect("resolved")
            .expect("loaded")
    };
    let (acme, alice, team) = (
        id(&mut env, "acme"),
        id(&mut env, "alice"),
        id(&mut env, "team"),
    );
    let rows = env.read(
        "contact",
        &MultipleIds::from(vec![acme, alice, team]),
        &["parent", "email"],
    )?;
    assert_eq!(rows[0].get_option::<&u32>("parent"), None);
    assert_eq!(
        rows[0].get_option::<&String>("email").map(String::as_str),
        Some("hello@acme.example")
    );
    assert_eq!(rows[1].get::<&u32>("parent"), &acme);
    assert_eq!(rows[2].get::<&u32>("parent"), &alice);
    Ok(())
}
