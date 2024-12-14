use crate::cache::Cache;
use crate::model::{MapOfFields, ModelManager, ModelNotFound, SimplifiedModel};
use std::error::Error;

use crate::cache::errors::RecordNotFoundError;
use crate::internal::internal_model::InternalModel;
use crate::model::Model;

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

    /// Load given record from the database to the cache.
    /// If the record is already present in cache, do nothing
    /// Returns true if the record has been found
    pub fn load_record_from_db(&mut self, model_name: &str, id: u32) -> Result<(), Box<dyn Error>> {
        if self.cache.is_record_present(model_name, id) {
            return Ok(());
        }

        let map_of_fields = self.get_data_from_db(model_name, id)?;
        self.cache
            .insert_record_model_with_map(model_name, id, map_of_fields);
        self.cache.clear_dirty(model_name, id);
        Ok(())
    }

    /// Save given record to the database.
    ///
    /// If the record is already saved, do nothing
    ///
    /// If the record is not present in cache, do nothing
    ///
    /// If given model does not exist, panic.
    pub fn save_record_to_db(&mut self, model_name: &str, id: u32) -> Result<(), Box<dyn Error>> {
        let cache_models = self.cache.get_cache_models(model_name);
        let dirty_fields = cache_models.get_dirty(id);
        if dirty_fields.is_none() {
            // Nothing to update
            return Ok(());
        }
        let cache_model = cache_models.get_model(id);
        if cache_model.is_none() {
            // Model not found in cache
            return Ok(());
        }
        let dirty_fields: Vec<&str> = dirty_fields.unwrap().iter().map(|f| f.as_str()).collect();
        self.save_data_to_db(
            model_name,
            id,
            &cache_model.unwrap().get_map_of_fields(&dirty_fields),
        )?;
        // Now that it's saved in db, clear dirty fields
        self.cache.get_cache_models_mut(model_name).clear_dirty(id);
        Ok(())
    }

    /// Retrieves data from the database
    pub fn get_data_from_db(
        &self,
        model_name: &str,
        id: u32,
    ) -> Result<MapOfFields, Box<dyn Error>> {
        // TODO Get data from db
        Err(RecordNotFoundError {
            model_name: model_name.to_string(),
            id,
        }
        .into())
    }

    /// Save existing data to the database
    pub fn save_data_to_db(
        &self,
        model_name: &str,
        id: u32,
        data: &MapOfFields,
    ) -> Result<(), Box<dyn Error>> {
        // TODO Save data to db
        Ok(())
    }

    /// Insert new data to the database
    pub fn insert_data_to_db(
        &mut self,
        model_name: &str,
        data: &MapOfFields,
    ) -> Result<u32, Box<dyn Error>> {
        // TODO Insert data to db
        let id = self.id;
        self.id += 1;
        self.cache
            .insert_record_model_with_map(model_name, id, data.clone());
        Ok(id)
    }

    /// Save given model in the cache
    pub fn save_record_from_name(&mut self, model_name: &str, record: &dyn SimplifiedModel) {
        assert_ne!(record.get_id(), 0, "Given model doesn't have any id");
        let id = record.get_id();
        let data = record.get_data();
        self.cache.insert_record_model_with_map(model_name, id, data);
    }

    /// Save given record to the cache
    pub fn save_record<M>(&mut self, record: &M)
    where
        M: Model,
    {
        let id = record.get_id();
        let model_name = M::get_model_name();
        let data = record.get_data();
        self.cache.insert_record_model_with_map(model_name, id, data);
    }

    /// Returns the first record of given model for a specific id
    ///
    /// If the record is not present in cache, loads it from the database
    pub fn get_record_from_name(
        &mut self,
        model_name: &str,
        id: u32,
    ) -> Result<Box<dyn SimplifiedModel>, Box<dyn Error>> {
        let map_of_fields = self.get_map_of_field(model_name, id)?;
        Ok(self
            .model_manager
            .create_instance_from_name(model_name, id, map_of_fields))
    }

    /// Returns the first record of given model for a specific id
    ///
    /// If the record is not present in cache, loads it from the database
    pub fn get_record_from_internal_model(
        &mut self,
        internal_model: &InternalModel,
        id: u32,
    ) -> Result<Box<dyn SimplifiedModel>, Box<dyn Error>> {
        let map_of_fields = self.get_map_of_field(internal_model.name.as_str(), id)?;
        Ok(self.model_manager.create_instance_from_internal_model(
            id,
            map_of_fields,
            internal_model,
        ))
    }

    pub fn cast_to<FROM, TO>(&mut self, from: &FROM)
    where
    FROM: Model,
    TO: Model {

    }

    /// Returns an instance of given model for a specific id
    ///
    /// If the record is not present in cache, loads it from the database
    pub fn get_record<M>(&mut self, id: u32) -> Result<M, Box<dyn Error>>
    where
        M: Model,
    {
        let model_name = M::get_model_name();
        let map_of_fields = self.get_map_of_field(model_name, id)?;
        Ok(self.model_manager.create_instance::<M>(id, map_of_fields))
    }

    /// Returns the MapOfField of given model for given id
    fn get_map_of_field(
        &mut self,
        model_name: &str,
        id: u32,
    ) -> Result<MapOfFields, Box<dyn Error>> {
        if !self.model_manager.is_valid_model(model_name) {
            return Err(ModelNotFound {
                model_name: model_name.to_string(),
            }
            .into());
        }
        self.load_record_from_db(model_name, id)?;
        let cache_record = self.cache.get_cache_record(model_name, id);
        if cache_record.is_none() {
            return Err(RecordNotFoundError {
                model_name: model_name.to_string(),
                id,
            }
            .into());
        }
        let record = cache_record.unwrap();
        Ok(record.transform_into_map_of_fields())
    }

    /// Create a new record for a specific model and a given model instance
    ///
    /// The returned model instance will be different that the original one
    pub fn create_new_record<M>(&mut self, model: M) -> Result<M, Box<dyn Error>>
    where
        M: Model,
    {
        self.create_new_record_from_map::<M>(&mut model.get_data())
    }

    /// Create a new record for a specific model and a given list of fields
    pub fn create_new_record_from_map<M>(
        &mut self,
        data: &mut MapOfFields,
    ) -> Result<M, Box<dyn Error>>
    where
        M: Model,
    {
        let model_name = M::get_model_name();
        let id = self._create_new_record(model_name, data)?;
        self.get_record::<M>(id)
    }

    /// Create a new record for a specific model and a given list of fields
    pub fn create_record_from_name(
        &mut self,
        model_name: &str,
        data: &mut MapOfFields,
    ) -> Result<Box<dyn SimplifiedModel>, Box<dyn Error>> {
        let id = self._create_new_record(model_name, data)?;
        self.get_record_from_name(model_name, id)
    }

    fn _create_new_record(
        &mut self,
        model_name: &str,
        data: &mut MapOfFields,
    ) -> Result<u32, Box<dyn Error>> {
        let missing_fields = self.fill_default_values_on_map(model_name, data);
        let id = self.insert_data_to_db(model_name, data)?;
        self.load_record_from_db(model_name, id)?;
        // Once loaded, we should call all computed methods
        if let Some(missing_fields) = missing_fields {
            let final_internal_model = self.model_manager.get_model(model_name);
            if let Some(final_internal_model) = final_internal_model {
                let computed_fields: Vec<String> = missing_fields
                    .into_iter()
                    .filter(|f| final_internal_model.is_computed_field(f))
                    .collect();
                self.call_compute_fields(model_name, id, &computed_fields)?;
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

    /// Call computed methods of given fields of given model for given id
    pub fn call_compute_fields(
        &mut self,
        model_name: &str,
        id: u32,
        fields: &[String],
    ) -> Result<(), Box<dyn Error>> {
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
                let mut record = env.get_record_from_internal_model(computed_field, id)?;
                record.call_compute_method(field.as_str(), env)?;
                env.save_record_from_name(model_name, &*record);
            }
            Ok(())
        })
    }
}
