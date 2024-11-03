use crate::internal::internal_field::{FinalInternalField, InternalField};
use crate::model::{MapOfFields, Model};
use std::any::TypeId;
use std::collections::HashMap;
use std::fmt::format;
use crate::field::{FieldDescriptor, FieldType};

/// Model descriptor represented by a single struct model
pub struct InternalModel {
    pub name: &'static str,
    pub description: Option<String>,
    pub fields: HashMap<&'static str, InternalField>,
    pub create_instance: fn(u32, MapOfFields) -> Box<dyn Model>,
}

/// Final descriptor of a model.
///
/// Represent all combined InternalModel
pub struct FinalInternalModel {
    pub name: &'static str,
    pub description: String,
    pub models: HashMap<TypeId, InternalModel>,
    pub fields: HashMap<&'static str, FinalInternalField>,
}

impl FinalInternalModel {
    pub fn new(model_name: &'static str) -> FinalInternalModel {
        FinalInternalModel {
            name: model_name,
            description: "".to_string(),
            models: HashMap::new(),
            fields: HashMap::new(),
        }
    }

    pub fn register_internal_model<M>(&mut self) where M: Model + 'static {
        let name = M::get_model_name();
        let model_descriptor = M::get_model_descriptor();
        let type_id = TypeId::of::<M>();

        let description = model_descriptor.description;
        let mut fields = HashMap::new();
        for field in &model_descriptor.fields {
            fields.insert(field.name, InternalField {
                name: field.name,
                default_value: field.default_value.clone(),
                description: field.description.clone(),
                required: field.required,
            });
            self.register_internal_field(field);
        }

        let create_instance: fn(u32, MapOfFields) -> Box<dyn Model> = |id, data| Box::new(M::create_model(id, data));

        let internal_model = InternalModel {
            name,
            description,
            fields,
            create_instance,
        };
        
        if let Some(description) = &internal_model.description {
            self.description = description.clone();
        }
        self.models.insert(type_id, internal_model);
    }

    fn register_internal_field(&mut self, field_descriptor: &FieldDescriptor) {
        let name = field_descriptor.name;
        let internal_field = self.fields.entry(name).or_insert_with(|| { FinalInternalField::new(name) });
        internal_field.register_internal_field(field_descriptor);
    }

    pub fn first(&self) -> &InternalModel {
        if let Some(first_value) = self.models.values().next() {
            first_value
        } else {
            panic!("Not a single model is present");
        }
    }

    pub fn get_internal_model<M>(&self) -> &InternalModel where M: Model + 'static {
        let type_id = TypeId::of::<M>();
        self.models.get(&type_id).expect("Internal model not registered")
    }

    pub fn get_internal_model_mut<M>(&mut self) -> &mut InternalModel where M: Model + 'static {
        let type_id = TypeId::of::<M>();
        self.models.get_mut(&type_id).expect("Internal model not registered")
    }

    /// Get a vector of all registered fields for this model
    pub fn get_fields_name(&self) -> Vec<&'static str> {
        self.fields.keys().cloned().collect()
    }

    /// Get a vector of difference between all registered fields for this model, and given vector
    pub fn get_missing_fields(&self, current_fields: Vec<&'static str>) -> Vec<&'static str> {
        self.fields.keys().filter(|x| !current_fields.contains(x)).cloned().collect()
    }

    /// Return default value for given field.
    /// If the first is not present, panic
    pub fn get_default_value(&self, field_name: &'static str) -> FieldType {
        let field = self.fields.get(field_name).expect(format!("Field {} is not present in model {}", field_name, field_name).as_str());
        field.default_value.clone()
    }
}

#[cfg(test)]
mod test {
    use crate::field::{FieldDescriptor, FieldType};
    use crate::internal::internal_model::FinalInternalModel;

    #[test]
    fn test_get_fields_name() {
        let mut internal_model = FinalInternalModel::new("");

        internal_model.register_internal_field(&FieldDescriptor {
            name: "name",
            default_value: Some(FieldType::String("0ddlyoko".to_string())),
            description: Some("This is the name".to_string()),
            required: None,
        });

        internal_model.register_internal_field(&FieldDescriptor {
            name: "age",
            default_value: Some(FieldType::Integer(42)),
            description: Some("This is the age of the person".to_string()),
            required: None,
        });

        assert_eq!({
            let mut fields = internal_model.get_fields_name();
            fields.sort();
            fields
        }, vec!["age", "name"]);
        assert_eq!(internal_model.get_missing_fields(vec!["age"]), vec!["name"]);
    }
}