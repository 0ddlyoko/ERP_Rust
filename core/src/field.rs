mod field_type;

pub use field_type::FieldType;

pub trait FromType<T> where Self: Sized {
    fn from_type(t: T) -> Option<Self>;
}

#[derive(Default)]
pub struct FieldDescriptor {
    pub name: String,
    pub default_value: Option<FieldType>,
    pub description: Option<String>,
    pub required: Option<bool>,
}
