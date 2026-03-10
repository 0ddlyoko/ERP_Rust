use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

// pub trait EnvironmentBase<'mm> {
//     // ------------------------------------------
//     // |             Database Logic             |
//     // ------------------------------------------
//
//     /// Flush cache to the database, commit, and close the transaction.
//     fn close(self) -> Result<()>;
//
//     /// Load given records from the database to the cache.
//     ///
//     /// If the record is already present in cache, do nothing
//     fn load_records_from_db<Mode: IdMode>(
//         &mut self,
//         model_name: &str,
//         ids: &Mode,
//     ) -> Result<()>;
//
//     /// Load fields of given records from the database to the cache.
//     ///
//     /// If fields are already loaded, they will still be retrieved from the database but not updated
//     fn load_records_fields_from_db<Mode: IdMode>(
//         &mut self,
//         model_name: &str,
//         ids: &Mode,
//         fields: &[&str],
//     ) -> Result<()>;
//
//     fn get_fields_to_save(
//         &self,
//         model_name: &str,
//         fields: &Vec<&LeftTuple>,
//     ) -> Result<HashMap<&'mm str, Vec<&'mm str>>>;
//
//     /// Save fields linked to a specific domain into the database
//     fn save_domain_fields_to_db(&mut self, model_name: &str, domain: &SearchType) -> Result<()>;
//     /// Save all data from cache to the database
//     fn save_all_to_db(&mut self) -> Result<()>;
//
//     /// Save all data related to given model to database.
//     ///
//     /// Compute non-stored fields related to this model if needed.
//     fn save_model_to_db(&mut self, model_name: &str) -> Result<()>;
//
//     /// Save given fields to database.
//     ///
//     /// Compute them if needed.
//     ///
//     /// Remove from the original list non-stored fields
//     fn save_fields_to_db(&mut self, model_name: &str, fields: &[&str]) -> Result<()>;
//
//     /// Save given record to the database.
//     ///
//     /// If the record is already saved, do nothing
//     ///
//     /// If the record is not present in cache, do nothing
//     ///
//     /// If given model does not exist, panic.
//     fn save_records_to_db<Mode: IdMode>(&mut self, model_name: &str, ids: &Mode) -> Result<()>;
//
//     /// Get all dirty stored fields for given model
//     fn get_dirty_stored_models(&self, model_name: &str) -> HashMap<u32, MapOfFields>;
//
//     /// Get dirty fields from given list of fields
//     fn get_dirty_fields(&self, model_name: &str, fields: &[&str]) -> HashMap<u32, MapOfFields>;
//
//     /// Get all dirty stored fields for given records
//     fn get_dirty_stored_records(
//         &self,
//         model_name: &str,
//         ids: &[u32],
//     ) -> HashMap<u32, MapOfFields>;
//
//     /// Get all dirty filtered fields for given records
//     fn get_dirty_filtered_records<F>(
//         &self,
//         model_name: &str,
//         ids: &[u32],
//         field_filter: F,
//     ) -> HashMap<u32, MapOfFields>
//     where
//         F: Fn(&str) -> bool;
//
//     fn get_fields_from_db(
//         &mut self,
//         model_name: &str,
//         ids: &MultipleIds,
//         fields: &[&str],
//     ) -> Result<HashMap<SingleId, MapOfFields>>;
//
//     /// Save existing data to the database.
//     ///
//     /// This method does not check if given fields are stored or not.
//     /// It's up to the caller to ensure given data are correct.
//     ///
//     /// Returns the number of lines updated
//     #[allow(dead_code)]
//     fn save_data_to_db(
//         &mut self,
//         model_name: &str,
//         data: &HashMap<u32, &MapOfFields>,
//     ) -> Result<u32>;
//
//     /// Insert new data to the database.
//     ///
//     /// This method does not check if given fields are stored or not.
//     /// It's up to the caller to ensure given data are correct.
//     fn insert_data_to_db(
//         &mut self,
//         model_name: &str,
//         data: &Vec<&MapOfFields>,
//     ) -> Result<Vec<u32>>;
//
//     // ------------------------------------------
//     // |             Retrieve Logic             |
//     // ------------------------------------------
//
//     /// Returns an instance of given model for a specific id
//     ///
//     /// Do not check if given id is valid id, or is present in the cache
//     ///
//     /// Do not load given id to the cache
//     fn get_record<M, Mode: IdMode>(&self, id: Mode) -> M
//     where
//         M: Model<Mode>;
//
//     /// Search given domain for given model, and return an instance of given model if found
//     ///
//     /// If not found, return an empty recordset
//     ///
//     /// This method does not load in cache any data related to the model.
//     /// It only performs a search, and return the given ids.
//     ///
//     /// Before performing any search, save any data related to any field given in the domain.
//     fn search<M>(&mut self, domain: &SearchType) -> Result<M>
//     where
//         M: Model<MultipleIds>;
//
//     /// Get the value of given field for given id.
//     ///
//     /// If field is not in cache, load it
//     ///
//     /// If field needs to be computed, compute it
//     fn get_field_value<'a>(
//         &'a mut self,
//         model_name: &str,
//         field_name: &str,
//         id: &SingleId,
//     ) -> Result<Option<&'a FieldType>>;
//
//     fn get_fields_value<Mode: IdMode>(
//         &mut self,
//         model_name: &str,
//         field_name: &str,
//         ids: &Mode,
//     ) -> Result<Vec<Option<&FieldType>>>;
//
//     /// Ensure given field is in cache for given ids
//     ///
//     /// If some ids are invalid or need to be loaded, load them (or compute them if needed)
//     ///
//     /// If given field_name is a O2M, load it along with its M2O
//     fn ensure_fields_in_cache<Mode: IdMode>(
//         &mut self,
//         model_name: &str,
//         field_name: &str,
//         ids: &Mode,
//     ) -> Result<()>;
//
//     // ------------------------------------------
//     // |           Save to Cache Logic          |
//     // ------------------------------------------
//
//     fn save_value_to_cache<Mode: IdMode, E>(
//         &mut self,
//         model_name: &str,
//         field_name: &str,
//         ids: &Mode,
//         value: E,
//     ) -> Result<()>
//     where
//         E: Into<FieldType>;
//
//     fn save_option_to_cache<Mode: IdMode, E>(
//         &mut self,
//         model_name: &str,
//         field_name: &str,
//         ids: &Mode,
//         value: Option<E>,
//     ) -> Result<()>
//     where
//         E: Into<FieldType>;
//
//     /// Retrieve given field from the cache, or from the database if not loaded in cache
//     ///
//     /// If field is retrieved from the database, it will not be added to the cache
//     ///
//     /// If field is not stored, return the default value
//     ///
//     /// Return a vector sorted by given ids of tuple.
//     /// First element is true if it's from the cache, or false if it's from the database.
//     /// Second element is the value
//     fn retrieve_field_from_cache_or_database<Mode: IdMode>(
//         &mut self,
//         model_name: &str,
//         field_name: &str,
//         ids: &Mode,
//     ) -> Result<Vec<(bool, Option<FieldType>)>>;
//
//     /// Save given field to cache.
//     ///
//     /// This method ensure M2O & O2M are correctly linked in cache (if those fields are loaded)
//     fn save_field_to_cache<Mode: IdMode>(
//         &mut self,
//         model_name: &str,
//         field_name: &str,
//         ids: &Mode,
//         value: Option<FieldType>,
//         update_dirty: &Dirty,
//         update_field: &Update,
//     ) -> Result<()>;
//
//     /// Method called when a field has changed, and will set as to recompute all fields that needs to be recomputed
//     ///
//     /// We shouldn't call this method from a O2M, as a O2M field shouldn't have any dependencies
//     fn check_compute_on_field(
//         &mut self,
//         model_name: &str,
//         field_name: &str,
//         ids: &[u32],
//     ) -> Result<()>;
//
//     /// Create a new record for a specific model and a given list of fields
//     fn create_new_record_from_map<M>(&mut self, data: MapOfFields) -> Result<M>
//     where
//         M: Model<SingleId>;
//
//     /// Create new records for a specific model and multiple lists of fields
//     fn create_new_records_from_maps<M>(&mut self, data: Vec<MapOfFields>) -> Result<M>
//     where
//         M: Model<MultipleIds>;
//
//     fn _create_new_records(
//         &mut self,
//         model_name: &str,
//         data: Vec<MapOfFields>,
//     ) -> Result<MultipleIds>;
//
//     /// Add default values for a given model on given data
//     fn fill_default_values_on_map(
//         &self,
//         model_name: &str,
//         data: &mut MapOfFields,
//     ) -> Option<Vec<&'mm str>>;
//
//     // ------------------------------------------
//     // |              Other Logic               |
//     // ------------------------------------------
//
//     /// Create a new savepoint and commit if the given method doesn't return any error.
//     /// If an error is returned, rollback the commit and put back the cache as it was
//     fn savepoint<F, R>(&mut self, func: F) -> Result<R>
//     where
//         F: FnOnce(&mut Self) -> Result<R>;
//
//     // ------------------------------------------
//     // |            Computed methods            |
//     // ------------------------------------------
//
//     /// Call computed method on all stored fields that need to be computed for given model
//     fn call_computed_method_on_all_fields(&mut self, model_name: &str) -> Result<()>;
//
//     /// Call computed method on computed & non-computed fields that need to be computed for given model
//     fn call_computed_method_on_fields(&mut self, model_name: &str, fields: &[&str]) -> Result<()>;
//
//     /// Call computed method on non-stored fields that need to be computed for given model, for given ids
//     fn call_computed_method_on_ids(&mut self, model_name: &str, ids: &[u32]) -> Result<()>;
//
//     /// Call computed methods of given fields of given model for given ids
//     fn call_compute_method<Mode: IdMode>(
//         &mut self,
//         model_name: &str,
//         ids: &Mode,
//         fields: &[&str],
//     ) -> Result<()>;
// }
