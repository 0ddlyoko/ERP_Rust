use crate::environment::Environment;
use crate::field::{FieldCompute, FieldType};
use crate::field::MultipleIds;
use crate::internal::internal_field::{FinalInternalField, InternalField};
use crate::model::ModelDescriptor;
use crate::model::{CommonModel, Model};
use std::any::TypeId;
use std::collections::HashMap;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Model descriptor represented by a single struct model
pub struct InternalModel {
    pub name: String,
    pub description: Option<String>,
    pub fields: HashMap<String, InternalField>,
    pub call_computed_method: fn(&str, MultipleIds, &mut Environment) -> Result<()>,
    pub plugin_name: String,
}

/// Final descriptor of a model.
///
/// Represent all combined InternalModel
pub struct FinalInternalModel {
    pub name: String,
    pub description: String,
    pub models: HashMap<TypeId, InternalModel>,
    pub fields: HashMap<String, FinalInternalField>,
}

impl FinalInternalModel {
    pub fn new(model_name: &str) -> FinalInternalModel {
        FinalInternalModel {
            name: model_name.to_string(),
            description: "".to_string(),
            models: HashMap::new(),
            fields: HashMap::new(),
        }
    }

    pub fn register_internal_model<M>(&mut self, plugin_name: &str)
    where
        M: Model<MultipleIds> + 'static,
    {
        let name = M::get_model_name();
        let model_descriptor = M::get_model_descriptor();
        let type_id = TypeId::of::<M>();

        let ModelDescriptor {
            name: _name,
            description,
            fields,
        } = model_descriptor;

        if name != _name {
            panic!("Model name mismatch! {name} != {_name}");
        }

        let mut final_fields = HashMap::new();
        for field in fields {
            let field_name = field.name;
            let internal_field = InternalField {
                name: field_name.clone(),
                default_value: field.default_value,
                description: field.description,
                required: field.required,
                compute: field.compute,
                field_ref: field.field_ref,
            };
            self.register_internal_field(&internal_field, &type_id);
            final_fields.insert(field_name, internal_field);
        }

        let call_computed_method: fn(&str, MultipleIds, &mut Environment) -> Result<()> = |field_name, id, env| M::call_compute_method(field_name, id, env);

        let internal_model = InternalModel {
            name: name.to_string(),
            description,
            fields: final_fields,
            call_computed_method,
            plugin_name: plugin_name.to_string(),
        };

        if let Some(description) = &internal_model.description {
            self.description = description.clone();
        }
        self.models.insert(type_id, internal_model);
    }

    pub fn register_internal_field(&mut self, field_descriptor: &InternalField, type_id: &TypeId) {
        let name = &field_descriptor.name;
        let internal_field = self
            .fields
            .entry(name.to_string())
            .or_insert_with(|| FinalInternalField::new(name));
        internal_field.register_internal_field(field_descriptor, type_id);
    }

    pub fn first(&self) -> &InternalModel {
        if let Some(first_value) = self.models.values().next() {
            first_value
        } else {
            panic!("Not a single model is present");
        }
    }

    pub fn get_internal_model<M>(&self) -> &InternalModel
    where
        M: CommonModel<MultipleIds> + 'static,
    {
        let type_id = TypeId::of::<M>();
        self.models
            .get(&type_id)
            .expect("Internal model not registered")
    }

    pub fn get_internal_model_mut<M>(&mut self) -> &mut InternalModel
    where
        M: CommonModel<MultipleIds> + 'static,
    {
        let type_id = TypeId::of::<M>();
        self.models
            .get_mut(&type_id)
            .expect("Internal model not registered")
    }

    /// Get a vector of all registered fields for this model
    pub fn get_fields_name(&self) -> Vec<&str> {
        self.fields.keys().map(|s| s.as_str()).collect()
    }

    /// Get a vector of difference between all registered fields for this model, and given vector.
    ///
    /// Do not add non-stored fields
    pub fn get_missing_fields(&self, current_fields: Vec<&str>) -> Vec<&str> {
        self.fields
            .iter().filter_map(|(key, value)| {
                if value.is_stored() && !current_fields.contains(&key.as_str()) {
                    Some(key.as_str())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get a vector of stored fields
    ///
    /// TODO Find a way to save this return somewhere, as it should not change when the application
    ///  is running
    pub fn get_stored_fields(&self) -> Vec<&str> {
        self.fields.iter()
            .filter_map(|(field_name, internal_field)| {
                if internal_field.is_stored() {
                    Some(field_name.as_str())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Return true if given field is stored.
    ///
    /// If field is not present, return false
    pub fn is_stored(&self, field_name: &str) -> bool {
        self.fields.get(field_name).is_some_and(|f| f.is_stored())
    }

    /// TODO Do not panic, but instead return an Option
    pub fn get_internal_field(&self, field_name: &str) -> &FinalInternalField {
        self.fields
            .get(field_name)
            .unwrap_or_else(|| panic!("Field {} is not present in model {}", field_name, self.name))
    }

    /// TODO Do not panic, but instead return an Option
    pub fn get_internal_field_mut(&mut self, field_name: &str) -> &mut FinalInternalField {
        self.fields
            .get_mut(field_name)
            .unwrap_or_else(|| panic!("Field {} is not present in model {}", field_name, self.name))
    }

    /// Return default value for given field.
    ///
    /// If the first is not present, panic
    pub fn get_default_value(&self, field_name: &str) -> FieldType {
        let field = self.get_internal_field(field_name);
        field.default_value.clone()
    }

    /// Return true if given field is a computed field.
    ///
    /// If field is not present on this model, return false
    pub fn is_computed_field(&self, field_name: &str) -> bool {
        self.fields
            .get(field_name)
            .map_or(false, |field| field.compute.is_some())
    }

    /// Return the internal model linked to the computed given field.
    ///
    /// If field is not present on this model, return None
    ///
    /// If field is not a computed field, return None
    pub fn get_computed_field(&self, field_name: &str) -> Option<&InternalModel> {
        let field = self.fields.get(field_name)?;
        if let Some(FieldCompute { type_id, .. }) = field.compute {
            self.models.get(&type_id)
        } else {
            None
        }
    }

    /// Return the internal model linked to the computed given field.
    ///
    /// If field is not present on this model, return None
    ///
    /// If field is not a computed field, return None
    pub fn get_computed_field_mut(&mut self, field_name: &str) -> Option<&mut InternalModel> {
        let field = self.fields.get(field_name)?;
        if let Some(FieldCompute { type_id, .. }) = field.compute {
            self.models.get_mut(&type_id)
        } else {
            None
        }
    }

    /// Retrieves all models created by a specific plugin
    pub fn get_all_models_for_plugin(&self, plugin_name: &str) -> Vec<&InternalModel> {
        let mut result = vec![];
        for model in self.models.values() {
            if model.plugin_name == plugin_name {
                result.push(model);
            }
        }
        result
    }
}
