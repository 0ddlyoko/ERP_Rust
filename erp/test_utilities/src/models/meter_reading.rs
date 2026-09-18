use code_gen::Model;
use erp::types::field::{Decimal, IdMode, NaiveDate};

/// Model whose identity differs from the table backing it.
///
/// Exists to pin down that `id` drives the registry, the cache and the generated `BaseMeterReading`
/// type, while `table_name` is carried purely as storage metadata.
#[derive(Model)]
#[erp(id = "meter_reading")]
#[erp(table_name = "legacy_meter_data")]
#[allow(dead_code)]
pub struct MeterReading<Mode: IdMode> {
    pub id: Mode,
    #[erp(default = "")]
    reference: String,
    #[erp(default = 0.000)]
    value: Decimal,
    read_on: NaiveDate,
}
