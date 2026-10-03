//! Writing field values into the cache, keeping relational mirrors coherent.
use super::*;
use crate::access::Operation;
use crate::errors::MissingRecords;
use crate::model::{WRITE_DATE, WRITE_UID};
use chrono::Utc;
use erp_internal_types::FinalInternalField;
use erp_types::field::FieldKind;

impl<'mm> Environment<'mm> {
    /// Write field values onto records, addressing the model and its fields by name.
    ///
    /// The counterpart of [`Environment::read`] for callers that only hold names at runtime.
    /// Values go through the same path as the generated setters, so relational mirrors stay
    /// coherent and dependent computes are flagged.
    ///
    /// Refused as a whole unless the caller may write every one of the records.
    pub fn write<Mode: IdMode>(
        &mut self,
        model_name: &str,
        ids: &Mode,
        values: MapOfFields,
    ) -> Result<()> {
        let fields: Vec<&str> = values.fields.keys().map(String::as_str).collect();
        self.check_access(model_name, Operation::Write, ids.get_ids_ref(), &fields)?;
        for (field_name, value) in values.fields {
            self.save_field_to_cache(
                model_name,
                &field_name,
                ids,
                value,
                &Dirty::UpdateDirty,
                &Update::UpdateIfExists,
            )?;
        }
        Ok(())
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

    /// Write one field, as a generated setter does.
    ///
    /// Refused unless the caller may write every one of the records, or the field is a stored one
    /// its compute is filling in right now.
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
        if !self.is_computing(model_name, field_name) {
            self.check_access(
                model_name,
                Operation::Write,
                ids.get_ids_ref(),
                &[field_name],
            )?;
        }
        self.save_option_to_cache_unchecked(model_name, field_name, ids, value)
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
                inverse_field: FieldReferenceType::M2M { relation, .. },
            }) = &field_info.inverse
            {
                // A many2many has no column of its own; its pairs come from the relation table.
                // Both sides may still be holding unwritten changes, so they reach the table
                // first — the same precaution the one2many path takes.
                self.save_relations_to_db(model_name, &[field_name])?;
                if let Some(mirror) = self.mirror_of_relation(target_model, relation) {
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

                let database_result = self.database.search(
                    target_model,
                    &[inverse_field],
                    &make_domain!([(inverse_field, "=", ids_not_in_cache)]),
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
                    let field_value = if ids.is_empty() {
                        None
                    } else {
                        Some(FieldType::Refs(ids))
                    };
                    map_result.insert(id, (false, field_value));
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

    /// Save given field to cache.
    ///
    /// This method ensure M2O & O2M are correctly linked in cache (if those fields are loaded)
    /// Field on `model_name` that is the other end of a relation table.
    ///
    /// The two sides of a many2many name the same table independently, so the pairing is found
    /// by matching on it rather than being declared twice.
    pub(super) fn mirror_of_relation(&self, model_name: &str, relation: &str) -> Option<String> {
        let model = self.model_manager.try_get_model(model_name).ok()?;
        model.field_of_relation(relation).map(str::to_string)
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
        if field_name == "id" {
            return Ok(());
        }
        // Loading a value from the database also lands here, and changes no rule.
        if matches!(update_dirty, Dirty::UpdateDirty) {
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
        if matches!(update_dirty, Dirty::UpdateDirty) {
            Self::refuse_automatic(model_name, field_info)?;
            Self::refuse_wrong_kind(model_name, field_info, &value)?;
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
                FieldReferenceType::M2M { relation, .. } => {
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

                    let mirror = self.mirror_of_relation(target_model, relation);
                    let touched: Vec<u32> = touched.into_iter().collect();

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

                    if !ids_removed.is_empty() {
                        self.save_field_to_cache::<MultipleIds>(
                            target_model,
                            inverse_field,
                            &ids_removed.into(),
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
                    Ok(())
                }
                FieldReferenceType::M2O { inverse_fields } => {
                    // TODO Later, when we will be able to create a O2M linked to a M2O but with a domain, we need to adapt this code to filter it

                    let new_id = match value.clone() {
                        None => None,
                        Some(FieldType::Ref(id)) => Some(id),
                        Some(other) => {
                            return Err(format!("A many2one takes a record, not {other:?}").into());
                        }
                    };

                    // For a M2O, we need to verify the old value compared to the new one, and update the related O2M if it's loaded in cache
                    // First, retrieve data from cache (or database) without loading it again
                    let mut old_values =
                        self.retrieve_field_from_cache_or_database(model_name, field_name, ids)?;
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
