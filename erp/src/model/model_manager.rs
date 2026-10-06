use crate::access::AccessRules;
use crate::assets::AssetRegistry;
use crate::environment::Environment;
use crate::http::ControllerRegistry;
use crate::identity::Identities;
use crate::model::HasMethods;
use crate::model::Model;
use crate::model::ModelNotFound;
use crate::model::rpc::{RpcFn, RpcRegistry};
use crate::model::selections::Selections;
use crate::shared_cache::SharedCaches;
use erp_internal_types::{FinalInternalField, FinalInternalModel, InternalField, InternalModel};
use erp_types::field::FieldCompute;
use erp_types::field::MultipleIds;
use erp_types::field::{FieldDepend, FieldReference, FieldReferenceType};
use erp_types::field::{FieldKind, FieldKinds, FieldType, Selection};
use erp_types::method::{MethodFn, Receiver};
use std::collections::{HashMap, HashSet};

/// Work a plugin asks to do once any plugin has loaded its data, given that plugin's name.
///
/// For what a plugin declares and others extend or supply: records to bring in line with what
/// the loaded plugin ships, or a check that must also cover what later plugins bring.
pub type LoadHook =
    fn(&mut Environment, &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

/// A tracked field of one record that changed: what it held, and what it holds now.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackedChange {
    pub field: String,
    pub old: Option<FieldType>,
    pub new: Option<FieldType>,
}

/// Work a plugin asks to do when tracked fields of a record changed, given the model, the
/// record's id, who changed them and what changed. Runs before the changes are saved, so what it
/// writes is saved with them.
pub type TrackingHook = fn(
    &mut Environment,
    &str,
    u32,
    Option<u32>,
    &[TrackedChange],
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

/// Work a plugin asks to do once records are created, given the model and their ids, as whoever
/// created them.
pub type CreateHook =
    fn(&mut Environment, &str, &[u32]) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

/// Work a plugin asks to do once records are deleted, given the model and their ids: removing
/// what pointed at them without a relation the ORM knows of.
pub type DeleteHook =
    fn(&mut Environment, &str, &[u32]) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

/// When a record was created, filled in by the ORM on every model.
pub const CREATE_DATE: &str = "create_date";
/// When a record was last changed.
pub const WRITE_DATE: &str = "write_date";
/// Who created a record, once a plugin names the model of users.
pub const CREATE_UID: &str = "create_uid";
/// Who last changed a record.
pub const WRITE_UID: &str = "write_uid";

/// Add a field the ORM fills in, unless the model has it already: a plugin may load after others.
fn add_automatic_field(
    model: &mut FinalInternalModel,
    name: &str,
    label: &str,
    kind: FieldKind,
    field_ref: Option<FieldReference>,
) {
    if model.fields.contains_key(name) {
        return;
    }
    let mut field = FinalInternalField::new(name);
    field.register_internal_field(&InternalField {
        name: name.to_string(),
        kind,
        default_value: None,
        label: Some(label.to_string()),
        description: None,
        required: false,
        private: false,
        asks_for_storage: false,
        compute: None,
        field_ref,
        selection: None,
        tracking: false,
        owned: false,
        on_delete: None,
        domain: None,
    });
    field.automatic = true;
    model.fields.insert(name.to_string(), field);
}

#[derive(Default)]
pub struct ModelManager {
    models: HashMap<String, FinalInternalModel>,
    /// Methods a remote caller may reach. Held here rather than on the model, so that the crates
    /// describing types never have to know about JSON.
    pub rpc: RpcRegistry,
    /// How a token identifies its caller. Beside the registry for the same reason as `rpc`: it is
    /// what a plugin contributes about reaching models, not what a model is.
    pub identities: Identities,
    pub access: AccessRules,
    pub controllers: ControllerRegistry,
    pub assets: AssetRegistry,
    pub load_hooks: Vec<LoadHook>,
    pub tracking_hooks: Vec<TrackingHook>,
    pub create_hooks: Vec<CreateHook>,
    pub delete_hooks: Vec<DeleteHook>,
    pub shared_caches: SharedCaches,
    pub selections: Selections,
    data_bodies: HashMap<String, String>,
    data_children: HashMap<String, String>,
    pub(crate) loaded_plugins: Vec<String>,
    pub(crate) current_plugin_loading: Option<String>,
    /// The demo documents of each plugin loaded, by its name.
    pub(crate) demo: HashMap<String, Vec<&'static str>>,
}

impl ModelManager {
    /// Register a model, and with it the overridable methods its struct declares.
    pub fn register_model<M>(&mut self)
    where
        M: Model<MultipleIds> + HasMethods + 'static,
    {
        let plugin_name = match &self.current_plugin_loading {
            Some(plugin_name) => plugin_name,
            None => "Unknown",
        };
        let model_name = M::_get_model_name();

        self.models
            .entry(model_name.to_string())
            .or_insert_with(|| FinalInternalModel::new(model_name))
            .register_internal_model::<M>(plugin_name);

        let plugin_name = plugin_name.to_string();
        super::register_crud::<M>(self);
        M::register_methods(self, &plugin_name);
    }

    /// Apply what an enum declared with `#[selection(extends = ...)]` names, adds, moves or
    /// relabels in its family. Enums are applied in the order plugins register them.
    pub fn register_selection<E: Selection>(&mut self) {
        self.selections.extend::<E>();
    }

    /// Expose a method to remote callers.
    ///
    /// Called by generated code for methods carrying `#[erp(rpc)]`, and by nothing else.
    pub fn register_rpc(&mut self, model_name: &str, method_name: &str, call: RpcFn) {
        self.rpc.register(model_name, method_name, call);
    }

    /// Add one implementation to a method's chain.
    ///
    /// Called by generated code, right after the model itself is registered.
    pub fn register_method<A, R>(
        &mut self,
        model_name: &str,
        method_name: &str,
        link: MethodFn<A, R>,
        receiver: Receiver,
        plugin_name: &str,
    ) where
        A: 'static,
        R: 'static,
    {
        self.models
            .entry(model_name.to_string())
            .or_insert_with(|| FinalInternalModel::new(model_name))
            .methods
            .register(model_name, method_name, link, receiver, plugin_name);
    }

    /// Execute some final modification when models are registered, like:
    /// - Linking M2O => O2M (as there is already a link between O2M => M2O)
    pub fn post_register(&mut self) {
        self._post_register_name_fields();
        self._post_register_automatic_fields();
        self._post_register_selections();
        self._post_register_storage();
        self._post_register_m2o_links();
        self._post_register_compute_links();
    }

    /// Give every model the fields the ORM fills in on its own: when each record was created and
    /// last changed, and — once a plugin names the model of users — by whom.
    ///
    /// Real fields, read, searched and sorted like any other, but written by nobody else.
    fn _post_register_automatic_fields(&mut self) {
        let user_model = self
            .identities
            .user_model()
            .filter(|model| self.models.contains_key(*model));
        for model in self.models.values_mut() {
            for (name, label) in [(CREATE_DATE, "Created on"), (WRITE_DATE, "Last updated on")] {
                add_automatic_field(model, name, label, FieldKind::DateTime, None);
            }
            if let Some(user_model) = user_model {
                for (name, label) in [(CREATE_UID, "Created by"), (WRITE_UID, "Last updated by")] {
                    let reference = FieldReference {
                        target_model: user_model,
                        inverse_field: FieldReferenceType::M2O {
                            inverse_fields: Vec::new(),
                        },
                    };
                    add_automatic_field(model, name, label, FieldKind::Ref, Some(reference));
                }
            }
        }
    }

    /// Start the family of every field holding an enum, and refuse a default its family lacks.
    fn _post_register_selections(&mut self) {
        for model in self.models.values() {
            for field in model.fields.values() {
                let Some(family) = field.selection else {
                    continue;
                };
                self.selections.ensure(family);
                if let Some(FieldType::String(default)) = &field.default_value
                    && !self.selections.contains(family.family, default)
                {
                    panic!(
                        "Field \"{}\" of model \"{}\" defaults to \"{default}\", which is not one of \
                         its values: {}",
                        field.name,
                        model.name,
                        self.selections
                            .choices(family.family)
                            .iter()
                            .map(|choice| choice.key.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
            }
        }
    }

    /// Refuse a declared name field the model does not have: names would quietly be ids instead.
    fn _post_register_name_fields(&self) {
        for model in self.models.values() {
            if let Some(name_field) = &model.declared_name_field
                && !model.fields.contains_key(name_field)
            {
                panic!(
                    "Model \"{}\" is named by its field \"{name_field}\", which it does not have",
                    model.name
                );
            }
        }
    }

    /// Decide which fields are kept in a column.
    ///
    /// Here rather than in the derive macro, because a struct alone cannot know: one may mention
    /// a field another computes without repeating the computation, and on its own it looks like a
    /// plain field. Only once every struct contributing to a model has been seen does "is this
    /// field computed" have an answer.
    fn _post_register_storage(&mut self) {
        for model in self.models.values_mut() {
            if let Err(wrong) = model.settle_storage() {
                panic!("{wrong}");
            }
        }
    }

    /// Tell each many2one which one2many fields mirror it.
    ///
    /// Refuses a one2many whose many2one is not kept in a column: the one2many is found by
    /// searching that column, and a search on a value worked out on each read has nothing to find.
    fn _post_register_m2o_links(&mut self) {
        // Clear M2O depends
        for model in self.models.values_mut() {
            for field in model.fields.values_mut() {
                if let Some(FieldReference {
                    inverse_field: FieldReferenceType::M2O { inverse_fields },
                    ..
                }) = &mut field.inverse
                {
                    inverse_fields.clear();
                }
            }
        }

        // Now, get the new list
        let mut fields_to_modify: HashMap<String, HashMap<String, Vec<String>>> = HashMap::new();
        for model in self.models.values() {
            for field in model.fields.values() {
                if let Some(FieldReference {
                    target_model,
                    inverse_field: FieldReferenceType::O2M { inverse_field },
                }) = &field.inverse
                {
                    let model_to_modify = fields_to_modify
                        .entry(target_model.to_string())
                        .or_default();
                    let field_to_modify = model_to_modify.entry(inverse_field.clone()).or_default();
                    field_to_modify.push(field.name.clone());
                }
            }
        }

        // Finally, modify them
        for (model_name, model_to_add) in fields_to_modify {
            let model = self.get_model_mut(&model_name);
            for (field_name, mut fields_to_add) in model_to_add {
                let field = model.get_internal_field_mut(&field_name);
                if field.inverse.is_some() && !field.is_stored() {
                    panic!(
                        "{model_name}.{field_name} is the inverse of a one2many, but is worked out \
                         on each read: a one2many is found by searching its column. Declare it \
                         `stored`."
                    );
                }
                if let Some(FieldReference {
                    inverse_field: FieldReferenceType::M2O { inverse_fields },
                    ..
                }) = &mut field.inverse
                {
                    inverse_fields.append(&mut fields_to_add);
                    // Check uniqueness
                    let mut seen = HashSet::new();
                    inverse_fields.retain(|field| seen.insert(field.clone()));
                } else {
                    panic!(
                        "A field is targeting {}.{} as an inverse field, but this field is not a M2O",
                        model_name, field_name
                    );
                }
            }
        }
    }

    fn _post_register_compute_links(&mut self) {
        // Clear depends
        for model in self.models.values_mut() {
            for field in model.fields.values_mut() {
                field.depends.clear();
            }
        }
        // Now, compute them
        let mut fields_to_update: HashMap<String, HashMap<String, Vec<Vec<FieldDepend>>>> =
            HashMap::new();
        for model in self.models.values() {
            for field in model.fields.values() {
                if let Some(FieldCompute { depends, .. }) = &field.compute {
                    let computed = &field.name;
                    for depend in depends {
                        let mut final_depends: Vec<FieldDepend> = vec![FieldDepend::SameModel {
                            field_name: field.name.clone(),
                        }];
                        let mut current_model = model;
                        let depend_split = depend.split(".").collect::<Vec<&str>>();
                        let size = depend_split.len();
                        for (i, d) in depend_split.iter().enumerate() {
                            let field = current_model.get_internal_field(d);
                            let is_last = i == size - 1;
                            if is_last {
                                // Save to field
                                let mut new_final_depends = final_depends.clone();
                                new_final_depends.reverse();
                                let vec = fields_to_update
                                    .entry(current_model.name.clone())
                                    .or_default()
                                    .entry(field.name.clone())
                                    .or_default();
                                vec.push(new_final_depends);
                            } else if let Some(FieldReference {
                                target_model,
                                inverse_field,
                            }) = &field.inverse
                            {
                                match inverse_field {
                                    // Symmetric with the one2many below: the hop back is the
                                    // field on the other side that names the same relation table.
                                    FieldReferenceType::M2M { relation, .. } => {
                                        let target = self.get_model(target_model);
                                        let Some(mirror) = target.field_of_relation(relation)
                                        else {
                                            panic!(
                                                "Field {}.{} names relation {relation}, which model {target_model} does not declare",
                                                current_model.name, field.name
                                            )
                                        };
                                        let mirror = mirror.to_string();
                                        final_depends.push(FieldDepend::CurrentFieldAnotherModel {
                                            target_model: current_model.name.clone(),
                                            field_name: mirror.clone(),
                                        });

                                        // Save to field
                                        let mut new_final_depends = final_depends.clone();
                                        new_final_depends.reverse();
                                        let vec = fields_to_update
                                            .entry(target_model.to_string())
                                            .or_default()
                                            .entry(mirror)
                                            .or_default();
                                        vec.push(new_final_depends);
                                    }
                                    FieldReferenceType::O2M { inverse_field } => {
                                        final_depends.push(FieldDepend::CurrentFieldAnotherModel {
                                            target_model: current_model.name.clone(),
                                            field_name: inverse_field.clone(),
                                        });

                                        // Save to field
                                        let mut new_final_depends = final_depends.clone();
                                        new_final_depends.reverse();
                                        let vec = fields_to_update
                                            .entry(target_model.to_string())
                                            .or_default()
                                            .entry(inverse_field.clone())
                                            .or_default();
                                        vec.push(new_final_depends);
                                    }
                                    FieldReferenceType::M2O { .. } => {
                                        // Save to field
                                        let mut new_final_depends = final_depends.clone();
                                        new_final_depends.reverse();
                                        let vec = fields_to_update
                                            .entry(current_model.name.clone())
                                            .or_default()
                                            .entry(field.name.clone())
                                            .or_default();
                                        vec.push(new_final_depends);

                                        // If it's a M2O, we need to add "AnotherModel", as the next link will be on another model, and the ref is in this model
                                        final_depends.push(FieldDepend::AnotherModel {
                                            target_model: current_model.name.clone(),
                                            target_field: field.name.clone(),
                                        });
                                    }
                                }
                                current_model = self.get_model(target_model);
                            } else {
                                panic!(
                                    "Field {}.{computed} depends on \"{depend}\", but {}.{d} is \
                                     not a relation that can be followed back: only a many2one, a \
                                     one2many or a many2many can be crossed",
                                    model.name, current_model.name
                                )
                            }
                        }
                    }
                }
            }
        }

        for (model_name, model_value) in fields_to_update {
            let model = self.get_model_mut(&model_name);
            for (field_name, field_values) in model_value {
                let field = model.get_internal_field_mut(&field_name);
                field.depends = field_values;
            }
        }
    }

    pub fn get_models(&self) -> &HashMap<String, FinalInternalModel> {
        &self.models
    }

    /// Where each model is stored, for the backends that address tables by name.
    ///
    /// Read from the registry rather than learned while synchronising a schema: every connection
    /// needs it, and only one of them ever runs the synchronisation.
    pub fn tables(&self) -> HashMap<String, String> {
        self.models
            .iter()
            .map(|(name, model)| (name.clone(), model.table_name.clone()))
            .collect()
    }

    /// Look a model up by name.
    ///
    /// This is the entry point for any name that did not come from the framework itself, such as
    /// one carried by an API request, and the only one that does not panic on a typo.
    pub fn try_get_model(&self, model_name: &str) -> Result<&FinalInternalModel, ModelNotFound> {
        self.models.get(model_name).ok_or_else(|| ModelNotFound {
            model_name: model_name.to_string(),
        })
    }

    /// Same as [`ModelManager::try_get_model`], for mutable access.
    pub fn try_get_model_mut(
        &mut self,
        model_name: &str,
    ) -> Result<&mut FinalInternalModel, ModelNotFound> {
        self.models
            .get_mut(model_name)
            .ok_or_else(|| ModelNotFound {
                model_name: model_name.to_string(),
            })
    }

    /// Look a model up by a name the framework itself produced.
    ///
    /// # Panics
    /// Panics if the model is unknown. Callers pass names obtained from `M::_get_model_name()`,
    /// which the derive macro fixes at compile time, so a failure here is a framework bug rather
    /// than bad input. Use [`ModelManager::try_get_model`] for anything else.
    pub fn get_model(&self, model_name: &str) -> &FinalInternalModel {
        self.try_get_model(model_name)
            .unwrap_or_else(|err| panic!("{err}"))
    }

    /// Same as [`ModelManager::get_model`], for mutable access.
    ///
    /// # Panics
    /// Panics if the model is unknown; see [`ModelManager::get_model`].
    pub fn get_model_mut(&mut self, model_name: &str) -> &mut FinalInternalModel {
        self.try_get_model_mut(model_name)
            .unwrap_or_else(|err| panic!("{err}"))
    }

    /// The plugins loaded in this process, in the order they loaded.
    pub fn loaded_plugins(&self) -> &[String] {
        &self.loaded_plugins
    }

    /// Say that in data files, what a record of this model holds is the value of this field.
    ///
    /// For a model whose records are mostly one piece of markup, such as a template: written as
    /// the content of `<template id="…">`, rather than wrapped in one more element.
    pub fn set_data_body(&mut self, model_name: &str, field_name: &str) {
        self.data_bodies
            .insert(model_name.to_string(), field_name.to_string());
    }

    /// Say that in data files, a record of this model written inside another one of the same model
    /// is its child: `field` of the inner record names the outer one.
    ///
    /// For a tree, such as menus: written nested as it is read, rather than flat with references.
    pub fn set_data_children(&mut self, model_name: &str, field_name: &str) {
        self.data_children
            .insert(model_name.to_string(), field_name.to_string());
    }

    /// The field naming a nested record's parent in data files, if its model nests records.
    pub fn data_children(&self, model_name: &str) -> Option<&str> {
        self.data_children.get(model_name).map(String::as_str)
    }

    /// The field a record element's content goes to in data files, if its model has one.
    pub fn data_body(&self, model_name: &str) -> Option<&str> {
        self.data_bodies.get(model_name).map(String::as_str)
    }

    pub fn is_valid_model(&self, model_name: &str) -> bool {
        self.models.contains_key(model_name)
    }

    /// Retrieves all models created by a specific plugin
    pub fn get_all_models_for_plugin(&self, plugin_name: &str) -> Vec<&InternalModel> {
        let mut result = vec![];
        for model in self.models.values() {
            result.extend(model.get_all_models_for_plugin(plugin_name));
        }
        result
    }
}

/// The fields of a model as values arriving over the wire are read against: those of the models
/// its one2many and many2many point to included, for the records they create or change.
pub struct RegisteredKinds<'a> {
    pub manager: &'a ModelManager,
    pub model: &'a FinalInternalModel,
}

impl FieldKinds for RegisteredKinds<'_> {
    fn kind_of(&self, field_name: &str) -> Option<FieldKind> {
        self.model.kind_of(field_name)
    }

    fn target(&self, field_name: &str) -> Option<Box<dyn FieldKinds + '_>> {
        let reference = self.model.fields.get(field_name)?.inverse.as_ref()?;
        if matches!(reference.inverse_field, FieldReferenceType::M2O { .. }) {
            return None;
        }
        let model = self.manager.try_get_model(reference.target_model).ok()?;
        Some(Box::new(RegisteredKinds {
            manager: self.manager,
            model,
        }))
    }
}
