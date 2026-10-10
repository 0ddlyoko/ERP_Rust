//! Writing field values into the cache, keeping relational mirrors coherent.
use super::*;
use crate::access::Operation;
use crate::errors::MissingRecords;
use crate::model::{WRITE, WriteArgs};
use crate::model::{WRITE_DATE, WRITE_UID};
use chrono::Utc;
use erp_internal_types::FinalInternalField;
use erp_types::field::{FieldKind, OnDelete};

impl<'mm> Environment<'mm> {
    /// Write field values onto records, addressing the model and its fields by name.
    ///
    /// The counterpart of [`Environment::read`] for callers that only hold names at runtime.
    /// Values go through the same path as the generated setters, so relational mirrors stay
    /// coherent and dependent computes are flagged. A one2many or a many2many may be given
    /// commands, carried out in order for each record, on what it holds.
    ///
    /// Refused as a whole unless the caller may write every one of the records. Goes through the
    /// model's `write`, so what a plugin overrode there runs.
    /// Write to one record only what differs from what it holds — a list of records compared
    /// whatever its order — and nothing at all when nothing does: no write, no `write_date`.
    /// Returns whether anything was written.
    ///
    /// For what loads the same values again and again — data files, the plugins' own rows.
    pub fn write_changes(
        &mut self,
        model_name: &str,
        id: u32,
        values: MapOfFields,
    ) -> Result<bool> {
        let names: Vec<String> = values.fields.keys().cloned().collect();
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let held = self
            .sudo()
            .read(model_name, &SingleId::from(id), &names)
            .ok()
            .and_then(|rows| rows.into_iter().next());
        let changed = match held {
            Some(held) => {
                let same = |given: &Option<FieldType>, held: Option<&Option<FieldType>>| match (
                    given, held,
                ) {
                    (Some(FieldType::Refs(given)), Some(Some(FieldType::Refs(held)))) => {
                        let (mut given, mut held) = (given.clone(), held.clone());
                        given.sort_unstable();
                        held.sort_unstable();
                        given == held
                    }
                    (given, Some(held)) => given == held,
                    (_, None) => false,
                };
                MapOfFields::new(
                    values
                        .fields
                        .into_iter()
                        .filter(|(name, given)| !same(given, held.fields.get(name)))
                        .collect(),
                )
            }
            None => values,
        };
        if changed.is_empty() {
            return Ok(false);
        }
        self.write(model_name, &SingleId::from(id), changed)?;
        Ok(true)
    }

    /// Write `values` on the records through the model's `write`; the checks the written fields
    /// concern run after. A refusal undoes nothing on its own: the unit of work fails with it, or
    /// the caller wanting to carry on opens a savepoint around it.
    pub fn write<Mode: IdMode>(
        &mut self,
        model_name: &str,
        ids: &Mode,
        values: MapOfFields,
    ) -> Result<()> {
        self.model_manager.try_get_model(model_name)?;
        let ids: MultipleIds = ids.clone().into();
        if !self.has_checks(model_name) {
            return self.call_method::<WriteArgs, ()>(model_name, WRITE, &ids, &(values,));
        }
        let written: Vec<String> = values.fields.keys().cloned().collect();
        self.checked(
            |env| env.call_method::<WriteArgs, ()>(model_name, WRITE, &ids, &(values,)),
            |env, ()| env.note_checks(model_name, &ids, &written, false),
        )
    }

    /// Whether creating or writing records of the model may concern a check: its own, or one
    /// naming a field of it along a path.
    pub(crate) fn has_checks(&self, model_name: &str) -> bool {
        self.model_manager
            .try_get_model(model_name)
            .is_ok_and(|model| !model.checks.is_empty())
            || self
                .model_manager
                .check_links
                .keys()
                .any(|(model, _)| model == model_name)
    }

    /// Note the checks concerned by `ids` of `model_name` — all of the model's own once
    /// `created`, and every check naming one of the fields `written`, on the records it leads back
    /// to: a line's order for its price — to run once the outermost creation or write is done.
    pub(crate) fn note_checks(
        &mut self,
        model_name: &str,
        ids: &MultipleIds,
        written: &[String],
        created: bool,
    ) -> Result<()> {
        if ids.get_ids_ref().is_empty() {
            return Ok(());
        }
        let mut due: Vec<(String, String, bool, Vec<u32>)> = Vec::new();
        if created {
            for check in &self.model_manager.try_get_model(model_name)?.checks {
                due.push((
                    model_name.to_string(),
                    check.method.clone(),
                    check.per_record,
                    ids.get_ids_ref().clone(),
                ));
            }
        }
        for field in written {
            let key = (model_name.to_string(), field.clone());
            let Some(links) = self.model_manager.check_links.get(&key) else {
                continue;
            };
            for link in links.clone() {
                let found = self.ids_back(model_name, ids.get_ids_ref(), &link.steps)?;
                due.push((link.model, link.method, link.per_record, found));
            }
        }
        for (model, method, per_record, found) in due {
            match self
                .pending_checks
                .iter_mut()
                .find(|(known, name, _, _)| *known == model && *name == method)
            {
                Some((_, _, _, ids)) => {
                    for id in found {
                        if !ids.contains(&id) {
                            ids.push(id);
                        }
                    }
                }
                None => self.pending_checks.push((model, method, per_record, found)),
            }
        }
        Ok(())
    }

    /// Create or write through `work`, then note the checks it concerns; once the outermost is
    /// done, run every check noted, in sudo — so a record created with its lines is checked with
    /// all of them, not as each comes. A failure forgets the checks noted.
    pub(crate) fn checked<R>(
        &mut self,
        work: impl FnOnce(&mut Self) -> Result<R>,
        noted: impl FnOnce(&mut Self, &R) -> Result<()>,
    ) -> Result<R> {
        self.check_depth += 1;
        let done = work(self).and_then(|result| noted(self, &result).map(|()| result));
        self.check_depth -= 1;
        let result = match done {
            Ok(result) => result,
            Err(error) => {
                if self.check_depth == 0 {
                    self.pending_checks.clear();
                }
                return Err(error);
            }
        };
        if self.check_depth == 0 {
            let pending = std::mem::take(&mut self.pending_checks);
            // A rule holds whoever wrote: checks read what they need, whatever the caller's rights.
            self.sudo_with(|env| {
                for (model, method, per_record, ids) in pending {
                    if ids.is_empty() {
                        continue;
                    }
                    if per_record {
                        for id in ids {
                            env.call_method::<(), ()>(
                                &model,
                                &method,
                                &MultipleIds::from(id),
                                &(),
                            )?;
                        }
                    } else {
                        env.call_method::<(), ()>(&model, &method, &MultipleIds::from(ids), &())?;
                    }
                }
                Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
            })?;
        }
        Ok(result)
    }

    /// The records `steps` lead back to from `ids` of `model_name`: each many2one read, each
    /// field pointing to them searched, as a computed field's dependencies are followed.
    fn ids_back(
        &mut self,
        model_name: &str,
        ids: &[u32],
        steps: &[FieldDepend],
    ) -> Result<Vec<u32>> {
        let mut at = model_name.to_string();
        let mut current = ids.to_vec();
        for step in steps {
            if current.is_empty() {
                break;
            }
            match step {
                FieldDepend::CurrentFieldAnotherModel {
                    target_model,
                    field_name,
                } => {
                    let values = self.get_fields_value_unchecked::<MultipleIds>(
                        &at,
                        field_name,
                        &current.clone().into(),
                    )?;
                    let mut next: Vec<u32> = Vec::new();
                    for value in values.into_iter().flatten() {
                        match value {
                            FieldType::Ref(id) => next.push(*id),
                            FieldType::Refs(ids) => next.extend(ids),
                            _ => {}
                        }
                    }
                    let mut seen = HashSet::new();
                    next.retain(|id| *id != 0 && seen.insert(*id));
                    current = next;
                    at = target_model.clone();
                }
                FieldDepend::AnotherModel {
                    target_model,
                    target_field,
                } => {
                    current = self.search_ids_unchecked(
                        target_model,
                        &make_domain!([(target_field.as_str(), "in", current.clone())]),
                        &SearchOptions::default(),
                    )?;
                    at = target_model.clone();
                }
                FieldDepend::SameModel { .. } => {}
            }
        }
        Ok(current)
    }

    /// What writing does, below every override of `write`.
    pub(crate) fn write_records<Mode: IdMode>(
        &mut self,
        model_name: &str,
        ids: &Mode,
        values: MapOfFields,
    ) -> Result<()> {
        let fields: Vec<&str> = values.fields.keys().map(String::as_str).collect();
        self.check_access(model_name, Operation::Write, ids.get_ids_ref(), &fields)?;
        let written_by_hand = self.editable_computed(model_name, &fields);
        for (field_name, value) in values.fields {
            if let Some(FieldType::Commands(commands)) = value {
                for id in ids.get_ids_ref() {
                    self.write_commands(model_name, &field_name, *id, commands.clone())?;
                }
                continue;
            }
            self.save_field_to_cache(
                model_name,
                &field_name,
                ids,
                value,
                &Dirty::UpdateDirty,
                &Update::UpdateIfExists,
            )?;
        }
        self.keep_written_by_hand(model_name, &written_by_hand, ids.get_ids_ref());
        self.refuse_emptied_relations()
    }

    /// Of `fields`, the computed ones that may be set by hand.
    pub(super) fn editable_computed(&self, model_name: &str, fields: &[&str]) -> Vec<String> {
        let model = self.model_manager.get_model(model_name);
        fields
            .iter()
            .filter(|field| {
                model
                    .fields
                    .get(**field)
                    .is_some_and(|field| field.editable && field.compute.is_some())
            })
            .map(|field| field.to_string())
            .collect()
    }

    /// Keep what was written by hand to editable computed fields: what else the same write
    /// changed would otherwise have them worked out again over it.
    pub(super) fn keep_written_by_hand(
        &mut self,
        model_name: &str,
        fields: &[String],
        ids: &[u32],
    ) {
        if fields.is_empty() {
            return;
        }
        let fields: Vec<&str> = fields.iter().map(String::as_str).collect();
        self.cache
            .get_cache_models_mut(model_name)
            .remove_to_recompute(&fields, ids);
    }

    pub(crate) fn save_value_to_cache<Mode: IdMode, E>(
        &mut self,
        model_name: &str,
        field_name: &str,
        ids: &Mode,
        value: E,
    ) -> Result<()>
    where
        E: Into<FieldType>,
    {
        self.save_option_to_cache(model_name, field_name, ids, Some(value))
    }

    /// Write one field, as a generated setter does: through the model's `write`, as setting a
    /// field in Odoo does, so that what a plugin overrode there runs.
    ///
    /// Except for the field a compute is filling in right now, and for virtual records: those are
    /// put in the cache as they are, as Odoo does too.
    pub(crate) fn save_option_to_cache<Mode: IdMode, E>(
        &mut self,
        model_name: &str,
        field_name: &str,
        ids: &Mode,
        value: Option<E>,
    ) -> Result<()>
    where
        E: Into<FieldType>,
    {
        let is_virtual = ids.get_ids_ref().iter().any(|id| onchange::is_virtual(*id));
        if self.is_computing(model_name, field_name) || is_virtual {
            return self.save_option_to_cache_unchecked(model_name, field_name, ids, value);
        }
        let mut values = MapOfFields::default();
        values.insert_option(field_name, value);
        self.write(model_name, ids, values)
    }

    /// Same, whatever the caller's rights.
    pub(super) fn save_option_to_cache_unchecked<Mode: IdMode, E>(
        &mut self,
        model_name: &str,
        field_name: &str,
        ids: &Mode,
        value: Option<E>,
    ) -> Result<()>
    where
        E: Into<FieldType>,
    {
        let field_type: Option<FieldType> = value.map(|value| value.into());

        self.save_field_to_cache(
            model_name,
            field_name,
            ids,
            field_type,
            &Dirty::UpdateDirty,
            &Update::UpdateIfExists,
        )
    }

    /// What [`Self::retrieve_field_from_cache_or_database`] would answer for records whose
    /// field is being loaded from the database with `loaded`: the cached value where there is
    /// one, else `loaded` itself, which is what the database holds — so it is not asked again.
    fn loaded_field_values<Mode: IdMode>(
        &self,
        model_name: &str,
        field_name: &str,
        ids: &Mode,
        loaded: &Option<FieldType>,
    ) -> Vec<(bool, Option<FieldType>)> {
        let cache_model = self.cache.get_cache_models(model_name);
        ids.get_ids_ref()
            .iter()
            .map(|id| {
                match cache_model
                    .get_model(*id)
                    .and_then(|model| model.get_field(field_name))
                {
                    Some(field_value) => (true, field_value.get().cloned()),
                    None => (false, loaded.clone()),
                }
            })
            .collect()
    }

    /// Retrieve given field from the cache, or from the database if not loaded in cache
    ///
    /// If field is retrieved from the database, it will not be added to the cache
    ///
    /// If field is not stored, return the default value
    ///
    /// Return a vector sorted by given ids of tuple, one per id given, an id named twice
    /// included. First element is true if it's from the cache, or false if it's from the
    /// database. Second element is the value.
    ///
    /// Refused when a record is in neither: it does not exist, or was deleted since.
    pub(super) fn retrieve_field_from_cache_or_database<Mode: IdMode>(
        &mut self,
        model_name: &str,
        field_name: &str,
        ids: &Mode,
    ) -> Result<Vec<(bool, Option<FieldType>)>> {
        let size = ids.get_ids_ref().len();
        let mut map_result: HashMap<u32, (bool, Option<FieldType>)> = HashMap::with_capacity(size);
        let cache_model = self.cache.get_cache_models(model_name);
        let mut ids_not_in_cache: Vec<u32> = Vec::with_capacity(size);
        for id in ids.get_ids_ref() {
            if let Some(model) = cache_model.get_model(*id) {
                if let Some(field_value) = model.get_field(field_name) {
                    map_result.insert(*id, (true, field_value.get().cloned()));
                } else {
                    ids_not_in_cache.push(*id);
                }
            } else {
                ids_not_in_cache.push(*id);
            }
        }

        if !ids_not_in_cache.is_empty() {
            let model_info = self.model_manager.try_get_model(model_name)?;

            let field_info = model_info.try_get_internal_field(field_name)?;
            if field_info.is_stored() {
                // Load from database
                let database_result = self.database.search(
                    model_name,
                    &[field_name],
                    &make_domain!([("id", "=", ids_not_in_cache)]),
                    self.model_manager,
                    &SearchOptions::default(),
                )?;
                for (id, mut map) in database_result {
                    let field_value = map.remove(field_name).unwrap();
                    map_result.insert(id, (false, field_value.map(|value| value.into())));
                }
            } else if let Some(FieldReference {
                target_model,
                inverse_field:
                    FieldReferenceType::M2M {
                        relation,
                        target_column,
                        ..
                    },
            }) = &field_info.inverse
            {
                // A many2many has no column of its own; its pairs come from the relation table.
                // Both sides may still be holding unwritten changes, so they reach the table
                // first — the same precaution the one2many path takes.
                self.save_relations_to_db(model_name, &[field_name])?;
                if let Some(mirror) = self.mirror_of_relation(target_model, relation, target_column)
                {
                    self.save_relations_to_db(target_model, &[&mirror])?;
                }
                let model_info = self.model_manager.try_get_model(model_name)?;
                let field_info = model_info.try_get_internal_field(field_name)?;
                let Some(FieldReference {
                    inverse_field:
                        FieldReferenceType::M2M {
                            relation,
                            column,
                            target_column,
                        },
                    ..
                }) = &field_info.inverse
                else {
                    unreachable!("just matched a many2many")
                };
                let pairs = self.database.read_relation(
                    relation,
                    column,
                    target_column,
                    &ids_not_in_cache,
                )?;
                for (id, targets) in pairs {
                    map_result.insert(id, (false, Some(FieldType::Refs(targets))));
                }
            } else if let Some(FieldReference {
                target_model,
                inverse_field: FieldReferenceType::O2M { inverse_field },
            }) = &field_info.inverse
            {
                // O2M, save data to the database, and then make a request
                self.save_fields_to_db(target_model, &[inverse_field])?;
                // Load from database
                let mut result: HashMap<u32, Vec<u32>> =
                    HashMap::with_capacity(ids_not_in_cache.len());
                for id in &ids_not_in_cache {
                    result.insert(*id, vec![]);
                }

                let pointing = self.pointing_within_domain(
                    model_name,
                    field_name,
                    target_model,
                    make_domain!([(inverse_field, "=", ids_not_in_cache)]),
                )?;
                let database_result = self.database.search(
                    target_model,
                    &[inverse_field],
                    &pointing,
                    self.model_manager,
                    &SearchOptions::default(),
                )?;
                for (id, mut map) in database_result {
                    // Data should exist in database, and should not be empty, so we unwrap 2 times
                    let field_value = map.remove(inverse_field.as_str()).unwrap().unwrap();
                    let target_id = match field_value {
                        crate::database::FieldType::UInteger(id) => id,
                        // Only "UInteger" should be there. If it's not the case, there is an issue somewhere
                        _ => {
                            return Err(format!(
                                "{target_model}.{inverse_field} holds {field_value}, not a record"
                            )
                            .into());
                        }
                    };
                    result.get_mut(&target_id).unwrap().push(id);
                }
                for (id, ids) in result.into_iter() {
                    map_result.insert(id, (false, Some(FieldType::Refs(ids))));
                }
            } else {
                // Load default value, which may simply be "empty".
                for id in ids_not_in_cache {
                    map_result.insert(id, (false, field_info.default_value.clone()));
                }
            }
        }

        let mut result: Vec<(bool, Option<FieldType>)> = Vec::with_capacity(size);
        let mut missing = Vec::new();
        for id in ids.get_ids_ref() {
            match map_result.get(id) {
                Some(found) => result.push(found.clone()),
                None => missing.push(*id),
            }
        }
        if !missing.is_empty() {
            return Err(MissingRecords {
                model_name: model_name.to_string(),
                ids: missing,
            }
            .into());
        }
        Ok(result)
    }

    /// Forget the one2many fields whose domain reads `model_name.field_name`, on every record:
    /// what they hold may no longer match.
    fn forget_domain_dependents(&mut self, model_name: &str, field_name: &str) {
        if self.model_manager.domain_dependents.is_empty() {
            return;
        }
        let key = (model_name.to_string(), field_name.to_string());
        let Some(dependents) = self.model_manager.domain_dependents.get(&key) else {
            return;
        };
        for (model, field) in dependents {
            self.cache.invalidate_field_everywhere(model, field);
        }
    }

    /// The records of `target_model` pointing back, `pointing`, narrowed to the domain of the
    /// one2many `model_name.field_name` when it has one — its fields saved first, so the
    /// database sees them as the cache does.
    pub(super) fn pointing_within_domain(
        &mut self,
        model_name: &str,
        field_name: &str,
        target_model: &str,
        pointing: SearchType,
    ) -> Result<SearchType> {
        let Some(domain) = self.model_manager.one2many_domain(model_name, field_name)? else {
            return Ok(pointing);
        };
        self.save_domain_fields_to_db(target_model, &domain)?;
        Ok(SearchType::And(Box::new(pointing), Box::new(domain)))
    }

    /// Save given field to cache.
    ///
    /// This method ensure M2O & O2M are correctly linked in cache (if those fields are loaded)
    /// Field on `model_name` that is the other end of a relation table.
    ///
    /// The two sides of a many2many name the same table independently, so the pairing is found
    /// by matching on it rather than being declared twice.
    pub(super) fn mirror_of_relation(
        &self,
        model_name: &str,
        relation: &str,
        column: &str,
    ) -> Option<String> {
        let model = self.model_manager.try_get_model(model_name).ok()?;
        model
            .field_of_relation(relation, column)
            .map(str::to_string)
    }

    /// Refuse writing a field the ORM fills in: when and by whom a record was created or changed.
    pub(super) fn refuse_automatic(model_name: &str, field: &FinalInternalField) -> Result<()> {
        if field.automatic {
            return Err(format!(
                "Field \"{}\" of model \"{model_name}\" is filled in by the ORM, and written by \
                 nobody else",
                field.name
            )
            .into());
        }
        Ok(())
    }

    /// A value with the references to id 0 left out: 0 is no record, so pointing to it is
    /// pointing nowhere.
    pub(super) fn without_empty_references(value: Option<FieldType>) -> Option<FieldType> {
        match value {
            Some(FieldType::Ref(0)) => None,
            Some(FieldType::Refs(ids)) => Some(FieldType::Refs(
                ids.into_iter().filter(|id| *id != 0).collect(),
            )),
            value => value,
        }
    }

    /// Note these records as changed now, by whoever this unit of work runs as.
    ///
    /// Written straight into the cache, marked dirty, so it is saved with the change itself and
    /// names whoever made it, even when saving happens later as somebody else.
    pub(super) fn stamp_written(&mut self, model_name: &str, ids: &[u32]) -> Result<()> {
        let model = self.model_manager.try_get_model(model_name)?;
        if model.fields.contains_key(WRITE_DATE) {
            self.cache.insert_field_in_cache(
                model_name,
                WRITE_DATE,
                ids,
                Some(FieldType::DateTime(Utc::now())),
                &Dirty::UpdateDirty,
                &Update::UpdateIfExists,
            );
        }
        if model.fields.contains_key(WRITE_UID) {
            self.cache.insert_field_in_cache(
                model_name,
                WRITE_UID,
                ids,
                self.uid.map(FieldType::Ref),
                &Dirty::UpdateDirty,
                &Update::UpdateIfExists,
            );
        }
        Ok(())
    }

    /// Refuse a value of another type than its field's: a many2one takes one record, a one2many or
    /// a many2many one or several, any other field a value of its own kind.
    ///
    /// Values from a client already arrive typed by their field; this is for those plugin code
    /// builds by hand, which would otherwise fail halfway through a write or at the database.
    pub(super) fn refuse_wrong_kind(
        model_name: &str,
        field: &FinalInternalField,
        value: &Option<FieldType>,
    ) -> Result<()> {
        let Some(value) = value else {
            return Ok(());
        };
        let given = value.kind();
        let fits =
            given == field.kind || (field.kind == FieldKind::Refs && given == FieldKind::Ref);
        if fits {
            return Ok(());
        }
        Err(format!(
            "Field \"{}\" of model \"{model_name}\" holds a {}, not a {given}",
            field.name, field.kind
        )
        .into())
    }

    /// Refuse a key the field's enums do not have, naming those they do.
    fn refuse_unknown_choice(
        &self,
        model_name: &str,
        field: &FinalInternalField,
        value: &Option<FieldType>,
    ) -> Result<()> {
        let (Some(family), Some(FieldType::String(key))) = (field.selection, value) else {
            return Ok(());
        };
        let selections = &self.model_manager.selections;
        if selections.contains(family.family, key) {
            return Ok(());
        }
        let known: Vec<&str> = selections
            .choices(family.family)
            .iter()
            .map(|choice| choice.key.as_str())
            .collect();
        Err(format!(
            "\"{key}\" is not a value of field \"{}\" of model \"{model_name}\": {}",
            field.name,
            known.join(", ")
        )
        .into())
    }

    pub(super) fn save_field_to_cache<Mode: IdMode>(
        &mut self,
        model_name: &str,
        field_name: &str,
        ids: &Mode,
        value: Option<FieldType>,
        update_dirty: &Dirty,
        update_field: &Update,
    ) -> Result<()> {
        if field_name == "id" || ids.is_empty() {
            return Ok(());
        }
        let value = Self::without_empty_references(value);
        // Loading a value from the database also lands here, and changes no rule.
        if matches!(update_dirty, Dirty::UpdateDirty) {
            self.forget_domain_dependents(model_name, field_name);
            self.forget_access_of(model_name, ids.get_ids_ref())?;
            self.forget_shared_of(model_name);
            if self.is_rule_target(model_name, field_name)
                && let Some(FieldType::String(target)) = &value
            {
                self.forget_rules_for(target);
            }
        }
        let is_update_if_exists = matches!(update_field, Update::UpdateIfExists);
        let internal_model = self.model_manager.try_get_model(model_name)?;
        let field_info = internal_model.try_get_internal_field(field_name)?;
        let value = match value {
            Some(FieldType::String(text))
                if text.is_empty()
                    && field_info.compute.is_none()
                    && matches!(update_dirty, Dirty::UpdateDirty) =>
            {
                None
            }
            value => value,
        };
        if matches!(update_dirty, Dirty::UpdateDirty) {
            Self::refuse_automatic(model_name, field_info)?;
            Self::refuse_wrong_kind(model_name, field_info, &value)?;
            self.refuse_empty_required(model_name, field_info, ids.get_ids_ref(), &value)?;
            self.refuse_unknown_choice(model_name, field_info, &value)?;
            self.remember_before_write(model_name, field_info, ids)?;
            if (field_info.is_stored() || field_info.inverse.is_some())
                && !self.is_computing(model_name, field_name)
            {
                self.stamp_written(model_name, ids.get_ids_ref())?;
            }
        }
        if let Some(FieldReference {
            target_model,
            inverse_field,
        }) = &field_info.inverse
        {
            return match inverse_field {
                // Both sides of a many2many live in the same table of pairs, so writing one side
                // cannot be mirrored by writing the other — that would recurse forever. The
                // other side is invalidated instead, and reloaded from the relation on next
                // read. One reload is cheaper than the bookkeeping, and cannot go stale.
                FieldReferenceType::M2M {
                    relation,
                    target_column,
                    ..
                } => {
                    let new_ids: Vec<u32> = match value.clone() {
                        None => Vec::new(),
                        Some(FieldType::Ref(id)) => vec![id],
                        Some(FieldType::Refs(ids)) => ids,
                        other => {
                            return Err(format!(
                                "A many2many takes a list of references, not {other:?}"
                            )
                            .into());
                        }
                    };

                    let old_values =
                        self.retrieve_field_from_cache_or_database(model_name, field_name, ids)?;
                    let mut touched: HashSet<u32> = new_ids.iter().copied().collect();
                    for (_, old) in old_values {
                        if let Some(FieldType::Refs(old_ids)) = old {
                            touched.extend(old_ids);
                        }
                    }

                    let mirror = self.mirror_of_relation(target_model, relation, target_column);
                    let touched: Vec<u32> = touched.into_iter().collect();
                    if let Some(mirror) = &mirror
                        && is_update_if_exists
                    {
                        self.note_maybe_emptied(target_model, mirror, touched.iter().copied())?;
                    }

                    // Dependents are collected under both states. A record that loses its last
                    // link is only reachable through the other side *before* the change; one
                    // that gains a link, only after. Invalidating the mirror alone would say
                    // nothing to either — the one2many path gets this for free by writing the
                    // children's own field, and a many2many has no field to write.
                    self.check_compute_on_field(model_name, field_name, ids.get_ids_ref())?;
                    if let Some(mirror) = &mirror {
                        self.check_compute_on_field(target_model, mirror, &touched)?;
                    }

                    self.cache.insert_field_in_cache(
                        model_name,
                        field_name,
                        ids.get_ids_ref(),
                        Some(FieldType::Refs(new_ids)),
                        update_dirty,
                        update_field,
                    );

                    if let Some(mirror) = &mirror {
                        // Whatever the other side had cached about these records is now wrong.
                        self.cache.invalidate_field(target_model, mirror, &touched);
                        self.check_compute_on_field(target_model, mirror, &touched)?;
                    }
                    self.check_compute_on_field(model_name, field_name, ids.get_ids_ref())?;
                    Ok(())
                }
                FieldReferenceType::O2M { inverse_field } => {
                    let new_ids = match value.clone() {
                        None => HashSet::new(),
                        Some(FieldType::Ref(id)) => HashSet::from([id]),
                        Some(FieldType::Refs(ids)) => ids.iter().copied().collect(),
                        Some(other) => {
                            return Err(format!("A one2many takes records, not {other:?}").into());
                        }
                    };

                    let old_values =
                        self.retrieve_field_from_cache_or_database(model_name, field_name, ids)?;

                    // For removed ids, we can batch the save call
                    let mut ids_removed: Vec<u32> = Vec::new();
                    let mut ids_added: HashMap<u32, Vec<u32>> = HashMap::new();

                    for (i, d) in old_values.into_iter().enumerate() {
                        let old_ids = match d {
                            (_, None) => HashSet::new(),
                            (_, Some(FieldType::Ref(id))) => HashSet::from([id]),
                            (_, Some(FieldType::Refs(ids))) => ids.iter().copied().collect(),
                            (_, Some(other)) => {
                                return Err(format!(
                                    "{model_name}.{field_name} holds {other:?}, not records"
                                )
                                .into());
                            }
                        };

                        ids_removed.extend(old_ids.difference(&new_ids));
                        ids_added.insert(
                            ids.get_ids_ref()[i],
                            new_ids.difference(&old_ids).copied().collect(),
                        );
                    }

                    // Records taken out follow what their many2one says of a deleted record:
                    // `cascade` — or a field owning them — deletes them, `restrict` refuses, and
                    // `set_null` lets them go. One moved to another record of this write stays.
                    let on_delete = self
                        .model_manager
                        .try_get_model(target_model)?
                        .try_get_internal_field(inverse_field)?
                        .on_delete;
                    let moved: HashSet<u32> = ids_added.values().flatten().copied().collect();
                    let mut seen = HashSet::new();
                    ids_removed.retain(|id| !moved.contains(id) && seen.insert(*id));
                    let checked = matches!(update_dirty, Dirty::UpdateDirty);
                    if checked && on_delete == OnDelete::Restrict && !ids_removed.is_empty() {
                        return Err(format!(
                            "{target_model} records cannot be taken out of {model_name}.{field_name}: \
                             their {inverse_field} restricts it"
                        )
                        .into());
                    }
                    let deleted = checked && (field_info.owned || on_delete == OnDelete::Cascade);
                    if !ids_removed.is_empty() && !deleted {
                        self.save_field_to_cache::<MultipleIds>(
                            target_model,
                            inverse_field,
                            &ids_removed.clone().into(),
                            None,
                            update_dirty,
                            update_field,
                        )?;
                    }
                    for (id, ids) in ids_added {
                        self.save_field_to_cache::<MultipleIds>(
                            target_model,
                            inverse_field,
                            &ids.into(),
                            Some(FieldType::Ref(id)),
                            update_dirty,
                            update_field,
                        )?;
                    }
                    if deleted && !ids_removed.is_empty() {
                        self.delete(target_model, &MultipleIds::from(ids_removed))?;
                    }
                    Ok(())
                }
                FieldReferenceType::M2O { inverse_fields } => {
                    let filtered: HashSet<&String> = inverse_fields
                        .iter()
                        .filter(|inverse| {
                            self.model_manager
                                .has_one2many_domain(target_model, inverse)
                        })
                        .collect();

                    let new_id = match value.clone() {
                        None => None,
                        Some(FieldType::Ref(id)) => Some(id),
                        Some(other) => {
                            return Err(format!("A many2one takes a record, not {other:?}").into());
                        }
                    };

                    // For a M2O, we need to verify the old value compared to the new one, and update the related O2M if it's loaded in cache
                    let mut old_values = if is_update_if_exists {
                        self.retrieve_field_from_cache_or_database(model_name, field_name, ids)?
                    } else {
                        self.loaded_field_values(model_name, field_name, ids, &value)
                    };
                    if is_update_if_exists {
                        let left: Vec<u32> = old_values
                            .iter()
                            .filter_map(|(_, old)| match old {
                                Some(FieldType::Ref(old_id)) if Some(*old_id) != new_id => {
                                    Some(*old_id)
                                }
                                _ => None,
                            })
                            .collect();
                        for inverse_field in inverse_fields {
                            self.note_maybe_emptied(target_model, inverse_field, left.clone())?;
                        }
                    }
                    // If we don't have to update loaded fields, remove them from the list
                    let mut ids = ids.get_ids_ref().clone();
                    if !is_update_if_exists {
                        let pos_to_remove = old_values
                            .iter()
                            .enumerate()
                            .filter_map(|(i, (in_cache, _))| if *in_cache { Some(i) } else { None })
                            .collect::<HashSet<_>>();
                        old_values = old_values
                            .into_iter()
                            .enumerate()
                            .filter_map(|(i, data)| {
                                if pos_to_remove.contains(&i) {
                                    None
                                } else {
                                    Some(data)
                                }
                            })
                            .collect();
                        ids = ids
                            .into_iter()
                            .enumerate()
                            .filter_map(|(i, data)| {
                                if pos_to_remove.contains(&i) {
                                    None
                                } else {
                                    Some(data)
                                }
                            })
                            .collect::<Vec<u32>>();
                    }
                    // As those fields could be modified, we need to set them and their dependencies as to_recompute
                    if is_update_if_exists {
                        self.check_compute_on_field(model_name, field_name, &ids)?;
                    }

                    // Now, remove the old id from the lists
                    // Only update if needed
                    let cache_models = self.cache.get_cache_models_mut(target_model);
                    for (i, old_value) in old_values.iter().enumerate() {
                        if let (_, Some(FieldType::Ref(ref_id))) = old_value {
                            // Go to this ref, and remove from the list the current id
                            let current_id = ids[i];
                            let cache_model = cache_models.get_model_mut(*ref_id);
                            let mut fields_to_remove_from_recompute =
                                Vec::with_capacity(inverse_fields.len());
                            // If this target is not in cache, we do nothing
                            if let Some(cache_model) = cache_model {
                                for inverse_field in inverse_fields {
                                    // TODO Pass by a new method in cache to add / remove values from list
                                    let cache_field = cache_model.get_field_mut(inverse_field);
                                    // If this field is not in cache, we do nothing
                                    if let Some(CacheField {
                                        value: Some(FieldType::Refs(vecs)),
                                    }) = cache_field
                                    {
                                        vecs.retain(|id| current_id != *id);
                                        if is_update_if_exists {
                                            fields_to_remove_from_recompute
                                                .push(inverse_field.clone());
                                        }
                                    }
                                }
                            }
                            cache_models.remove_to_recompute(
                                &fields_to_remove_from_recompute
                                    .iter()
                                    .map(|f| f.as_ref())
                                    .collect::<Vec<&str>>(),
                                &[current_id],
                            );
                        }
                    }

                    // Done, now update the id in the cache
                    self.cache.insert_field_in_cache(
                        model_name,
                        field_name,
                        &ids,
                        value.clone(),
                        update_dirty,
                        update_field,
                    );

                    // Finally, add the id to the new list
                    // Only update if needed
                    if let Some(new_id) = new_id {
                        let cache_models = self.cache.get_cache_models_mut(target_model);
                        let mut fields_to_remove_from_recompute =
                            Vec::with_capacity(inverse_fields.len());

                        // We only modify if the target model is present in cache
                        if let Some(cache_model) = cache_models.get_model_mut(new_id) {
                            for inverse_field in inverse_fields {
                                if filtered.contains(inverse_field) {
                                    cache_model.remove_field(inverse_field);
                                    continue;
                                }
                                let cache_field = cache_model.get_field_mut(inverse_field);
                                // If this field is not in cache, we do nothing
                                if let Some(cache_field) = cache_field {
                                    // TODO Pass by a new method in cache to add / remove values from list
                                    if let Some(FieldType::Refs(vecs)) = &mut cache_field.value {
                                        vecs.extend(&ids);
                                    } else {
                                        cache_field.set(FieldType::Refs(ids.to_vec()));
                                    }
                                    if is_update_if_exists {
                                        fields_to_remove_from_recompute.push(inverse_field.clone());
                                    }
                                }
                            }
                        }
                        cache_models.remove_to_recompute(
                            &fields_to_remove_from_recompute
                                .iter()
                                .map(|f| f.as_ref())
                                .collect::<Vec<&str>>(),
                            &[new_id],
                        );
                    }

                    let mut old_values_ids: Vec<u32> = Vec::new();

                    for (_, old_value) in old_values {
                        if let Some(old_value) = old_value {
                            match old_value {
                                FieldType::Ref(id) => old_values_ids.push(id),
                                other => {
                                    return Err(format!(
                                        "{model_name}.{field_name} holds {other:?}, not a record"
                                    )
                                    .into());
                                }
                            }
                        }
                    }

                    // Now that we have the correct value, set again dependencies of this field as to_recompute
                    if is_update_if_exists {
                        self.check_compute_on_field(model_name, field_name, &ids)?;
                    }

                    Ok(())
                }
            };
        }

        let modified_ids = self.cache.insert_field_in_cache(
            model_name,
            field_name,
            ids.get_ids_ref(),
            value.clone(),
            update_dirty,
            update_field,
        );
        if is_update_if_exists {
            self.check_compute_on_field(model_name, field_name, &modified_ids)?;
        }
        Ok(())
    }
}
