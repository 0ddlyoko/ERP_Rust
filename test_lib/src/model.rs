use std::collections::HashMap;
use crate::field::GeneratedFieldDescriptor;
use crate::{Environment, FieldDescriptor, FieldType};

#[derive(Debug)]
pub struct GeneratedModelDescriptor {
    pub table_name: String,
    pub fields: HashMap<String, GeneratedFieldDescriptor>,
}

impl GeneratedModelDescriptor {
    pub fn name(&self) -> &String {
        &self.table_name
    }

    pub fn fields(&self) -> Vec<&GeneratedFieldDescriptor> {
        self.fields.values().collect()
    }

    pub fn field(&self, field_name: &str) -> Option<&GeneratedFieldDescriptor> {
        self.fields.get(field_name)
    }
}

pub trait InternalModelGetterDescriptor<'env> {
    fn _name() -> &'static str;
    fn _get_generated_model_descriptor() -> GeneratedModelDescriptor;
    fn _from_map(id: u32, map: HashMap<String, FieldType>, env: std::rc::Weak<std::cell::RefCell<Environment<'env>>>) -> Self;
    fn id(&self) -> u32;
    fn env(&self) -> &std::rc::Weak<std::cell::RefCell<Environment<'env>>>;
    fn _to_map(&self) -> HashMap<String, FieldType>;
    fn _to_map_dirty(&self) -> HashMap<String, FieldType>;
    fn _remove_dirty(&mut self);

    fn new<IMD>(env: std::rc::Weak<std::cell::RefCell<Environment<'env>>>) -> IMD where IMD: InternalModelGetterDescriptor<'env> {
        let name = IMD::_name();
        let (id, new_fields) = match env.upgrade() {
            Some(env_borrow) => {
                let mut env = env_borrow.borrow_mut();
                env.counter += 1;
                let id = env.counter;
                let cached_record = env.cache_mut().new_cached_record(name, id);
                (id, cached_record.get_new_fields())
            }
            None => {
                // Should be there
                panic!("Environment should exist!")
            }
        };
        IMD::_from_map(id, new_fields, env)
    }

    fn load<IMD>(id: u32, env: std::rc::Weak<std::cell::RefCell<Environment<'env>>>) -> IMD where IMD: InternalModelGetterDescriptor<'env> {
        let name = IMD::_name();
        let new_fields = match env.upgrade() {
            Some(env_borrow) => {
                let mut env = env_borrow.borrow_mut();
                let cached_record = env.cache_mut().get_cached_record(name, id);
                match cached_record {
                    Some(record) => {
                        record.get_new_fields()
                    }
                    None => {
                        // TODO Load it
                        panic!("TODO Load it")
                    }
                }
            },
            None => {
                // Should be there
                panic!("Environment should exist!")
            }
        };
        IMD::_from_map(id, new_fields, env)
    }

    fn convert_to<IMD>(&mut self) -> IMD where IMD: InternalModelGetterDescriptor<'env> {
        // Before converting, save in cache
        self.save();
        let id = self.id();
        let env = self.env();
        return Self::load(id, env.clone());
    }

    fn save(&mut self) {
        let map = self._to_map_dirty();
        if map.is_empty() { return; }
        println!("Map = {:?}", map);
        match self.env().upgrade() {
            Some(env_borrow) => {
                let mut env = env_borrow.borrow_mut();
                env.cache_mut().save_cached_record(Self::_name(), self.id(), map);
                // Remove dirty
                self._remove_dirty();
            }
            None => {
                // Should be there
                panic!("Environment should exist!")
            }
        }
    }

    fn save_field(&self, field_name: &str, field: &FieldType) {
        match self.env().upgrade() {
            Some(env_borrow) => {
                let mut env = env_borrow.borrow_mut();
                env.cache_mut().save_cached_field(Self::_name(), self.id(), field_name, field)
            }
            None => {
                // Should be there
                panic!("Environment should exist!")
            }
        }
    }
}

#[derive(Debug)]
pub struct ModelDescriptor {
    generated_models: HashMap<String, GeneratedModelDescriptor>,
    table_name: String,
    fields: HashMap<String, FieldDescriptor>,
}

impl ModelDescriptor {
    fn new(table_name: String) -> Self {
        Self {
            generated_models: HashMap::new(),
            table_name: table_name,
            fields: HashMap::new(),
        }
    }

    pub fn get_table_name(&self) -> &String {
        &self.table_name
    }

    pub fn get_fields(&self) -> &HashMap<String, FieldDescriptor> {
        &self.fields
    }

    pub fn add_generated_model(&mut self, module_name: &str, generated_model_descriptor: GeneratedModelDescriptor) {
        self.generated_models.insert(module_name.to_string(), generated_model_descriptor);
        let generated_model_descriptor = &self.generated_models[module_name];
        for (name, field) in &generated_model_descriptor.fields {
            let existing_field = self.fields.entry(name.clone()).or_insert_with(|| {
                FieldDescriptor::default(name.as_str(), field.default_field.clone())
            });
            // Required
            match field.required() {
                Some(required) => existing_field.is_required = *required,
                _ => {}
            }
            // Default
            let default_value = field.default_field();
            let first_discriminant = std::mem::discriminant(&existing_field.default_value);
            let second_discriminant = std::mem::discriminant(default_value);
            if first_discriminant != second_discriminant {
                panic!("Redefinition of field \"{}\" with a different type! ({:?} != {:?})", name, existing_field.default_value, default_value);
            }
            if default_value.has_entry() {
                // Update the default value
                existing_field.default_value.update_value(default_value);
            }
        }
    }
}

#[derive(Debug)]
pub struct ModelManager {
    models: HashMap<String, ModelDescriptor>,
}

impl ModelManager {
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
        }
    }

    pub fn register<'env, IMD>(&mut self, module_name: &str) where IMD: InternalModelGetterDescriptor<'env> {
        let generated_model_descriptor = IMD::_get_generated_model_descriptor();
        let table_name = &generated_model_descriptor.table_name;
        let model_descriptor = self.models.entry(table_name.clone()).or_insert_with(|| {
            ModelDescriptor::new(table_name.clone())
        });
        model_descriptor.add_generated_model(module_name, generated_model_descriptor);
    }

    pub fn models(&self) -> &HashMap<String, ModelDescriptor> {
        &self.models
    }

    pub fn model(&self, model: &str) -> &ModelDescriptor {
        &self.models[model]
    }
}
