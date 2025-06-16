use crate::field::{FieldCompute, FieldDepend, FieldReference, FieldReferenceType, MultipleIds};
use crate::internal::internal_model::{FinalInternalModel, InternalModel};
use crate::model::Model;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub struct ModelManager {
    models: HashMap<String, FinalInternalModel>,
    pub(crate) current_plugin_loading: Option<String>,
}

impl ModelManager {
    pub fn register_model<M>(&mut self)
    where
        M: Model<MultipleIds> + 'static,
    {
        let plugin_name = match &self.current_plugin_loading {
            Some(plugin_name) => plugin_name,
            None => "Unknown",
        };
        let model_name = M::get_model_name();

        self.models
            .entry(model_name.to_string())
            .or_insert_with(|| FinalInternalModel::new(model_name))
            .register_internal_model::<M>(plugin_name);
    }

    /// Execute some final modification when models are registered, like:
    /// - Linking M2O => O2M (as there is already a link between O2M => M2O)
    pub fn post_register(&mut self) {
        self._post_register_m2o_links();
        self._post_register_compute_links();
    }

    fn _post_register_m2o_links(&mut self) {
        // Clear M2O depends
        for model in self.models.values_mut() {
            for field in model.fields.values_mut() {
                if let Some(FieldReference { inverse_field: FieldReferenceType::M2O { inverse_fields }, .. }) = &mut field.inverse {
                    inverse_fields.clear();
                }
            }
        }

        // Now, get the new list
        let mut fields_to_modify: HashMap<String, HashMap<String, Vec<String>>> = HashMap::new();
        for model in self.models.values() {
            for field in model.fields.values() {
                if let Some(FieldReference { target_model, inverse_field: FieldReferenceType::O2M { inverse_field } }) = &field.inverse {
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
                if let Some(FieldReference { inverse_field: FieldReferenceType::M2O { inverse_fields }, .. }) = &mut field.inverse {
                    inverse_fields.append(&mut fields_to_add);
                    // Check uniqueness
                    let mut seen = HashSet::new();
                    inverse_fields.retain(|field| seen.insert(field.clone()));
                } else {
                    panic!("A field is targeting {}.{} as an inverse field, but this field is not a M2O", model_name, field_name);
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
        let mut fields_to_update: HashMap<String, HashMap<String, Vec<Vec<FieldDepend>>>> = HashMap::new();
        for model in self.models.values() {
            for field in model.fields.values() {
                if let Some(FieldCompute { depends, .. }) = &field.compute {
                    for depend in depends {
                        let mut final_depends: Vec<FieldDepend> = vec![
                            FieldDepend::SameModel { field_name: field.name.clone() },
                        ];
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
                                    .entry(current_model.name.clone()).or_default()
                                    .entry(field.name.clone()).or_default();
                                vec.push(new_final_depends);
                            } else if let Some(FieldReference { target_model, inverse_field }) = &field.inverse {
                                match inverse_field {
                                    FieldReferenceType::O2M { inverse_field } => {
                                        final_depends.push(FieldDepend::AnotherModel2 {
                                            target_model: current_model.name.clone(),
                                            field_name: inverse_field.clone(),
                                        });

                                        // Save to field
                                        let mut new_final_depends = final_depends.clone();
                                        new_final_depends.reverse();
                                        let vec = fields_to_update
                                            .entry(target_model.clone()).or_default()
                                            .entry(inverse_field.clone()).or_default();
                                        vec.push(new_final_depends);
                                    },
                                    FieldReferenceType::M2O { .. } => {
                                        // Save to field
                                        let mut new_final_depends = final_depends.clone();
                                        new_final_depends.reverse();
                                        let vec = fields_to_update
                                            .entry(current_model.name.clone()).or_default()
                                            .entry(field.name.clone()).or_default();
                                        vec.push(new_final_depends);

                                        // If it's a M2O, we need to add "AnotherModel", as the next link will be on another model, and the ref is in this model
                                        final_depends.push(FieldDepend::AnotherModel {
                                            target_model: current_model.name.clone(),
                                            target_field: field.name.clone(),
                                        });
                                    },
                                }
                                current_model = self.get_model(target_model);
                            } else {
                                panic!("Field {}.{} has invalid depends! (Field \"{}\" of depends \"{:?}\" is not a M2O / O2M)", model.name, field.name, d, depend)
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

    pub fn get_model(&self, model_name: &str) -> &FinalInternalModel {
        self.models.get(model_name).unwrap()
    }

    pub fn get_model_mut(&mut self, model_name: &str) -> &mut FinalInternalModel {
        self.models.get_mut(model_name).unwrap()
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
