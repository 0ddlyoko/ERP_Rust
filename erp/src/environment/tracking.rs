//! Noting what tracked fields held before a write, and handing what changed to the plugins that
//! asked.
use super::*;
use crate::model::TrackedChange;
use erp_internal_types::FinalInternalField;
use std::collections::BTreeMap;

/// How many times hooks may write tracked fields in turn before giving up: a hook tracking what
/// another hook writes, and so on, never ends.
const MAX_TRACKING_ROUNDS: usize = 16;

/// What tracked fields held before this unit of work first wrote them, by record and by who wrote
/// them: the changes are saved later, maybe once the work no longer runs as the writer.
pub(crate) type Tracked = BTreeMap<(String, u32, Option<u32>), BTreeMap<String, Option<FieldType>>>;

impl<'mm> Environment<'mm> {
    /// Remember what a tracked field of these records held, the first time it is written.
    ///
    /// Only when a plugin listens: without one, nothing is read.
    pub(super) fn remember_before_write<Mode: IdMode>(
        &mut self,
        model_name: &str,
        field: &FinalInternalField,
        ids: &Mode,
    ) -> Result<()> {
        if !field.tracking || self.model_manager.tracking_hooks.is_empty() {
            return Ok(());
        }
        let unseen: Vec<u32> = ids
            .get_ids_ref()
            .iter()
            .copied()
            .filter(|id| {
                !self
                    .tracked
                    .get(&(model_name.to_string(), *id, self.uid))
                    .is_some_and(|fields| fields.contains_key(&field.name))
            })
            .collect();
        if unseen.is_empty() {
            return Ok(());
        }
        let before = self.retrieve_field_from_cache_or_database(
            model_name,
            &field.name,
            &MultipleIds::from(unseen.clone()),
        )?;
        for (id, (_, value)) in unseen.into_iter().zip(before) {
            let value = comparable(value);
            self.tracked
                .entry((model_name.to_string(), id, self.uid))
                .or_default()
                .insert(field.name.clone(), value);
        }
        Ok(())
    }

    /// Forget what was remembered of these records: what they are created with is no change.
    pub(super) fn forget_tracked(&mut self, model_name: &str, ids: &[u32]) {
        if self.tracked.is_empty() {
            return;
        }
        self.tracked
            .retain(|(model, id, _), _| model != model_name || !ids.contains(id));
    }

    /// Hand each record's changed tracked fields to the tracking hooks.
    ///
    /// A field written back to what it held is no change — a list of references in another order
    /// neither; a record deleted since is skipped.
    /// What the hooks write is tracked in turn.
    pub(super) fn report_tracked_changes(&mut self) -> Result<()> {
        for _ in 0..MAX_TRACKING_ROUNDS {
            let tracked = std::mem::take(&mut self.tracked);
            if tracked.is_empty() {
                return Ok(());
            }
            let hooks = self.model_manager.tracking_hooks.clone();
            for ((model_name, id, author), fields) in tracked {
                if self
                    .cache
                    .get_cache_models(&model_name)
                    .get_model(id)
                    .is_none()
                {
                    continue;
                }
                let mut changes = Vec::with_capacity(fields.len());
                for (field, old) in fields {
                    let new = self
                        .retrieve_field_from_cache_or_database(
                            &model_name,
                            &field,
                            &SingleId::from(id),
                        )?
                        .pop()
                        .and_then(|(_, value)| comparable(value));
                    if new != old {
                        changes.push(TrackedChange { field, old, new });
                    }
                }
                if changes.is_empty() {
                    continue;
                }
                for hook in &hooks {
                    hook(self, &model_name, id, author, &changes)?;
                }
            }
        }
        Err(format!(
            "Tracked fields kept changing after {MAX_TRACKING_ROUNDS} rounds of tracking hooks"
        )
        .into())
    }
}

/// A value as tracking compares it: a list of references sorted, and empty when it holds none.
fn comparable(value: Option<FieldType>) -> Option<FieldType> {
    match value {
        Some(FieldType::Refs(ids)) if ids.is_empty() => None,
        Some(FieldType::Refs(mut ids)) => {
            ids.sort_unstable();
            Some(FieldType::Refs(ids))
        }
        other => other,
    }
}
