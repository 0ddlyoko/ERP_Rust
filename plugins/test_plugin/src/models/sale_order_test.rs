use code_gen::{Model, erp_methods};
use erp::environment::Environment;
use erp::types::field::{IdMode, MultipleIds};
use std::error::Error;

#[derive(Model)]
#[erp(id = "sale_order_test", methods)]
#[allow(dead_code)]
pub struct SaleOrderTest<Mode: IdMode> {
    id: Mode,
    name: String,
    age: i32,
    #[erp(compute = "compute_label", depends = ["name"])]
    label: String,
    #[erp(compute = "compute_replaced", depends = ["name"])]
    replaced: String,
    #[erp(compute = "compute_narrowed", depends = ["name"])]
    narrowed: String,
}

#[erp_methods]
impl SaleOrderTest<MultipleIds> {
    /// Base implementation of the chain.
    #[erp(overridable)]
    pub fn compute_label(
        &self,
        env: &mut Environment,
        _parent: Super,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        for record in self {
            let name = record.get_name(env)?.clone();
            record.set_label(format!("base:{name}"), env)?;
        }
        Ok(())
    }

    #[erp(overridable)]
    pub fn compute_replaced(
        &self,
        env: &mut Environment,
        _parent: Super,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        for record in self {
            record.set_replaced("base".to_string(), env)?;
        }
        Ok(())
    }

    #[erp(overridable)]
    pub fn compute_narrowed(
        &self,
        env: &mut Environment,
        _parent: Super,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        for record in self {
            record.set_narrowed("base".to_string(), env)?;
        }
        Ok(())
    }
}

#[derive(Model)]
#[erp(id = "sale_order_test", methods)]
#[erp(derived_model = "")]
#[allow(dead_code)]
pub struct SaleOrderTest2<Mode: IdMode> {
    id: Mode,
    #[erp(description = "New name of the SO")]
    name: String,
    #[erp(compute = "compute_label", depends = ["name"])]
    label: String,
    #[erp(compute = "compute_replaced", depends = ["name"])]
    replaced: String,
    #[erp(compute = "compute_narrowed", depends = ["name"])]
    narrowed: String,
}

#[erp_methods]
impl SaleOrderTest2<MultipleIds> {
    /// Extends the base: calls it, then appends to what it produced.
    #[erp(overridable)]
    pub fn compute_label(
        &self,
        env: &mut Environment,
        parent: Super,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        assert!(parent.exists(), "the base implementation must be reachable");
        parent.call(env)?;
        for record in self {
            let current = record.get_label(env)?.clone();
            record.set_label(format!("{current}+derived"), env)?;
        }
        Ok(())
    }

    /// Replaces the base: never calls it.
    #[erp(overridable)]
    pub fn compute_replaced(
        &self,
        env: &mut Environment,
        _parent: Super,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        for record in self {
            record.set_replaced("derived".to_string(), env)?;
        }
        Ok(())
    }

    /// Handles some records itself and hands only the rest down the chain.
    #[erp(overridable)]
    pub fn compute_narrowed(
        &self,
        env: &mut Environment,
        parent: Super,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut handed_down = Vec::new();
        for record in self {
            if record.get_name(env)? == "skip" {
                record.set_narrowed("skipped".to_string(), env)?;
            } else {
                handed_down.push(record.get_id());
            }
        }
        parent.call_on(handed_down, env)
    }
}
