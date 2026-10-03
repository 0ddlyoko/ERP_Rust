use crate::FinalInternalField;
use crate::errors::FieldNotFound;
use crate::field::InternalField;
use crate::method::MethodRegistry;
use erp_types::field::{FieldKind, FieldKinds, FieldType, MultipleIds};
use erp_types::method::MethodFn;
use erp_types::model::{CommonModel, ModelDescriptor};
use std::any::TypeId;
use std::collections::HashMap;

/// Model descriptor represented by a single struct model
pub struct InternalModel {
    pub name: String,
    pub description: Option<String>,
    pub fields: HashMap<String, InternalField>,
    pub plugin_name: String,
}

/// Final descriptor of a model.
///
/// Represent all combined InternalModel
pub struct FinalInternalModel {
    /// Identity of the model: cache key, registry key, API name.
    pub name: String,
    /// Physical table, the only name the database layer ever sees.
    pub table_name: String,
    pub description: String,
    /// The field naming a record, when a struct said it is not `name`.
    pub declared_name_field: Option<String>,
    pub models: HashMap<TypeId, InternalModel>,
    pub fields: HashMap<String, FinalInternalField>,
    /// Methods a plugin may override, keyed by the name a caller uses.
    pub methods: MethodRegistry,
    stored_fields: Vec<String>,
}

impl FinalInternalModel {
    pub fn new(model_name: &str) -> FinalInternalModel {
        FinalInternalModel {
            name: model_name.to_string(),
            table_name: model_name.to_string(),
            description: "".to_string(),
            declared_name_field: None,
            models: HashMap::new(),
            fields: HashMap::new(),
            methods: MethodRegistry::default(),
            stored_fields: Vec::new(),
        }
    }

    pub fn register_internal_model<M>(&mut self, plugin_name: &str)
    where
        M: CommonModel<MultipleIds> + 'static,
    {
        let name = M::_get_model_name();
        let model_descriptor = M::get_model_descriptor();
        let type_id = TypeId::of::<M>();

        let ModelDescriptor {
            name: _name,
            table_name,
            description,
            name_field,
            fields,
        } = model_descriptor;

        if name != _name {
            panic!("Model name mismatch! {name} != {_name}");
        }
        // Every struct contributing to a model must agree on where it is stored.
        if self.models.is_empty() {
            self.table_name = table_name;
        } else if self.table_name != table_name {
            panic!(
                "Table name mismatch for model {name}: {} != {table_name}",
                self.table_name
            );
        }

        let mut final_fields = HashMap::new();
        for field in fields {
            let field_name = field.name;
            let internal_field = InternalField {
                name: field_name.clone(),
                kind: field.kind,
                default_value: field.default_value,
                label: field.label,
                description: field.description,
                required: field.required,
                private: field.private,
                asks_for_storage: field.asks_for_storage,
                compute: field.compute,
                field_ref: field.field_ref,
                selection: field.selection,
                tracking: field.tracking,
                owned: field.owned,
            };
            self.register_internal_field(&internal_field);
            final_fields.insert(field_name, internal_field);
        }

        let internal_model = InternalModel {
            name: name.to_string(),
            description,
            fields: final_fields,
            plugin_name: plugin_name.to_string(),
        };

        if let Some(description) = &internal_model.description {
            self.description = description.clone();
        }
        if name_field.is_some() {
            self.declared_name_field = name_field;
        }
        self.models.insert(type_id, internal_model);
    }

    /// The field naming a record: the one declared, else `name` when the model has one.
    ///
    /// A record whose name is worked out is named by a computed field, declared like any other.
    pub fn name_field(&self) -> Option<&str> {
        match &self.declared_name_field {
            Some(declared) => Some(declared.as_str()),
            None => self.fields.contains_key("name").then_some("name"),
        }
    }

    pub fn register_internal_field(&mut self, field_descriptor: &InternalField) {
        let name = &field_descriptor.name;
        let internal_field = self
            .fields
            .entry(name.to_string())
            .or_insert_with(|| FinalInternalField::new(name));
        internal_field.register_internal_field(field_descriptor);
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

    /// Decide which of this model's fields are kept in a column, and remember the list.
    ///
    /// Once, when every struct contributing to the model has been registered — never per struct.
    /// A struct may mention a field another one computes without repeating the computation, and
    /// on its own it looks like a plain field, which would be kept.
    pub fn settle_storage(&mut self) -> Result<(), String> {
        for field in self.fields.values_mut() {
            field
                .settle_storage()
                .map_err(|wrong| format!("Model {}: {wrong}", self.name))?;
        }
        self.stored_fields = self
            .fields
            .values()
            .filter(|field| field.is_stored())
            .map(|field| field.name.clone())
            .collect();
        self.stored_fields.sort_unstable();
        Ok(())
    }

    /// Do not add non-stored fields
    pub fn get_missing_fields(&self, current_fields: Vec<&str>) -> Vec<&str> {
        self.fields
            .iter()
            .filter_map(|(key, value)| {
                if value.is_stored() && !current_fields.contains(&key.as_str()) {
                    Some(key.as_str())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Fields kept in a column, worked out once when storage is settled rather than on each load.
    pub fn get_stored_fields(&self) -> Vec<&str> {
        self.stored_fields.iter().map(String::as_str).collect()
    }

    /// Return true if given field is stored.
    ///
    /// If field is not present, return false
    pub fn is_stored(&self, field_name: &str) -> bool {
        self.fields.get(field_name).is_some_and(|f| f.is_stored())
    }

    /// Look a field up by name.
    ///
    /// This is the entry point for any name that did not come from the framework itself, such as
    /// one carried by an API request, and the only one that does not panic on a typo.
    pub fn try_get_internal_field(
        &self,
        field_name: &str,
    ) -> std::result::Result<&FinalInternalField, FieldNotFound> {
        self.fields.get(field_name).ok_or_else(|| FieldNotFound {
            model_name: self.name.clone(),
            field_name: field_name.to_string(),
        })
    }

    /// Same as [`FinalInternalModel::try_get_internal_field`], for mutable access.
    pub fn try_get_internal_field_mut(
        &mut self,
        field_name: &str,
    ) -> std::result::Result<&mut FinalInternalField, FieldNotFound> {
        let model_name = self.name.clone();
        self.fields
            .get_mut(field_name)
            .ok_or_else(|| FieldNotFound {
                model_name,
                field_name: field_name.to_string(),
            })
    }

    /// Look a field up by a name the framework itself produced.
    ///
    /// # Panics
    /// Panics if the field is unknown. Callers pass names fixed at compile time by the derive
    /// macro, so a failure here is a framework bug rather than bad input. Use
    /// [`FinalInternalModel::try_get_internal_field`] for anything else.
    pub fn get_internal_field(&self, field_name: &str) -> &FinalInternalField {
        self.try_get_internal_field(field_name)
            .unwrap_or_else(|err| panic!("{err}"))
    }

    /// Same as [`FinalInternalModel::get_internal_field`], for mutable access.
    ///
    /// # Panics
    /// Panics if the field is unknown; see [`FinalInternalModel::get_internal_field`].
    pub fn get_internal_field_mut(&mut self, field_name: &str) -> &mut FinalInternalField {
        self.try_get_internal_field_mut(field_name)
            .unwrap_or_else(|err| panic!("{err}"))
    }

    /// Default value declared for given field, if it declares one.
    pub fn get_default_value(&self, field_name: &str) -> Option<FieldType> {
        let field = self.get_internal_field(field_name);
        field.default_value.clone()
    }

    /// Field on this model that is the other end of a relation table.
    ///
    /// The two sides of a many2many name the same table independently, so the pairing is found
    /// by matching on it rather than being declared twice.
    pub fn field_of_relation(&self, relation: &str) -> Option<&str> {
        self.fields.iter().find_map(|(name, field)| {
            matches!(
                &field.inverse,
                Some(erp_types::field::FieldReference {
                    inverse_field: erp_types::field::FieldReferenceType::M2M { relation: other, .. },
                    ..
                }) if other == relation
            )
            .then_some(name.as_str())
        })
    }

    /// Method that fills a field, if it is computed at all.
    pub fn compute_method(&self, field_name: &str) -> Option<&str> {
        let field = self.fields.get(field_name)?;
        field
            .compute
            .as_ref()
            .map(|compute| compute.method.as_str())
    }

    /// Implementations of a field's compute, most-derived first.
    ///
    /// A compute is an overridable method taking no arguments and returning nothing, so its
    /// implementations live in the same registry as every other method's.
    ///
    /// `None` when the field is unknown, carries no compute, or names a method no struct
    /// implemented.
    pub fn compute_chain(&self, field_name: &str) -> Option<&[MethodFn<(), ()>]> {
        self.methods
            .chain::<(), ()>(self.compute_method(field_name)?)
    }

    /// Return true if given field is a computed field.
    ///
    /// If field is not present on this model, return false
    pub fn is_computed_field(&self, field_name: &str) -> bool {
        self.fields
            .get(field_name)
            .is_some_and(|field| field.compute.is_some())
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

/// Lets a record be read off the wire straight against the registry.
impl FieldKinds for FinalInternalModel {
    fn kind_of(&self, field_name: &str) -> Option<FieldKind> {
        self.fields.get(field_name).map(|field| field.kind)
    }
}
