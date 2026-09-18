//! Reading records: browsing, searching and filling the cache on demand.
use super::*;

impl<'mm> Environment<'mm> {
    /// Returns an instance of given model for a specific id
    ///
    /// Do not check if given id is valid id, or is present in the cache
    ///
    /// Do not load given id to the cache
    pub fn get_record<M, Mode: IdMode>(&self, id: Mode) -> M
    where
        M: Model<Mode>,
    {
        M::create_instance(id)
    }

    /// Search given domain for given model, and return an instance of given model if found
    ///
    /// If not found, return an empty recordset
    ///
    /// This method does not load in cache any data related to the model.
    /// It only performs a search, and return the given ids.
    ///
    /// Before performing any search, save any data related to any field given in the domain.
    pub fn search<M>(&mut self, domain: &SearchType) -> Result<M>
    where
        M: Model<MultipleIds>,
    {
        // TODO Add limit
        let model_name = M::_get_model_name();
        self.save_domain_fields_to_db(model_name, domain)?;

        let ids = self
            .database
            .browse(M::_get_model_name(), domain, self.model_manager)?;
        Ok(M::create_instance(ids.into()))
    }

    /// Get the value of given field for given id.
    ///
    /// If field is not in cache, load it
    ///
    /// If field needs to be computed, compute it
    pub(crate) fn get_field_value<'a>(
        &'a mut self,
        model_name: &str,
        field_name: &str,
        id: &SingleId,
    ) -> Result<Option<&'a FieldType>> {
        self.ensure_fields_in_cache(model_name, field_name, id)?;

        // TODO In case of O2M / M2M, cache could be invalid.

        // Now, everything should be good
        Ok(self
            .cache
            .get_field_from_cache(model_name, field_name, &id.get_id()))
    }

    pub(crate) fn get_fields_value<Mode: IdMode>(
        &mut self,
        model_name: &str,
        field_name: &str,
        ids: &Mode,
    ) -> Result<Vec<Option<&FieldType>>> {
        self.ensure_fields_in_cache(model_name, field_name, ids)?;

        // TODO In case of O2M / M2M, cache could be invalid.

        // Now, everything should be good
        Ok(ids
            .get_ids_ref()
            .iter()
            .map(|id| self.cache.get_field_from_cache(model_name, field_name, id))
            .collect())
    }

    /// Ensure given field is in cache for given ids
    ///
    /// If some ids are invalid or need to be loaded, load them (or compute them if needed)
    ///
    /// If given field_name is a O2M, load it along with its M2O
    pub(super) fn ensure_fields_in_cache<Mode: IdMode>(
        &mut self,
        model_name: &str,
        field_name: &str,
        ids: &Mode,
    ) -> Result<()> {
        // TODO Allow to pass a list of fields
        let ids_ref = ids.get_ids_ref();
        let mut ids_not_in_cache: MultipleIds = self
            .cache
            .get_ids_not_in_cache(model_name, field_name, ids_ref)
            .into();
        let ids_to_recompute: MultipleIds = self
            .cache
            .get_ids_to_recompute(model_name, field_name, ids_ref)
            .into();

        if !ids_not_in_cache.is_empty() || !ids_to_recompute.is_empty() {
            // Load given fields
            let model_info = self.model_manager.get_model(model_name);
            let field_info = model_info.get_internal_field(field_name);
            let is_computed_method = field_info.compute.is_some();
            if !ids_to_recompute.is_empty() && is_computed_method {
                self.call_compute_method(model_name, &ids_to_recompute, &[field_name])?;
                // Here, we can assume that ids in ids_to_recompute are in cache
                ids_not_in_cache -= ids_to_recompute;
            }
            if ids_not_in_cache.is_empty() {
                return Ok(());
            }
            if model_info.is_stored(field_name) {
                // This is a stored field, load it along with all the other stored fields to avoid
                //  multiple database calls
                let fields_to_load = model_info.get_stored_fields();
                // TODO Shouldn't we save those fields (if they are dirty in cache) to the database ?
                self.load_records_fields_from_db(model_name, &ids_not_in_cache, &fields_to_load)?;
            } else if is_computed_method {
                // TODO Check if a O2M computed field is correctly handled here
                // This could be a computed one. Call it
                self.call_compute_method(model_name, &ids_not_in_cache, &[field_name])?;
            } else if let Some(FieldReference {
                target_model,
                inverse_field: FieldReferenceType::O2M { inverse_field },
            }) = &field_info.inverse
            {
                // O2M, save data to the database, and then load the field with related M2O
                // TODO Add search_group(...)
                self.save_fields_to_db(target_model, &[inverse_field])?;
                // Load from database
                let mut result: HashMap<u32, Vec<u32>> =
                    HashMap::with_capacity(ids_not_in_cache.get_ids_ref().len());
                for id in &ids_not_in_cache {
                    result.insert(id.get_id(), vec![]);
                }

                let database_result = self.database.search(
                    target_model,
                    &[inverse_field],
                    &make_domain!([(inverse_field, "=", ids_not_in_cache)]),
                    self.model_manager,
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

                for (id, ids) in result {
                    let field_value = if ids.is_empty() {
                        None
                    } else {
                        Some(FieldType::Refs(ids))
                    };
                    // Save the O2M to the cache. This will also save the M2O thanks to the save_field_to_cache method
                    self.cache.insert_field_in_cache(
                        model_name,
                        field_name,
                        &[id],
                        field_value,
                        &Dirty::UpdateDirty,
                        &Update::UpdateIfExists,
                    );
                }
            } else {
                // State where field is not computed nor stored. This behavior is unexpected
                // TODO Find what to do in this case
            }
        }

        Ok(())
    }
}
