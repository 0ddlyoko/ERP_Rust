//! Creating records and applying default values.
use super::*;
use crate::access::{Access, AccessDenied, Operation};
use crate::model::{CREATE, CreateArgs};
use crate::model::{CREATE_DATE, CREATE_UID, WRITE_DATE, WRITE_UID};
use chrono::Utc;
use erp_internal_types::{FinalInternalField, FinalInternalModel};
use erp_types::field::Command;

impl<'mm> Environment<'mm> {
    /// Create a new record for a specific model and a given list of fields; an empty record when
    /// an override of its `create` made none.
    pub fn create_new_record_from_map<M>(&mut self, data: MapOfFields) -> Result<M>
    where
        M: Model<SingleId>,
    {
        let model_name = M::_get_model_name();
        let ids = self.create_records(model_name, vec![data])?;
        let id = ids.get_ids_ref().first().copied().unwrap_or(0);
        Ok(self.get_record::<M, SingleId>(id.into()))
    }

    /// Create new records for a specific model and multiple lists of fields
    pub fn create_new_records_from_maps<M>(&mut self, data: Vec<MapOfFields>) -> Result<M>
    where
        M: Model<MultipleIds>,
    {
        let model_name = M::_get_model_name();
        let ids = self.create_records(model_name, data)?;
        Ok(self.get_record::<M, MultipleIds>(ids))
    }

    /// Create records from untyped field maps, addressing the model by name.
    ///
    /// The entry point for callers that only hold a model name at runtime; the typed
    /// [`Environment::create_new_record_from_map`] resolves the name from `M` and lands here.
    /// A one2many or a many2many may be given commands: its lines are created once the record
    /// exists, pointing back to it.
    ///
    /// Goes through the model's `create`, so what a plugin overrode there runs.
    pub fn create_records(
        &mut self,
        model_name: &str,
        data: Vec<MapOfFields>,
    ) -> Result<MultipleIds> {
        self.model_manager.try_get_model(model_name)?;
        self.call_method::<CreateArgs, MultipleIds>(
            model_name,
            CREATE,
            &MultipleIds::default(),
            &(data,),
        )
    }

    /// Create a record from its name alone, as `(id, name)`: what typing a name that matches
    /// nothing offers to do. Refused for a model named by no field, or by a private one, and
    /// when other fields are required.
    pub fn name_create(&mut self, model_name: &str, name: &str) -> Result<(u32, String)> {
        let model = self.model_manager.try_get_model(model_name)?;
        let Some(name_field) = model
            .name_field()
            .filter(|field| !model.fields[*field].private)
        else {
            return Err(format!("A {model_name} is not created from a name").into());
        };
        let mut values = MapOfFields::default();
        values.insert(name_field, name);
        let ids = self.create_records(model_name, vec![values])?;
        let id = *ids
            .get_ids_ref()
            .first()
            .ok_or("Creating the record returned no id")?;
        Ok((id, name.to_string()))
    }

    /// Create records, refused unless the caller may create every one of them.
    ///
    /// Whether a record falls within the rights depends on its values, so the check runs once it
    /// exists — inside a savepoint, so that a refused record does not outlive the refusal. The
    /// savepoint copies the cache, and is only paid for when a domain actually restricts.
    pub(crate) fn _create_new_records(
        &mut self,
        model_name: &str,
        data: Vec<MapOfFields>,
    ) -> Result<MultipleIds> {
        let fields: Vec<String> = data
            .iter()
            .flat_map(|map| map.fields.keys().cloned())
            .collect();
        let fields: Vec<&str> = fields.iter().map(String::as_str).collect();
        let ids = match self.access(model_name, Operation::Create)? {
            Access::Unrestricted | Access::Restricted(SearchType::Nothing) => {
                self.insert_new_records(model_name, data)?
            }
            Access::Denied => {
                return Err(
                    AccessDenied::new(model_name, Operation::Create, &fields, Vec::new()).into(),
                );
            }
            Access::Restricted(_) => self.savepoint(|env| {
                let ids = env.insert_new_records(model_name, data)?;
                env.check_access(model_name, Operation::Create, ids.get_ids_ref(), &fields)?;
                Ok(ids)
            })?,
        };
        self.refuse_emptied_relations()?;
        Ok(ids)
    }

    /// Refuse a record created with a required field left empty, its defaults already given.
    fn refuse_empty_required_on_create(
        model: &FinalInternalModel,
        values: &MapOfFields,
    ) -> Result<()> {
        for field in model.fields.values() {
            if field.required
                && field.compute.is_none()
                && !is_x2many(field)
                && values
                    .fields
                    .get(&field.name)
                    .is_none_or(required::is_empty_value)
            {
                return Err(Self::required_error(&model.name, field));
            }
        }
        Ok(())
    }

    fn insert_new_records(
        &mut self,
        model_name: &str,
        mut data: Vec<MapOfFields>,
    ) -> Result<MultipleIds> {
        let final_model = self.model_manager.get_model(model_name);
        let mut missing_fields_lst = Vec::new();

        // Commands of a one2many or a many2many are carried out once the records exist: a line
        // created for one points back to its record.
        let commands: Vec<Vec<(String, Vec<Command>)>> = data
            .iter_mut()
            .map(|d| {
                let names: Vec<String> = d
                    .fields
                    .iter()
                    .filter(|(_, value)| matches!(value, Some(FieldType::Commands(_))))
                    .map(|(name, _)| name.clone())
                    .collect();
                names
                    .into_iter()
                    .filter_map(|name| match d.fields.remove(&name) {
                        Some(Some(FieldType::Commands(commands))) => Some((name, commands)),
                        _ => None,
                    })
                    .collect()
            })
            .collect();

        let written_by_hand: Vec<Vec<String>> = data
            .iter()
            .map(|d| {
                let fields: Vec<&str> = d.fields.keys().map(String::as_str).collect();
                self.editable_computed(model_name, &fields)
            })
            .collect();

        // Add missing fields
        for d in data.iter_mut() {
            for value in d.fields.values_mut() {
                *value = match Self::without_empty_references(value.take()) {
                    Some(FieldType::String(text)) if text.is_empty() => None,
                    value => value,
                };
            }
            for (field_name, value) in &d.fields {
                let field = final_model.try_get_internal_field(field_name)?;
                Self::refuse_automatic(model_name, field)?;
                Self::refuse_wrong_kind(model_name, field, value)?;
            }
            let missing_fields = self.fill_default_values_on_map(model_name, d);
            Self::refuse_empty_required_on_create(final_model, d)?;
            missing_fields_lst.push(missing_fields);
            self.stamp_created(final_model, d);
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
                    self.save_option_to_cache_unchecked::<SingleId, _>(
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

        for (id, fields) in ids.iter().zip(&written_by_hand) {
            self.keep_written_by_hand(model_name, fields, &[*id]);
        }
        self.forget_access_of(model_name, &ids)?;
        self.forget_shared_of(model_name);
        for (id, commands) in ids.iter().zip(commands) {
            for (field_name, commands) in commands {
                self.write_commands(model_name, &field_name, *id, commands)?;
            }
        }
        for (field_name, field) in &final_model.fields {
            if is_x2many(field) {
                self.note_maybe_emptied(model_name, field_name, ids.iter().copied())?;
            }
        }
        // What a record is created with is no change of it.
        self.forget_tracked(model_name, &ids);
        for hook in self.model_manager.create_hooks.clone() {
            hook(self, model_name, &ids)?;
        }
        Ok(ids.into())
    }

    /// Note when, and by whom, a record about to be created is created — and so last changed.
    fn stamp_created(&self, model: &FinalInternalModel, values: &mut MapOfFields) {
        let now = Utc::now();
        for field in [CREATE_DATE, WRITE_DATE] {
            if model.fields.contains_key(field) {
                values.insert(field, now);
            }
        }
        if let Some(uid) = self.uid {
            for field in [CREATE_UID, WRITE_UID] {
                if model.fields.contains_key(field) {
                    values.insert(field, uid);
                }
            }
        }
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
            // A field that declares no default simply starts empty.
            match final_internal_model.get_default_value(missing_field_to_load) {
                Some(default_value) => data.insert_field_type(missing_field_to_load, default_value),
                None => data.insert_none(missing_field_to_load),
            }
        }
        Some(missing_fields_to_load)
    }
}

/// Whether a field holds a list of records: a one2many or a many2many.
fn is_x2many(field: &FinalInternalField) -> bool {
    matches!(
        field.inverse,
        Some(FieldReference {
            inverse_field: FieldReferenceType::O2M { .. } | FieldReferenceType::M2M { .. },
            ..
        })
    )
}
