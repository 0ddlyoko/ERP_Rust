//! Writing field values into the cache, keeping relational mirrors coherent.
use super::*;

impl<'mm> Environment<'mm> {
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
    /// Return a vector sorted by given ids of tuple.
    /// First element is true if it's from the cache, or false if it's from the database.
    /// Second element is the value
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
            if let Some(model) = cache_model.get_model(id) {
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
                inverse_field: FieldReferenceType::O2M { inverse_field },
            }) = &field_info.inverse
            {
                // TODO Check if target field is stored or not
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
                        _ => panic!("Only UInteger should return here, and not {field_value}"),
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
                // Load default value
                for id in ids_not_in_cache {
                    let default_value = match field_info.default_value.clone() {
                        FieldType::Ref(id) => {
                            if id == 0 {
                                None
                            } else {
                                Some(FieldType::Ref(id))
                            }
                        }
                        FieldType::Refs(ids) => {
                            if ids.is_empty() {
                                None
                            } else {
                                Some(FieldType::Refs(ids))
                            }
                        }
                        other => Some(other),
                    };
                    map_result.insert(id, (false, default_value));
                }
            }
        }

        let mut result: Vec<(bool, Option<FieldType>)> = Vec::with_capacity(size);
        for id in ids.get_ids_ref() {
            result.push(map_result.remove(id).unwrap());
        }

        Ok(result)
    }

    /// Save given field to cache.
    ///
    /// This method ensure M2O & O2M are correctly linked in cache (if those fields are loaded)
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
        let is_update_if_exists = matches!(update_field, Update::UpdateIfExists);
        let internal_model = self.model_manager.try_get_model(model_name)?;
        let field_info = internal_model.try_get_internal_field(field_name)?;
        if let Some(FieldReference {
            target_model,
            inverse_field,
        }) = &field_info.inverse
        {
            // M2O or O2M
            return match inverse_field {
                // For now, M2M is not handled, so it's a O2M (as there is an inverse field)
                // Call this method for each field that has been modified.
                // To be able to do this, we need to retrieve old data, and compare it with the new data
                //FieldReferenceType::M2M { ... } => { ... }
                FieldReferenceType::O2M { inverse_field } => {
                    let new_ids = match value.clone() {
                        None => HashSet::new(),
                        Some(FieldType::Ref(id)) => HashSet::from([id]),
                        Some(FieldType::Refs(ids)) => ids.iter().copied().collect(),
                        _ => panic!(
                            "Only Ref and Refs are accepted field type, and not {:?}",
                            value
                        ),
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
                            _ => panic!(
                                "Only Ref and Refs are accepted field type, and not {:?}",
                                value
                            ),
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
                        _ => panic!("Only Ref is accepted field type, and not {:?}", value),
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
                    // TODO Pass by another method, so that it also automatically triggers computes
                    // Only update if needed
                    let cache_models = self.cache.get_cache_models_mut(target_model);
                    for (i, old_value) in old_values.iter().enumerate() {
                        if let (_, Some(FieldType::Ref(ref_id))) = old_value {
                            // Go to this ref, and remove from the list the current id
                            let current_id = ids[i];
                            let cache_model = cache_models.get_model_mut(ref_id);
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
                    // TODO Pass by another method, so that it also automatically triggers computes
                    // Only update if needed
                    if let Some(new_id) = new_id {
                        let cache_models = self.cache.get_cache_models_mut(target_model);
                        let mut fields_to_remove_from_recompute =
                            Vec::with_capacity(inverse_fields.len());

                        // We only modify if the target model is present in cache
                        if let Some(cache_model) = cache_models.get_model_mut(&new_id) {
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
                                _ => panic!(
                                    "Only Ref is accepted field type, and not {:?}",
                                    old_value
                                ),
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
