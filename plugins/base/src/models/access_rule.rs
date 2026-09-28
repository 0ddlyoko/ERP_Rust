use crate::models::{BaseGroup, Group, Users};
use code_gen::Model;
use erp::access::{Rule, RuleSource};
use erp::environment::Environment;
use erp::search::SearchType;
use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};
use erp_search_code_gen::make_domain;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

/// Which records of a model a group may read, create, write or delete.
///
/// Without a group the rule is global: it restricts everybody instead of granting anything. Each
/// domain is JSON, in the form a caller sends over the wire; left empty, the rule says nothing
/// about that operation.
#[derive(Model)]
#[erp(id = "access_rule")]
#[allow(dead_code)]
pub struct AccessRule<Mode: IdMode> {
    pub id: Mode,
    #[erp(default = "")]
    name: String,
    #[erp(default = "")]
    model: String,
    group: Reference<BaseGroup, SingleId>,
    domain_read: Option<String>,
    domain_create: Option<String>,
    domain_write: Option<String>,
    domain_delete: Option<String>,
}

impl AccessRule<SingleId> {
    /// Where the core gets its rules from, and what makes them stale.
    ///
    /// Who is in a group is written from either side of the relation, so both `users` and
    /// `group` count as membership.
    pub fn source() -> RuleSource {
        RuleSource {
            rules: Self::rules_for,
            groups: Self::groups_of,
            check: Self::check_all,
            rules_model: "access_rule",
            rule_target: "model",
            membership: &["users", "group"],
        }
    }

    /// Every rule written for a model.
    fn rules_for(env: &mut Environment, model_name: &str) -> Result<Vec<Rule>> {
        let found: AccessRule<MultipleIds> =
            env.search(&make_domain!([("model", "=", model_name)]))?;
        let mut rules = Vec::with_capacity(found.get_ids_ref().len());
        for rule in found {
            rules.push(rule.to_rule(env)?);
        }
        Ok(rules)
    }

    fn groups_of(env: &mut Environment, uid: u32) -> Result<Vec<u32>> {
        let user = Users::<SingleId>::from_id(uid, env);
        Ok(user.get_groups::<Group<MultipleIds>>(env)?.get_ids())
    }

    fn to_rule(&self, env: &mut Environment) -> Result<Rule> {
        let group = self
            .get_group::<Group<SingleId>>(env)?
            .map(|group| group.get_id());
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

    /// Refuse a rule naming a model that does not exist, or holding a domain that does not parse.
    ///
    /// A mistyped model name would otherwise be a rule nobody ever reads, and the right it was
    /// meant to grant silently missing.
    fn check_all(env: &mut Environment) -> Result<()> {
        let all: AccessRule<MultipleIds> = env.search(&SearchType::Nothing)?;
        for rule in all {
            let model = rule.get_model(env)?.clone();
            if env.model_manager.try_get_model(&model).is_err() {
                let name = rule.get_name(env)?.clone();
                return Err(format!(
                    "Access rule {} ({name}) names unknown model {model:?}",
                    rule.get_id()
                )
                .into());
            }
            rule.to_rule(env)?;
        }
        Ok(())
    }
}
