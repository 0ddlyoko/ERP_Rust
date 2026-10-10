use crate::structured::normalize_belgian_vat;
use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{IdMode, MultipleIds};
use erp::types::model::MapOfFields;

/// A Belgian VAT number is checked, and kept as `BE0477472701`.
#[derive(Model)]
#[erp(id = "contact", methods)]
#[erp(derived_model = "base::models")]
#[allow(dead_code)]
pub struct ContactBe<Mode: IdMode> {
    id: Mode,
}

/// The VAT number of `values` checked and written the Belgian way when it is a Belgian one.
fn checked(mut values: MapOfFields) -> Result<MapOfFields> {
    if let Some(vat) = values.get_option::<&String>("vat").cloned() {
        let compact: String = vat.chars().filter(|c| !c.is_whitespace()).collect();
        if compact.to_uppercase().starts_with("BE") {
            values.insert("vat", normalize_belgian_vat(&compact)?);
        }
    }
    Ok(values)
}

#[erp_methods]
impl ContactBe<MultipleIds> {
    pub fn create(
        &self,
        env: &mut Environment,
        values: Vec<MapOfFields>,
        sup: Super,
    ) -> Result<MultipleIds> {
        let values = values
            .into_iter()
            .map(checked)
            .collect::<Result<Vec<_>>>()?;
        Ok(sup.call_with(values, env)?)
    }

    pub fn write(&self, env: &mut Environment, values: MapOfFields, sup: Super) -> Result<()> {
        Ok(sup.call_with(checked(values)?, env)?)
    }
}
