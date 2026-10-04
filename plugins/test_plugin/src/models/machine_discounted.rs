use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{IdMode, MultipleIds};

/// Extends `machine` from another crate, and overrides its rate.
#[derive(Model)]
#[erp(id = "machine", methods)]
#[erp(derived_model = "test_utilities::models")]
#[allow(dead_code)]
pub struct MachineDiscounted<Mode: IdMode> {
    id: Mode,
    #[erp(default = 0)]
    discount: i32,
}

#[erp_methods]
impl MachineDiscounted<MultipleIds> {
    /// Same name, same signature, same model: that alone puts it ahead of the one
    /// `test_lib_plugin` declared, which `sup` then reaches.
    pub fn daily_rate(&self, env: &mut Environment, sup: Super) -> Result<i32> {
        let base = sup.call(env)?;
        let discount: i32 = self.get_discount(env)?.into_iter().sum();
        Ok(base - discount)
    }

    /// Appelle `daily_rate` depuis une méthode voisine du modèle qui l'override.
    ///
    /// Passe par la tête de chaîne, donc par la surcharge, et non par l'implémentation d'à côté.
    pub fn weekly_rate(&self, env: &mut Environment) -> Result<i32> {
        Ok(self.daily_rate(env)? * 7)
    }
}
