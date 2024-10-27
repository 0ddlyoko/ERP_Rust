use crate::cache;
use crate::cache::Cache;
use crate::field::FieldType;
use crate::model::{MapOfFields, Model, ModelManager};
use std::error::Error;

pub struct Environment<'model_manager> {
    pub cache: Cache,
    pub model_manager: &'model_manager ModelManager,
}

impl<'model_manager> Environment<'model_manager> {
    pub fn new(model_manager: &'model_manager ModelManager) -> Self {
        Environment {
            cache: Cache::new(),
            model_manager,
        }
    }

    /// Load given record from the database to the cache.
    /// If the record is already present in cache, do nothing
    /// Returns true if the record has been found
    pub fn load_record_from_db(&mut self, model_name: &'static str, id: u32) -> Result<(), Box<dyn Error>> {
        if self.cache.is_record_present(model_name, id) {
            return Ok(());
        }

        let map_of_fields = self.get_data_from_db(model_name, id)?;
        let map_of_fields = Cache::transform_map_to_fields_into_cache(&map_of_fields);
        self.cache.insert_record_model_with_map(model_name, id, map_of_fields);
        self.cache.clear_all_dirty_of_model(model_name, id);
        Ok(())
    }

    /// Save given record to the database.
    ///
    /// If the record is already saved, do nothing
    ///
    /// If the record is not present in cache, do nothing
    pub fn save_record_to_db(&mut self, model_name: &'static str, id: u32) -> Result<(), Box<dyn Error>> {
        let cache_model = self.cache.get_cache_record(model_name, id);
        if cache_model.is_none() {
            // Nothing to update
            return Ok(());
        }
        let dirty_fields = cache_model.unwrap().get_fields_dirty();
        self.save_data_to_db(model_name, id, dirty_fields)?;
        Ok(())
    }

    pub fn get_data_from_db(&self, model_name: &'static str, id: u32) -> Result<MapOfFields, Box<dyn Error>> {
        todo!("Save data to db")
    }

    /// Save given data in the database
    pub fn save_data_to_db(&self, model_name: &'static str, id: u32, data: MapOfFields) -> Result<(), Box<dyn Error>> {
        todo!("Save data to db")
    }

    pub fn insert_data_to_db(&self, model_name: &'static str, data: MapOfFields) -> Result<u32, Box<dyn Error>> {
        todo!("Insert data to db")
    }

    /// Insert given model in the cache
    pub fn save_record_from_name(&mut self, model_name: &'static str, record: &dyn Model) {
        assert_ne!(record.get_id(), 0, "Given model doesn't have any id");
        let id = record.get_id();
        let model_name = model_name;
        let data = record.get_data();
        self.cache.insert_record_model_with_map(model_name, id, Cache::transform_map_to_fields_into_cache(&data));
    }

    /// Save given record to the cache
    pub fn save_record<M>(&mut self, record: &M) where M: Model {
        let id = record.get_id();
        let model_name = M::get_model_name();
        let data = record.get_data();
        self.cache.insert_record_model_with_map(model_name, id, Cache::transform_map_to_fields_into_cache(&data));
    }

    /// Returns the first record of given model for a specific id
    ///
    /// If the record is not present in cache, loads it from the database
    pub fn get_record_from_name(&mut self, model_name: &'static str, id: u32) -> Result<Option<Box<dyn Model>>, Box<dyn Error>> {
        self.load_record_from_db(model_name, id)?;
        let cache_record = self.cache.get_cache_record(model_name, id);
        if cache_record.is_none() {
            panic!("Cache record not found. This should not occurs.");
        }
        let record = cache_record.unwrap();
        let map_of_fields = record.transform_into_map_of_fields();
        Ok(self.model_manager.create_instance_from_name(model_name, id, map_of_fields))
    }

    /// Returns an instance of given model for a specific id
    ///
    /// If the record is not present in cache, loads it from the database
    pub fn get_record<M>(&mut self, id: u32) -> Result<Option<M>, Box<dyn Error>> where M: Model + 'static {
        let model_name = M::get_model_name();
        self.load_record_from_db(model_name, id)?;
        let cache_record = self.cache.get_cache_record(model_name, id);
        if cache_record.is_none() {
            return Err(cache::errors::RecordNotFoundError {
                model_name,
                id,
            }.into());
        }
        let record = cache_record.unwrap();
        let map_of_fields = record.transform_into_map_of_fields();
        Ok(self.model_manager.create_instance::<M>(id, map_of_fields))
    }

    /// Create a new record for a specific model and a given list of fields
    pub fn create_record_from_name(&mut self, model_name: &'static str, mut data: MapOfFields) -> Result<Box<dyn Model>, Box<dyn Error>> {
        self.fill_default_values_on_map(model_name, &mut data);

        let id = self.insert_data_to_db(model_name, data)?;
        self.load_record_from_db(model_name, id)?;
        Ok(self.get_record_from_name(model_name, id)?.unwrap())
    }

    /// Create a new record for a specific model and a given list of fields
    pub fn create_new_record_from_map<M>(&mut self, mut data: MapOfFields) -> Result<M, Box<dyn Error>> where M: Model + 'static {
        let model_name = M::get_model_name();
        self.fill_default_values_on_map(model_name, &mut data);
        let id = self.insert_data_to_db(model_name, data)?;
        self.load_record_from_db(model_name, id)?;
        Ok(self.get_record::<M>(id)?.unwrap())
    }

    /// Create a new record for a specific model and a given model instance
    ///
    /// The returned model instance will be different that the original one
    pub fn create_new_record<M>(&mut self, model: M) -> Result<M, Box<dyn Error>> where M: Model + 'static {
        self.create_new_record_from_map::<M>(model.get_data())
    }

    /// Add default values for a given model on given data
    fn fill_default_values_on_map(&self, model_name: &'static str, data: &mut MapOfFields) {
        todo!("Insert default values");
        data.insert("test", Some(FieldType::Integer(42)));
    }
}
