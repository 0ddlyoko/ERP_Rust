use erp::app::Application;
use erp_search::{OrderBy, SearchOptions};
use erp_search_code_gen::make_domain;
use erp_types::field::{Decimal, IdMode, MultipleIds};
use erp_types::model::MapOfFields;
use std::collections::HashMap;
use std::error::Error;
use std::str::FromStr;
use test_utilities::models::Invoice;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

fn new_app() -> Application {
    let mut app = Application::new_test();
    app.model_manager.register_model::<Invoice<_>>();
    app.model_manager.post_register();
    app
}

fn seed(env: &mut erp::environment::Environment, names: &[&str]) -> Result<()> {
    let maps: Vec<MapOfFields> = names
        .iter()
        .map(|name| {
            let mut map = MapOfFields::new(HashMap::new());
            map.insert("name", *name);
            map
        })
        .collect();
    env.create_records("invoice", maps)?;
    Ok(())
}

fn all(env: &mut erp::environment::Environment, options: &SearchOptions) -> Result<Vec<u32>> {
    env.search_ids_with("invoice", &make_domain!([]), options)
}

/// Without a deterministic order, `limit` and `offset` would slice an arbitrary permutation.
#[test]
fn test_search_order_is_reproducible() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    seed(&mut env, &["a", "b", "c", "d", "e", "f", "g", "h"])?;

    let first = all(&mut env, &SearchOptions::default())?;
    for _ in 0..5 {
        assert_eq!(all(&mut env, &SearchOptions::default())?, first);
    }
    Ok(())
}

#[test]
fn test_limit_and_offset() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    seed(&mut env, &["a", "b", "c", "d", "e"])?;

    let everything = all(&mut env, &SearchOptions::default())?;
    assert_eq!(everything.len(), 5);

    let two = all(&mut env, &SearchOptions::new().with_limit(2))?;
    assert_eq!(two, everything[..2].to_vec());

    let skipped = all(&mut env, &SearchOptions::new().with_offset(3))?;
    assert_eq!(skipped, everything[3..].to_vec());

    let window = all(&mut env, &SearchOptions::new().with_offset(1).with_limit(2))?;
    assert_eq!(window, everything[1..3].to_vec());

    let past_the_end = all(&mut env, &SearchOptions::new().with_offset(99))?;
    assert!(past_the_end.is_empty());
    Ok(())
}

#[test]
fn test_order_ascending_and_descending() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    seed(&mut env, &["charlie", "alpha", "bravo"])?;

    let ascending = all(
        &mut env,
        &SearchOptions::new().order_by(OrderBy::asc("name")),
    )?;
    let names: Vec<String> = env
        .read("invoice", &MultipleIds::from(ascending.clone()), &["name"])?
        .iter()
        .map(|row| row.get::<&String>("name").clone())
        .collect();
    assert_eq!(names, vec!["alpha", "bravo", "charlie"]);

    let descending = all(
        &mut env,
        &SearchOptions::new().order_by(OrderBy::desc("name")),
    )?;
    let mut reversed = ascending;
    reversed.reverse();
    assert_eq!(descending, reversed);
    Ok(())
}

/// NULLs sort last in ascending order, the way PostgreSQL does it.
#[test]
fn test_nulls_sort_last() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let mut with_date: MapOfFields = MapOfFields::new(HashMap::new());
    with_date.insert("name", "dated");
    let mut without: MapOfFields = MapOfFields::new(HashMap::new());
    without.insert("name", "undated");
    without.insert_none("signed_on");
    env.create_records("invoice", vec![without, with_date])?;

    let ordered = all(
        &mut env,
        &SearchOptions::new().order_by(OrderBy::asc("signed_on")),
    )?;
    let names: Vec<String> = env
        .read("invoice", &MultipleIds::from(ordered), &["name"])?
        .iter()
        .map(|row| row.get::<&String>("name").clone())
        .collect();
    assert_eq!(names, vec!["dated", "undated"], "the NULL must come last");
    Ok(())
}

/// Ties on the sort key keep a stable order rather than shuffling.
#[test]
fn test_ties_fall_back_to_the_id() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    seed(&mut env, &["same", "same", "same"])?;

    let ordered = all(
        &mut env,
        &SearchOptions::new().order_by(OrderBy::asc("name")),
    )?;
    let mut sorted = ordered.clone();
    sorted.sort_unstable();
    assert_eq!(ordered, sorted);
    Ok(())
}

#[test]
fn test_count_ignores_limit() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    seed(&mut env, &["a", "b", "c", "d"])?;

    assert_eq!(env.count("invoice", &make_domain!([]))?, 4);
    assert_eq!(
        env.count("invoice", &make_domain!([("name", "=", "a")]))?,
        1
    );
    assert_eq!(
        all(&mut env, &SearchOptions::new().with_limit(2))?.len(),
        2,
        "the limit still applies to the search itself"
    );
    Ok(())
}

#[test]
fn test_like_uses_sql_wildcards() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    seed(&mut env, &["Facture", "facture", "Fac", "Brouillon"])?;

    let prefix = env.count("invoice", &make_domain!([("name", "like", "Fac%")]))?;
    assert_eq!(prefix, 2, "Facture and Fac");

    let one_char = env.count("invoice", &make_domain!([("name", "like", "Fac_ure")]))?;
    assert_eq!(one_char, 1, "_ stands for the t of Facture");

    let too_long = env.count("invoice", &make_domain!([("name", "like", "Fac_ture")]))?;
    assert_eq!(too_long, 0, "_ matches exactly one character, never zero");

    let anywhere = env.count("invoice", &make_domain!([("name", "like", "%act%")]))?;
    assert_eq!(anywhere, 2, "Facture and facture both contain act");

    let case_sensitive = env.count("invoice", &make_domain!([("name", "like", "%ACT%")]))?;
    assert_eq!(case_sensitive, 0, "like is case-sensitive");
    Ok(())
}

#[test]
fn test_ilike_ignores_case() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    seed(&mut env, &["Facture", "facture", "Brouillon"])?;

    assert_eq!(
        env.count("invoice", &make_domain!([("name", "ilike", "FACT%")]))?,
        2
    );
    assert_eq!(
        env.count("invoice", &make_domain!([("name", "ilike", "%OUILL%")]))?,
        1
    );
    Ok(())
}

#[test]
fn test_in_and_not_in() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    seed(&mut env, &["a", "b", "c"])?;

    let wanted = vec!["a", "c"];
    assert_eq!(
        env.count("invoice", &make_domain!([("name", "in", wanted.clone())]))?,
        2
    );
    assert_eq!(
        env.count("invoice", &make_domain!([("name", "not in", wanted)]))?,
        1
    );
    Ok(())
}

/// `=` against an array already meant membership and several internal call sites rely on it.
#[test]
fn test_equal_against_an_array_still_means_in() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    seed(&mut env, &["a", "b", "c"])?;

    let wanted = vec!["a", "b"];
    assert_eq!(
        env.count("invoice", &make_domain!([("name", "=", wanted)]))?,
        2
    );
    Ok(())
}

/// `in` against something that is not an array matches nothing, consistent with how a
/// type-mismatched comparison already behaves.
#[test]
fn test_in_against_a_scalar_matches_nothing() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    seed(&mut env, &["a"])?;

    assert_eq!(
        env.count("invoice", &make_domain!([("name", "in", "a")]))?,
        0
    );
    Ok(())
}

/// Ordering must see values that are still only in the cache.
#[test]
fn test_order_sees_uncommitted_writes() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("name", "zzz");
    map.insert("amount_untaxed", Decimal::from_str("1")?);
    let ids = env.create_records("invoice", vec![map])?;

    let mut other: MapOfFields = MapOfFields::new(HashMap::new());
    other.insert("name", "aaa");
    other.insert("amount_untaxed", Decimal::from_str("2")?);
    env.create_records("invoice", vec![other])?;

    let ordered = all(
        &mut env,
        &SearchOptions::new().order_by(OrderBy::asc("name")),
    )?;
    assert_eq!(
        ordered.last(),
        ids.get_ids_ref().first(),
        "the record named zzz must sort last"
    );
    Ok(())
}

/// `search_read` pages at the database, not after the fact: only the records that come back are
/// ever materialised.
#[test]
fn test_search_read_applies_the_options() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    seed(&mut env, &["delta", "alpha", "charlie", "bravo"])?;

    let page = env.search_read(
        "invoice",
        &["name"],
        &make_domain!([]),
        &SearchOptions::new()
            .order_by(OrderBy::asc("name"))
            .with_limit(2),
    )?;
    let names: Vec<String> = page
        .iter()
        .map(|row| row.get::<&String>("name").clone())
        .collect();
    assert_eq!(names, vec!["alpha", "bravo"]);

    let second_page = env.search_read(
        "invoice",
        &["name"],
        &make_domain!([]),
        &SearchOptions::new()
            .order_by(OrderBy::asc("name"))
            .with_offset(2)
            .with_limit(2),
    )?;
    let names: Vec<String> = second_page
        .iter()
        .map(|row| row.get::<&String>("name").clone())
        .collect();
    assert_eq!(names, vec!["charlie", "delta"]);
    Ok(())
}

/// `search_read` goes through the cache, so a computed field is computed rather than read stale.
#[test]
fn test_search_read_computes_fields() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;

    let mut map: MapOfFields = MapOfFields::new(HashMap::new());
    map.insert("name", "taxed");
    map.insert("amount_untaxed", Decimal::from_str("100")?);
    env.create_records("invoice", vec![map])?;

    let rows = env.search_read(
        "invoice",
        &["name", "amount_untaxed"],
        &make_domain!([("name", "=", "taxed")]),
        &SearchOptions::default(),
    )?;
    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].get::<&Decimal>("amount_untaxed"),
        &Decimal::from_str("100")?
    );
    Ok(())
}
