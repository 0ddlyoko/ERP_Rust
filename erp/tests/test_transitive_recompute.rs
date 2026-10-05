//! A stored computed field depending on another stored computed field, of related records: it
//! is worked out again as soon as what the other depends on changes, read first or not.

use erp::app::Application;
use erp_types::field::{MultipleIds, SingleId};
use erp_types::model::MapOfFields;
use std::error::Error;

type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

mod baskets {
    use code_gen::{Model, erp_methods};
    use erp::environment::Environment;
    use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};
    use std::error::Error;

    type Result<T> = std::result::Result<T, Box<dyn Error + Send + Sync>>;

    /// Counts its lines' totals, each worked out from the line's own fields.
    #[derive(Model)]
    #[erp(id = "chain_order", methods)]
    #[allow(dead_code)]
    pub struct ChainOrder<Mode: IdMode> {
        pub id: Mode,
        #[erp(inverse = "order")]
        lines: Reference<BaseChainLine, MultipleIds>,
        #[erp(compute = "compute_total", depends = ["lines.total"], stored)]
        total: i32,
    }

    #[derive(Model)]
    #[erp(id = "chain_line", methods)]
    #[allow(dead_code)]
    pub struct ChainLine<Mode: IdMode> {
        pub id: Mode,
        #[erp(ondelete = "cascade")]
        order: Reference<BaseChainOrder, SingleId>,
        #[erp(default = 1)]
        quantity: i32,
        #[erp(default = 0)]
        price: i32,
        #[erp(compute = "compute_total", depends = ["quantity", "price"], stored)]
        total: i32,
    }

    #[erp_methods]
    impl ChainOrder<MultipleIds> {
        pub fn compute_total(&self, env: &mut Environment) -> Result<()> {
            for order in self {
                let lines: ChainLine<MultipleIds> = order.get_lines(env)?;
                let total = lines.get_total(env)?.into_iter().sum();
                order.set_total(total, env)?;
            }
            Ok(())
        }
    }

    #[erp_methods]
    impl ChainLine<MultipleIds> {
        pub fn compute_total(&self, env: &mut Environment) -> Result<()> {
            for line in self {
                let total = *line.get_quantity(env)? * *line.get_price(env)?;
                line.set_total(total, env)?;
            }
            Ok(())
        }
    }
}

use baskets::{ChainLine, ChainOrder};

fn new_app() -> Application {
    let mut app = Application::new_test();
    app.model_manager.register_model::<ChainOrder<_>>();
    app.model_manager.register_model::<ChainLine<_>>();
    app.model_manager.post_register();
    app
}

/// Changing a line's quantity changes its order's total, read without reading the line.
#[test]
fn test_a_total_of_totals_follows_without_reading_the_lines() -> Result<()> {
    let app = new_app();
    let mut env = app.new_env()?;
    let order: ChainOrder<SingleId> = env.create_new_record_from_map(MapOfFields::default())?;
    let mut line = MapOfFields::default();
    line.insert("order", order.get_id());
    line.insert("price", 5);
    let line: ChainLine<SingleId> = env.create_new_record_from_map(line)?;
    assert_eq!(*order.get_total(&mut env)?, 5);

    line.set_quantity(4, &mut env)?;
    assert_eq!(
        *order.get_total(&mut env)?,
        20,
        "read first, the order follows its line"
    );
    env.close()?;

    let mut env = app.new_env()?;
    let line: ChainLine<SingleId> = env.get_record(line.get_id().into());
    line.set_price(7, &mut env)?;
    let order: ChainOrder<SingleId> = env.get_record(order.get_id().into());
    assert_eq!(*order.get_total(&mut env)?, 28);
    let lines: ChainLine<MultipleIds> = order.get_lines(&mut env)?;
    assert_eq!(lines.get_ids_ref().len(), 1);
    Ok(())
}
