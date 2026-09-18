//! Creating records and applying default values.
use super::*;

impl<'mm> Environment<'mm> {
    /// Create a new record for a specific model and a given list of fields
    pub fn create_new_record_from_map<M>(&mut self, data: MapOfFields) -> Result<M>
    where
        M: Model<SingleId>,
    {
        let model_name = M::_get_model_name();
        let ids = self._create_new_records(model_name, vec![data])?;
        let id = ids.get_id_at(0);
        Ok(self.get_record::<M, SingleId>(id.into()))
    }

    /// Create new records for a specific model and multiple lists of fields
    pub fn create_new_records_from_maps<M>(&mut self, data: Vec<MapOfFields>) -> Result<M>
    where
        M: Model<MultipleIds>,
    {
        let model_name = M::_get_model_name();
        let ids = self._create_new_records(model_name, data)?;
        Ok(self.get_record::<M, MultipleIds>(ids))
    }

    /// Create records from untyped field maps, addressing the model by name.
    ///
    /// The entry point for callers that only hold a model name at runtime; the typed
    /// [`Environment::create_new_record_from_map`] resolves the name from `M` and lands here.
    pub fn create_records(
        &mut self,
        model_name: &str,
        data: Vec<MapOfFields>,
    ) -> Result<MultipleIds> {
        self._create_new_records(model_name, data)
    }

    pub(super) fn _create_new_records(
        &mut self,
        model_name: &str,
        mut data: Vec<MapOfFields>,
    ) -> Result<MultipleIds> {
        let final_model = self.model_manager.get_model(model_name);
        let mut missing_fields_lst = Vec::new();

        // Add missing fields
        for d in data.iter_mut() {
            let missing_fields = self.fill_default_values_on_map(model_name, d);
            missing_fields_lst.push(missing_fields)
        }
        // Create a list that will only contain stored fields (to save in db)
        let mut stored_data = data.clone();
        stored_data.iter_mut().for_each(|map| {
            map.fields
                .retain(|field, _value| final_model.is_stored(field));
        });

        let stored_data_ref: Vec<&MapOfFields> = stored_data.iter().collect();
        let ids = self.insert_data_to_db(model_name, &stored_data_ref)?;

        missing_fields_lst.reverse();
        // Once added in database, we should set all stored computed methods as to_recompute
        for id in &ids {
            if let Some(missing_fields) = missing_fields_lst.pop().flatten() {
                let final_internal_model = self.model_manager.get_model(model_name);
                let computed_fields_string = missing_fields
                    .into_iter()
                    .filter(|f| final_internal_model.is_computed_field(f))
                    .collect::<Vec<&str>>();
                self.cache
                    .add_ids_to_recompute(model_name, &computed_fields_string, &[*id]);
            }
        }
        // Now, we can update cache for given fields
        for (i, d) in data.into_iter().enumerate() {
            let id = ids[i];
            for (field_name, value) in d.fields {
                if final_model.is_stored(&field_name) {
                    // If it's stored, it's already in the database. Load it in cache
                    self.ensure_fields_in_cache::<SingleId>(model_name, &field_name, &id.into())?;
                } else {
                    self.save_option_to_cache::<SingleId, _>(
                        model_name,
                        &field_name,
                        &id.into(),
                        value,
                    )?;
                }
            }
        }

        // Finally, for all stored fields, call method "check_compute_on_field" to ensure fields
        //  that are dependencies of those one are correctly calculated
        // We don't need to call this method for non-stored fields, as those fields are added in
        //  cache (see few lines before with the "self.save_option_to_cache(...)" method)
        for (field_name, final_field) in &final_model.fields {
            if final_field.is_stored() {
                self.check_compute_on_field(&final_model.name, field_name, &ids)?;
            }
        }

        Ok(ids.into())
    }

    /// Add default values for a given model on given data
    pub fn fill_default_values_on_map(
        &self,
        model_name: &str,
        data: &mut MapOfFields,
    ) -> Option<Vec<&'mm str>> {
        let final_internal_model = self.model_manager.get_model(model_name);
        let missing_fields_to_load = final_internal_model.get_missing_fields(data.get_keys());
        for missing_field_to_load in &missing_fields_to_load {
            let default_value = final_internal_model.get_default_value(missing_field_to_load);
            if matches!(default_value, FieldType::Ref(0)) {
                // Do not insert a reference if it's 0 (the default value)
                // TODO Do not handle this here, but add a real default value to "None"
                data.insert_none(missing_field_to_load);
            } else {
                data.insert_field_type(missing_field_to_load, default_value);
            }
        }
        Some(missing_fields_to_load)
    }
}
