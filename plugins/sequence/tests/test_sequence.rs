//! Numbering: names in order, restarting with the year or month, the rules a series follows,
//! who may manage them, and their views.

use base::BasePlugin;
use base::models::View;
use erp::Result;
use erp::app::Application;
use erp::environment::Environment;
use erp::types::field::{IdMode, NaiveDate, SingleId};
use erp::types::model::MapOfFields;
use erp_test_support::{admin_env, user_env, xml_id};
use sequence::SequencePlugin;
use sequence::models::Sequence;
use serde_json::json;
use web::WebPlugin;

fn new_app() -> Result<Application> {
    erp_test_support::committing_app(
        vec![
            Box::new(BasePlugin {}),
            Box::new(WebPlugin {}),
            Box::new(SequencePlugin {}),
        ],
        "sequence",
    )
}

fn date(text: &str) -> NaiveDate {
    NaiveDate::parse_from_str(text, "%Y-%m-%d").expect("a date")
}

fn new_series(env: &mut Environment, code: &str, prefix: &str, reset: &str) -> Result<u32> {
    let mut values = MapOfFields::default();
    values.insert("name", code);
    values.insert("code", code);
    values.insert("prefix", prefix);
    values.insert("reset", reset);
    Ok(env.create_records("sequence", vec![values])?.get_ids_ref()[0])
}

/// Names follow each other, padded, with the date's year.
#[test]
fn test_names_follow_each_other() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    new_series(&mut env, "invoice", "INV/{year}/", "yearly")?;
    let on = date("2026-03-01");
    assert_eq!(
        Sequence::next_by_code(&mut env, "invoice".to_string(), on)?,
        "INV/2026/00001"
    );
    assert_eq!(
        Sequence::next_by_code(&mut env, "invoice".to_string(), on)?,
        "INV/2026/00002"
    );
    assert_eq!(
        Sequence::next_by_code(&mut env, "invoice".to_string(), on)?,
        "INV/2026/00003"
    );
    Ok(())
}

/// A yearly series starts over at 1 with a new year, a monthly one with a new month, and one
/// that never restarts goes on.
#[test]
fn test_series_restart_with_their_period() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    new_series(&mut env, "yearly", "Y{y}-", "yearly")?;
    new_series(&mut env, "monthly", "M{year}{month}-", "monthly")?;
    new_series(&mut env, "never", "N", "never")?;
    let next = |env: &mut Environment, code: &str, on: &str| {
        Sequence::next_by_code(env, code.to_string(), date(on)).expect("a name")
    };
    assert_eq!(next(&mut env, "yearly", "2026-12-31"), "Y26-00001");
    assert_eq!(next(&mut env, "yearly", "2026-12-31"), "Y26-00002");
    assert_eq!(next(&mut env, "yearly", "2027-01-01"), "Y27-00001");
    assert_eq!(next(&mut env, "monthly", "2026-01-15"), "M202601-00001");
    assert_eq!(next(&mut env, "monthly", "2026-01-31"), "M202601-00002");
    assert_eq!(next(&mut env, "monthly", "2026-02-01"), "M202602-00001");
    assert_eq!(next(&mut env, "never", "2026-12-31"), "N00001");
    assert_eq!(next(&mut env, "never", "2027-01-01"), "N00002");
    Ok(())
}

/// Numbers handed out are kept once the work is committed, and the series goes on from there.
#[test]
fn test_numbers_survive_the_transaction() -> Result<()> {
    let app = new_app()?;
    {
        let mut env = admin_env(&app)?;
        new_series(&mut env, "order", "SO", "never")?;
        assert_eq!(
            Sequence::next_by_code(&mut env, "order".to_string(), date("2026-01-01"))?,
            "SO00001"
        );
        env.close()?;
    }
    let mut env = admin_env(&app)?;
    assert_eq!(
        Sequence::next_by_code(&mut env, "order".to_string(), date("2026-01-01"))?,
        "SO00002"
    );
    Ok(())
}

/// A step greater than one skips numbers; the next number may be set by hand.
#[test]
fn test_steps_and_manual_restart() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let id = new_series(&mut env, "step", "S", "never")?;
    let series: Sequence<SingleId> = env.get_record(id.into());
    series.set_number_increment(10, &mut env)?;
    assert_eq!(series.next(&mut env, date("2026-01-01"))?, "S00001");
    assert_eq!(series.next(&mut env, date("2026-01-01"))?, "S00011");
    series.set_number_next(500, &mut env)?;
    assert_eq!(series.next(&mut env, date("2026-01-01"))?, "S00500");
    Ok(())
}

/// A code nothing numbers is an error, and so is an archived series.
#[test]
fn test_an_unknown_code_is_an_error() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    let error = Sequence::next_by_code(&mut env, "nothing".to_string(), date("2026-01-01"))
        .expect_err("no such series")
        .to_string();
    assert!(error.contains("nothing"), "{error}");
    let id = new_series(&mut env, "archived", "A", "never")?;
    let series: Sequence<SingleId> = env.get_record(id.into());
    series.set_active(false, &mut env)?;
    assert!(Sequence::next_by_code(&mut env, "archived".to_string(), date("2026-01-01")).is_err());
    Ok(())
}

/// One active series per code; numbers start at 1 or more; padding stays within 0 and 20.
#[test]
fn test_series_follow_their_rules() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    new_series(&mut env, "unique", "U", "never")?;
    let error = new_series(&mut env, "unique", "V", "never")
        .expect_err("taken")
        .to_string();
    assert!(error.contains("unique"), "{error}");

    let id = new_series(&mut env, "rules", "R", "never")?;
    let series: Sequence<SingleId> = env.get_record(id.into());
    assert!(series.set_number_next(0, &mut env).is_err());
    assert!(series.set_number_increment(0, &mut env).is_err());
    assert!(series.set_padding(21, &mut env).is_err());
    assert_eq!(
        *series.get_number_next(&mut env)?,
        1,
        "refused changes are undone"
    );
    Ok(())
}

/// Anyone creating a document gets its number; only administrators manage the series.
#[test]
fn test_access_rights() -> Result<()> {
    let app = new_app()?;
    {
        let mut env = admin_env(&app)?;
        new_series(&mut env, "shared", "SH", "never")?;
        env.close()?;
    }
    let mut env = user_env(&app, "employee", &["base.group_user"])?;
    assert_eq!(
        Sequence::next_by_code(&mut env, "shared".to_string(), date("2026-01-01"))?,
        "SH00001"
    );
    assert!(new_series(&mut env, "mine", "M", "never").is_err());
    let found = env.call_rpc("sequence", "search", &json!({"domain": []}));
    assert!(
        found.map(|found| found == json!([])).unwrap_or(true),
        "employees do not browse the series"
    );
    Ok(())
}

/// The views resolve, and the series are reached from the technical settings.
#[test]
fn test_views_and_menu() -> Result<()> {
    let app = new_app()?;
    let mut env = admin_env(&app)?;
    for kind in ["list", "form", "search"] {
        let arch = env.get_empty_record::<View<_>>().load(
            &mut env,
            "sequence".to_string(),
            kind.to_string(),
        )?;
        assert!(arch.starts_with(&format!("<{kind}")), "{kind}: {arch}");
    }
    xml_id(&mut env, "sequence.action_sequences");
    let tree = env.call_rpc("menu", "tree", &json!({}))?;
    let text = tree.to_string();
    assert!(text.contains("\"Numbering\""), "{text}");
    Ok(())
}

/// Documents numbered at the same time each get their own number: the series waits for the
/// transaction holding it. Only PostgreSQL runs transactions side by side; three at once, as many
/// as the tests' pool of connections lends.
#[test]
fn test_numbers_taken_at_once_differ() -> Result<()> {
    if !erp_test_support::on_postgres() {
        eprintln!("skipping: needs PostgreSQL");
        return Ok(());
    }
    let app = new_app()?;
    {
        let mut env = admin_env(&app)?;
        new_series(&mut env, "busy", "B", "never")?;
        env.close()?;
    }
    let start = std::sync::Barrier::new(3);
    let mut names: Vec<String> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..3)
            .map(|_| {
                scope.spawn(|| -> Result<String> {
                    let mut env = admin_env(&app)?;
                    start.wait();
                    let name =
                        Sequence::next_by_code(&mut env, "busy".to_string(), date("2026-01-01"))?;
                    // Held a moment, as a document is written, before the work is committed.
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    env.close()?;
                    Ok(name)
                })
            })
            .collect();
        workers
            .into_iter()
            .map(|worker| worker.join().expect("a worker"))
            .collect::<Result<Vec<_>>>()
    })?;
    names.sort();
    assert_eq!(names, ["B00001", "B00002", "B00003"]);
    Ok(())
}
