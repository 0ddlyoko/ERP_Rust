//! Access rights: which records of a model a user may read, create, write or delete.
//!
//! Group rules grant and are ORed, global rules restrict and are ANDed on top, and an operation no
//! group rule grants is refused. Root, sudo and the process itself are not checked. Rights are
//! enforced by the ORM, so a method reached over the wire is held to them as much as a verb is.

use base::BasePlugin;
use base::models::{Group, Users};
use erp::access::{Access, AccessDenied, Operation, Rule};
use erp::app::Application;
use erp::environment::Environment;
use erp::plugin::Plugin;
use erp_search::SearchType;
use erp_search_code_gen::make_domain;
use erp_types::field::{FieldType, IdMode, MultipleIds, SingleId};
use erp_types::model::MapOfFields;
use serde_json::json;
use std::error::Error;
use std::sync::atomic::{AtomicUsize, Ordering};
use test_utilities::TestLibPlugin;
use test_utilities::models::{SaleOrder, SaleOrderLine, Tag};

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

fn new_app() -> Result<Application> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    app.load_plugin("base")?;
    app.load_plugin("test_lib_plugin")?;
    Ok(app)
}

/// The domains of a rule, each left out unless given.
#[derive(Default, Clone, Copy)]
struct Domains<'a> {
    read: Option<&'a str>,
    create: Option<&'a str>,
    write: Option<&'a str>,
    delete: Option<&'a str>,
}

const EVERYTHING: Domains = Domains {
    read: Some("[]"),
    create: Some("[]"),
    write: Some("[]"),
    delete: Some("[]"),
};

const READ_ONLY: Domains = Domains {
    read: Some("[]"),
    create: None,
    write: None,
    delete: None,
};

fn make_user(env: &mut Environment, login: &str) -> Result<u32> {
    let mut values = MapOfFields::default();
    values.insert("login", login);
    values.insert("name", login);
    Ok(env.create_records("users", vec![values])?.get_ids_ref()[0])
}

fn make_group(env: &mut Environment, name: &str, users: &[u32]) -> Result<u32> {
    let mut values = MapOfFields::default();
    values.insert("name", name);
    values.insert("users", FieldType::Refs(users.to_vec()));
    Ok(env.create_records("group", vec![values])?.get_ids_ref()[0])
}

fn make_rule(
    env: &mut Environment,
    model: &str,
    group: Option<u32>,
    domains: Domains,
) -> Result<u32> {
    let mut values = MapOfFields::default();
    values.insert("name", format!("{model} for {group:?}"));
    values.insert("model", model);
    if let Some(group) = group {
        values.insert("group", FieldType::Ref(group));
    }
    for (field, domain) in [
        ("domain_read", domains.read),
        ("domain_create", domains.create),
        ("domain_write", domains.write),
        ("domain_delete", domains.delete),
    ] {
        if let Some(domain) = domain {
            values.insert(field, domain);
        }
    }
    Ok(env
        .create_records("access_rule", vec![values])?
        .get_ids_ref()[0])
}

fn make_tag(env: &mut Environment, name: &str) -> Result<u32> {
    let mut values = MapOfFields::default();
    values.insert("name", name);
    Ok(env.create_records("tag", vec![values])?.get_ids_ref()[0])
}

/// A user in a group of their own, with these rules on `tag`; plus two tags to look at.
struct Fixture {
    uid: u32,
    group: u32,
    public: u32,
    secret: u32,
}

fn fixture(app: &Application, domains: Domains) -> Result<Fixture> {
    let mut env = app.new_env_as_option(None)?;
    let uid = make_user(&mut env, "alice")?;
    let group = make_group(&mut env, "Tag keepers", &[uid])?;
    make_rule(&mut env, "tag", Some(group), domains)?;
    let public = make_tag(&mut env, "public")?;
    let secret = make_tag(&mut env, "secret")?;
    env.close()?;
    Ok(Fixture {
        uid,
        group,
        public,
        secret,
    })
}

fn tag_values(name: &str) -> MapOfFields {
    let mut values = MapOfFields::default();
    values.insert("name", name);
    values
}

/// The refusal behind an error, which must be one.
fn refusal(error: Box<dyn Error + Send + Sync>) -> AccessDenied {
    match error.downcast::<AccessDenied>() {
        Ok(denied) => *denied,
        Err(other) => panic!("expected an access refusal, got: {other}"),
    }
}

fn names_of(env: &mut Environment, ids: &[u32]) -> Result<Vec<String>> {
    let env = &mut *env.sudo();
    let rows = env.read("tag", &MultipleIds::from(ids.to_vec()), &["name"])?;
    Ok(rows
        .iter()
        .map(|row| row.get::<&String>("name").clone())
        .collect())
}

// ---- how rules combine ----

fn domain(name: &str) -> SearchType {
    make_domain!([("name", "=", name)])
}

#[test]
fn test_group_rules_grant_and_are_ored() {
    let rules = vec![
        Rule {
            group: Some(1),
            read: Some(domain("a")),
            ..Rule::default()
        },
        Rule {
            group: Some(2),
            read: Some(domain("b")),
            ..Rule::default()
        },
        Rule {
            group: Some(3),
            read: Some(domain("c")),
            ..Rule::default()
        },
    ];

    assert_eq!(
        Access::evaluate(&rules, &[1, 2], Operation::Read),
        Access::Restricted(SearchType::Or(Box::new(domain("a")), Box::new(domain("b")))),
        "the groups the user is in, and only those"
    );
    assert_eq!(
        Access::evaluate(&rules, &[4], Operation::Read),
        Access::Denied
    );
}

#[test]
fn test_a_global_rule_restricts_but_never_grants() {
    let global = Rule {
        group: None,
        read: Some(domain("a")),
        ..Rule::default()
    };
    let granted = Rule {
        group: Some(1),
        read: Some(SearchType::Nothing),
        ..Rule::default()
    };

    assert_eq!(
        Access::evaluate(std::slice::from_ref(&global), &[1], Operation::Read),
        Access::Denied,
        "a global rule alone grants nothing"
    );
    assert_eq!(
        Access::evaluate(&[global, granted], &[1], Operation::Read),
        Access::Restricted(domain("a")),
        "it narrows what a group was granted"
    );
}

/// An absent domain says nothing about the operation; an empty one covers every record.
#[test]
fn test_an_absent_domain_is_not_an_empty_one() {
    let rules = vec![Rule {
        group: Some(1),
        read: Some(SearchType::Nothing),
        ..Rule::default()
    }];

    assert_eq!(
        Access::evaluate(&rules, &[1], Operation::Read),
        Access::Restricted(SearchType::Nothing)
    );
    for operation in [Operation::Create, Operation::Write, Operation::Delete] {
        assert_eq!(
            Access::evaluate(&rules, &[1], operation),
            Access::Denied,
            "{operation} was granted by a rule that says nothing about it"
        );
    }
}

/// Whoever hits a refusal is told which right is missing.
#[test]
fn test_a_refusal_says_what_was_refused() {
    let denied = AccessDenied::new(
        "tag",
        Operation::Write,
        &["name", "field", "name"],
        vec![4, 7],
    );
    assert_eq!(
        denied.to_string(),
        "You are not allowed to write fields field, name of tag (ids 4, 7)"
    );
    let denied = AccessDenied::new("tag", Operation::Create, &[], Vec::new());
    assert_eq!(denied.to_string(), "You are not allowed to create tag");
}

// ---- who is not checked ----

#[test]
fn test_the_process_root_and_sudo_are_not_checked() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;
    let uid = make_user(&mut env, "nobody_special")?;

    make_tag(&mut env, "by the process")?;
    make_tag(&mut *env.as_root()?, "by root")?;

    let mut user = env.as_user(uid);
    assert!(make_tag(&mut user, "by the user").is_err());
    make_tag(&mut user.sudo(), "by the user, as sudo")?;
    assert_eq!(
        user.uid(),
        Some(uid),
        "sudo does not change who the work is for"
    );
    Ok(())
}

/// Nothing to enforce without a plugin saying where rules come from.
#[test]
fn test_without_rules_nothing_is_checked() -> Result<()> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    app.load_plugin("test_lib_plugin")?;

    let mut env = app.new_env_as(42)?;
    let id = make_tag(&mut env, "anything")?;
    assert_eq!(env.search_ids("tag", &SearchType::Nothing)?, vec![id]);
    Ok(())
}

/// Acting as somebody means acting with their rights, even from inside sudo.
#[test]
fn test_switching_user_drops_sudo() -> Result<()> {
    let app = new_app()?;
    let fixture = fixture(&app, READ_ONLY)?;
    let mut env = app.new_env_as_option(None)?;
    let mut sudo = env.sudo();
    assert!(sudo.is_sudo());

    let mut user = sudo.as_user(fixture.uid);
    assert!(!user.is_sudo());
    assert!(user.create_records("tag", vec![tag_values("x")]).is_err());
    drop(user);
    assert!(sudo.is_sudo(), "and it comes back with the previous user");
    Ok(())
}

// ---- refusing by default ----

#[test]
fn test_a_model_nobody_wrote_a_rule_for_is_closed() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;
    let uid = make_user(&mut env, "alice")?;
    let tag = make_tag(&mut env, "public")?;
    env.close()?;

    let mut env = app.new_env_as(uid)?;
    let denied = refusal(env.search_ids("tag", &SearchType::Nothing).unwrap_err());
    assert_eq!(denied.operation, Operation::Read);
    assert!(env.count("tag", &SearchType::Nothing).is_err());

    let denied = refusal(
        env.read("tag", &SingleId::from(tag), &["name"])
            .unwrap_err(),
    );
    assert_eq!(
        denied,
        AccessDenied::new("tag", Operation::Read, &["name"], vec![tag])
    );

    let denied = refusal(
        env.create_records("tag", vec![tag_values("mine")])
            .unwrap_err(),
    );
    assert_eq!(
        denied,
        AccessDenied::new("tag", Operation::Create, &["name"], Vec::new())
    );
    assert!(
        env.write("tag", &SingleId::from(tag), tag_values("x"))
            .is_err()
    );
    assert!(env.delete("tag", &SingleId::from(tag)).is_err());
    Ok(())
}

/// Nobody authenticated is the portal user, and base grants the portal user nothing.
#[test]
fn test_the_anonymous_caller_reads_nothing() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env()?;
    assert!(env.search_ids("contact", &SearchType::Nothing).is_err());
    assert!(env.search_ids("users", &SearchType::Nothing).is_err());
    Ok(())
}

// ---- reading ----

#[test]
fn test_searching_only_finds_what_may_be_read() -> Result<()> {
    let app = new_app()?;
    let fixture = fixture(
        &app,
        Domains {
            read: Some(r#"[["name", "=", "public"]]"#),
            ..Domains::default()
        },
    )?;
    let mut env = app.new_env_as(fixture.uid)?;

    assert_eq!(
        env.search_ids("tag", &SearchType::Nothing)?,
        vec![fixture.public]
    );
    assert_eq!(env.count("tag", &SearchType::Nothing)?, 1);
    assert_eq!(
        env.search_ids("tag", &make_domain!([("name", "=", "secret")]))?,
        Vec::<u32>::new(),
        "asking for it by name does not get round the rule"
    );
    let found: Tag<MultipleIds> = env.search(&SearchType::Nothing)?;
    assert_eq!(found.get_ids(), vec![fixture.public]);
    Ok(())
}

#[test]
fn test_reading_a_record_outside_the_rule_is_refused() -> Result<()> {
    let app = new_app()?;
    let fixture = fixture(
        &app,
        Domains {
            read: Some(r#"[["name", "=", "public"]]"#),
            ..Domains::default()
        },
    )?;
    let mut env = app.new_env_as(fixture.uid)?;

    let rows = env.read("tag", &SingleId::from(fixture.public), &["name"])?;
    assert_eq!(rows[0].get::<&String>("name"), "public");

    let denied = refusal(
        env.read(
            "tag",
            &MultipleIds::from(vec![fixture.public, fixture.secret]),
            &["name"],
        )
        .unwrap_err(),
    );
    assert_eq!(
        denied.ids,
        vec![fixture.secret],
        "only the refused one is named"
    );

    let secret: Tag<SingleId> = env.get_record(fixture.secret.into());
    assert!(
        secret.get_name(&mut env).is_err(),
        "a generated getter is held to the same rule"
    );
    Ok(())
}

/// A record that does not exist is not reported as one the caller may not read.
#[test]
fn test_a_missing_record_is_not_a_refused_one() -> Result<()> {
    let app = new_app()?;
    let fixture = fixture(
        &app,
        Domains {
            read: Some(r#"[["name", "=", "public"]]"#),
            ..Domains::default()
        },
    )?;
    let mut env = app.new_env_as(fixture.uid)?;
    env.check_access("tag", Operation::Read, &[fixture.public, 9999], &["name"])?;
    Ok(())
}

// ---- writing ----

#[test]
fn test_writing_is_all_or_nothing() -> Result<()> {
    let app = new_app()?;
    let fixture = fixture(
        &app,
        Domains {
            read: Some("[]"),
            write: Some(r#"[["name", "=", "public"]]"#),
            ..Domains::default()
        },
    )?;
    let mut env = app.new_env_as(fixture.uid)?;
    let both = MultipleIds::from(vec![fixture.public, fixture.secret]);

    let denied = refusal(env.write("tag", &both, tag_values("renamed")).unwrap_err());
    assert_eq!(
        denied,
        AccessDenied::new("tag", Operation::Write, &["name"], vec![fixture.secret])
    );
    assert_eq!(
        names_of(&mut env, both.get_ids_ref())?,
        vec!["public", "secret"],
        "the allowed record was not written either"
    );

    env.write(
        "tag",
        &SingleId::from(fixture.public),
        tag_values("renamed"),
    )?;
    assert_eq!(names_of(&mut env, &[fixture.public])?, vec!["renamed"]);
    Ok(())
}

#[test]
fn test_a_generated_setter_is_held_to_the_rule() -> Result<()> {
    let app = new_app()?;
    let fixture = fixture(&app, READ_ONLY)?;
    let mut env = app.new_env_as(fixture.uid)?;

    let tag: Tag<SingleId> = env.get_record(fixture.public.into());
    let denied = refusal(tag.set_name("renamed".to_string(), &mut env).unwrap_err());
    assert_eq!(
        denied,
        AccessDenied::new("tag", Operation::Write, &["name"], vec![fixture.public])
    );
    Ok(())
}

// ---- creating ----

/// Whether a record is within the rule depends on its values, so it is checked once it exists.
#[test]
fn test_creating_is_checked_against_the_new_record() -> Result<()> {
    let app = new_app()?;
    let fixture = fixture(
        &app,
        Domains {
            read: Some("[]"),
            create: Some(r#"[["name", "=", "draft"]]"#),
            ..Domains::default()
        },
    )?;
    let mut env = app.new_env_as(fixture.uid)?;

    env.create_records("tag", vec![tag_values("draft")])?;
    let denied = refusal(
        env.create_records("tag", vec![tag_values("published")])
            .unwrap_err(),
    );
    assert_eq!(denied.operation, Operation::Create);
    assert_eq!(denied.ids.len(), 1);

    assert_eq!(
        env.sudo()
            .search_ids("tag", &make_domain!([("name", "=", "published")]))?,
        Vec::<u32>::new(),
        "a refused record does not outlive the refusal"
    );
    Ok(())
}

// ---- deleting ----

#[test]
fn test_deleting_is_all_or_nothing() -> Result<()> {
    let app = new_app()?;
    let fixture = fixture(
        &app,
        Domains {
            read: Some("[]"),
            delete: Some(r#"[["name", "=", "public"]]"#),
            ..Domains::default()
        },
    )?;
    let mut env = app.new_env_as(fixture.uid)?;

    let both = MultipleIds::from(vec![fixture.public, fixture.secret]);
    let denied = refusal(env.delete("tag", &both).unwrap_err());
    assert_eq!(
        denied,
        AccessDenied::new("tag", Operation::Delete, &[], vec![fixture.secret])
    );
    assert_eq!(env.search_ids("tag", &SearchType::Nothing)?.len(), 2);

    assert_eq!(env.delete("tag", &SingleId::from(fixture.public))?, 1);
    assert_eq!(
        env.search_ids("tag", &SearchType::Nothing)?,
        vec![fixture.secret]
    );
    Ok(())
}

// ---- several rules ----

#[test]
fn test_a_global_rule_narrows_every_group() -> Result<()> {
    let app = new_app()?;
    let fixture = fixture(&app, READ_ONLY)?;
    let mut env = app.new_env_as_option(None)?;
    make_rule(
        &mut env,
        "tag",
        None,
        Domains {
            read: Some(r#"[["name", "!=", "secret"]]"#),
            ..Domains::default()
        },
    )?;
    let outsider = make_user(&mut env, "outsider")?;
    env.close()?;

    let mut env = app.new_env_as(fixture.uid)?;
    assert_eq!(
        env.search_ids("tag", &SearchType::Nothing)?,
        vec![fixture.public]
    );
    drop(env);

    let mut env = app.new_env_as(outsider)?;
    assert!(
        env.search_ids("tag", &SearchType::Nothing).is_err(),
        "the global rule grants nothing to a user no group rule covers"
    );
    Ok(())
}

#[test]
fn test_two_groups_add_up() -> Result<()> {
    let app = new_app()?;
    let fixture = fixture(
        &app,
        Domains {
            read: Some(r#"[["name", "=", "public"]]"#),
            ..Domains::default()
        },
    )?;
    let mut env = app.new_env_as_option(None)?;
    let other = make_group(&mut env, "Secret keepers", &[fixture.uid])?;
    make_rule(
        &mut env,
        "tag",
        Some(other),
        Domains {
            read: Some(r#"[["name", "=", "secret"]]"#),
            ..Domains::default()
        },
    )?;
    make_tag(&mut env, "neither")?;
    env.close()?;

    let mut env = app.new_env_as(fixture.uid)?;
    let mut found = env.search_ids("tag", &SearchType::Nothing)?;
    found.sort_unstable();
    assert_eq!(found, vec![fixture.public, fixture.secret]);
    Ok(())
}

// ---- computed fields ----

/// A stored value is the same for every reader, so saving it does not depend on who triggered it.
///
/// The seller may add lines but only read orders, and adding a line still updates the order's
/// stored total — which the compute writes on the order.
#[test]
fn test_a_stored_compute_saves_its_value_whoever_triggered_it() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;
    let uid = make_user(&mut env, "seller")?;
    let group = make_group(&mut env, "Sellers", &[uid])?;
    make_rule(&mut env, "sale_order_line", Some(group), EVERYTHING)?;
    make_rule(&mut env, "sale_order", Some(group), READ_ONLY)?;
    let mut values = MapOfFields::default();
    values.insert("name", "Order");
    let order = env
        .create_records("sale_order", vec![values])?
        .get_ids_ref()[0];
    env.close()?;

    let mut env = app.new_env_as(uid)?;
    let order: SaleOrder<SingleId> = env.get_record(order.into());
    assert_eq!(*order.get_total_price(&mut env)?, 0);

    let mut values = MapOfFields::default();
    values.insert("order", FieldType::Ref(order.get_id()));
    values.insert("price", 10);
    values.insert("amount", 2);
    env.create_records("sale_order_line", vec![values])?;
    assert_eq!(*order.get_total_price(&mut env)?, 20);

    let denied = refusal(order.set_name("renamed".to_string(), &mut env).unwrap_err());
    assert_eq!(
        denied.operation,
        Operation::Write,
        "the order itself stays read-only"
    );
    let denied = refusal(order.set_total_price(99, &mut env).unwrap_err());
    assert_eq!(
        denied,
        AccessDenied::new(
            "sale_order",
            Operation::Write,
            &["total_price"],
            vec![order.get_id()]
        ),
        "outside its compute, a stored computed field is written like any other"
    );
    env.close()?;
    Ok(())
}

/// Only saving the value escapes the caller's rights: what the compute reads is still read as
/// them.
#[test]
fn test_a_stored_compute_reads_as_the_caller() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;
    let uid = make_user(&mut env, "order_reader")?;
    let group = make_group(&mut env, "Order readers", &[uid])?;
    make_rule(&mut env, "sale_order", Some(group), READ_ONLY)?;
    let mut values = MapOfFields::default();
    values.insert("name", "Order");
    let order = env
        .create_records("sale_order", vec![values])?
        .get_ids_ref()[0];
    let mut values = MapOfFields::default();
    values.insert("order", FieldType::Ref(order));
    env.create_records("sale_order_line", vec![values])?;

    let mut user = env.as_user(uid);
    let order: SaleOrder<SingleId> = user.get_record(order.into());
    let denied = refusal(order.get_total_price(&mut user).unwrap_err());
    assert_eq!(
        denied.model_name, "sale_order_line",
        "the order's total reads its lines, which the caller may not"
    );
    Ok(())
}

/// A value worked out on each read is worked out as whoever reads it.
#[test]
fn test_a_compute_on_read_runs_as_the_caller() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;
    let uid = make_user(&mut env, "line_reader")?;
    let group = make_group(&mut env, "Line readers", &[uid])?;
    make_rule(&mut env, "sale_order_line", Some(group), READ_ONLY)?;
    let mut values = MapOfFields::default();
    values.insert("name", "Order");
    let order = env
        .create_records("sale_order", vec![values])?
        .get_ids_ref()[0];
    let mut values = MapOfFields::default();
    values.insert("order", FieldType::Ref(order));
    let line = env
        .create_records("sale_order_line", vec![values])?
        .get_ids_ref()[0];
    env.close()?;

    let mut env = app.new_env_as(uid)?;
    let line: SaleOrderLine<SingleId> = env.get_record(line.into());
    let denied = refusal(line.get_siblings_total(&mut env).unwrap_err());
    assert_eq!(
        denied.model_name, "sale_order",
        "the compute reads the order as the caller"
    );
    Ok(())
}

// ---- methods ----

/// A method reached over the wire is held to the caller's rights on what it touches.
#[test]
fn test_a_method_is_held_to_the_callers_rights() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;
    let uid = make_user(&mut env, "operator")?;
    let group = make_group(&mut env, "Operators", &[uid])?;
    make_rule(&mut env, "machine", Some(group), READ_ONLY)?;
    let mut values = MapOfFields::default();
    values.insert("base_rate", 100);
    values.insert("days", 3);
    let machine = env.create_records("machine", vec![values])?.get_ids_ref()[0];
    env.close()?;

    let mut env = app.new_env_as(uid)?;
    let params = json!({"ids": [machine]});
    assert_eq!(env.call_rpc("machine", "quote", &params)?, json!(300));

    let denied = refusal(
        env.call_rpc("machine", "refuse_after_writing", &params)
            .unwrap_err(),
    );
    assert_eq!(
        denied,
        AccessDenied::new("machine", Operation::Write, &["name"], vec![machine]),
        "the write inside the method is refused before the method gets to fail on its own"
    );
    Ok(())
}

/// Over JSON-RPC, a refusal is an answer, and nothing the call wrote survives it.
#[test]
fn test_a_refused_call_rolls_back() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;
    let portal = app
        .model_manager
        .identities
        .default_user()
        .expect("base names the anonymous caller");
    let group = make_group(&mut env, "Anonymous taggers", &[portal])?;
    make_rule(
        &mut env,
        "tag",
        Some(group),
        Domains {
            read: Some("[]"),
            create: Some(r#"[["name", "!=", "forbidden"]]"#),
            ..Domains::default()
        },
    )?;
    env.close()?;

    let body = json!({
        "jsonrpc": "2.0",
        "method": "tag.create",
        "params": {"values": [{"name": "allowed"}, {"name": "forbidden"}]},
        "id": 1,
    })
    .to_string();
    let answer = erp::jsonrpc::handle(&app, None, &body).expect("an answer");
    let message = answer["error"]["message"].as_str().unwrap_or_default();
    assert!(
        message.starts_with("You are not allowed to create fields name of tag"),
        "got {answer}"
    );

    let mut env = app.new_env_as_option(None)?;
    assert_eq!(
        env.search_ids("tag", &SearchType::Nothing)?,
        Vec::<u32>::new()
    );
    Ok(())
}

// ---- remembering rules ----

#[test]
fn test_a_new_rule_applies_within_the_same_transaction() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;
    let uid = make_user(&mut env, "alice")?;
    let group = make_group(&mut env, "Late", &[uid])?;
    make_tag(&mut env, "public")?;

    let mut user = env.as_user(uid);
    assert!(user.search_ids("tag", &SearchType::Nothing).is_err());
    make_rule(&mut user.sudo(), "tag", Some(group), READ_ONLY)?;
    assert_eq!(user.search_ids("tag", &SearchType::Nothing)?.len(), 1);
    Ok(())
}

#[test]
fn test_leaving_a_group_applies_at_once() -> Result<()> {
    let app = new_app()?;
    let fixture = fixture(&app, READ_ONLY)?;
    let mut env = app.new_env_as(fixture.uid)?;
    assert_eq!(env.search_ids("tag", &SearchType::Nothing)?.len(), 2);

    let group: Group<SingleId> = env.get_record(fixture.group.into());
    group.set_users(Vec::<u32>::new().into(), &mut env.sudo())?;
    assert!(env.search_ids("tag", &SearchType::Nothing).is_err());

    let user: Users<SingleId> = env.get_record(fixture.uid.into());
    user.set_groups(vec![fixture.group].into(), &mut env.sudo())?;
    assert_eq!(
        env.search_ids("tag", &SearchType::Nothing)?.len(),
        2,
        "joining again, from the user's side of the relation"
    );
    Ok(())
}

#[test]
fn test_a_rule_that_is_rolled_back_is_forgotten() -> Result<()> {
    let app = new_app()?;
    let fixture = fixture(&app, READ_ONLY)?;
    let mut env = app.new_env_as(fixture.uid)?;

    let outcome: Result<()> = env.savepoint(|env| {
        make_rule(&mut env.sudo(), "tag", Some(fixture.group), EVERYTHING)?;
        env.create_records("tag", vec![tag_values("inside")])?;
        Err("undo it".into())
    });
    assert!(outcome.is_err());
    assert!(
        env.create_records("tag", vec![tag_values("outside")])
            .is_err(),
        "the rule granting it went with the savepoint"
    );
    Ok(())
}

static TAG_RULE_LOADS: AtomicUsize = AtomicUsize::new(0);
static INVOICE_RULE_LOADS: AtomicUsize = AtomicUsize::new(0);
static GROUP_LOADS: AtomicUsize = AtomicUsize::new(0);

fn counted_rules(_env: &mut Environment, model_name: &str) -> Result<Vec<Rule>> {
    match model_name {
        "tag" => TAG_RULE_LOADS.fetch_add(1, Ordering::SeqCst),
        "invoice" => INVOICE_RULE_LOADS.fetch_add(1, Ordering::SeqCst),
        _ => 0,
    };
    Ok(vec![Rule {
        group: Some(1),
        read: Some(SearchType::Nothing),
        create: Some(SearchType::Nothing),
        write: Some(SearchType::Nothing),
        delete: Some(SearchType::Nothing),
    }])
}

fn counted_groups(_env: &mut Environment, _uid: u32) -> Result<Vec<u32>> {
    GROUP_LOADS.fetch_add(1, Ordering::SeqCst);
    Ok(vec![1])
}

fn no_check(_env: &mut Environment) -> Result<()> {
    Ok(())
}

fn loads() -> (usize, usize, usize) {
    (
        TAG_RULE_LOADS.load(Ordering::SeqCst),
        INVOICE_RULE_LOADS.load(Ordering::SeqCst),
        GROUP_LOADS.load(Ordering::SeqCst),
    )
}

/// Changing a rule forgets only the rules of the model it is about, and changing who is in a group
/// only the groups; reading forgets nothing.
///
/// Tags stand in for rules here, a tag's name being the model it is about, and records for the
/// membership model.
#[test]
fn test_a_change_forgets_only_what_it_touches() -> Result<()> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(TestLibPlugin {}))?;
    app.load_plugin("test_lib_plugin")?;
    app.model_manager.access.register(erp::access::RuleSource {
        rules: counted_rules,
        groups: counted_groups,
        check: no_check,
        rules_model: "tag",
        rule_target: "name",
        membership: &["record"],
    });
    let mut env = app.new_env_as_option(None)?;
    let about_tags = make_tag(&mut env, "tag")?;
    env.close()?;

    let mut env = app.new_env_as(7)?;
    env.search_ids("tag", &SearchType::Nothing)?;
    env.search_ids("invoice", &SearchType::Nothing)?;
    assert_eq!(loads(), (1, 1, 1));

    env.read("tag", &SingleId::from(about_tags), &["name", "field"])?;
    env.search_ids("tag", &SearchType::Nothing)?;
    assert_eq!(
        loads(),
        (1, 1, 1),
        "loading a rule into the cache changes nothing"
    );

    let about_invoices = make_tag(&mut env, "invoice")?;
    env.search_ids("tag", &SearchType::Nothing)?;
    env.search_ids("invoice", &SearchType::Nothing)?;
    assert_eq!(
        loads(),
        (1, 2, 1),
        "a new rule on invoice forgets invoice only"
    );

    env.write(
        "tag",
        &SingleId::from(about_invoices),
        tag_values("machine"),
    )?;
    env.search_ids("invoice", &SearchType::Nothing)?;
    env.search_ids("tag", &SearchType::Nothing)?;
    assert_eq!(
        loads(),
        (1, 3, 1),
        "moving it away from invoice forgets invoice too"
    );

    env.write("tag", &SingleId::from(about_tags), tag_values("invoice"))?;
    env.search_ids("tag", &SearchType::Nothing)?;
    env.search_ids("invoice", &SearchType::Nothing)?;
    assert_eq!(loads(), (2, 4, 1), "and moving one onto it, both ends");

    env.delete("tag", &SingleId::from(about_tags))?;
    env.search_ids("tag", &SearchType::Nothing)?;
    env.search_ids("invoice", &SearchType::Nothing)?;
    assert_eq!(
        loads(),
        (2, 5, 1),
        "deleting a rule forgets what it was about"
    );

    let mut values = MapOfFields::default();
    values.insert("name", "a membership");
    env.create_records("record", vec![values])?;
    env.search_ids("tag", &SearchType::Nothing)?;
    env.search_ids("invoice", &SearchType::Nothing)?;
    assert_eq!(
        loads(),
        (2, 5, 2),
        "a membership change forgets the groups, not the rules"
    );
    Ok(())
}

// ---- what base ships ----

#[test]
fn test_the_administrator_may_do_anything_to_base() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;
    let admin: Users<SingleId> = env.named("base.user_admin")?;
    let mut env = app.new_env_as(admin.get_id())?;

    let mut values = MapOfFields::default();
    values.insert("name", "Someone");
    let contact = env.create_records("contact", vec![values])?;
    env.write("contact", &contact, tag_values("Someone else"))?;
    env.delete("contact", &contact)?;
    env.count("session", &SearchType::Nothing)?;
    env.count("access_rule", &SearchType::Nothing)?;
    Ok(())
}

/// The data file puts the administrator in both groups, and that is where its rights come from.
#[test]
fn test_the_administrator_is_seeded_in_both_groups() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;
    let admin: Users<SingleId> = env.named("base.user_admin")?;
    let group_user: Group<SingleId> = env.named("base.group_user")?;
    let group_admin: Group<SingleId> = env.named("base.group_admin")?;
    let groups: Group<MultipleIds> = admin.get_groups(&mut env)?;
    let mut groups = groups.get_ids();
    groups.sort_unstable();
    assert_eq!(groups, vec![group_user.get_id(), group_admin.get_id()]);
    drop(env);

    let mut env = app.new_env_as(admin.get_id())?;
    env.create_records("users", vec![tag_values("bob")])?;
    Ok(())
}

#[test]
fn test_a_user_reads_what_is_shared_and_nothing_else() -> Result<()> {
    let app = new_app()?;
    let mut env = app.new_env_as_option(None)?;
    let uid = make_user(&mut env, "employee")?;
    let group_user: Group<SingleId> = env.named("base.group_user")?;
    let user: Users<SingleId> = env.get_record(uid.into());
    user.set_groups(vec![group_user.get_id()].into(), &mut env)?;
    env.close()?;

    let mut env = app.new_env_as(uid)?;
    env.count("country", &SearchType::Nothing)?;
    env.count("users", &SearchType::Nothing)?;
    let mut values = MapOfFields::default();
    values.insert("name", "A customer");
    env.create_records("contact", vec![values])?;

    assert!(env.count("session", &SearchType::Nothing).is_err());
    assert!(env.count("access_rule", &SearchType::Nothing).is_err());
    let mut values = MapOfFields::default();
    values.insert("name", "Atlantis");
    assert!(env.create_records("country", vec![values]).is_err());
    Ok(())
}

/// Ships a rule on a model nobody declares.
struct MistypedRulePlugin;

impl Plugin for MistypedRulePlugin {
    fn name(&self) -> String {
        "mistyped_rule".to_string()
    }

    fn init_models(&self, _model_manager: &mut erp::model::ModelManager) {}

    fn data(&self) -> Vec<&'static str> {
        vec![
            r#"<erp>
                <access_rule id="nowhere">
                    <name>A typo</name>
                    <model>contcat</model>
                    <domain_read>[]</domain_read>
                </access_rule>
            </erp>"#,
        ]
    }

    fn get_depends(&self) -> Vec<String> {
        vec!["base".to_string()]
    }
}

/// A rule naming a model that does not exist would grant nothing and say nothing; loading fails
/// instead, whichever plugin shipped it.
#[test]
fn test_a_rule_on_an_unknown_model_fails_the_load() -> Result<()> {
    let mut app = Application::new_test();
    app.register_plugin(Box::new(BasePlugin {}))?;
    app.register_plugin(Box::new(MistypedRulePlugin {}))?;
    let error = app.load_plugin("mistyped_rule").unwrap_err();
    assert!(error.to_string().contains("contcat"), "got {error}");
    Ok(())
}

/// A domain that does not parse is refused when it is used, naming the rule it belongs to.
#[test]
fn test_a_rule_whose_domain_does_not_parse_says_so() -> Result<()> {
    let app = new_app()?;
    let fixture = fixture(
        &app,
        Domains {
            read: Some("not a domain"),
            ..Domains::default()
        },
    )?;
    let mut env = app.new_env_as(fixture.uid)?;
    let error = env.search_ids("tag", &SearchType::Nothing).unwrap_err();
    assert!(error.to_string().contains("read domain"), "got {error}");
    Ok(())
}
