use code_gen::Model;
use erp::types::field::{IdMode, MultipleIds, Reference, SingleId};

/// A notebook owns its pages: a page removed from it is deleted rather than left on its own.
#[derive(Model)]
#[erp(id = "notebook")]
#[allow(dead_code)]
pub struct Notebook<Mode: IdMode> {
    pub id: Mode,
    #[erp(default = "")]
    name: String,
    #[erp(inverse = "notebook", owned)]
    pages: Reference<BasePage, MultipleIds>,
}

#[derive(Model)]
#[erp(id = "page")]
#[allow(dead_code)]
pub struct Page<Mode: IdMode> {
    pub id: Mode,
    #[erp(default = "")]
    text: String,
    notebook: Reference<BaseNotebook, SingleId>,
}
