use crate::models::contact_tag::BaseContactTag;
use crate::models::country::{BaseCountry, Country};
use crate::models::lang::BaseLang;
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};

/// Someone, or a company, the business deals with: a customer, a supplier, a user.
///
/// A person may work for a company, its `parent`, and is then named after it: `Acme, John Doe`.
#[derive(Model)]
#[erp(id = "contact", name_field = "complete_name", methods)]
#[allow(dead_code)]
pub struct Contact<Mode: IdMode> {
    id: Mode,
    #[erp(tracking, index = "trigram")]
    name: String,
    #[erp(
        label = "Full name",
        compute = "compute_complete_name",
        depends = ["name", "is_company", "parent.name"],
        stored,
        index = "trigram",
    )]
    complete_name: String,
    #[erp(label = "Is a company", tracking)]
    is_company: bool,
    #[erp(label = "Company", domain = r#"[["is_company", "=", true]]"#, tracking)]
    parent: Reference<BaseContact, SingleId>,
    #[erp(label = "Contacts", inverse = "parent")]
    children: Reference<BaseContact, MultipleIds>,
    #[erp(label = "Job position")]
    function: Option<String>,
    #[erp(label = "Tax ID", tracking, index)]
    vat: Option<String>,
    #[erp(label = "Reference", index)]
    reference: Option<String>,
    street: Option<String>,
    #[erp(label = "Street 2")]
    street2: Option<String>,
    #[erp(label = "ZIP")]
    zip: Option<String>,
    city: Option<String>,
    #[erp(tracking)]
    country: Reference<BaseCountry, SingleId>,
    #[erp(
        description = "Street, city and country, one per line, as an envelope shows them",
        compute = "compute_address",
        depends = ["street", "street2", "zip", "city", "country.name"]
    )]
    address: Option<String>,
    #[erp(tracking, index = "trigram")]
    email: Option<String>,
    #[erp(tracking)]
    phone: Option<String>,
    mobile: Option<String>,
    website: Option<String>,
    #[erp(label = "Language")]
    lang: Reference<BaseLang, SingleId>,
    #[erp(relation = "contact_tag_rel")]
    tags: Reference<BaseContactTag, MultipleIds>,
    notes: Option<String>,
    #[erp(default = true, tracking)]
    active: bool,
}

#[erp_methods]
impl Contact<MultipleIds> {
    /// A person working for a company is named after it, `Acme, John Doe`; anyone else by name.
    ///
    /// As sudo: the name is stored, the same for every reader, and may be worked out once the
    /// work creating the contact is over — for a user signing up, as the portal user.
    pub fn compute_complete_name(&self, env: &mut Environment) -> Result<()> {
        let env = &mut *env.sudo();
        for contact in self {
            let name = contact.get_name(env)?.clone();
            let parent: Contact<SingleId> = contact.get_parent(env)?;
            let complete_name = if *contact.get_is_company(env)? || parent.is_empty() {
                name
            } else {
                format!("{}, {name}", parent.get_name(env)?)
            };
            contact.set_complete_name(complete_name, env)?;
        }
        Ok(())
    }

    /// The streets, then the ZIP code and the city, then the country: the lines filled in.
    pub fn compute_address(&self, env: &mut Environment) -> Result<()> {
        for contact in self {
            let country: Country<SingleId> = contact.get_country(env)?;
            let country = if country.is_empty() {
                None
            } else {
                Some(country.get_name(env)?.clone())
            };
            let town = [
                contact.get_zip(env)?.cloned(),
                contact.get_city(env)?.cloned(),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" ");
            let lines: Vec<String> = [
                contact.get_street(env)?.cloned(),
                contact.get_street2(env)?.cloned(),
                Some(town),
                country,
            ]
            .into_iter()
            .flatten()
            .filter(|line| !line.trim().is_empty())
            .collect();
            let address = (!lines.is_empty()).then(|| lines.join("\n"));
            contact.set_address(address, env)?;
        }
        Ok(())
    }

    /// Archive the contacts: they are no longer offered, and stay where they are used.
    #[erp(rpc)]
    pub fn archive(&self, env: &mut Environment) -> Result<bool> {
        self.set_active(false, env)?;
        Ok(true)
    }

    /// Bring archived contacts back.
    #[erp(rpc)]
    pub fn unarchive(&self, env: &mut Environment) -> Result<bool> {
        self.set_active(true, env)?;
        Ok(true)
    }
}
