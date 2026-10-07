use crate::model::Model;
use erp_types::field::SingleId;
use erp_types::field::{IdsRefIntoIterator, MultipleIdsIntoIterator};
use std::marker::PhantomData;
use std::sync::Arc;

/// Hands out each record of a recordset, every one remembering the recordset so that reading a
/// field of one loads it for all.
pub struct ModelIntoIterator<M: Model<SingleId>> {
    ids: MultipleIdsIntoIterator,
    _phantom_data: PhantomData<M>,
}

impl<M: Model<SingleId>> ModelIntoIterator<M> {
    /// Each record of `ids`, remembering `prefetch`, the recordset they were taken from.
    pub fn new(ids: Vec<u32>, prefetch: Arc<[u32]>) -> Self {
        ModelIntoIterator {
            ids: MultipleIdsIntoIterator::within(ids, prefetch),
            _phantom_data: PhantomData,
        }
    }
}

impl<M: Model<SingleId>> Iterator for ModelIntoIterator<M> {
    type Item = M;

    fn next(&mut self) -> Option<Self::Item> {
        self.ids.next().map(M::create_instance)
    }
}

/// Same as [`ModelIntoIterator`], borrowing the recordset.
pub struct ModelIterator<'a, M: Model<SingleId>> {
    ids: IdsRefIntoIterator<'a>,
    _phantom_data: PhantomData<M>,
}

impl<'a, M: Model<SingleId>> ModelIterator<'a, M> {
    /// Each record of `ids`, remembering `prefetch`, the recordset they were taken from.
    pub fn new(ids: &'a [u32], prefetch: Arc<[u32]>) -> Self {
        ModelIterator {
            ids: IdsRefIntoIterator::within(ids, prefetch),
            _phantom_data: PhantomData,
        }
    }
}

impl<'a, M: Model<SingleId>> Iterator for ModelIterator<'a, M> {
    type Item = M;

    fn next(&mut self) -> Option<Self::Item> {
        self.ids.next().map(M::create_instance)
    }
}
