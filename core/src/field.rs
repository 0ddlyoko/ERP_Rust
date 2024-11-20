mod field_type;

use crate::model::Model;
pub use field_type::FieldType;

type ComputeFn<M> = Box<dyn Fn(&mut M) + Send + Sync>;

pub trait FromType<T> where Self: Sized {
    fn from_type(t: T) -> Self;
}

#[derive(Default)]
pub struct FieldDescriptor<M> where M: Model + Default {
    pub name: String,
    pub default_value: Option<FieldType>,
    pub description: Option<String>,
    pub required: Option<bool>,
    pub compute: Option<ComputeFn<M>>,
    
    // pub compute: Option<Box<dyn Fn(&HashSet<u64>) -> dyn Future<Output=Option<FieldType>>> + Send>,
}
