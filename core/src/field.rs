mod field_type;

pub use field_type::FieldType;
use std::collections::HashSet;
use std::future::Future;

// type MyFn = dyn Fn(&HashSet<u64>) -> dyn Future<Output=Option<FieldType>>;
pub type MyFnResult = Box<dyn Future<Output = Option<FieldType>> + Send>;
type MyFn = Box<dyn Fn(&HashSet<u64>) -> MyFnResult>;

pub trait FromType<T> where Self: Sized {
    fn from_type(t: T) -> Self;
}

#[derive(Default)]
pub struct FieldDescriptor {
    pub name: String,
    pub default_value: Option<FieldType>,
    pub description: Option<String>,
    pub required: Option<bool>,
    pub compute: Option<MyFn>,
    
    // pub compute: Option<Box<dyn Fn(&HashSet<u64>) -> dyn Future<Output=Option<FieldType>>> + Send>,
}
