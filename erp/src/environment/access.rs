//! Checking the caller's access rights before records are touched.
use super::*;
use crate::access::{Access, AccessDenied, Operation, Rule};

/// Rules per model and groups per user, remembered for the length of a transaction.
#[derive(Default)]
pub(super) struct AccessMemo {
    rules: HashMap<String, Vec<Rule>>,
    groups: HashMap<u32, Vec<u32>>,
}

impl<'mm> Environment<'mm> {
    /// Which records of a model the current user may touch with an operation.
    ///
    /// Unrestricted for root, under [`Environment::sudo`], with no user at all — the process
    /// working on its own account — and when no plugin defines rules. Otherwise the rules and the
    /// user's groups are loaded once, as sudo so that reading them needs no right of its own, and
    /// remembered until a watched model changes.
    pub fn access(&mut self, model_name: &str, operation: Operation) -> Result<Access> {
        let model_manager = self.model_manager;
        let (Some(uid), Some(source)) = (self.uid, model_manager.access.source()) else {
            return Ok(Access::Unrestricted);
        };
        if self.sudo || self.is_root() {
            return Ok(Access::Unrestricted);
        }

        if !self.access_memo.rules.contains_key(model_name) {
            let rules = (source.rules)(&mut self.sudo(), model_name)?;
            self.access_memo.rules.insert(model_name.to_string(), rules);
        }
        if !self.access_memo.groups.contains_key(&uid) {
            let groups = (source.groups)(&mut self.sudo(), uid)?;
            self.access_memo.groups.insert(uid, groups);
        }
        Ok(Access::evaluate(
            &self.access_memo.rules[model_name],
            &self.access_memo.groups[&uid],
            operation,
        ))
    }

    /// Refuse the operation unless every one of these records is within the caller's rights.
    ///
    /// All or nothing: touching the allowed records and quietly skipping the others would be worse
    /// than a refusal. Records that do not exist are not reported as refused — a second search
    /// tells them apart, and only runs when the first one came back short.
    pub fn check_access(
        &mut self,
        model_name: &str,
        operation: Operation,
        ids: &[u32],
        fields: &[&str],
    ) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        let denied = |ids: Vec<u32>| AccessDenied::new(model_name, operation, fields, ids);
        let domain = match self.access(model_name, operation)? {
            Access::Unrestricted | Access::Restricted(SearchType::Nothing) => return Ok(()),
            Access::Denied => return Err(denied(ids.to_vec()).into()),
            Access::Restricted(domain) => domain,
        };

        let asked: HashSet<u32> = ids.iter().copied().collect();
        let within = make_domain!([("id", "=", ids.to_vec())]);
        let allowed: HashSet<u32> = self
            .search_ids_unchecked(
                model_name,
                &crate::access::restrict(&within, &domain),
                &SearchOptions::default(),
            )?
            .into_iter()
            .collect();
        if allowed.len() == asked.len() {
            return Ok(());
        }
        let mut refused: Vec<u32> = self
            .search_ids_unchecked(model_name, &within, &SearchOptions::default())?
            .into_iter()
            .filter(|id| !allowed.contains(id))
            .collect();
        if refused.is_empty() {
            return Ok(());
        }
        refused.sort_unstable();
        Err(denied(refused).into())
    }

    /// A caller's domain, narrowed to what they may read.
    pub(super) fn readable_domain(
        &mut self,
        model_name: &str,
        domain: &SearchType,
    ) -> Result<SearchType> {
        match self.access(model_name, Operation::Read)? {
            Access::Unrestricted => Ok(domain.clone()),
            Access::Restricted(allowed) => Ok(crate::access::restrict(domain, &allowed)),
            Access::Denied => {
                Err(AccessDenied::new(model_name, Operation::Read, &[], Vec::new()).into())
            }
        }
    }

    /// Whether a compute running right now is filling this stored field.
    pub(super) fn is_computing(&self, model_name: &str, field_name: &str) -> bool {
        self.computing
            .iter()
            .any(|(model, field)| model == model_name && field == field_name)
    }

    /// Forget what a change to these records may have made stale.
    ///
    /// For a rule, only the rules remembered for the model it is about; for a membership model,
    /// only the groups. Called before the change and, for a new record, once it exists — so a
    /// rule moved from one model to another forgets both. Reads nothing while nothing is
    /// remembered.
    pub(super) fn forget_access_of(&mut self, model_name: &str, ids: &[u32]) -> Result<()> {
        let Some(source) = self.model_manager.access.source() else {
            return Ok(());
        };
        if source.membership.contains(&model_name) {
            self.access_memo.groups.clear();
            return Ok(());
        }
        if model_name != source.rules_model || self.access_memo.rules.is_empty() || ids.is_empty() {
            return Ok(());
        }
        let targets: Vec<String> = self
            .get_fields_value_unchecked(
                source.rules_model,
                source.rule_target,
                &MultipleIds::from(ids.to_vec()),
            )?
            .into_iter()
            .flatten()
            .filter_map(|target| match target {
                FieldType::String(target) => Some(target.clone()),
                _ => None,
            })
            .collect();
        for target in targets {
            self.forget_rules_for(&target);
        }
        Ok(())
    }

    /// Forget the rules remembered for one model.
    pub(super) fn forget_rules_for(&mut self, model_name: &str) {
        self.access_memo.rules.remove(model_name);
    }

    /// Whether writing this field of a rule points it at another model.
    pub(super) fn is_rule_target(&self, model_name: &str, field_name: &str) -> bool {
        self.model_manager.access.source().is_some_and(|source| {
            source.rules_model == model_name && source.rule_target == field_name
        })
    }

    pub(super) fn forget_access(&mut self) {
        self.access_memo = AccessMemo::default();
    }
}
