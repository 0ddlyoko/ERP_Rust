use std::collections::HashMap;
use crate::{Field, FieldDescriptor};
use crate::field::FieldType;

/// Cached field descriptor for a model
/// Contains the name of the field and his default value
#[derive(Debug)]
pub enum CachedFieldDescriptor {
    String(String, Option<String>),
    Integer(String, Option<i32>),
    Boolean(String, Option<bool>),
}

impl CachedFieldDescriptor {
    pub fn from_field_descriptor(field_descriptor: &FieldDescriptor) -> Self {
        match &field_descriptor.default_value {
            FieldType::String(field_type) => CachedFieldDescriptor::String(
                field_descriptor.field_name.clone(), field_type.value().clone()
            ),
            FieldType::Integer(field_type) => CachedFieldDescriptor::Integer(
                field_descriptor.field_name.clone(), field_type.value().clone()
            ),
            FieldType::Boolean(field_type) => CachedFieldDescriptor::Boolean(
                field_descriptor.field_name.clone(), field_type.value().clone()
            ),
        }
    }
}

/// Entry of a model
/// Contains data of a specific record for a specific model
#[derive(Debug)]
pub struct CachedRecord {
    id: u32,
    fields: HashMap<String, FieldType>,
}

impl CachedRecord {
    pub fn new(id: u32, fields: HashMap<String, FieldType>) -> Self {
        Self {
            id: id,
            fields: fields,
        }
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn is_dirty(&self) -> bool {
        self.fields.values().any(|field_type| field_type.is_dirty())
    }

    pub fn clean(&mut self) {
        self.fields.iter_mut().for_each(|(_, field_type)| field_type.clear());
    }

    pub fn fields(&self) -> &HashMap<String, FieldType> {
        &self.fields
    }

    pub fn field(&self, field_name: &str) -> &FieldType {
        &self.fields[field_name]
    }

    pub fn field_mut(&mut self, field_name: &str) -> &mut FieldType {
        self.fields.get_mut(field_name).expect("no entry found for key")
    }

    pub fn get_new_fields(&self) -> HashMap<String, FieldType> {
        let mut map = HashMap::new();
        self.fields.iter().for_each(|(field_name, field_type)| {
            map.insert(field_name.clone(), field_type.clone());
        });
        map
    }

    pub fn update_fields(&mut self, fields: HashMap<String, FieldType>) {
        fields.iter().for_each(|(field_name, field_type)| {
            self.field_mut(field_name).update_value(field_type);
        });
    }

    pub fn update_fields_ref(&mut self, fields: HashMap<String, &FieldType>) {
        fields.iter().for_each(|(field_name, field_type)| {
            self.field_mut(field_name).update_value(field_type);
        });
    }

    pub fn update_field(&mut self, field_name: &str, field: &FieldType) {
        self.field_mut(field_name).update_value(field)
    }
}

/// Cache for a model
/// Contains cached data for a specific model
#[derive(Debug)]
pub struct CachedModel {
    cache: HashMap<u32, CachedRecord>,
    fields: Vec<CachedFieldDescriptor>,
    table_name: String,
}

impl CachedModel {
    pub fn new(table_name: &str, fields: Vec<CachedFieldDescriptor>) -> Self {
        Self {
            cache: HashMap::new(),
            fields: fields,
            table_name: table_name.to_string(),
        }
    }

    pub fn get_cached_record(&self, id: u32) -> Option<&CachedRecord> {
        self.cache.get(&id)
    }

    pub fn get_cached_record_mut(&mut self, id: u32) -> Option<&mut CachedRecord> {
        self.cache.get_mut(&id)
    }

    pub fn create_new_entry(&mut self, id: u32) -> &mut CachedRecord {
        let mut fields = HashMap::new();
        fields.insert("id".to_string(), FieldType::Integer(Field::new(Some(id as i32))));
        self.fields.iter().for_each(|field| {
            match field {
                CachedFieldDescriptor::String(field_name, default_value) => {
                    fields.insert(field_name.to_string(), FieldType::String(Field::new(default_value.clone())));
                },
                CachedFieldDescriptor::Integer(field_name, default_value) => {
                    fields.insert(field_name.to_string(), FieldType::Integer(Field::new(default_value.clone())));
                },
                CachedFieldDescriptor::Boolean(field_name, default_value) => {
                    fields.insert(field_name.to_string(), FieldType::Boolean(Field::new(default_value.clone())));
                },
            }
        });
        let record = CachedRecord {
            id: id,
            fields: fields,
        };
        self.cache.insert(id, record);
        self.cache.get_mut(&id).unwrap()
    }

    pub fn get_or_create_new_entry(&mut self, id: u32) -> &mut CachedRecord {
        if self.cache.contains_key(&id) {
            self.cache.get_mut(&id).unwrap()
        } else {
            self.create_new_entry(id)
        }
    }

    pub fn save_entry(&mut self, id: u32, fields: HashMap<String, FieldType>) {
        let entry = self.get_or_create_new_entry(id);
        entry.update_fields(fields);
    }

    pub fn save_entry_ref(&mut self, id: u32, fields: HashMap<String, &FieldType>) {
        let entry = self.get_or_create_new_entry(id);
        entry.update_fields_ref(fields);
    }

    pub fn save_entry_field(&mut self, id: u32, field_name: &str, field: &FieldType) {
        let entry = self.get_or_create_new_entry(id);
        entry.update_field(field_name, field);
    }
}

#[derive(Debug)]
pub struct CachedModels {
    pub cache: HashMap<String, CachedModel>,
}

impl CachedModels {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn get_cached_record(&self, table_name: &str, id: u32) -> Option<&CachedRecord> {
        let cached_model = self.cache.get(table_name)?;
        cached_model.get_cached_record(id)
    }

    pub fn get_cached_record_mut(&mut self, table_name: &str, id: u32) -> Option<&mut CachedRecord> {
        let cached_model = self.cache.get_mut(table_name)?;
        cached_model.get_cached_record_mut(id)
    }

    pub fn new_cached_record(&mut self, table_name: &str, id: u32) -> &mut CachedRecord {
        let cached_model = self.cache.get_mut(table_name).unwrap();
        cached_model.create_new_entry(id)
    }

    pub fn save_cached_record(&mut self, table_name: &str, id: u32, fields: HashMap<String, FieldType>) {
        let cached_model = self.cache.get_mut(table_name).unwrap();
        cached_model.save_entry(id, fields)
    }

    pub fn save_cached_record_ref(&mut self, table_name: &str, id: u32, fields: HashMap<String, &FieldType>) {
        let cached_model = self.cache.get_mut(table_name).unwrap();
        cached_model.save_entry_ref(id, fields)
    }

    pub fn save_cached_field(&mut self, table_name: &str, id: u32, field_name: &str, field: &FieldType) {
        let cached_model = self.cache.get_mut(table_name).unwrap();
        cached_model.save_entry_field(id, field_name, field)
    }

    pub fn add_cache_model(&mut self, table_name: &str, fields: Vec<CachedFieldDescriptor>) -> &mut CachedModel {
        let cached_model = CachedModel::new(table_name, fields);
        self.cache.insert(table_name.to_string(), cached_model);
        self.cache.get_mut(table_name).unwrap()
    }
}
