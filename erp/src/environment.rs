use crate::cache::errors::RecordsNotFoundError;
use crate::cache::{Cache, Dirty, Update};
use crate::field::{FieldType, IdMode, MultipleIds, Reference, SingleId};
use crate::model::{BaseModel, MapOfFields, Model, ModelManager, ModelNotFound};
use std::collections::HashMap;
use std::error::Error;

pub struct Environment<'model_manager> {
    pub cache: Cache,
    pub model_manager: &'model_manager ModelManager,
    id: u32,
}

impl<'model_manager> Environment<'model_manager> {
    pub fn new(model_manager: &'model_manager ModelManager) -> Self {
        Environment {
            cache: Cache::new(model_manager),
            model_manager,
            id: 1,
        }
    }

    // ------------------------------------------
    // |             Database Logic             |
    // ------------------------------------------

    /// Load given records from the database to the cache.
    ///
    /// If the record is already present in cache, do nothing
    pub fn load_records_from_db<Mode: IdMode>(&mut self, model_name: &str, ids: &Mode) -> Result<(), Box<dyn Error>>
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
    pub fn load_records_fields_from_db<Mode: IdMode>(&mut self, model_name: &str, ids: &Mode, fields: &[String]) -> Result<(), Box<dyn Error>> {
        let ids_to_load: Vec<&u32> = ids.get_ids_ref().iter().filter(|id| !self.cache.is_record_present(model_name, id)).collect();
        let ids_to_load: MultipleIds = ids_to_load.into();
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

    /// Save given record to the database.
    ///
    /// If the record is already saved, do nothing
    ///
    /// If the record is not present in cache, do nothing
    ///
    /// If given model does not exist, panic.
    /// TODO Use IdMode
    pub fn save_record_to_db(&mut self, model_name: &str, id: &u32) -> Result<(), Box<dyn Error>> {
        let dirty_map_of_fields = self.cache.get_dirty_map_of_fields(model_name, id);
        if dirty_map_of_fields.is_none() {
            // Nothing to update
            return Ok(());
        }
        self.save_data_to_db(
            model_name,
            &id.into(),
            &dirty_map_of_fields.unwrap(),
        )?;
        // Now that it's saved in db, clear dirty fields
        self.cache.clear_dirty(model_name, id);
        Ok(())
    }

    pub fn get_fields_from_db(
        &self,
        model_name: &str,
        ids: &MultipleIds,
        _fields: &[String],
    ) -> Result<HashMap<SingleId, MapOfFields>, Box<dyn Error>> {
        // TODO Get data from db
        Err(RecordsNotFoundError {
            model_name: model_name.to_string(),
            ids: ids.get_ids_ref().clone(),
        }
            .into())
    }

    /// Save existing data to the database
    #[allow(dead_code)]
    pub fn save_data_to_db(
        &self,
        _model_name: &str,
        _id: &SingleId,
        _data: &MapOfFields,
    ) -> Result<(), Box<dyn Error>> {
        // TODO Save data to db
        Ok(())
    }

    /// Insert new data to the database
    ///
    /// TODO Allow to call this method with multiple data
    pub fn insert_data_to_db(
        &mut self,
        model_name: &str,
        data: &MapOfFields,
    ) -> Result<SingleId, Box<dyn Error>> {
        // TODO Insert data to db
        let id = self.id;
        self.id += 1;
        self.cache.insert_fields_in_cache(model_name, id, data.clone(), &Dirty::NotUpdateDirty, &Update::UpdateIfExists);
        Ok(id.into())
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

    /// Returns a reference to given id
    ///
    /// Do not check if given id is valid id, or is present in the cache
    ///
    /// Do not load given id to the cache
    pub fn get_record_ref<BM>(&self, id: u32) -> Reference<BM, SingleId>
    where
        BM: BaseModel,
    {
        id.into()
    }

    /// Returns a reference to given ids.
    ///
    /// Do not check if given ids are valid ids, or is present in the cache.
    ///
    /// Do not load those ids to the cache
    pub fn get_records_ref<BM>(&self, ids: Vec<u32>) -> Reference<BM, MultipleIds>
    where
        BM: BaseModel,
    {
        ids.into()
    }

    /// Get the value of given field for given id.
    ///
    /// If field is not in cache, load it
    ///
    /// If field needs to be computed, compute it
    pub fn get_field_value<'a>(&'a mut self, model_name: &str, field_name: &str, id: &SingleId) -> Result<Option<&'a FieldType>, Box<dyn Error>> {
        self.ensure_fields_in_cache(model_name, field_name, id)?;

        // TODO In case of O2M / M2M, cache could be invalid.

        // Now, everything should be good
        Ok(self.cache.get_field_from_cache(model_name, field_name, &id.get_id()))
    }

    pub fn get_fields_value<Mode: IdMode>(&mut self, model_name: &str, field_name: &str, ids: &Mode) -> Result<Vec<Option<&FieldType>>, Box<dyn Error>>
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
    pub fn ensure_fields_in_cache<Mode: IdMode>(&mut self, model_name: &str, field_name: &str, ids: &Mode) -> Result<(), Box<dyn Error>> {
        let ids_ref = ids.get_ids_ref();
        let ids_not_in_cache: MultipleIds = self.cache.get_ids_not_in_cache(model_name, field_name, ids_ref).into();
        let ids_to_recompute: MultipleIds = self.cache.get_ids_to_recompute(model_name, field_name, ids_ref).into();

        // TODO Merge the following 2 methods into a single one
        if !ids_not_in_cache.is_empty() && !ids_to_recompute.is_empty() {
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
                self.call_compute_method(model_name, &ids_to_recompute, &[field_name.to_string()])?;
            }
            // TODO Once we add non-stored field, fix this condition
            if true {
                // This is a stored field, load it along with all the other stored fields to avoid
                //  multiple database calls
                let fields_to_load = model_info.get_stored_fields();
                self.load_records_fields_from_db(model_name, &ids_not_in_cache, &fields_to_load)?;
            } else if is_computed_method {
                // This could be a computed one. Call it
                self.call_compute_method(model_name, &ids_not_in_cache, &[field_name.to_string()])?;
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

    pub fn save_field_value<Mode: IdMode, E>(&mut self, model_name: &str, field_name: &str, ids: &Mode, value: E) -> Result<(), Box<dyn Error>>
    where
        E: Into<FieldType>,
        for<'a> &'a Mode: IntoIterator<Item = SingleId>,
    {
        let field_type: FieldType = value.into();
        self.cache.insert_field_in_cache(model_name, field_name, ids.get_ids_ref(), Some(field_type), &Dirty::UpdateDirty, &Update::UpdateIfExists);
        Ok(())
    }

    pub fn save_option_field_value<Mode: IdMode, E>(&mut self, model_name: &str, field_name: &str, ids: &Mode, value: Option<E>) -> Result<(), Box<dyn Error>>
    where
        E: Into<FieldType>,
        for<'a> &'a Mode: IntoIterator<Item = SingleId>,
    {
        let field_type: Option<FieldType> = value.map(|value| value.into());
        self.cache.insert_field_in_cache(model_name, field_name, ids.get_ids_ref(), field_type, &Dirty::UpdateDirty, &Update::UpdateIfExists);
        Ok(())
    }

    /// Create a new record for a specific model and a given list of fields
    ///
    /// TODO Allow to call this method with multiple data
    pub fn create_new_record_from_map<M>(
        &mut self,
        data: &mut MapOfFields,
    ) -> Result<M, Box<dyn Error>>
    where
        M: Model<SingleId>,
    {
        let model_name = M::get_model_name();
        let id = self._create_new_record(model_name, data)?;
        Ok(self.get_record::<M, SingleId>(id))
    }

    /// TODO Allow to call this method with multiple data
    fn _create_new_record(
        &mut self,
        model_name: &str,
        data: &mut MapOfFields,
    ) -> Result<SingleId, Box<dyn Error>>
    {
        // TODO Change this line to not return anything, but instead update the cache
        let missing_fields = self.fill_default_values_on_map(model_name, data);
        let id = self.insert_data_to_db(model_name, data)?;
        // Once added in database, we should call all stored computed methods
        // TODO Do not call all computed method, but instead set them as to_recompute
        if let Some(missing_fields) = missing_fields {
            let final_internal_model = self.model_manager.get_model(model_name);
            if let Some(final_internal_model) = final_internal_model {
                let computed_fields: Vec<String> = missing_fields
                    .into_iter()
                    .filter(|f| final_internal_model.is_computed_field(f))
                    .collect();
                self.call_compute_method(model_name, &id, &computed_fields)?;
            }
        }

        Ok(id)
    }

    /// Add default values for a given model on given data
    pub fn fill_default_values_on_map(
        &self,
        model_name: &str,
        data: &mut MapOfFields,
    ) -> Option<Vec<String>> {
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
    pub fn savepoint<F, R>(&mut self, func: F) -> Result<R, Box<dyn Error>>
    where
        F: FnOnce(&mut Environment) -> Result<R, Box<dyn Error>>,
    {
        let cache_copy = self.cache.export_cache();
        // TODO Savepoint db

        let result = func(self);
        if result.is_ok() {
            // Commit
            // TODO Commit db
        } else {
            // Rollback
            // TODO Rollback db
            self.cache.import_cache(cache_copy);
        }
        result
    }

    /// Call computed methods of given fields of given model for given ids
    pub fn call_compute_method<Mode: IdMode>(
        &mut self,
        model_name: &str,
        ids: &Mode,
        // TODO Find a way to pass &[&str] instead of &[String]
        fields: &[String],
    ) -> Result<(), Box<dyn Error>>
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
                    (computed_field.call_computed_method)(field.as_str(), ids.get_ids_ref().into(), env)?;
                }
            }
            Ok(())
        })
    }
}
