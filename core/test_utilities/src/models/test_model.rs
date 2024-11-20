

use core::model::MapOfFields;
use core::model::ModelDescriptor;
use core::model::Model;

#[derive(Default)]
pub struct TestEmptyModel {}

impl Model for TestEmptyModel {
    fn get_model_name() -> String
    where
        Self: Sized
    {
        todo!()
    }

    fn get_model_descriptor() -> ModelDescriptor<Self>
    where
        Self: Model + Default
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
        Self: Sized
    {
        todo!()
    }
}