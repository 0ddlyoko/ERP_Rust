use crate::cache::{Cache, Compute, Dirty, Update};
use crate::database::{Database, DatabaseType};
use crate::errors::MaximumRecursionDepthCompute;
use crate::field::{FieldReference, FieldType, IdMode, MultipleIds, SingleId};
use crate::model::{MapOfFields, Model, ModelManager, ModelNotFound};
use erp_search::{LeftTuple, SearchType};
use erp_search_code_gen::make_domain;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use uuid::Uuid;

const MAX_NUMBER_OF_RECURSION: i32 = 1024;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub struct Environment<'db, 'mm> {
    pub cache: Cache,
    pub model_manager: &'mm ModelManager,
    pub database: &'db mut DatabaseType,
}

impl<'db, 'mm> Environment<'db, 'mm> {
    pub fn new(model_manager: &'mm ModelManager, database: &'db mut DatabaseType) -> Self {
        Environment {
            cache: Cache::new(model_manager),
            model_manager,
            database,
        }
    }

    // ------------------------------------------
    // |             Database Logic             |
    // ------------------------------------------

    /// Load given records from the database to the cache.
    ///
    /// If the record is already present in cache, do nothing
    pub fn load_records_from_db<Mode: IdMode>(&mut self, model_name: &str, ids: &Mode) -> Result<()>
    where for<'a> &'a Mode: IntoIterator<Item = SingleId>
    {
        let internal_model = self.model_manager.get_model(model_name);
        if internal_model.is_none() {
            return Err(ModelNotFound {
                model_name: model_name.to_string(),
            }.into());
        }
        let internal_model = internal_model.unwrap();
        let fields = internal_model.get_stored_fields();
        self.load_records_fields_from_db(model_name, ids, &fields)
    }

    /// Load fields of given records from the database to the cache.
    ///
    /// If fields are already loaded, they will still be retrieved from the database but not updated
    pub fn load_records_fields_from_db<'a, Mode: IdMode>(&'a mut self, model_name: &str, ids: &Mode, fields: &'a [&str]) -> Result<()> {
        let ids_to_load: MultipleIds = ids.get_ids_ref().into();
        let fields_from_db = self.get_fields_from_db(model_name, &ids_to_load, fields);
        match fields_from_db {
            Ok(values) => {
                for (id, map_of_fields) in values {
                    self.cache.insert_fields_in_cache(model_name, id.get_id(), map_of_fields, &Dirty::NotUpdateDirty, &Update::NotUpdateIfExists);
                }
                Ok(())
            }
            Err(err) => {
                Err(err)
            }
        }
    }

    pub fn get_fields_to_save(&self, model_name: &str, fields: &Vec<&LeftTuple>) -> Result<HashMap<&'mm str, Vec<&'mm str>>> {
        // TODO Save this result somewhere to avoid recomputing it again
        let mut fields_to_save: HashMap<&str, HashSet<&str>> = HashMap::new();
        if let Some(model) = self.model_manager.get_model(model_name) {
            for field in fields {
                let mut current_model = model;
                for elem in &field.path {
                    let final_field = current_model.get_internal_field(elem);
                    let is_stored = final_field.is_stored();
                    if is_stored {
                        // If stored field, we need to save it to the database
                        fields_to_save.entry(&current_model.name).or_default().insert(&final_field.name);
                    }
                    if let Some(FieldReference {target_model, inverse_field}) = &final_field.inverse {
                        // Get target model to continue the process.
                        // Also, if this field (or the target one) is a stored field, save it
                        if let Some(target_model) = self.model_manager.get_model(target_model) {
                            current_model = target_model;
                            if !is_stored {
                                if let Some(inverse_field) = inverse_field {
                                    // If there is an inverse field, and it's a stored field, save it
                                    let target_field = target_model.get_internal_field(inverse_field);
                                    if target_field.is_stored() {
                                        fields_to_save.entry(&target_model.name).or_default().insert(&target_field.name);
                                    }
                                }
                            }
                        } else {
                            // TODO Add the domain in the error message
                            return Err(ModelNotFound {
                                model_name: model_name.to_string(),
                            }.into());
                        }
                    }
                }
            }
            // Transform HashMap<&str, HashSet<&str>> => HashMap<&str, Vec<&str>>
            let map: HashMap<&str, Vec<&str>> = fields_to_save.into_iter().map(|(key, value)| (key, value.into_iter().collect::<Vec<_>>())).collect();
            Ok(map)
        } else {
            Err(ModelNotFound {
                model_name: model_name.to_string(),
            }.into())
        }
    }

    /// Save fields linked to a specific domain into the database
    fn save_domain_fields_to_db(&mut self, model_name: &str, domain: &SearchType) -> Result<()> {
        let fields = domain.get_fields();
        let fields_to_save = self.get_fields_to_save(model_name, &fields)?;
        for (model_name, fields) in fields_to_save {
            self.save_fields_to_db(model_name, &fields)?;
        }
        Ok(())
    }

    /// Save all data related to given model to database.
    ///
    /// Compute non-stored fields related to this model if needed.
    pub fn save_model_to_db(&mut self, model_name: &str) -> Result<()> {
        self.call_computed_method_on_all_fields(model_name)?;

        let dirty_map_of_fields = self.get_dirty_stored_models(model_name);

        if dirty_map_of_fields.is_empty() {
            return Ok(());
        }
        let dirty_map_of_fields_ref = dirty_map_of_fields.iter().map(|(&key, value)| (key, value)).collect();
        self.save_data_to_db(model_name, &dirty_map_of_fields_ref)?;

        // Now that it's saved in db, clear dirty fields
        let dirty_ids: MultipleIds = dirty_map_of_fields.keys().collect::<MultipleIds>();
        self.cache.clear_dirty_records(model_name, &dirty_ids);
        Ok(())
    }

    /// Save given fields to database.
    ///
    /// Compute them if needed.
    ///
    /// Remove from the original list non-stored fields
    pub fn save_fields_to_db(&mut self, model_name: &str, fields: &[&str]) -> Result<()> {
        if let Some(model) = self.model_manager.get_model(model_name) {
            let fields = fields.iter().filter_map(|&f| {
                if model.is_stored(f) {
                    Some(f)
                } else {
                    // We don't save non-stored fields
                    // TODO Handle related fields (O2M => M2O)
                    None
                }
            }).collect::<Vec<&str>>();

            self.call_computed_method_on_fields(model_name, &fields)?;

            let dirty_map_of_fields = self.get_dirty_fields(model_name, &fields);

            if dirty_map_of_fields.is_empty() {
                return Ok(());
            }
            let dirty_map_of_fields_ref = dirty_map_of_fields.iter().map(|(&key, value)| (key, value)).collect();
            self.save_data_to_db(model_name, &dirty_map_of_fields_ref)?;

            // Now that it's saved in db, clear dirty fields
            let dirty_ids: MultipleIds = dirty_map_of_fields.keys().collect::<MultipleIds>();
            self.cache.clear_dirty_fields(model_name, &fields, &dirty_ids);
            Ok(())
        } else {
            Err(ModelNotFound {
                model_name: model_name.to_string(),
            }.into())
        }
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
        let dirty_map_of_fields_ref = dirty_map_of_fields.iter().map(|(&key, value)| (key, value)).collect();
        self.save_data_to_db(model_name, &dirty_map_of_fields_ref)?;

        // Now that it's saved in db, clear dirty fields
        let dirty_ids: MultipleIds = dirty_map_of_fields.keys().collect::<MultipleIds>();
        self.cache.clear_dirty_records(model_name, &dirty_ids);
        Ok(())
    }

    /// Get all dirty stored fields for given model
    pub fn get_dirty_stored_models(&self, model_name: &str) -> HashMap<u32, MapOfFields> {
        if let Some(model) = self.model_manager.get_model(model_name) {
            self.cache.get_dirty_models(model_name, |field_name| model.is_stored(field_name))
        } else {
            HashMap::new()
        }
    }

    /// Get dirty fields from given list of fields
    pub fn get_dirty_fields(&self, model_name: &str, fields: &[&str]) -> HashMap<u32, MapOfFields> {
        self.cache.get_dirty_fields(model_name, fields)
    }

    /// Get all dirty stored fields for given records
    pub fn get_dirty_stored_records(&self, model_name: &str, ids: &[u32]) -> HashMap<u32, MapOfFields> {
        if let Some(model) = self.model_manager.get_model(model_name) {
            self.get_dirty_filtered_records(model_name, ids, |field_name| model.is_stored(field_name))
        } else {
            HashMap::new()
        }
    }

    /// Get all dirty filtered fields for given records
    pub fn get_dirty_filtered_records<F>(&self, model_name: &str, ids: &[u32], field_filter: F) -> HashMap<u32, MapOfFields>
    where
        F: Fn(&str) -> bool,
    {
        self.cache.get_dirty_records(model_name, ids, field_filter)
    }

    pub fn get_fields_from_db(
        &mut self,
        model_name: &str,
        ids: &MultipleIds,
        fields: &[&str],
    ) -> Result<HashMap<SingleId, MapOfFields>> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        let domain = make_domain!([("id", "=", ids.get_ids_ref().clone())]);
        let data = self.database.search(model_name, fields, &domain, self.model_manager)?;
        Ok(data.into_iter().map(|(id, map)| {
            let map: HashMap<String, Option<FieldType>> = map.into_iter().map(|(key, value)| {
                (key.to_string(), value.map(|v| v.into()))
            }).collect();
            let mut map_of_fields = MapOfFields::new(map);
            map_of_fields.insert("id", id);
            (id.into(), map_of_fields)
        }).collect())
    }

    /// Save existing data to the database.
    ///
    /// This method does not check if given fields are stored or not.
    /// It's up to the caller to ensure given data are correct.
    ///
    /// Returns the number of lines updated
    #[allow(dead_code)]
    pub fn save_data_to_db(
        &mut self,
        model_name: &str,
        data: &HashMap<u32, &MapOfFields>,
    ) -> Result<u32> {
        self.database.update(model_name, data)
    }

    /// Insert new data to the database.
    ///
    /// This method does not check if given fields are stored or not.
    /// It's up to the caller to ensure given data are correct.
    pub fn insert_data_to_db(
        &mut self,
        model_name: &str,
        data: Vec<MapOfFields>,
    ) -> Result<Vec<u32>> {
        self.database.create(model_name, data)
    }

    // ------------------------------------------
    // |             Retrieve Logic             |
    // ------------------------------------------

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
        let model_name = M::get_model_name();
        self.save_domain_fields_to_db(model_name, domain)?;

        let ids = self.database.browse(M::get_model_name(), domain, self.model_manager)?;
        Ok(M::create_instance(ids.into()))
    }

    /// Get the value of given field for given id.
    ///
    /// If field is not in cache, load it
    ///
    /// If field needs to be computed, compute it
    pub fn get_field_value<'a>(&'a mut self, model_name: &str, field_name: &str, id: &SingleId) -> Result<Option<&'a FieldType>> {
        self.ensure_fields_in_cache(model_name, field_name, id)?;

        // TODO In case of O2M / M2M, cache could be invalid.

        // Now, everything should be good
        Ok(self.cache.get_field_from_cache(model_name, field_name, &id.get_id()))
    }

    pub fn get_fields_value<Mode: IdMode>(&mut self, model_name: &str, field_name: &str, ids: &Mode) -> Result<Vec<Option<&FieldType>>>
    where
        for<'a> &'a Mode: IntoIterator<Item = SingleId>,
    {
        self.ensure_fields_in_cache(model_name, field_name, ids)?;

        // TODO In case of O2M / M2M, cache could be invalid.

        // Now, everything should be good
        Ok(ids.get_ids_ref().iter().map(|id| {
            self.cache.get_field_from_cache(model_name, field_name, id)
        }).collect())
    }

    /// Ensure given field is in cache for given ids
    ///
    /// If some ids are invalid or need to be loaded, load them (or compute them if needed)
    pub fn ensure_fields_in_cache<Mode: IdMode>(&mut self, model_name: &str, field_name: &str, ids: &Mode) -> Result<()> {
        // TODO Allow to pass a list of fields
        let ids_ref = ids.get_ids_ref();
        let mut ids_not_in_cache: MultipleIds = self.cache.get_ids_not_in_cache(model_name, field_name, ids_ref).into();
        let ids_to_recompute: MultipleIds = self.cache.get_ids_to_recompute(model_name, field_name, ids_ref).into();

        if !ids_not_in_cache.is_empty() || !ids_to_recompute.is_empty() {
            // Load given fields
            let model_info = self.model_manager.get_model(model_name);
            if model_info.is_none() {
                return Err(ModelNotFound {
                    model_name: model_name.to_string(),
                }.into());
            }
            let model_info = model_info.unwrap();
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
            // TODO Once we add non-stored field, fix this condition
            if model_info.is_stored(field_name) {
                // This is a stored field, load it along with all the other stored fields to avoid
                //  multiple database calls
                let fields_to_load = model_info.get_stored_fields();
                self.load_records_fields_from_db(model_name, &ids_not_in_cache, &fields_to_load)?;
            } else if is_computed_method {
                // This could be a computed one. Call it
                self.call_compute_method(model_name, &ids_not_in_cache, &[field_name])?;
            } else {
                // State where field is not computed nor stored. This behavior is unexpected
                // TODO Find what to do in this case
            }
        }

        Ok(())
    }

    // ------------------------------------------
    // |           Save to Cache Logic          |
    // ------------------------------------------

    pub fn save_value_to_cache<Mode: IdMode, E>(&mut self, model_name: &str, field_name: &str, ids: &Mode, value: E) -> Result<()>
    where
        E: Into<FieldType>,
        for<'a> &'a Mode: IntoIterator<Item = SingleId>,
    {
        let field_type: FieldType = value.into();
        self.cache.insert_field_in_cache(model_name, field_name, ids.get_ids_ref(), Some(field_type), &Dirty::UpdateDirty, &Update::UpdateIfExists, &Compute::ResetCompute);
        Ok(())
    }

    pub fn save_option_to_cache<Mode: IdMode, E>(&mut self, model_name: &str, field_name: &str, ids: &Mode, value: Option<E>) -> Result<()>
    where
        E: Into<FieldType>,
        for<'a> &'a Mode: IntoIterator<Item = SingleId>,
    {
        let field_type: Option<FieldType> = value.map(|value| value.into());
        self.cache.insert_field_in_cache(model_name, field_name, ids.get_ids_ref(), field_type, &Dirty::UpdateDirty, &Update::UpdateIfExists, &Compute::ResetCompute);
        Ok(())
    }

    /// Create a new record for a specific model and a given list of fields
    pub fn create_new_record_from_map<M>(
        &mut self,
        data: MapOfFields,
    ) -> Result<M>
    where
        M: Model<SingleId>,
    {
        let model_name = M::get_model_name();
        let ids = self._create_new_records(model_name, vec![data])?;
        let id = ids.get_id_at(0);
        Ok(self.get_record::<M, SingleId>(id.into()))
    }

    /// Create new records for a specific model and multiple lists of fields
    pub fn create_new_records_from_maps<M>(
        &mut self,
        data: Vec<MapOfFields>,
    ) -> Result<M>
    where
        M: Model<MultipleIds>,
    {
        let model_name = M::get_model_name();
        let ids = self._create_new_records(model_name, data)?;
        Ok(self.get_record::<M, MultipleIds>(ids))
    }

    fn _create_new_records(
        &mut self,
        model_name: &str,
        mut data: Vec<MapOfFields>,
    ) -> Result<MultipleIds>
    {
        if let Some(final_model) = self.model_manager.get_model(model_name) {
            let mut missing_fields_lst = Vec::new();

            for d in &mut data {
                // Remove non-stored fields
                d.fields.retain(|field, _value| final_model.is_stored(field));

                // Method "fill_default_values_on_map" should not add non-stored fields
                let missing_fields = self.fill_default_values_on_map(model_name, d);
                missing_fields_lst.push(missing_fields)
            }

            let ids = self.insert_data_to_db(model_name, data)?;
            // This should not fail
            assert_eq!(ids.len(), missing_fields_lst.len(), "Len of ids should be equals to len of missing fields. {} != {}. Ids = {:?}", ids.len(), missing_fields_lst.len(), ids);

            missing_fields_lst.reverse();
            // Once added in database, we should set all stored computed methods as to_recompute
            for id in &ids {
                if let Some(missing_fields) = missing_fields_lst.pop().flatten() {
                    let final_internal_model = self.model_manager.get_model(model_name);
                    if let Some(final_internal_model) = final_internal_model {
                        // I need to transform this Vec<&str> to a Vec<String> to a Vec<&str>,
                        // otherwise I had a mutable error in add_ids_to_recompute
                        // If someone has a solution to this, please let me know
                        let computed_fields_string = missing_fields
                            .into_iter()
                            .filter_map(|f| {
                                if final_internal_model.is_computed_field(f) {
                                    Some(f.to_string())
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<String>>();
                        let computed_fields: Vec<&str> = computed_fields_string.iter().map(|s| s.as_str()).collect();
                        self.cache.add_ids_to_recompute(model_name, &computed_fields, &[*id]);
                    }
                }
            }

            Ok(ids.into())
        } else {
            Err(ModelNotFound {
                model_name: model_name.to_string(),
            }.into())
        }
    }

    /// Add default values for a given model on given data
    pub fn fill_default_values_on_map(
        &self,
        model_name: &str,
        data: &mut MapOfFields,
    ) -> Option<Vec<&'mm str>> {
        let final_internal_model = self.model_manager.get_model(model_name)?;
        let missing_fields_to_load = final_internal_model.get_missing_fields(data.get_keys());
        for missing_field_to_load in &missing_fields_to_load {
            let default_value = final_internal_model.get_default_value(missing_field_to_load);
            data.insert_field_type(missing_field_to_load, default_value);
        }
        Some(missing_fields_to_load)
    }

    // ------------------------------------------
    // |              Other Logic               |
    // ------------------------------------------

    /// Create a new savepoint and commit if the given method doesn't return any error.
    /// If an error is returned, rollback the commit and put back the cache as it was
    pub fn savepoint<F, R>(&mut self, func: F) -> Result<R>
    where
        F: FnOnce(&mut Environment) -> Result<R>,
    {
        let cache_copy = self.cache.export_cache();
        let uuid = Uuid::new_v4().to_string();
        self.database.savepoint(uuid.as_str())?;

        let result = func(self);
        if result.is_ok() {
            // Commit
            self.database.savepoint_commit(uuid.as_str())?;
        } else {
            // Rollback
            self.cache.import_cache(cache_copy);
            self.database.savepoint_rollback(uuid.as_str())?;
        }
        result
    }

    // ------------------------------------------
    // |            Computed methods            |
    // ------------------------------------------

    /// Call computed method on all stored fields that need to be computed for given model
    fn call_computed_method_on_all_fields(
        &mut self,
        model_name: &str,
    ) -> Result<()> {
        if let Some(model) = self.model_manager.get_model(model_name) {
            for i in 0..=MAX_NUMBER_OF_RECURSION {
                let cache_models = self.cache.get_cache_models(model_name);
                // TODO The filter should not be useful here, as we should add a way to not set as to_recompute non-computed fields
                if let Some((key, value)) = cache_models.to_recompute.iter().find(|(key, _value)| model.is_stored(key)) {
                    let ids: MultipleIds = MultipleIds {
                        ids: value.iter().copied().collect(),
                    };
                    self.call_compute_method(model_name, &ids, &[key.clone().as_str()])?;
                }
                let cache_models = self.cache.get_cache_models(model_name);
                // TODO The filter should not be useful here, as we should add a way to not set as to_recompute non-computed fields
                if cache_models.to_recompute.keys().filter(|key| model.is_stored(key)).peekable().peek().is_some() {
                    break;
                }
                if i == MAX_NUMBER_OF_RECURSION {
                    return Err(MaximumRecursionDepthCompute {
                        model_name: model_name.to_string(),
                        fields_name: cache_models.to_recompute.keys().cloned().collect(),
                        ids: cache_models.to_recompute.values().flatten().copied().collect::<Vec<u32>>(),
                    }.into());
                }
            }

            Ok(())
        } else {
            Err(ModelNotFound {
                model_name: model_name.to_string(),
            }.into())
        }
    }

    /// Call computed method on computed & non-computed fields that need to be computed for given model
    fn call_computed_method_on_fields(
        &mut self,
        model_name: &str,
        fields: &[&str],
    ) -> Result<()> {
        for i in 0..=MAX_NUMBER_OF_RECURSION {
            let cache_models = self.cache.get_cache_models(model_name);
            if let Some((key, value)) = cache_models.to_recompute.iter().find(|(key, _value)| fields.contains(&key.as_str())) {
                let ids: MultipleIds = MultipleIds {
                    ids: value.iter().copied().collect(),
                };
                self.call_compute_method(model_name, &ids, &[key.clone().as_str()])?;
            }
            let cache_models = self.cache.get_cache_models(model_name);
            if !cache_models.to_recompute.iter().any(|(key, value)| !value.is_empty() && fields.contains(&key.as_str())) {
                break;
            }
            if i == MAX_NUMBER_OF_RECURSION {
                return Err(MaximumRecursionDepthCompute {
                    model_name: model_name.to_string(),
                    fields_name: cache_models.to_recompute.keys().cloned().collect(),
                    ids: cache_models.to_recompute.values().flatten().copied().collect::<Vec<u32>>(),
                }.into());
            }
        }

        Ok(())
    }

    /// Call computed method on non-stored fields that need to be computed for given model, for given ids
    fn call_computed_method_on_ids(
        &mut self,
        model_name: &str,
        ids: &[u32],
    ) -> Result<()> {
        if let Some(model) = self.model_manager.get_model(model_name) {
            for i in 0..=MAX_NUMBER_OF_RECURSION {
                let cache_models = self.cache.get_cache_models(model_name);
                if let Some((field, value)) = cache_models.to_recompute.iter().find_map(|(field, value)| {
                    if model.is_stored(field) {
                        let result = value.iter().filter(|id| ids.contains(id)).collect::<Vec<&u32>>();
                        if result.is_empty() {
                            None
                        } else {
                            Some((field, result))
                        }
                    } else {
                        None
                    }
                }) {
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
                        ids: cache_models.to_recompute.values().flatten().copied().collect::<Vec<u32>>(),
                    }.into());
                }
            }

            Ok(())
        } else {
            Err(ModelNotFound {
                model_name: model_name.to_string(),
            }.into())
        }
    }

    /// Call computed methods of given fields of given model for given ids
    pub fn call_compute_method<Mode: IdMode>(
        &mut self,
        model_name: &str,
        ids: &Mode,
        fields: &[&str],
    ) -> Result<()>
    {
        let final_internal_model = self.model_manager.get_model(model_name);
        if final_internal_model.is_none() {
            return Err(ModelNotFound {
                model_name: model_name.to_string(),
            }
            .into());
        }
        let final_internal_model = final_internal_model.unwrap();
        self.savepoint(|env| {
            for field in fields {
                if let Some(computed_field) = final_internal_model.get_computed_field(field) {
                    // TODO Try to find a way to not clone the id
                    (computed_field.call_computed_method)(field, ids.get_ids_ref().into(), env)?;
                }
            }
            Ok(())
        })
    }
}
