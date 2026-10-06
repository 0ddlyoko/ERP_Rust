use code_gen::{Model, erp_methods};
use erp::Result;
use erp::environment::Environment;
use erp::types::field::{IdMode, MultipleIds, Password, SingleId};
use std::sync::atomic::{AtomicUsize, Ordering};

static RUNNING: AtomicUsize = AtomicUsize::new(0);
static HIGH_WATER: AtomicUsize = AtomicUsize::new(0);

/// Carries an override point another plugin extends, to pin down that a call reaches the most
/// derived implementation rather than the one next to it.
#[derive(Model)]
#[erp(id = "machine", methods)]
#[allow(dead_code)]
pub struct Machine<Mode: IdMode> {
    pub id: Mode,
    name: Option<String>,
    #[erp(default = 100)]
    base_rate: i32,
    #[erp(default = 1)]
    days: i32,
    /// Never leaves the process, whatever a caller asks for.
    #[erp(private)]
    unlock_code: Option<String>,
    service_key: Password,
}

#[erp_methods]
impl Machine<SingleId> {
    /// How one machine is named in a list: its name and its rate. On one record, overridable as
    /// any other.
    #[erp(rpc)]
    pub fn label(&self, env: &mut Environment) -> Result<String> {
        let name = self.get_name(env)?.cloned().unwrap_or_default();
        Ok(format!("{name} at {}", self.get_base_rate(env)?))
    }
}

#[erp_methods]
impl Machine<MultipleIds> {
    /// The rate a new machine starts at: a method of the model, called without records.
    #[erp(rpc)]
    pub fn standard_rate(env: &mut Environment) -> Result<i32> {
        let _ = env;
        Ok(100)
    }

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
            machine.set_name(Some("written before failing".to_string()), env)?;
        }
        Err("refused on purpose".into())
    }

    /// Writes, then panics. The call fails alone: its work is undone, and nothing else stops.
    #[erp(rpc)]
    pub fn explode(&self, env: &mut Environment) -> Result<i32> {
        self.set_name(Some("written before panicking".to_string()), env)?;
        panic!("exploded on purpose");
    }

    /// Records how many copies of itself are running, so a test can see a concurrency limit
    /// from the inside rather than inferring it from a stopwatch.
    #[erp(rpc)]
    pub fn busy(&self, env: &mut Environment) -> Result<i32> {
        let _ = env;
        let now = RUNNING.fetch_add(1, Ordering::SeqCst) + 1;
        HIGH_WATER.fetch_max(now, Ordering::SeqCst);
        std::thread::sleep(std::time::Duration::from_millis(30));
        RUNNING.fetch_sub(1, Ordering::SeqCst);
        Ok(now as i32)
    }

    /// The most that were ever running at the same time.
    #[erp(rpc)]
    pub fn high_water(&self, env: &mut Environment) -> Result<i32> {
        let _ = env;
        Ok(HIGH_WATER.load(Ordering::SeqCst) as i32)
    }

    /// Not exposed: reachable from Rust, absent from the wire.
    pub fn internal_rate(&self, env: &mut Environment) -> Result<i32> {
        self.daily_rate(env)
    }
}
