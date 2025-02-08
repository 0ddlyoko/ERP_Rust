use crate::field::SingleId;
use crate::model::Model;
use std::marker::PhantomData;
use std::slice::Iter;
use std::vec::IntoIter;

pub struct ModelIntoIterator<M: Model<SingleId>> {
    pub ids: IntoIter<u32>,
    pub _phantom_data: PhantomData<M>,
}

impl<M: Model<SingleId>> Iterator for ModelIntoIterator<M> {
    type Item = M;

    fn next(&mut self) -> Option<Self::Item> {
        self.ids.next().map(|id| {
            M::create_instance(id.into())
        })
    }
}

pub struct ModelIterator<'a, M: Model<SingleId>> {
    pub ids: Iter<'a, u32>,
    pub _phantom_data: PhantomData<M>,
}

impl<'a, M: Model<SingleId>> Iterator for ModelIterator<'a, M> {
    type Item = M;
    
    fn next(&mut self) -> Option<Self::Item> {
        self.ids.next().map(|id| {
            M::create_instance(id.into())
        })
    }
}
