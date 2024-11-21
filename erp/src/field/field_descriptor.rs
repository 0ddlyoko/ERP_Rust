use crate::field::FieldType;
use crate::model::Model;

pub(crate) type ComputeFn<M> = Box<dyn Fn(&mut M) + Send + Sync>;

#[derive(Default)]
pub struct FieldDescriptor<M> where M: Model + Default {
    pub name: String,
    pub default_value: Option<FieldType>,
    pub description: Option<String>,
    pub required: Option<bool>,
    pub compute: Option<ComputeFn<M>>,
}