//! Scheduling and running computed fields.
use super::*;

impl<'mm> Environment<'mm> {
    /// Work out a computed field for every record that already exists, and write it down.
    ///
    /// For the moment a field becomes stored. Until then its value was produced on each read and
    /// never kept; the column that has just appeared beside it is empty, and nothing else would
    /// ever fill it — a read finds the column, takes what is in it, and does not compute.
    ///
    /// Computes and writes here rather than marking the records and leaving it to the next
    /// flush: whoever calls this must not have to close the environment for it to have happened.
    ///
    /// A no-op on a model with no records, which is what a plugin being installed looks like.
    pub fn fill_stored_field(&mut self, model_name: &str, field_name: &str) -> Result<()> {
        let ids =
            self.search_ids_unchecked(model_name, &SearchType::Nothing, &SearchOptions::default())?;
        if ids.is_empty() {
            // A model with nothing in it is what installing a plugin looks like, and every
            // stored computed field goes through here then. Saying so would be noise.
            return Ok(());
        }
        // Said before the work rather than after: on a table of any size this is the part of a
        // startup that takes time, and a line that only appears once it is over explains a wait
        // that has already happened.
        tracing::info!(
            model = %model_name,
            field = %field_name,
            records = ids.len(),
            "Filling a column that has just appeared, for records that predate it"
        );
        let started = std::time::Instant::now();
        let ids: MultipleIds = ids.into();
        self.call_compute_method(model_name, &ids, &[field_name])?;
        self.save_fields_to_db(model_name, &[field_name])?;
        tracing::info!(
            model = %model_name,
            field = %field_name,
            took = ?started.elapsed(),
            "Filled"
        );
        Ok(())
    }

    /// Method called when a field has changed, and will set as to recompute all fields that needs to be recomputed
    ///
    /// We shouldn't call this method from a O2M, as a O2M field shouldn't have any dependencies
    pub(super) fn check_compute_on_field(
        &mut self,
        model_name: &str,
        field_name: &str,
        ids: &[u32],
    ) -> Result<()> {
        let internal_model = self.model_manager.get_model(model_name);
        let internal_field = internal_model.get_internal_field(field_name);
        let all_depends = &internal_field.depends;
        for depend_path in all_depends {
            let mut current_model = internal_model;
            let mut current_field = internal_field;
            let mut current_ids = ids.to_vec();
            for depend in depend_path {
                match depend {
                    FieldDepend::SameModel { field_name } => {
                        current_field = current_model.get_internal_field(field_name);
                    }
                    FieldDepend::AnotherModel {
                        target_model,
                        target_field,
                    } => {
                        // O2M field, we need to perform a search on this field, so we need to compute it
                        self.save_fields_to_db(target_model, &[target_field])?;
                        let (virtual_ids, real_ids): (Vec<u32>, Vec<u32>) = current_ids
                            .iter()
                            .partition(|id| onchange::is_virtual(**id));
                        // Only the cache knows what points to a virtual record.
                        let mut found =
                            self.virtual_pointing_to(target_model, target_field, &virtual_ids);
                        if !real_ids.is_empty() {
                            let database_result = self.database.search(
                                target_model,
                                &[target_field],
                                &make_domain!([(target_field, "=", real_ids)]),
                                self.model_manager,
                                &SearchOptions::default(),
                            )?;
                            found.extend(database_result.into_iter().map(|(id, _)| id));
                        }
                        current_ids = found;
                        current_model = self.model_manager.get_model(target_model);
                    }
                    FieldDepend::CurrentFieldAnotherModel {
                        target_model,
                        field_name,
                    } => {
                        // M2O field, we can get the value of this field and continue
                        let all_ids = self.get_fields_value_unchecked::<MultipleIds>(
                            &current_model.name,
                            field_name,
                            &current_ids.clone().into(),
                        )?;
                        current_ids = all_ids
                            .into_iter()
                            .flatten()
                            .flat_map(|id| match id {
                                FieldType::Ref(r) => vec![*r],
                                FieldType::Refs(r) => r.clone(),
                                _ => panic!(
                                    "Only Ref & Refs are accepted field type, and not {:?}",
                                    id
                                ),
                            })
                            .collect::<Vec<_>>();
                        current_model = self.model_manager.get_model(target_model);
                    }
                }
            }
            // We found ids to recompute, set them as to_recompute
            if !current_ids.is_empty() {
                let already = self
                    .cache
                    .get_cache_models(&current_model.name)
                    .to_recompute
                    .get(&current_field.name)
                    .cloned()
                    .unwrap_or_default();
                let newly: Vec<u32> = current_ids
                    .iter()
                    .copied()
                    .filter(|id| !already.contains(id))
                    .collect();
                self.cache.add_ids_to_recompute(
                    &current_model.name,
                    &[&current_field.name],
                    &current_ids,
                );
                // What depends on a field about to change is about to change too: flagged now,
                // it is worked out before being read, rather than read as it was.
                if !newly.is_empty() {
                    let (model_name, field_name) =
                        (current_model.name.clone(), current_field.name.clone());
                    self.check_compute_on_field(&model_name, &field_name, &newly)?;
                }
            }
        }
        Ok(())
    }
    /// Call computed method on all stored fields that need to be computed for given model
    pub(super) fn call_computed_method_on_all_fields(&mut self, model_name: &str) -> Result<()> {
        let model = self.model_manager.get_model(model_name);
        for i in 0..=MAX_RECOMPUTE_ROUNDS {
            let cache_models = self.cache.get_cache_models(model_name);
            // Only kept values are settled here; one worked out on each read waits for its next read.
            if let Some((key, value)) = cache_models
                .to_recompute
                .iter()
                .find(|(key, _value)| model.is_kept(key))
            {
                let ids: MultipleIds =
                    MultipleIds::from(value.iter().copied().collect::<Vec<u32>>());
                self.call_compute_method(model_name, &ids, &[key.clone().as_str()])?;
            }
            let cache_models = self.cache.get_cache_models(model_name);
            // Only kept values are settled here; one worked out on each read waits for its next read.
            if !cache_models
                .to_recompute
                .iter()
                .any(|(key, value)| !value.is_empty() && model.is_kept(key))
            {
                break;
            }
            if i == MAX_RECOMPUTE_ROUNDS {
                return Err(MaximumRecursionDepthCompute {
                    model_name: model_name.to_string(),
                    fields_name: cache_models.to_recompute.keys().cloned().collect(),
                    ids: cache_models
                        .to_recompute
                        .values()
                        .flatten()
                        .copied()
                        .collect::<Vec<u32>>(),
                }
                .into());
            }
        }

        Ok(())
    }

    /// Call computed method on computed & non-computed fields that need to be computed for given model
    pub(super) fn call_computed_method_on_fields(
        &mut self,
        model_name: &str,
        fields: &[&str],
    ) -> Result<()> {
        for i in 0..=MAX_RECOMPUTE_ROUNDS {
            let cache_models = self.cache.get_cache_models(model_name);
            if let Some((key, value)) = cache_models
                .to_recompute
                .iter()
                .find(|(key, _value)| fields.contains(&key.as_str()))
            {
                let ids: MultipleIds =
                    MultipleIds::from(value.iter().copied().collect::<Vec<u32>>());
                self.call_compute_method(model_name, &ids, &[key.clone().as_str()])?;
            }
            let cache_models = self.cache.get_cache_models(model_name);
            if !cache_models
                .to_recompute
                .iter()
                .any(|(key, value)| !value.is_empty() && fields.contains(&key.as_str()))
            {
                break;
            }
            if i == MAX_RECOMPUTE_ROUNDS {
                return Err(MaximumRecursionDepthCompute {
                    model_name: model_name.to_string(),
                    fields_name: cache_models.to_recompute.keys().cloned().collect(),
                    ids: cache_models
                        .to_recompute
                        .values()
                        .flatten()
                        .copied()
                        .collect::<Vec<u32>>(),
                }
                .into());
            }
        }

        Ok(())
    }

    /// Call computed method on non-stored fields that need to be computed for given model, for given ids
    pub(super) fn call_computed_method_on_ids(
        &mut self,
        model_name: &str,
        ids: &[u32],
    ) -> Result<()> {
        let model = self.model_manager.get_model(model_name);
        for i in 0..=MAX_RECOMPUTE_ROUNDS {
            let cache_models = self.cache.get_cache_models(model_name);
            if let Some((field, value)) =
                cache_models.to_recompute.iter().find_map(|(field, value)| {
                    if model.is_kept(field) {
                        let result = value
                            .iter()
                            .filter(|id| ids.contains(id))
                            .collect::<Vec<&u32>>();
                        if result.is_empty() {
                            None
                        } else {
                            Some((field, result))
                        }
                    } else {
                        None
                    }
                })
            {
                let ids: MultipleIds =
                    MultipleIds::from(value.into_iter().copied().collect::<Vec<u32>>());
                self.call_compute_method(model_name, &ids, &[field.clone().as_str()])?;
            }
            let cache_models = self.cache.get_cache_models(model_name);
            if !cache_models.to_recompute.iter().any(|(field, value)| {
                model.is_kept(field) && value.iter().any(|id| ids.contains(id))
            }) {
                break;
            }
            if i == MAX_RECOMPUTE_ROUNDS {
                return Err(MaximumRecursionDepthCompute {
                    model_name: model_name.to_string(),
                    fields_name: cache_models.to_recompute.keys().cloned().collect(),
                    ids: cache_models
                        .to_recompute
                        .values()
                        .flatten()
                        .copied()
                        .collect::<Vec<u32>>(),
                }
                .into());
            }
        }

        Ok(())
    }

    /// Call computed methods of given fields of the given model for given ids
    ///
    /// A compute is an overridable method taking no arguments and returning nothing, so this goes
    /// through the same dispatch as any other: the most derived implementation runs first, and
    /// reaches the ones it overrides through `super`.
    ///
    /// It runs as the caller, whose rights every read and write inside it is held to, except the
    /// value of the stored fields it computes: a stored value is the same for every reader, so
    /// saving it cannot depend on who triggered it.
    pub(super) fn call_compute_method<Mode: IdMode>(
        &mut self,
        model_name: &str,
        ids: &Mode,
        fields: &[&str],
    ) -> Result<()> {
        let final_internal_model = self.model_manager.get_model(model_name);
        let ids: MultipleIds = ids.get_ids_ref().into();
        // One method may fill several fields, and asking for both would otherwise run it twice.
        let mut plan: Vec<(&'mm str, MultipleIds)> = Vec::new();
        for field in fields {
            if let Some(method) = final_internal_model.compute_method(field)
                && !plan.iter().any(|(planned, _)| *planned == method)
            {
                plan.push((method, ids.clone()));
            }
        }
        self.call_compute_plan(model_name, plan)
    }

    /// Run compute methods, each on its own records, as one logical operation.
    ///
    /// One savepoint for the whole plan: a link that fails must take the ones before it with it,
    /// and a savepoint copies the cache, which is worth paying once rather than once per method.
    pub(super) fn call_compute_plan(
        &mut self,
        model_name: &str,
        plan: Vec<(&'mm str, MultipleIds)>,
    ) -> Result<()> {
        if plan.is_empty() {
            return Ok(());
        }
        let final_internal_model = self.model_manager.get_model(model_name);
        let depth = self.computing.len();
        for (field_name, field) in &final_internal_model.fields {
            if field.is_kept()
                && let Some(method) = final_internal_model.compute_method(field_name)
                && plan.iter().any(|(planned, _)| *planned == method)
            {
                self.computing
                    .push((model_name.to_string(), field_name.clone()));
            }
        }
        let result = self.savepoint(move |env| {
            for (method, ids) in &plan {
                env.call_method::<(), ()>(model_name, method, ids, &())?;
            }
            Ok(())
        });
        self.computing.truncate(depth);
        result
    }
}
