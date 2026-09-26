use crate::field::{FieldCompute, FieldKind, FieldReference, FieldType};

#[derive(Default)]
pub struct FieldDescriptor {
    pub name: String,
    /// Type of the field. Carried explicitly rather than read off the default, so a field can
    /// have no default at all.
    pub kind: FieldKind,
    pub default_value: Option<FieldType>,
    pub description: Option<String>,
    pub required: bool,
    /// Whether the field never leaves the process.
    ///
    /// A password, a token, a note nobody outside should read. It is written, computed and
    /// stored like any other; what changes is that the API refuses to hand it over, to filter on
    /// it, or to sort by it — filtering is the worse of the three, because comparing a value
    /// reconstructs it without ever reading it.
    pub private: bool,
    /// Whether this struct asked for the field to be kept in a column.
    ///
    /// Only meaningful for a computed field; everything else is kept anyway. What the field ends
    /// up being is settled once every struct contributing to the model has been seen, because
    /// being computed belongs to the model's field and not to one struct's view of it.
    pub asks_for_storage: bool,
    pub compute: Option<FieldCompute>,
    pub field_ref: Option<FieldReference>,
}
