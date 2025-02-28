use crate::cache::Cache;
use crate::model::{BaseModel, MapOfFields, Model, ModelManager, ModelNotFound};
use std::error::Error;

use crate::cache::errors::RecordNotFoundError;
use crate::field::{FieldType, IdMode, MultipleIds, Reference, SingleId};

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
    ///
    /// Returns true if the record has been found
    pub fn load_records_from_db<'a, Mode: IdMode + 'a>(&mut self, model_name: &str, ids: &Mode) -> Result<(), Box<dyn Error>>
    where &'a Mode: IntoIterator<Item = SingleId>
    {
        // TODO Correctly write this method with MultipleIds
        let ids_to_load: Vec<&u32> = ids.get_ids_ref().iter().filter(|id| !self.cache.is_record_present(model_name, id)).collect();
        let ids_to_load: MultipleIds = ids_to_load.into();
        for id in ids_to_load {
            let map_of_fields = self.get_data_from_db(model_name, &id)?;
            self.cache.insert_record_fields(model_name, id.get_id(), map_of_fields, false);
        }
        Ok(())
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

    /// Retrieves data from the database
    pub fn get_data_from_db(
        &self,
        model_name: &str,
        id: &SingleId
    ) -> Result<MapOfFields, Box<dyn Error>> {
        // TODO Get data from db
        Err(RecordNotFoundError {
            model_name: model_name.to_string(),
            id: id.get_id(),
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
    /// TODO Allow to call this method with multiple data
    pub fn insert_data_to_db(
        &mut self,
        model_name: &str,
        data: &MapOfFields,
    ) -> Result<SingleId, Box<dyn Error>> {
        // TODO Insert data to db
        let id = self.id;
        self.id += 1;
        self.cache.insert_record_fields(model_name, id, data.clone(), false);
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

    /// Return the value of a specific field
    ///
    /// If record is not loaded, load it
    ///
    /// If field is a computed one and is not already computed, compute it
    pub fn get_field_value<'a>(&'a mut self, model_name: &str, field_name: &str, id: &SingleId) -> Result<Option<&'a FieldType>, Box<dyn Error>> {
        if !self.model_manager.is_valid_model(model_name) {
            return Err(ModelNotFound {
                model_name: model_name.to_string(),
            }
                .into());
        }
        if !self.cache.is_record_field(model_name, field_name, &id.get_id()) {
            // TODO Do not load all records if it's already available in cache
            //  Call a custom method here that will load this field, and this method will check
            //  if it's a computed method or not
            self.load_records_from_db(model_name, id)?;
            //
            if self.cache.is_to_recompute(model_name, field_name, &id.get_id()) {
                // We need to compute this field
                self.call_compute_method(model_name, id, &[field_name.to_string()])?;
            }
        }

        Ok(self.cache.get_record_field(model_name, field_name, &id.get_id()))
    }

    pub fn get_fields_value<'a, Mode: IdMode + 'a>(&'a mut self, model_name: &str, field_name: &str, ids: &Mode) -> Result<Vec<Option<&'a FieldType>>, Box<dyn Error>>
    where
        &'a Mode: IntoIterator<Item = SingleId>,
    {
        if !self.model_manager.is_valid_model(model_name) {
            return Err(ModelNotFound {
                model_name: model_name.to_string(),
            }.into());
        }
        self.load_records_from_db(model_name, ids)?;
        // TODO Check if it's a computed field and if we need to compute it
        Ok(ids.get_ids_ref().iter().map(|id| {
            self.cache.get_record_field(model_name, field_name, id)
        }).collect())
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
        self.cache.insert_record_field(model_name, field_name, ids, Some(field_type), true);
        Ok(())
    }

    pub fn save_option_field_value<Mode: IdMode, E>(&mut self, model_name: &str, field_name: &str, ids: &Mode, value: Option<E>) -> Result<(), Box<dyn Error>>
    where
        E: Into<FieldType>,
        for<'a> &'a Mode: IntoIterator<Item = SingleId>,
    {
        let field_type: Option<FieldType> = value.map(|value| value.into());
        self.cache.insert_record_field(model_name, field_name, ids, field_type, true);
        Ok(())
    }

    /// Create a new record for a specific model and a given list of fields
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
        self.load_records_from_db(model_name, &id)?;
        // Once loaded, we should call all computed methods
        // TODO Do not call all computed method, but only the ones whe need
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
                let compute_field = final_internal_model.get_computed_field(field);
                if compute_field.is_none() {
                    continue;
                }
                let computed_field = compute_field.unwrap();
                // TODO Try to find a way to not clone the id
                (computed_field.call_computed_method)(field.as_str(), ids.get_ids_ref().into(), env)?;
            }
            Ok(())
        })
    }
}
