use crate::model::{BaseModel, Model};
use std::marker::PhantomData;
use std::ops;
use std::slice::Iter;
use std::vec::IntoIter;
use crate::field::{IdMode, MultipleIds, SingleId};

#[derive(Default, Debug)]
pub struct Reference<BM: BaseModel, Mode: IdMode> {
    pub id_mode: Mode,
    _phantom_data: PhantomData<BM>,
}

impl<BM: BaseModel> Reference<BM, SingleId> {

    /// Retrieves the instance of this ref.
    ///
    /// We don't load the record in cache, neither perform any modification / search to the database.
    pub fn get<M>(&self) -> M
    where
        M: Model<BaseModel=BM>,
    {
        M::create_model(self.id_mode.id)
    }
}

impl<BM: BaseModel, Mode: IdMode> Reference<BM, Mode> {
    pub fn get_multiple<M>(&self) -> Vec<M>
    where
        M: Model<BaseModel=BM>,
    {
        self.id_mode.get_ids().into_iter().map(|id| M::create_model(id)).collect()
    }

    /// Check if given id is contained in the current reference
    pub fn contains(&self, id: &u32) -> bool {
        self.id_mode.contains(id)
    }
}

impl<BM: BaseModel> From<u32> for Reference<BM, SingleId> {
    fn from(value: u32) -> Self {
        Reference {
            id_mode: SingleId { id: value },
            _phantom_data: Default::default(),
        }
    }
}

impl<BM: BaseModel> From<&u32> for Reference<BM, SingleId> {
    fn from(value: &u32) -> Self {
        (*value).into()
    }
}

impl<BM: BaseModel> From<Vec<u32>> for Reference<BM, MultipleIds> {
    fn from(value: Vec<u32>) -> Self {
        Reference {
            id_mode: MultipleIds { ids: value },
            _phantom_data: Default::default(),
        }
    }
}

impl<BM: BaseModel> From<u32> for Reference<BM, MultipleIds> {
    fn from(value: u32) -> Self {
        vec![value].into()
    }
}

impl<BM: BaseModel> From<&u32> for Reference<BM, MultipleIds> {
    fn from(value: &u32) -> Self {
        vec![*value].into()
    }
}

/// Allow SingleId => MultipleIds
impl<BM: BaseModel> From<Reference<BM, SingleId>> for Reference<BM, MultipleIds> {
    fn from(value: Reference<BM, SingleId>) -> Self {
        value.id_mode.id.into()
    }
}

/// Allow merging 2 references together
impl<BM: BaseModel, Mode1: IdMode, Mode2: IdMode> ops::Add<Reference<BM, Mode1>> for Reference<BM, Mode2> {
    type Output = Reference<BM, MultipleIds>;

    fn add(self, rhs: Reference<BM, Mode1>) -> Self::Output {
        let mut vecs = self.id_mode.get_ids();
        vecs.append(&mut rhs.id_mode.get_ids());
        vecs.into()
    }
}

/// Allow removing some ids from a reference
impl<BM: BaseModel, Mode1: IdMode, Mode2: IdMode> ops::Sub<Reference<BM, Mode1>> for Reference<BM, Mode2> {
    type Output = Reference<BM, MultipleIds>;

    fn sub(self, rhs: Reference<BM, Mode1>) -> Self::Output {
        let mut vecs = self.id_mode.get_ids();
        vecs.retain(|id| rhs.contains(id));
        vecs.into()
    }
}

// Iterators
// TODO Add the original list of ids somewhere to also load data of the other ids if we try to
//  access to a field from a specific element
impl<E: BaseModel> IntoIterator for Reference<E, MultipleIds> {
    type Item = Reference<E, SingleId>;
    type IntoIter = ReferenceIntoIterator<E>;

    fn into_iter(self) -> Self::IntoIter {
        ReferenceIntoIterator {
            ids: self.id_mode.ids.into_iter(),
            _phantom_data: PhantomData,
        }
    }
}

pub struct ReferenceIntoIterator<E: BaseModel> {
    ids: IntoIter<u32>,
    _phantom_data: PhantomData<E>,
}

impl<E: BaseModel> Iterator for ReferenceIntoIterator<E> {
    type Item = Reference<E, SingleId>;

    fn next(&mut self) -> Option<Self::Item> {
        self.ids.next().map(|id| id.into())
    }
}

impl<'a, E: BaseModel> IntoIterator for &'a Reference<E, MultipleIds> {
    type Item = Reference<E, SingleId>;
    type IntoIter = ReferenceIterator<'a, E>;

    fn into_iter(self) -> Self::IntoIter {
        ReferenceIterator {
            ids: self.id_mode.ids.iter(),
            _phantom_data: PhantomData,
        }
    }
}

pub struct ReferenceIterator<'a, E: BaseModel> {
    ids: Iter<'a, u32>,
    _phantom_data: PhantomData<E>,
}

impl<'a, E: BaseModel> Iterator for ReferenceIterator<'a, E> {
    type Item = Reference<E, SingleId>;

    fn next(&mut self) -> Option<Self::Item> {
        self.ids.next().map(|id| id.into())
    }
}


