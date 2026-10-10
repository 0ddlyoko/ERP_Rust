//! Access rights: which records of a model a user may read, create, write or delete.
//!
//! The core knows how a rule is evaluated, and nothing about where rules are kept. They are
//! records of a model, and models belong to plugins — so whichever plugin defines them registers
//! how to load them here, and `erp` never names it.

use crate::Result;
use crate::environment::Environment;
use erp_search::SearchType;
use std::error::Error;
use std::fmt;

/// What a caller is trying to do to records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Operation {
    Read,
    Create,
    Write,
    Delete,
}

impl Operation {
    pub fn name(self) -> &'static str {
        match self {
            Operation::Read => "read",
            Operation::Create => "create",
            Operation::Write => "write",
            Operation::Delete => "delete",
        }
    }
}

impl fmt::Display for Operation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// One rule on one model.
///
/// No group makes it a global rule. A domain left empty means the rule says nothing about that
/// operation, which is not the same as an empty domain: `Some(SearchType::Nothing)` covers every
/// record.
#[derive(Debug, Clone, Default)]
pub struct Rule {
    pub group: Option<u32>,
    pub read: Option<SearchType>,
    pub create: Option<SearchType>,
    pub write: Option<SearchType>,
    pub delete: Option<SearchType>,
}

impl Rule {
    pub fn domain(&self, operation: Operation) -> Option<&SearchType> {
        match operation {
            Operation::Read => self.read.as_ref(),
            Operation::Create => self.create.as_ref(),
            Operation::Write => self.write.as_ref(),
            Operation::Delete => self.delete.as_ref(),
        }
    }
}

/// Which records an operation may touch.
///
/// `Unrestricted` for root, sudo, the process itself, or when no plugin defines rules. `Denied`
/// when no rule grants the operation to a group the user is in.
#[derive(Debug, Clone, PartialEq)]
pub enum Access {
    Unrestricted,
    Restricted(SearchType),
    Denied,
}

impl Access {
    /// Combine the rules of a model into what `groups` may do.
    ///
    /// Group rules grant, and are ORed; global rules restrict, and are ANDed on top. Global rules
    /// alone grant nothing: without a group rule the operation is denied, so a model nobody wrote
    /// a rule for is closed rather than open.
    pub fn evaluate(rules: &[Rule], groups: &[u32], operation: Operation) -> Access {
        let mut granted: Option<SearchType> = None;
        let mut restriction = SearchType::Nothing;
        for rule in rules {
            let Some(domain) = rule.domain(operation) else {
                continue;
            };
            match rule.group {
                None => restriction = and(restriction, domain.clone()),
                Some(group) if groups.contains(&group) => {
                    granted = Some(match granted {
                        None => domain.clone(),
                        Some(previous) => or(previous, domain.clone()),
                    });
                }
                Some(_) => {}
            }
        }
        match granted {
            None => Access::Denied,
            Some(granted) => Access::Restricted(and(restriction, granted)),
        }
    }
}

fn and(left: SearchType, right: SearchType) -> SearchType {
    match (left, right) {
        (SearchType::Nothing, other) | (other, SearchType::Nothing) => other,
        (SearchType::Never, _) | (_, SearchType::Never) => SearchType::Never,
        (left, right) => SearchType::And(Box::new(left), Box::new(right)),
    }
}

fn or(left: SearchType, right: SearchType) -> SearchType {
    match (left, right) {
        (SearchType::Nothing, _) | (_, SearchType::Nothing) => SearchType::Nothing,
        (SearchType::Never, other) | (other, SearchType::Never) => other,
        (left, right) => SearchType::Or(Box::new(left), Box::new(right)),
    }
}

/// Restrict a caller's domain to what they may read.
pub fn restrict(domain: &SearchType, allowed: &SearchType) -> SearchType {
    and(domain.clone(), allowed.clone())
}

/// The rules written for a model.
pub type LoadRules = fn(&mut Environment, &str) -> Result<Vec<Rule>>;

/// The groups a user is in.
pub type LoadGroups = fn(&mut Environment, u32) -> Result<Vec<u32>>;

/// Refuse rules that cannot be what their author meant.
pub type CheckRules = fn(&mut Environment) -> Result<()>;

/// Where rules come from, and what makes a remembered answer stale.
///
/// Rules are records of `rules_model`, whose `rule_target` field names the model a rule is about:
/// changing a rule forgets only what was remembered for that model. A change to one of the
/// `membership` models may change who is in a group, and forgets only the groups remembered.
/// `check` runs once every plugin has loaded its data, so a rule shipped by any plugin is checked,
/// not only those of the plugin owning rules.
pub struct RuleSource {
    pub rules: LoadRules,
    pub groups: LoadGroups,
    pub check: CheckRules,
    pub rules_model: &'static str,
    pub rule_target: &'static str,
    pub membership: &'static [&'static str],
}

/// Who answers "what may this user do".
///
/// Empty until a plugin fills it, and then nothing is checked: an application that defines no
/// rules has no rights to enforce, the way one that defines no users has nobody to authenticate.
#[derive(Default)]
pub struct AccessRules {
    source: Option<RuleSource>,
}

impl AccessRules {
    /// Say where rules come from.
    ///
    /// Panics on a second source, for the same reason as [`crate::identity::Identities::register`]:
    /// which one won would depend on load order.
    pub fn register(&mut self, source: RuleSource) {
        if self.source.is_some() {
            panic!(
                "Two plugins both register where access rules come from. Only one can answer, \
                 and which one would depend on load order."
            );
        }
        self.source = Some(source);
    }

    pub fn source(&self) -> Option<&RuleSource> {
        self.source.as_ref()
    }
}

/// An operation the caller's rights do not cover.
///
/// Names what was refused — the model, the operation, the fields and the records — so whoever
/// hits it can tell which right is missing rather than only that one is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessDenied {
    pub model_name: String,
    pub operation: Operation,
    pub fields: Vec<String>,
    pub ids: Vec<u32>,
}

impl AccessDenied {
    /// Fields sorted, so the same refusal always reads the same.
    pub fn new(model_name: &str, operation: Operation, fields: &[&str], ids: Vec<u32>) -> Self {
        let mut fields: Vec<String> = fields.iter().map(|field| field.to_string()).collect();
        fields.sort_unstable();
        fields.dedup();
        AccessDenied {
            model_name: model_name.to_string(),
            operation,
            fields,
            ids,
        }
    }
}

impl fmt::Display for AccessDenied {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "You are not allowed to {} ", self.operation)?;
        if !self.fields.is_empty() {
            write!(f, "fields {} of ", self.fields.join(", "))?;
        }
        write!(f, "{}", self.model_name)?;
        if !self.ids.is_empty() {
            let ids: Vec<String> = self.ids.iter().map(u32::to_string).collect();
            write!(f, " (ids {})", ids.join(", "))?;
        }
        Ok(())
    }
}

impl Error for AccessDenied {}
