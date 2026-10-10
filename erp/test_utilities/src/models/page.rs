use crate::models::BaseNotebook;
use code_gen::Model;
use erp::types::field::{IdMode, Reference, SingleId};

#[derive(Model)]
#[erp(id = "page")]
#[allow(dead_code)]
pub struct Page<Mode: IdMode> {
    pub id: Mode,
    text: String,
    notebook: Reference<BaseNotebook, SingleId>,
}
