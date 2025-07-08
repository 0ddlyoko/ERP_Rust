use crate::cache::{Cache, CacheField, Dirty, Update};
use crate::database::{Database, DatabaseType};
use crate::errors::MaximumRecursionDepthCompute;
use crate::field::{FieldDepend, FieldReference, FieldReferenceType, FieldType, IdMode, MultipleIds, SingleId};
use crate::model::{MapOfFields, Model, ModelManager};
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
    {
        let internal_model = self.model_manager.get_model(model_name);
        let fields = internal_model.get_stored_fields();
        self.load_records_fields_from_db(model_name, ids, &fields)
    }

    /// Load fields of given records from the database to the cache.
    ///
    /// If fields are already loaded, they will still be retrieved from the database but not updated
    pub fn load_records_fields_from_db<Mode: IdMode>(&mut self, model_name: &str, ids: &Mode, fields: &[&str]) -> Result<()> {
        let ids_to_load: MultipleIds = ids.get_ids_ref().into();
        let fields_from_db = self.get_fields_from_db(model_name, &ids_to_load, fields);
        match fields_from_db {
            Ok(values) => {
                for (id, map_of_fields) in values {
                    for (field_name, field_value) in map_of_fields.fields {
                        self.save_field_to_cache(model_name, &field_name, &id, field_value, &Dirty::NotUpdateDirty, &Update::NotUpdateIfExists)?;
                    }
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
        let model = self.model_manager.get_model(model_name);
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
                    let target_model = self.model_manager.get_model(target_model);
                    current_model = target_model;
                    if !is_stored {
                        if let FieldReferenceType::O2M { inverse_field } = inverse_field {
                            // If there is an inverse field, and it's a stored field, save it
                            let target_field = target_model.get_internal_field(inverse_field);
                            if target_field.is_stored() {
                                fields_to_save.entry(&target_model.name).or_default().insert(&target_field.name);
                            }
                        }
                    }
                }
            }
        }
        // Transform HashMap<&str, HashSet<&str>> => HashMap<&str, Vec<&str>>
        let map: HashMap<&str, Vec<&str>> = fields_to_save.into_iter().map(|(key, value)| (key, value.into_iter().collect::<Vec<_>>())).collect();
        Ok(map)
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
        let model = self.model_manager.get_model(model_name);
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
        let model = self.model_manager.get_model(model_name);
        self.cache.get_dirty_models(model_name, |field_name| model.is_stored(field_name))
    }

    /// Get dirty fields from given list of fields
    pub fn get_dirty_fields(&self, model_name: &str, fields: &[&str]) -> HashMap<u32, MapOfFields> {
        self.cache.get_dirty_fields(model_name, fields)
    }

    /// Get all dirty stored fields for given records
    pub fn get_dirty_stored_records(&self, model_name: &str, ids: &[u32]) -> HashMap<u32, MapOfFields> {
        let model = self.model_manager.get_model(model_name);
        self.get_dirty_filtered_records(model_name, ids, |field_name| model.is_stored(field_name))
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
        let domain = make_domain!([("id", "=", ids.clone())]);
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
        data: &Vec<&MapOfFields>,
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
    ///
    /// If given field_name is a O2M, load it along with its M2O
    pub fn ensure_fields_in_cache<Mode: IdMode>(&mut self, model_name: &str, field_name: &str, ids: &Mode) -> Result<()> {
        // TODO Allow to pass a list of fields
        let ids_ref = ids.get_ids_ref();
        let mut ids_not_in_cache: MultipleIds = self.cache.get_ids_not_in_cache(model_name, field_name, ids_ref).into();
        let ids_to_recompute: MultipleIds = self.cache.get_ids_to_recompute(model_name, field_name, ids_ref).into();

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
            } else if let Some(FieldReference { target_model, inverse_field: FieldReferenceType::O2M { inverse_field } }) = &field_info.inverse {
                // O2M, save data to the database, and then load the field with related M2O
                // TODO Add search_group(...)
                self.save_fields_to_db(target_model, &[inverse_field])?;
                // Load from database
                let mut result: HashMap<u32, Vec<u32>> = HashMap::with_capacity(ids_not_in_cache.get_ids_ref().len());
                for id in &ids_not_in_cache {
                    result.insert(id.get_id(), vec![]);
                }

                let database_result = self.database.search(target_model, &[inverse_field], &make_domain!([(inverse_field, "=", ids_not_in_cache)]), self.model_manager)?;
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
                    self.cache.insert_field_in_cache(model_name, field_name, &[id], field_value, &Dirty::UpdateDirty, &Update::UpdateIfExists);
                }
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
    {
        self.save_option_to_cache(model_name, field_name, ids, Some(value))
    }

    pub fn save_option_to_cache<Mode: IdMode, E>(&mut self, model_name: &str, field_name: &str, ids: &Mode, value: Option<E>) -> Result<()>
    where
        E: Into<FieldType>,
    {
        let field_type: Option<FieldType> = value.map(|value| value.into());

        self.save_field_to_cache(model_name, field_name, ids, field_type, &Dirty::UpdateDirty, &Update::UpdateIfExists)
    }

    /// Retrieve given field from the cache, or from the database if not loaded in cache
    ///
    /// If field is retrieved from the database, it will not be loaded in cache
    ///
    /// If field is not stored, return the default value
    ///
    /// Return a vector sorted by given ids of tuple.
    /// First element is true if it's from the cache, or false if it's from the database.
    /// Second element is the value
    fn retrieve_field_from_cache_or_database<Mode: IdMode>(&mut self, model_name: &str, field_name: &str, ids: &Mode) -> Result<Vec<(bool, Option<FieldType>)>> {
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
            let model_info = self.model_manager.get_model(model_name);

            let field_info = model_info.get_internal_field(field_name);
            if field_info.is_stored() {
                // Load from database
                let database_result = self.database.search(model_name, &[field_name], &make_domain!([("id", "=", ids_not_in_cache)]), self.model_manager)?;
                for (id, mut map) in database_result {
                    let field_value = map.remove(field_name).unwrap();
                    map_result.insert(id, (false, field_value.map(|value| value.into())));
                }
            } else if let Some(FieldReference { target_model, inverse_field: FieldReferenceType::O2M { inverse_field } }) = &field_info.inverse {
                // TODO Check if target field is stored or not
                // O2M, save data to the database, and then make a request
                self.save_fields_to_db(target_model, &[inverse_field])?;
                // Load from database
                let mut result: HashMap<u32, Vec<u32>> = HashMap::with_capacity(ids_not_in_cache.len());
                for id in &ids_not_in_cache {
                    result.insert(*id, vec![]);
                }

                let database_result = self.database.search(target_model, &[inverse_field], &make_domain!([(inverse_field, "=", ids_not_in_cache)]), self.model_manager)?;
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
                        },
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
    fn save_field_to_cache<Mode: IdMode>(&mut self, model_name: &str, field_name: &str, ids: &Mode, value: Option<FieldType>, update_dirty: &Dirty, update_field: &Update) -> Result<()> {
        if field_name == "id" {
            return Ok(())
        }
        let is_update_if_exists = matches!(update_field, Update::UpdateIfExists);
        let internal_model = self.model_manager.get_model(model_name);
        let field_info = internal_model.get_internal_field(field_name);
        if let Some(FieldReference { target_model, inverse_field }) = &field_info.inverse {
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
                        _ => panic!("Only Ref and Refs are accepted field type, and not {:?}", value),
                    };

                    let old_values = self.retrieve_field_from_cache_or_database(model_name, field_name, ids)?;

                    // For removed ids, we can batch the save call
                    let mut ids_removed: Vec<u32> = Vec::new();
                    let mut ids_added: HashMap<u32, Vec<u32>> = HashMap::new();

                    for (i, d) in old_values.into_iter().enumerate() {
                        let old_ids = match d {
                            (_, None) => HashSet::new(),
                            (_, Some(FieldType::Ref(id))) => HashSet::from([id]),
                            (_, Some(FieldType::Refs(ids))) => ids.iter().copied().collect(),
                            _ => panic!("Only Ref and Refs are accepted field type, and not {:?}", value),
                        };

                        ids_removed.extend(old_ids.difference(&new_ids));
                        ids_added.insert(ids.get_ids_ref()[i], new_ids.difference(&old_ids).copied().collect());
                    }

                    if !ids_removed.is_empty() {
                        self.save_field_to_cache::<MultipleIds>(target_model, inverse_field, &ids_removed.into(), None, update_dirty, update_field)?;
                    }
                    for (id, ids) in ids_added {
                        self.save_field_to_cache::<MultipleIds>(target_model, inverse_field, &ids.into(), Some(FieldType::Ref(id)), update_dirty, update_field)?;
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
                    let mut old_values = self.retrieve_field_from_cache_or_database(model_name, field_name, ids)?;
                    // If we don't have to update loaded fields, remove them from the list
                    let mut ids = ids.get_ids_ref().clone();
                    if !is_update_if_exists {
                        let pos_to_remove = old_values.iter().enumerate().filter_map(|(i, (in_cache, _))| {
                            if *in_cache {
                                Some(i)
                            } else {
                                None
                            }
                        }).collect::<HashSet<_>>();
                        old_values = old_values.into_iter().enumerate().filter_map(|(i, data)| {
                            if pos_to_remove.contains(&i) {
                                None
                            } else {
                                Some(data)
                            }
                        }).collect();
                        ids = ids.into_iter().enumerate().filter_map(|(i, data)| {
                            if pos_to_remove.contains(&i) {
                                None
                            } else {
                                Some(data)
                            }
                        }).collect::<Vec<u32>>();
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
                            let mut fields_to_remove_from_recompute = Vec::with_capacity(inverse_fields.len());
                            // If this target is not in cache, we do nothing
                            if let Some(cache_model) = cache_model {
                                for inverse_field in inverse_fields {
                                    // TODO Pass by a new method in cache to add / remove values from list
                                    let cache_field = cache_model.get_field_mut(inverse_field);
                                    // If this field is not in cache, we do nothing
                                    if let Some(CacheField { value: Some(FieldType::Refs(vecs)) }) = cache_field {
                                        vecs.retain(|id| current_id != *id);
                                        if is_update_if_exists {
                                            fields_to_remove_from_recompute.push(inverse_field.clone());
                                        }
                                    }
                                }
                            }
                            cache_models.remove_to_recompute(&fields_to_remove_from_recompute.iter().map(|f| f.as_ref()).collect::<Vec<&str>>(), &[current_id]);
                        }
                    }

                    // Done, now update the id in the cache
                    self.cache.insert_field_in_cache(model_name, field_name, &ids, value.clone(), update_dirty, update_field);

                    // Finally, add the id to the new list
                    // TODO Pass by another method, so that it also automatically triggers computes
                    // Only update if needed
                    if let Some(new_id) = new_id {
                        let cache_models = self.cache.get_cache_models_mut(target_model);
                        let mut fields_to_remove_from_recompute = Vec::with_capacity(inverse_fields.len());

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
                        cache_models.remove_to_recompute(&fields_to_remove_from_recompute.iter().map(|f| f.as_ref()).collect::<Vec<&str>>(), &[new_id]);
                    }

                    let mut old_values_ids: Vec<u32> = Vec::new();

                    for (_, old_value) in old_values {
                        if let Some(old_value) = old_value {
                            match old_value {
                                FieldType::Ref(id) => old_values_ids.push(id),
                                _ => panic!("Only Ref is accepted field type, and not {:?}", old_value),
                            }
                        }
                    }

                    // Now that we have the correct value, set again dependencies of this field as to_recompute
                    if is_update_if_exists {
                        self.check_compute_on_field(model_name, field_name, &ids)?;
                    }

                    Ok(())
                }
            }
        }

        let modified_ids = self.cache.insert_field_in_cache(model_name, field_name, ids.get_ids_ref(), value.clone(), update_dirty, update_field);
        if is_update_if_exists {
            self.check_compute_on_field(model_name, field_name, &modified_ids)?;
        }
        Ok(())
    }

    /// Method called when a field has changed, and will set as to recompute all fields that needs to be recomputed
    ///
    /// We shouldn't call this method from a O2M, as a O2M field shouldn't have any dependencies
    fn check_compute_on_field(&mut self, model_name: &str, field_name: &str, ids: &[u32]) -> Result<()> {
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
                    },
                    FieldDepend::AnotherModel { target_model, target_field } => {
                        // O2M field, we need to perform a search on this field, so we need to compute it
                        self.save_fields_to_db(target_model, &[target_field])?;
                        // Now, make a search request
                        let database_result = self.database.search(target_model, &[target_field], &make_domain!([(target_field, "=", current_ids.clone())]), self.model_manager)?;
                        current_ids = database_result.into_iter().map(|(id, _)| id).collect::<Vec<_>>();
                        current_model = self.model_manager.get_model(target_model);
                    },
                    FieldDepend::CurrentFieldAnotherModel { target_model, field_name } => {
                        // M2O field, we can get the value of this field and continue
                        let all_ids = self.get_fields_value::<MultipleIds>(&current_model.name, field_name, &current_ids.clone().into())?;
                        current_ids = all_ids.into_iter().flatten().flat_map(|id| {
                            match id {
                                FieldType::Ref(r) => vec![*r],
                                FieldType::Refs(r) => r.clone(),
                                _ => panic!("Only Ref & Refs are accepted field type, and not {:?}", id),
                            }
                        }).collect::<Vec<_>>();
                        current_model = self.model_manager.get_model(target_model);
                    },
                }
            }
            // We found ids to recompute, set them as to_recompute
            if !current_ids.is_empty() {
                self.cache.add_ids_to_recompute(&current_model.name, &[&current_field.name], &current_ids);
            }
        }
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
            map.fields.retain(|field, _value| {
                final_model.is_stored(field)
            });
        });


        let ids = self.insert_data_to_db(model_name, &stored_data.iter().collect())?;

        missing_fields_lst.reverse();
        // Once added in database, we should set all stored computed methods as to_recompute
        for id in &ids {
            if let Some(missing_fields) = missing_fields_lst.pop().flatten() {
                let final_internal_model = self.model_manager.get_model(model_name);
                let computed_fields_string = missing_fields
                    .into_iter()
                    .filter(|f| {
                        final_internal_model.is_computed_field(f)
                    })
                    .collect::<Vec<&str>>();
                self.cache.add_ids_to_recompute(model_name, &computed_fields_string, &[*id]);
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
                    self.save_option_to_cache::<SingleId, _>(model_name, &field_name, &id.into(), value)?;
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
        let uuid = "svp_".to_string() + &Uuid::new_v4().to_string()[..6];
        self.database.savepoint(uuid.as_str())?;

        let result = func(self);
        if result.is_ok() {
            // Commit
            self.database.savepoint_commit(uuid.as_str())?;
        } else {
            // Rollback
            self.database.savepoint_rollback(uuid.as_str())?;
            self.cache.import_cache(cache_copy);
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
        let model = self.model_manager.get_model(model_name);
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
        let model = self.model_manager.get_model(model_name);
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
