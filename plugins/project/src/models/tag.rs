use code_gen::{Model, selection};
use erp::types::field::IdMode;

#[selection]
pub enum TagColor {
    #[default]
    #[selection(label = "Grey")]
    Grey,
    #[selection(label = "Red")]
    Red,
    #[selection(label = "Orange")]
    Orange,
    #[selection(label = "Yellow")]
    Yellow,
    #[selection(label = "Green")]
    Green,
    #[selection(label = "Teal")]
    Teal,
    #[selection(label = "Blue")]
    Blue,
    #[selection(label = "Purple")]
    Purple,
    #[selection(label = "Pink")]
    Pink,
}

/// A word sorting tasks across projects, in a colour of its own: `Bug`, `Urgent client`, `Design`.
#[derive(Model)]
#[erp(id = "project_tag")]
#[allow(dead_code)]
pub struct Tag<Mode: IdMode> {
    id: Mode,
    #[erp(index = "trigram")]
    name: String,
    color: TagColor,
}
