use crate::environment::Environment;
use crate::model::{BaseModel, Model};
use std::marker::PhantomData;
use std::ops;
use std::slice::Iter;
use std::vec::IntoIter;
use crate::field::reference::sealed::Sealed;

// TODO Move all those in another file
#[derive(Default)]
pub struct SingleId {
    pub(crate) id: u32,
}
#[derive(Default)]
pub struct MultipleIds {
    pub(crate) ids: Vec<u32>,
}

mod sealed {
    pub trait Sealed {}
}
pub trait IdMode: Sealed {
    /// Returns a vector containing reference to ids saved in this reference
    fn get_ids_ref(&self) -> Vec<&u32>;
    /// Returns a vector containing ids saved in this reference
    fn get_ids(&self) -> Vec<u32>;
}
// TODO Add iter support
impl IdMode for SingleId {
    fn get_ids_ref(&self) -> Vec<&u32> {
        vec![&self.id]
    }
    fn get_ids(&self) -> Vec<u32> {
        vec![self.id]
    }
}
impl Sealed for SingleId {}

impl IdMode for MultipleIds {
    fn get_ids_ref(&self) -> Vec<&u32> {
        self.ids.iter().collect()
    }
    fn get_ids(&self) -> Vec<u32> {
        self.ids.clone()
    }
}
impl Sealed for MultipleIds {}

#[derive(Default)]
pub struct Reference<BM: BaseModel, Mode: IdMode> {
    pub id_mode: Mode,
    _phantom_data: PhantomData<BM>,
}

impl<BM: BaseModel> Reference<BM, SingleId> {

    /// Retrieves the instance of this ref.
    ///
    /// We don't load the record in cache, neither perform any modification / search to the database.
    pub fn get<M>(&self, env: &mut Environment) -> M
    where
        M: Model<BaseModel=BM>,
    {
        // TODO Check if we need to pass "env" here
        //  If we don't need "env", check if we can use "From" trait
        //  F::create_model(self.id_mode.id)
        env.get_record::<M>(self.id_mode.id)
    }
}

impl<BM: BaseModel, Mode: IdMode> Reference<BM, Mode> {
    pub fn get_multiple<M>(&self, env: &mut Environment) -> Vec<M>
    where
        M: Model<BaseModel=BM>,
    {
        // TODO Check if we need to pass "env" here
        //  If we don't need "env", check if we can use "From" trait
        self.id_mode.get_ids().into_iter().map(|id| env.get_record::<M>(id)).collect()
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
// TODO Allow removing 2 references together

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


