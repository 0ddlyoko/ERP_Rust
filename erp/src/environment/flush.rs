//! Moving records between the cache and the database.
use super::*;

impl<'mm> Environment<'mm> {
    /// Load fields of given records from the database to the cache.
    ///
    /// If fields are already loaded, they will still be retrieved from the database but not updated
    pub(super) fn load_records_fields_from_db<Mode: IdMode>(
        &mut self,
        model_name: &str,
        ids: &Mode,
        fields: &[&str],
    ) -> Result<()> {
        let ids_to_load: MultipleIds = ids.get_ids_ref().into();
        let fields_from_db = self.get_fields_from_db(model_name, &ids_to_load, fields);
        match fields_from_db {
            Ok(values) => {
                for (id, map_of_fields) in values {
                    for (field_name, field_value) in map_of_fields.fields {
                        self.save_field_to_cache(
                            model_name,
                            &field_name,
                            &id,
                            field_value,
                            &Dirty::NotUpdateDirty,
                            &Update::NotUpdateIfExists,
                        )?;
                    }
                }
                Ok(())
            }
            Err(err) => Err(err),
        }
    }

    pub fn get_fields_to_save(
        &self,
        model_name: &str,
        fields: &Vec<&LeftTuple>,
    ) -> Result<HashMap<&'mm str, Vec<&'mm str>>> {
        let mut fields_to_save: HashMap<&str, HashSet<&str>> = HashMap::new();
        let model = self.model_manager.try_get_model(model_name)?;
        for field in fields {
            let mut current_model = model;
            for elem in &field.path {
                // The primary key is a real column that no model declares, and it is never
                // dirty. Both backends already answer a domain on it; without this they never
                // get the chance, because looking it up in the registry fails first.
                if elem == "id" {
                    continue;
                }
                let final_field = current_model.try_get_internal_field(elem)?;
                let is_stored = final_field.is_kept();
                if is_stored {
                    // If stored field, we need to save it to the database
                    fields_to_save
                        .entry(&current_model.name)
                        .or_default()
                        .insert(&final_field.name);
                }
                if let Some(FieldReference {
                    target_model,
                    inverse_field,
                }) = &final_field.inverse
                {
                    // Get target model to continue the process.
                    // Also, if this field (or the target one) is a stored field, save it
                    let target_model = self.model_manager.get_model(target_model);
                    current_model = target_model;
                    if !is_stored && let FieldReferenceType::O2M { inverse_field } = inverse_field {
                        // If there is an inverse field, and it's a stored field, save it
                        let target_field = target_model.get_internal_field(inverse_field);
                        if target_field.is_stored() {
                            fields_to_save
                                .entry(&target_model.name)
                                .or_default()
                                .insert(&target_field.name);
                        }
                    }
                }
            }
        }
        // Transform HashMap<&str, HashSet<&str>> => HashMap<&str, Vec<&str>>
        let map: HashMap<&str, Vec<&str>> = fields_to_save
            .into_iter()
            .map(|(key, value)| (key, value.into_iter().collect::<Vec<_>>()))
            .collect();
        Ok(map)
    }

    /// Save fields linked to a specific domain into the database
    ///
    /// A computed field among them may be waiting on a recompute elsewhere, which only settling
    /// every pending one brings in.
    pub fn save_domain_fields_to_db(
        &mut self,
        model_name: &str,
        domain: &SearchType,
    ) -> Result<()> {
        let fields = domain.get_fields();
        let fields_to_save = self.get_fields_to_save(model_name, &fields)?;
        if fields_to_save.iter().any(|(model_name, fields)| {
            let model = self.model_manager.get_model(model_name);
            fields.iter().any(|field| model.is_computed_field(field))
        }) {
            self.recompute_all_stored()?;
        }
        for (model_name, fields) in fields_to_save {
            self.save_fields_to_db(model_name, &fields)?;
        }
        Ok(())
    }

    /// Flush every registered model that holds dirty records to the database.
    ///
    /// Every pending recompute is settled first: recomputing one model can ask for another to be
    /// recomputed, and that one may otherwise already have been flushed. Then tracked changes are
    /// handed to the tracking hooks, so what they write is saved too.
    ///
    /// The `&ModelManager` is copied out first so the registry borrow stays independent of the
    /// `&mut self` that `save_model_to_db` requires.
    pub fn save_all_to_db(&mut self) -> Result<()> {
        self.refuse_emptied_relations()?;
        self.recompute_all_stored()?;
        self.report_tracked_changes()?;
        self.recompute_all_stored()?;
        let model_manager = self.model_manager;
        for model_name in model_manager.get_models().keys() {
            self.save_model_to_db(model_name)?;
        }
        Ok(())
    }

    /// Recompute every stored field waiting for it, until none is left.
    ///
    /// A stored value may wait on a recompute of another model's that has not asked for it yet:
    /// an order's total is only flagged once its lines' totals are worked out. Settling everything
    /// is what makes the stored value current, whichever model is looked at first.
    pub(super) fn recompute_all_stored(&mut self) -> Result<()> {
        for _ in 0..=MAX_RECOMPUTE_ROUNDS {
            let Some(model_name) = self.model_with_stored_to_recompute() else {
                return Ok(());
            };
            self.call_computed_method_on_all_fields(&model_name)?;
        }
        let Some(model_name) = self.model_with_stored_to_recompute() else {
            return Ok(());
        };
        let to_recompute = &self.cache.get_cache_models(&model_name).to_recompute;
        Err(MaximumRecursionDepthCompute {
            fields_name: to_recompute.keys().cloned().collect(),
            ids: to_recompute.values().flatten().copied().collect(),
            model_name,
        }
        .into())
    }

    /// A model with a stored field still waiting to be recomputed.
    fn model_with_stored_to_recompute(&self) -> Option<String> {
        self.model_manager
            .get_models()
            .iter()
            .find(|(model_name, model)| {
                self.cache
                    .get_cache_models(model_name)
                    .to_recompute
                    .iter()
                    .any(|(field, ids)| !ids.is_empty() && model.is_kept(field))
            })
            .map(|(model_name, _)| model_name.clone())
    }

    /// Save all data related to given model to database.
    ///
    /// Compute non-stored fields related to this model if needed.
    pub fn save_model_to_db(&mut self, model_name: &str) -> Result<()> {
        self.call_computed_method_on_all_fields(model_name)?;

        // Relation tables are not reached by the row operations below.
        let relational: Vec<&'mm str> = self
            .model_manager
            .try_get_model(model_name)?
            .fields
            .keys()
            .map(String::as_str)
            .collect();
        self.save_relations_to_db(model_name, &relational)?;

        let dirty_map_of_fields = self.get_dirty_stored_models(model_name);

        if dirty_map_of_fields.is_empty() {
            return Ok(());
        }
        let dirty_map_of_fields_ref = dirty_map_of_fields
            .iter()
            .map(|(&key, value)| (key, value))
            .collect();
        self.save_data_to_db(model_name, &dirty_map_of_fields_ref)?;

        // Now that it's saved in db, clear dirty fields
        let dirty_ids: MultipleIds = dirty_map_of_fields.keys().collect::<MultipleIds>();
        self.cache.clear_dirty_records(model_name, &dirty_ids);
        Ok(())
    }

    /// Write dirty many2many fields to their relation tables.
    ///
    /// They have no column, so they never reach the row operations that save the rest.
    pub(super) fn save_relations_to_db(&mut self, model_name: &str, fields: &[&str]) -> Result<()> {
        let model = self.model_manager.try_get_model(model_name)?;
        let relations: Vec<(&'mm str, &'mm str, &'mm str, &'mm str)> = fields
            .iter()
            .filter_map(|field_name| {
                let field = model.try_get_internal_field(field_name).ok()?;
                match &field.inverse {
                    Some(FieldReference {
                        inverse_field:
                            FieldReferenceType::M2M {
                                relation,
                                column,
                                target_column,
                            },
                        ..
                    }) => Some((
                        field.name.as_str(),
                        relation.as_str(),
                        column.as_str(),
                        target_column.as_str(),
                    )),
                    _ => None,
                }
            })
            .collect();

        for (field_name, relation, column, target_column) in relations {
            let dirty = self.cache.get_dirty_fields(model_name, &[field_name]);
            if dirty.is_empty() {
                continue;
            }
            for (id, values) in dirty.iter().filter(|(id, _)| !onchange::is_virtual(**id)) {
                let targets = values
                    .get_option::<&Vec<u32>>(field_name)
                    .cloned()
                    .unwrap_or_default();
                self.database
                    .write_relation(relation, column, target_column, *id, &targets)?;
            }
            let ids: MultipleIds = dirty.keys().collect::<MultipleIds>();
            self.cache
                .clear_dirty_fields(model_name, &[field_name], &ids);
        }
        Ok(())
    }

    /// Save given fields to the database, computing them first if needed.
    ///
    /// A one2many has no column of its own: saving it saves the many2one it is found through.
    pub fn save_fields_to_db(&mut self, model_name: &str, fields: &[&str]) -> Result<()> {
        let model = self.model_manager.get_model(model_name);
        // A many2many worked out and kept has its pairs worked out before they are written.
        let kept: Vec<&str> = fields
            .iter()
            .copied()
            .filter(|field| model.is_kept(field))
            .collect();
        self.call_computed_method_on_fields(model_name, &kept)?;
        self.save_relations_to_db(model_name, fields)?;
        for field in fields {
            if let Some(FieldReference {
                target_model,
                inverse_field: FieldReferenceType::O2M { inverse_field },
            }) = &model.get_internal_field(field).inverse
            {
                self.save_fields_to_db(target_model, &[inverse_field])?;
            }
        }
        let fields = fields
            .iter()
            .copied()
            .filter(|field| model.is_stored(field))
            .collect::<Vec<&str>>();

        self.call_computed_method_on_fields(model_name, &fields)?;

        let dirty_map_of_fields = self.get_dirty_fields(model_name, &fields);

        if dirty_map_of_fields.is_empty() {
            return Ok(());
        }
        let dirty_map_of_fields_ref = dirty_map_of_fields
            .iter()
            .map(|(&key, value)| (key, value))
            .collect();
        self.save_data_to_db(model_name, &dirty_map_of_fields_ref)?;

        // Now that it's saved in db, clear dirty fields
        let dirty_ids: MultipleIds = dirty_map_of_fields.keys().collect::<MultipleIds>();
        self.cache
            .clear_dirty_fields(model_name, &fields, &dirty_ids);
        Ok(())
    }

    /// Save given record to the database.
    ///
    /// If the record is already saved, do nothing
    ///
    /// If the record is not present in cache, do nothing
    ///
    /// If given model does not exist, panic.
    pub fn save_records_to_db<Mode: IdMode>(&mut self, model_name: &str, ids: &Mode) -> Result<()> {
        self.call_computed_method_on_ids(model_name, ids.get_ids_ref())?;

        let dirty_map_of_fields = self.get_dirty_stored_records(model_name, ids.get_ids_ref());

        if dirty_map_of_fields.is_empty() {
            return Ok(());
        }
        let dirty_map_of_fields_ref = dirty_map_of_fields
            .iter()
            .map(|(&key, value)| (key, value))
            .collect();
        self.save_data_to_db(model_name, &dirty_map_of_fields_ref)?;

        // Now that it's saved in db, clear dirty fields
        let dirty_ids: MultipleIds = dirty_map_of_fields.keys().collect::<MultipleIds>();
        self.cache.clear_dirty_records(model_name, &dirty_ids);
        Ok(())
    }

    /// Get all dirty stored fields for given model
    pub(super) fn get_dirty_stored_models(&self, model_name: &str) -> HashMap<u32, MapOfFields> {
        let model = self.model_manager.get_model(model_name);
        self.cache
            .get_dirty_models(model_name, |field_name| model.is_stored(field_name))
    }

    /// Get dirty fields from given list of fields
    pub(super) fn get_dirty_fields(
        &self,
        model_name: &str,
        fields: &[&str],
    ) -> HashMap<u32, MapOfFields> {
        self.cache.get_dirty_fields(model_name, fields)
    }

    /// Get all dirty stored fields for given records
    pub(super) fn get_dirty_stored_records(
        &self,
        model_name: &str,
        ids: &[u32],
    ) -> HashMap<u32, MapOfFields> {
        let model = self.model_manager.get_model(model_name);
        self.get_dirty_filtered_records(model_name, ids, |field_name| model.is_stored(field_name))
    }

    /// Get all dirty filtered fields for given records
    pub(super) fn get_dirty_filtered_records<F>(
        &self,
        model_name: &str,
        ids: &[u32],
        field_filter: F,
    ) -> HashMap<u32, MapOfFields>
    where
        F: Fn(&str) -> bool,
    {
        self.cache.get_dirty_records(model_name, ids, field_filter)
    }

    pub(super) fn get_fields_from_db(
        &mut self,
        model_name: &str,
        ids: &MultipleIds,
        fields: &[&str],
    ) -> Result<HashMap<SingleId, MapOfFields>> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let domain = make_domain!([("id", "=", ids.clone())]);
        let data = self.database.search(
            model_name,
            fields,
            &domain,
            self.model_manager,
            &SearchOptions::default(),
        )?;
        Ok(data
            .into_iter()
            .map(|(id, map)| {
                let map: HashMap<String, Option<FieldType>> = map
                    .into_iter()
                    .map(|(key, value)| (key.to_string(), value.map(|v| v.into())))
                    .collect();
                let mut map_of_fields = MapOfFields::new(map);
                map_of_fields.insert("id", id);
                (id.into(), map_of_fields)
            })
            .collect())
    }

    /// Save existing data to the database.
    ///
    /// This method does not check if given fields are stored or not.
    /// It's up to the caller to ensure given data are correct.
    ///
    /// Returns the number of lines updated
    #[allow(dead_code)]
    /// Write values to the database; those of virtual records, which only the cache holds, are
    /// never written.
    pub(super) fn save_data_to_db(
        &mut self,
        model_name: &str,
        data: &HashMap<u32, &MapOfFields>,
    ) -> Result<u32> {
        if data.keys().any(|id| onchange::is_virtual(*id)) {
            let real: HashMap<u32, &MapOfFields> = data
                .iter()
                .filter(|(id, _)| !onchange::is_virtual(**id))
                .map(|(id, values)| (*id, *values))
                .collect();
            return self.database.update(model_name, &real);
        }
        self.database.update(model_name, data)
    }

    /// Insert new data to the database.
    ///
    /// This method does not check if given fields are stored or not.
    /// It's up to the caller to ensure given data are correct.
    pub(super) fn insert_data_to_db(
        &mut self,
        model_name: &str,
        data: &[&MapOfFields],
    ) -> Result<Vec<u32>> {
        self.database.create(model_name, data)
    }
}
