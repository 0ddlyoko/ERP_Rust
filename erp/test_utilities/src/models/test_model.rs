use erp::environment::Environment;
use erp::model::MapOfFields;
use erp::model::Model;
use erp::model::ModelDescriptor;
use std::error::Error;

#[derive(Default)]
pub struct TestEmptyModel {}

impl Model for TestEmptyModel {
    fn get_model_name() -> String
    where
        Self: Sized,
    {
        todo!()
    }

    fn get_model_descriptor() -> ModelDescriptor
    where
        Self: Model + Default,
    {
        todo!()
    }

    fn get_id(&self) -> u32 {
        todo!()
    }

    fn get_data(&self) -> MapOfFields {
        todo!()
    }

    fn create_model(_id: u32, _data: MapOfFields) -> Self
    where
        Self: Sized,
    {
        todo!()
    }

    fn call_compute_method(
        &mut self,
        field_name: &str,
        env: &mut Environment,
    ) -> Result<(), Box<dyn Error>> {
        todo!()
    }
}
