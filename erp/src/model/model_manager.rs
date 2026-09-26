use crate::identity::Identities;
use crate::model::HasMethods;
use crate::model::Model;
use crate::model::ModelNotFound;
use crate::model::rpc::{RpcFn, RpcRegistry};
use erp_internal_types::{FinalInternalModel, InternalModel};
use erp_types::field::FieldCompute;
use erp_types::field::MultipleIds;
use erp_types::field::{FieldDepend, FieldReference, FieldReferenceType};
use erp_types::method::MethodFn;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub struct ModelManager {
    models: HashMap<String, FinalInternalModel>,
    /// Methods a remote caller may reach. Held here rather than on the model, so that the crates
    /// describing types never have to know about JSON.
    pub rpc: RpcRegistry,
    /// How a token identifies its caller. Beside the registry for the same reason as `rpc`: it is
    /// what a plugin contributes about reaching models, not what a model is.
    pub identities: Identities,
    pub(crate) current_plugin_loading: Option<String>,
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
        M::register_methods(self, &plugin_name);
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
        plugin_name: &str,
    ) where
        A: 'static,
        R: 'static,
    {
        self.models
            .entry(model_name.to_string())
            .or_insert_with(|| FinalInternalModel::new(model_name))
            .methods
            .register(model_name, method_name, link, plugin_name);
    }

    /// Execute some final modification when models are registered, like:
    /// - Linking M2O => O2M (as there is already a link between O2M => M2O)
    pub fn post_register(&mut self) {
        self._post_register_storage();
        self._post_register_m2o_links();
        self._post_register_compute_links();
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
                    let model_to_modify = fields_to_modify.entry(target_model.clone()).or_default();
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
                                            .entry(target_model.clone())
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
                                            .entry(target_model.clone())
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
                                    "Field {}.{} has invalid depends! (Field \"{}\" of depends \"{:?}\" is not a M2O / O2M)",
                                    model.name, field.name, d, depend
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
