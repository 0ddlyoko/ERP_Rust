use erp::Result;
use erp::app::Application;
use erp_search_code_gen::make_domain;
use erp_types::field::{IdMode, MultipleIds, SingleId};
use erp_types::model::MapOfFields;
use std::collections::HashMap;
use test_utilities::models::{Invoice, Tag};

fn new_app() -> Application {
    let mut app = Application::new_test();
    app.model_manager.register_model::<Invoice<_>>();
    app.model_manager.register_model::<Tag<_>>();
    app.model_manager.post_register();
    app
}

fn create(env: &mut erp::environment::Environment, model: &str, name: &str) -> Result<u32> {
    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("name", name);
    let ids: MultipleIds = env.create_records(model, vec![map])?;
    Ok(*ids.get_ids_ref().first().unwrap())
}

fn set_tags(env: &mut erp::environment::Environment, invoice: u32, tags: Vec<u32>) -> Result<()> {
    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("tags", tags);
    env.write("invoice", &SingleId::from(invoice), map)
}

fn tags_of(env: &mut erp::environment::Environment, invoice: u32) -> Result<Vec<u32>> {
    let rows = env.read("invoice", &SingleId::from(invoice), &["tags"])?;
    Ok(rows[0]
        .get_option::<&Vec<u32>>("tags")
        .cloned()
        .unwrap_or_default())
}

fn invoices_of(env: &mut erp::environment::Environment, tag: u32) -> Result<Vec<u32>> {
    let rows = env.read("tag", &SingleId::from(tag), &["invoices"])?;
    Ok(rows[0]
        .get_option::<&Vec<u32>>("invoices")
        .cloned()
        .unwrap_or_default())
}

/// Both sides of the relation are declared, and agree on the table.
#[test]
fn test_both_sides_share_the_relation() {
    let app = new_app();
    use erp_types::field::{FieldReference, FieldReferenceType};

    for (model, field) in [("invoice", "tags"), ("tag", "invoices")] {
        let inverse = &app
            .model_manager
            .get_model(model)
            .get_internal_field(field)
            .inverse;
        let Some(FieldReference {
            inverse_field: FieldReferenceType::M2M { relation, .. },
            ..
        }) = inverse
        else {
            panic!("{model}.{field} should be a many2many");
        };
        assert_eq!(relation, "invoice_tag_rel");
    }
}

/// A many2many has no column of its own.
#[test]
fn test_many2many_is_not_stored_as_a_column() {
    let app = new_app();
    assert!(!app.model_manager.get_model("invoice").is_stored("tags"));
    assert!(!app.model_manager.get_model("tag").is_stored("invoices"));
}

/// Linking from one side is visible from the other.
#[test]
fn test_link_is_visible_from_both_sides() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let invoice = create(&mut env, "invoice", "INV")?;
    let urgent = create(&mut env, "tag", "urgent")?;
    let late = create(&mut env, "tag", "late")?;

    set_tags(&mut env, invoice, vec![urgent, late])?;
    assert_eq!(tags_of(&mut env, invoice)?, vec![urgent, late]);
    assert_eq!(
        invoices_of(&mut env, urgent)?,
        vec![invoice],
        "the other side must see the link without being written to"
    );
    assert_eq!(invoices_of(&mut env, late)?, vec![invoice]);
    Ok(())
}

/// Unlinking one target leaves the others alone, on both sides.
#[test]
fn test_unlinking_one_target() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let invoice = create(&mut env, "invoice", "INV")?;
    let urgent = create(&mut env, "tag", "urgent")?;
    let late = create(&mut env, "tag", "late")?;
    set_tags(&mut env, invoice, vec![urgent, late])?;

    set_tags(&mut env, invoice, vec![urgent])?;
    assert_eq!(tags_of(&mut env, invoice)?, vec![urgent]);
    assert_eq!(invoices_of(&mut env, urgent)?, vec![invoice]);
    assert!(
        invoices_of(&mut env, late)?.is_empty(),
        "the dropped tag must no longer see the invoice"
    );
    Ok(())
}

/// Clearing the list removes every pair.
#[test]
fn test_clearing_the_list() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let invoice = create(&mut env, "invoice", "INV")?;
    let urgent = create(&mut env, "tag", "urgent")?;
    set_tags(&mut env, invoice, vec![urgent])?;

    set_tags(&mut env, invoice, vec![])?;
    assert!(tags_of(&mut env, invoice)?.is_empty());
    assert!(invoices_of(&mut env, urgent)?.is_empty());
    Ok(())
}

/// Several records may share a target.
#[test]
fn test_a_target_can_be_shared() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let first = create(&mut env, "invoice", "A")?;
    let second = create(&mut env, "invoice", "B")?;
    let shared = create(&mut env, "tag", "shared")?;

    set_tags(&mut env, first, vec![shared])?;
    set_tags(&mut env, second, vec![shared])?;

    let mut invoices = invoices_of(&mut env, shared)?;
    invoices.sort_unstable();
    let mut expected = vec![first, second];
    expected.sort_unstable();
    assert_eq!(invoices, expected);
    Ok(())
}

/// Pairs survive a commit and a fresh environment.
#[test]
fn test_links_survive_a_commit() -> Result<()> {
    let app = new_app();

    let mut env = app.new_env()?;
    let invoice = create(&mut env, "invoice", "INV")?;
    let urgent = create(&mut env, "tag", "urgent")?;
    set_tags(&mut env, invoice, vec![urgent])?;
    env.close()?;

    let mut env = app.new_env()?;
    assert_eq!(tags_of(&mut env, invoice)?, vec![urgent]);
    assert_eq!(invoices_of(&mut env, urgent)?, vec![invoice]);
    Ok(())
}

/// A rolled back transaction leaves no pair behind.
#[test]
fn test_links_are_rolled_back() -> Result<()> {
    let app = new_app();

    let mut env = app.new_env()?;
    let invoice = create(&mut env, "invoice", "INV")?;
    let urgent = create(&mut env, "tag", "urgent")?;
    env.close()?;

    let mut env = app.new_env()?;
    set_tags(&mut env, invoice, vec![urgent])?;
    drop(env);

    let mut env = app.new_env()?;
    assert!(tags_of(&mut env, invoice)?.is_empty());
    Ok(())
}

/// Records with no link at all read as an empty list, not as an error.
#[test]
fn test_unlinked_record_reads_empty() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let invoice = create(&mut env, "invoice", "lonely")?;
    assert!(tags_of(&mut env, invoice)?.is_empty());
    assert_eq!(env.count("invoice", &make_domain!([]))?, 1);
    Ok(())
}

fn summary_of(env: &mut erp::environment::Environment, invoice: u32) -> Result<String> {
    let rows = env.read("invoice", &SingleId::from(invoice), &["tag_summary"])?;
    Ok(rows[0].get::<&String>("tag_summary").clone())
}

/// A compute may depend on a path that crosses the relation table.
#[test]
fn test_compute_depends_across_a_many2many() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let invoice = create(&mut env, "invoice", "INV")?;
    let urgent = create(&mut env, "tag", "urgent")?;
    let late = create(&mut env, "tag", "late")?;

    set_tags(&mut env, invoice, vec![urgent, late])?;
    assert_eq!(summary_of(&mut env, invoice)?, "late,urgent");
    Ok(())
}

/// Renaming a tag recomputes every invoice it is linked to.
#[test]
fn test_changing_a_target_recomputes_the_source() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let invoice = create(&mut env, "invoice", "INV")?;
    let urgent = create(&mut env, "tag", "urgent")?;
    set_tags(&mut env, invoice, vec![urgent])?;
    assert_eq!(summary_of(&mut env, invoice)?, "urgent");

    let mut rename = MapOfFields::new(HashMap::new());
    rename.insert("name", "critical");
    env.write("tag", &SingleId::from(urgent), rename)?;

    assert_eq!(
        summary_of(&mut env, invoice)?,
        "critical",
        "the invoice must be recomputed through the relation"
    );
    Ok(())
}

/// Linking and unlinking recompute too.
#[test]
fn test_linking_recomputes() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let invoice = create(&mut env, "invoice", "INV")?;
    let urgent = create(&mut env, "tag", "urgent")?;
    let late = create(&mut env, "tag", "late")?;

    set_tags(&mut env, invoice, vec![urgent])?;
    assert_eq!(summary_of(&mut env, invoice)?, "urgent");

    set_tags(&mut env, invoice, vec![urgent, late])?;
    assert_eq!(summary_of(&mut env, invoice)?, "late,urgent");

    set_tags(&mut env, invoice, vec![])?;
    assert_eq!(summary_of(&mut env, invoice)?, "");
    Ok(())
}

/// Only the invoices actually linked are recomputed.
#[test]
fn test_recompute_reaches_every_linked_source() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let first = create(&mut env, "invoice", "A")?;
    let second = create(&mut env, "invoice", "B")?;
    let untouched = create(&mut env, "invoice", "C")?;
    let shared = create(&mut env, "tag", "shared")?;

    set_tags(&mut env, first, vec![shared])?;
    set_tags(&mut env, second, vec![shared])?;
    assert_eq!(summary_of(&mut env, untouched)?, "");

    let mut rename = MapOfFields::new(HashMap::new());
    rename.insert("name", "renamed");
    env.write("tag", &SingleId::from(shared), rename)?;

    assert_eq!(summary_of(&mut env, first)?, "renamed");
    assert_eq!(summary_of(&mut env, second)?, "renamed");
    assert_eq!(summary_of(&mut env, untouched)?, "");
    Ok(())
}

/// Deleting a record takes its half of every link with it.
///
/// The relation table has no foreign keys, so nothing but this cleans it: a pair left behind
/// would point at a record that no longer exists, and would come back the day the id is reused.
#[test]
fn test_deleting_a_record_removes_its_links() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let invoice = create(&mut env, "invoice", "invoice")?;
    let kept = create(&mut env, "invoice", "kept")?;
    let first = create(&mut env, "tag", "first")?;
    let second = create(&mut env, "tag", "second")?;
    set_tags(&mut env, invoice, vec![first, second])?;
    set_tags(&mut env, kept, vec![first])?;

    env.delete("invoice", &SingleId::from(invoice))?;

    assert_eq!(
        invoices_of(&mut env, first)?,
        vec![kept],
        "the deleted invoice must be gone from the tag it shared"
    );
    assert!(
        invoices_of(&mut env, second)?.is_empty(),
        "and from the one only it carried"
    );
    Ok(())
}

/// The same from the other side: deleting a tag unlinks it from the invoices listing it.
#[test]
fn test_deleting_the_other_side_removes_its_links() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let invoice = create(&mut env, "invoice", "invoice")?;
    let doomed = create(&mut env, "tag", "doomed")?;
    let kept = create(&mut env, "tag", "kept")?;
    set_tags(&mut env, invoice, vec![doomed, kept])?;

    env.delete("tag", &SingleId::from(doomed))?;

    assert_eq!(tags_of(&mut env, invoice)?, vec![kept]);
    Ok(())
}

/// The links are gone from storage too, not only from the cache.
#[test]
fn test_the_links_do_not_come_back_after_a_commit() -> Result<()> {
    let app = new_app();

    let (invoice, tag) = {
        let mut env = app.new_env()?;
        let invoice = create(&mut env, "invoice", "invoice")?;
        let tag = create(&mut env, "tag", "tag")?;
        set_tags(&mut env, invoice, vec![tag])?;
        env.close()?;
        (invoice, tag)
    };

    let mut env = app.new_env()?;
    env.delete("invoice", &SingleId::from(invoice))?;
    env.close()?;

    let mut env = app.new_env()?;
    assert!(
        invoices_of(&mut env, tag)?.is_empty(),
        "a pair left in storage would resurface here"
    );
    Ok(())
}
