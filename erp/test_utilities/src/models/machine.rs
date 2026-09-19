use code_gen::{Model, erp_methods};
use erp::environment::Environment;
use erp::types::field::{IdMode, MultipleIds};
use std::error::Error;

/// Carries an override point another plugin extends, to pin down that a call reaches the most
/// derived implementation rather than the one next to it.
#[derive(Model)]
#[erp(id = "machine", methods)]
#[allow(dead_code)]
pub struct Machine<Mode: IdMode> {
    pub id: Mode,
    #[erp(default = "")]
    name: String,
    #[erp(default = 100)]
    base_rate: i32,
    #[erp(default = 1)]
    days: i32,
}

#[erp_methods]
impl Machine<MultipleIds> {
    /// What one day of this machine costs.
    #[erp(overridable)]
    pub fn daily_rate(
        &self,
        env: &mut Environment,
        sup: Super,
    ) -> Result<i32, Box<dyn Error + Send + Sync>> {
        debug_assert!(!sup.exists(), "the base implementation ends the chain");
        Ok(self.get_base_rate(env)?.into_iter().sum())
    }

    /// Carries no attribute of its own, and still reaches an override through `daily_rate`.
    pub fn quote(&self, env: &mut Environment) -> Result<i32, Box<dyn Error + Send + Sync>> {
        let rate = self.daily_rate(env)?;
        let days: i32 = self.get_days(env)?.into_iter().sum();
        Ok(rate * days)
    }
}
