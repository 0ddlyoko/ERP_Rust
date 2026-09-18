//! Scheduling and running computed fields.
use super::*;

impl<'mm> Environment<'mm> {
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
                        // Now, make a search request
                        let database_result = self.database.search(
                            target_model,
                            &[target_field],
                            &make_domain!([(target_field, "=", current_ids.clone())]),
                            self.model_manager,
                        )?;
                        current_ids = database_result
                            .into_iter()
                            .map(|(id, _)| id)
                            .collect::<Vec<_>>();
                        current_model = self.model_manager.get_model(target_model);
                    }
                    FieldDepend::CurrentFieldAnotherModel {
                        target_model,
                        field_name,
                    } => {
                        // M2O field, we can get the value of this field and continue
                        let all_ids = self.get_fields_value::<MultipleIds>(
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
                self.cache.add_ids_to_recompute(
                    &current_model.name,
                    &[&current_field.name],
                    &current_ids,
                );
            }
        }
        Ok(())
    }
    /// Call computed method on all stored fields that need to be computed for given model
    pub(super) fn call_computed_method_on_all_fields(&mut self, model_name: &str) -> Result<()> {
        let model = self.model_manager.get_model(model_name);
        for i in 0..=MAX_NUMBER_OF_RECURSION {
            let cache_models = self.cache.get_cache_models(model_name);
            // TODO The filter should not be useful here, as we should add a way to not set as to_recompute non-computed fields
            if let Some((key, value)) = cache_models
                .to_recompute
                .iter()
                .find(|(key, _value)| model.is_stored(key))
            {
                let ids: MultipleIds = MultipleIds {
                    ids: value.iter().copied().collect(),
                };
                self.call_compute_method(model_name, &ids, &[key.clone().as_str()])?;
            }
            let cache_models = self.cache.get_cache_models(model_name);
            // TODO The filter should not be useful here, as we should add a way to not set as to_recompute non-computed fields
            if !cache_models
                .to_recompute
                .iter()
                .any(|(key, value)| !value.is_empty() && model.is_stored(key))
            {
                break;
            }
            if i == MAX_NUMBER_OF_RECURSION {
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
        for i in 0..=MAX_NUMBER_OF_RECURSION {
            let cache_models = self.cache.get_cache_models(model_name);
            if let Some((key, value)) = cache_models
                .to_recompute
                .iter()
                .find(|(key, _value)| fields.contains(&key.as_str()))
            {
                let ids: MultipleIds = MultipleIds {
                    ids: value.iter().copied().collect(),
                };
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
            if i == MAX_NUMBER_OF_RECURSION {
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
        for i in 0..=MAX_NUMBER_OF_RECURSION {
            let cache_models = self.cache.get_cache_models(model_name);
            if let Some((field, value)) =
                cache_models.to_recompute.iter().find_map(|(field, value)| {
                    if model.is_stored(field) {
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
                let ids: MultipleIds = MultipleIds {
                    ids: value.into_iter().copied().collect(),
                };
                self.call_compute_method(model_name, &ids, &[field.clone().as_str()])?;
            }
            let cache_models = self.cache.get_cache_models(model_name);
            if !cache_models.to_recompute.iter().any(|(field, value)| {
                model.is_stored(field) && value.iter().any(|id| ids.contains(id))
            }) {
                break;
            }
            if i == MAX_NUMBER_OF_RECURSION {
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
    pub(super) fn call_compute_method<Mode: IdMode>(
        &mut self,
        model_name: &str,
        ids: &Mode,
        fields: &[&str],
    ) -> Result<()> {
        let final_internal_model = self.model_manager.get_model(model_name);
        self.savepoint(move |env| {
            for field in fields {
                if let Some(computed_field) = final_internal_model.get_computed_field(field) {
                    // TODO Try to find a way to not clone the id
                    computed_field.call_computed_method(field, ids.get_ids_ref().into(), env)?;
                }
            }
            Ok(())
        })
    }
}
