use crate::models::lang::BaseLang;
use erp::environment::Environment;
use erp::field::Reference;
use erp::model::BaseModel;
use erp::model::Model;
use std::error::Error;
use code_gen::Model;

pub struct BaseContact;

impl BaseModel for BaseContact {
    fn get_model_name() -> &'static str {
        "contact"
    }
}

#[derive(Model)]
#[erp(table_name="contact")]
pub struct Contact {
    id: u32,
    name: String,
    email: Option<String>,
    phone: Option<String>,
    website: Option<String>,
    lang: Reference<BaseLang>,
    // TODO Link to country
    // TODO Link to another contact (company)
}

impl Contact {
    pub fn get_id(&self) -> u32 {
        self.id
    }

    pub fn get_name(&self) -> &String {
        &self.name
    }

    pub fn get_email(&self) -> Option<&String> {
        self.email.as_ref()
    }

    pub fn get_phone(&self) -> Option<&String> {
        self.phone.as_ref()
    }

    pub fn get_website(&self) -> Option<&String> {
        self.website.as_ref()
    }

    pub fn get_lang<E>(&mut self, env: &mut Environment) -> Result<Option<E>, Box<dyn Error>>
    where
        E: Model<BaseModel=BaseLang> {
        self.lang.get(env)
    }
}

impl Model for Contact {
    type BaseModel = BaseContact;
}



// impl SimplifiedModel for Contact {
//     fn get_model_descriptor() -> ModelDescriptor {
//         ModelDescriptor {
//             name: Self::get_model_name().to_string(),
//             description: Some("Contact".to_string()),
//             fields: vec![
//                 FieldDescriptor {
//                     name: "name".to_string(),
//                     default_value: Some(FieldType::String("".to_string())),
//                     description: Some("Name of the contact".to_string()),
//                     required: false,
//                     ..FieldDescriptor::default()
//                 },
//                 FieldDescriptor {
//                     name: "email".to_string(),
//                     default_value: Some(FieldType::String("".to_string())),
//                     description: Some("Email of the contact".to_string()),
//                     required: false,
//                     ..FieldDescriptor::default()
//                 },
//                 FieldDescriptor {
//                     name: "phone".to_string(),
//                     default_value: Some(FieldType::String("".to_string())),
//                     description: Some("Phone number of the contact".to_string()),
//                     required: false,
//                     ..FieldDescriptor::default()
//                 },
//                 FieldDescriptor {
//                     name: "website".to_string(),
//                     default_value: Some(FieldType::String("".to_string())),
//                     description: Some("Website of the contact".to_string()),
//                     required: false,
//                     ..FieldDescriptor::default()
//                 },
//                 FieldDescriptor {
//                     name: "lang".to_string(),
//                     default_value: Some(FieldType::Ref(0)),
//                     description: Some("Language of the contact".to_string()),
//                     required: false,
//                     // TODO As it's a ref, pass the name of the ref table as an argument here
//                     ..FieldDescriptor::default()
//                 },
//             ],
//         }
//     }
//
//     fn get_id(&self) -> u32 {
//         self.id
//     }
//
//     fn get_data(&self) -> MapOfFields {
//         let mut result = MapOfFields::default();
//         result.insert("name", &self.name);
//         result.insert_option("email", self.email.as_ref());
//         result.insert_option("phone", self.phone.as_ref());
//         result.insert_option("website", self.website.as_ref());
//         result.insert("lang", &self.lang);
//         result
//     }
//
//     fn create_model(id: u32, data: MapOfFields) -> Self {
//         Self {
//             id,
//             name: data.get("name"),
//             email: data.get_option("email"),
//             phone: data.get_option("phone"),
//             website: data.get_option("website"),
//             lang: data.get("lang"),
//         }
//     }
//
//     fn call_compute_method(
//         &mut self,
//         field_name: &str,
//         env: &mut Environment,
//     ) -> Result<(), Box<dyn Error>> {
//         Ok(())
//     }
// }
