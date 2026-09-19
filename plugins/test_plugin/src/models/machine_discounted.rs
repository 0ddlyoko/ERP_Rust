use code_gen::{Model, erp_methods};
use erp::environment::Environment;
use erp::types::field::{IdMode, MultipleIds};
use std::error::Error;

/// Extends `machine` from another crate, and overrides its rate.
#[derive(Model)]
#[erp(id = "machine", methods)]
#[erp(derived_model = "test_utilities::models")]
#[allow(dead_code)]
pub(crate) struct MachineDiscounted<Mode: IdMode> {
    id: Mode,
    #[erp(default = 0)]
    discount: i32,
}

#[erp_methods]
impl MachineDiscounted<MultipleIds> {
    /// Takes the rate the declaring plugin computed, and knocks the discount off it.
    #[erp(overrides = "test_utilities::models::MachineDailyRate")]
    pub fn daily_rate(
        &self,
        env: &mut Environment,
        sup: Super,
    ) -> Result<i32, Box<dyn Error + Send + Sync>> {
        let base = sup.call(env)?;
        let discount: i32 = self.get_discount(env)?.into_iter().sum();
        Ok(base - discount)
    }
}
