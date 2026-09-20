use code_gen::{Model, erp_methods};
use erp::environment::Environment;
use erp::types::field::{IdMode, MultipleIds};
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

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
    ///
    /// Declares no cursor: it ends the chain and has nothing to call down to. An override still
    /// reaches it through its own.
    pub fn daily_rate(&self, env: &mut Environment) -> Result<i32> {
        Ok(self.get_base_rate(env)?.into_iter().sum())
    }

    /// Reaches an override through `daily_rate`, though it was compiled before one existed.
    ///
    /// Exposed: a remote caller names it, and lands on the same chain.
    #[erp(rpc)]
    pub fn quote(&self, env: &mut Environment) -> Result<i32> {
        let rate = self.daily_rate(env)?;
        let days: i32 = self.get_days(env)?.into_iter().sum();
        Ok(rate * days)
    }

    /// Takes an argument, to pin down that they arrive named.
    #[erp(rpc)]
    pub fn quote_for(&self, env: &mut Environment, days: i32) -> Result<i32> {
        Ok(self.daily_rate(env)? * days)
    }

    /// Writes, then refuses. What it did must not outlive the call.
    #[erp(rpc)]
    pub fn refuse_after_writing(&self, env: &mut Environment) -> Result<i32> {
        for machine in self {
            machine.set_name("written before failing".to_string(), env)?;
        }
        Err("refused on purpose".into())
    }

    /// Not exposed: reachable from Rust, absent from the wire.
    pub fn internal_rate(&self, env: &mut Environment) -> Result<i32> {
        self.daily_rate(env)
    }
}
