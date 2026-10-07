use crate::models::{BaseGroup, Group, ModelData, Users};
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::access::{Rule, RuleSource};
use erp::environment::Environment;
use erp::search::SearchType;
use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};
use erp_search_code_gen::make_domain;

/// Which records of a model a group may read, create, write or delete.
///
/// Without a group the rule is global: it restricts everybody instead of granting anything. Each
/// domain is JSON, in the form a caller sends over the wire; left empty, the rule says nothing
/// about that operation.
#[derive(Model)]
#[erp(id = "access_rule", methods)]
#[allow(dead_code)]
pub struct AccessRule<Mode: IdMode> {
    pub id: Mode,
    name: String,
    #[erp(index)]
    model: String,
    #[erp(ondelete = "cascade")]
    group: Reference<BaseGroup, SingleId>,
    domain_read: Option<String>,
    domain_create: Option<String>,
    domain_write: Option<String>,
    domain_delete: Option<String>,
}

/// The rules of each model, by model name.
pub const RULES_CACHE: &str = "base.access_rules";
/// The groups of each user, by id.
pub const GROUPS_CACHE: &str = "base.user_groups";

#[erp_methods]
impl AccessRule<SingleId> {
    /// Every rule written for a model, kept across requests until a rule changes.
    fn rules_for(env: &mut Environment, model_name: String) -> Result<Vec<Rule>> {
        let rules = env.cached(RULES_CACHE, &model_name, |env| {
            let found: AccessRule<MultipleIds> =
                env.search(&make_domain!([("model", "=", model_name.as_str())]))?;
            let mut rules = Vec::with_capacity(found.get_ids_ref().len());
            for rule in found {
                rules.push(rule.to_rule(env)?);
            }
            Ok(rules)
        })?;
        Ok(rules.as_ref().clone())
    }

    /// The groups of a user, kept across requests until a user or a group changes.
    fn groups_of(env: &mut Environment, uid: u32) -> Result<Vec<u32>> {
        let groups = env.cached(GROUPS_CACHE, &uid.to_string(), |env| {
            let user = Users::<SingleId>::from_id(uid, env);
            Ok(user.get_groups::<Group<MultipleIds>>(env)?.get_ids())
        })?;
        Ok(groups.as_ref().clone())
    }

    fn to_rule(&self, env: &mut Environment) -> Result<Rule> {
        let group = self.get_group::<Group<SingleId>>(env)?.get_optional_id();
        let read = self.get_domain_read(env)?.cloned();
        let create = self.get_domain_create(env)?.cloned();
        let write = self.get_domain_write(env)?.cloned();
        let delete = self.get_domain_delete(env)?.cloned();
        let name = self.get_name(env)?.clone();
        Ok(Rule {
            group,
            read: self.parse(&name, "read", read)?,
            create: self.parse(&name, "create", create)?,
            write: self.parse(&name, "write", write)?,
            delete: self.parse(&name, "delete", delete)?,
        })
    }

    /// Refuse a rule holding a domain that does not parse, and warn of one naming a model that
    /// does not exist.
    ///
    /// Only a warning, and not for the rules of a plugin not loaded yet: they name models
    /// registered once it is. A mistyped model name shows up in the log rather than in rights
    /// silently missing — or, for a global rule, a restriction silently not applied.
    fn check_all(env: &mut Environment) -> Result<()> {
        let all: AccessRule<MultipleIds> = env.search(&SearchType::Nothing)?;
        let not_loaded = ModelData::of_plugins_not_loaded(env, "access_rule".to_string())?;
        for rule in all {
            let model = rule.get_model(env)?.clone();
            if env.model_manager.try_get_model(&model).is_err()
                && !not_loaded.contains(&rule.get_id())
            {
                let name = rule.get_name(env)?.clone();
                tracing::warn!(
                    rule = rule.get_id(),
                    name = %name,
                    model = %model,
                    "An access rule names an unknown model: nothing reads it"
                );
                continue;
            }
            rule.to_rule(env)?;
        }
        Ok(())
    }
}

impl AccessRule<SingleId> {
    /// Where the core gets its rules from, and what makes them stale.
    ///
    /// Who is in a group is written from either side of the relation, so both `users` and
    /// `group` count as membership.
    pub fn source() -> RuleSource {
        RuleSource {
            rules: |env, model_name| Self::rules_for(env, model_name.to_string()),
            groups: Self::groups_of,
            check: Self::check_all,
            rules_model: "access_rule",
            rule_target: "model",
            membership: &["users", "group"],
        }
    }

    /// Read one domain, saying which rule it belongs to when it does not parse.
    fn parse(
        &self,
        name: &str,
        operation: &str,
        raw: Option<String>,
    ) -> Result<Option<SearchType>> {
        let Some(raw) = raw else {
            return Ok(None);
        };
        erp::serde_json::from_str(&raw).map(Some).map_err(|error| {
            format!(
                "Access rule {} ({name}) has a {operation} domain that is not one: {error}",
                self.get_id()
            )
            .into()
        })
    }
}
