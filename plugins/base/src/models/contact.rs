use code_gen::Model;
use erp::environment::Environment;
use erp::field::Reference;
use erp::model::Model;
use std::error::Error;
use crate::models::lang::BaseLang;

#[derive(Model)]
#[erp(table_name="contact")]
pub struct Contact {
    pub id: u32,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub website: Option<String>,
    pub lang: Reference<BaseLang>,
    // TODO Link to country
    // TODO Link to another contact (company)
}

// TODO impl this in the derive Model
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

#[derive(Model)]
#[erp(table_name="contact")]
// #[erp(base_model)]
#[erp(derived_model = "crate::models::contact")]
pub struct Contact2 {
    id: u32,
    name: String,
    email: Option<String>,
    phone: Option<String>,
    website: Option<String>,
    lang: Reference<BaseLang>,
}
