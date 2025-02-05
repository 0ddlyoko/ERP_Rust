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
        M: Model<SingleId, BaseModel=BM>,
    {
        M::create_instance(self.id_mode.clone())
    }
}

impl<BM: BaseModel, Mode: IdMode> Reference<BM, Mode> {
    pub fn get_multiple<M>(&self) -> M
    where
        M: Model<MultipleIds, BaseModel=BM>,
    {
        M::create_instance(MultipleIds { ids: self.id_mode.get_ids_ref().clone() })
    }

    /// Check if given id is contained in the current reference
    pub fn contains(&self, id: &u32) -> bool {
        self.id_mode.contains(id)
    }

    /// Remove duplicated ids from this reference
    pub fn remove_dup(&mut self) {
        self.id_mode.remove_dup();
    }
}

impl<BM, E> From<E> for Reference<BM, SingleId>
where
    BM: BaseModel,
    SingleId: From<E>,
{
    fn from(value: E) -> Self {
        Reference {
            id_mode: value.into(),
            _phantom_data: Default::default(),
        }
    }
}

impl<BM, E> From<E> for Reference<BM, MultipleIds>
where
    BM: BaseModel,
    MultipleIds: From<E>,
{
    fn from(value: E) -> Self {
        Reference {
            id_mode: value.into(),
            _phantom_data: Default::default(),
        }
    }
}

/// Allow merging 2 references together
impl<BM: BaseModel, Mode1: IdMode, Mode2: IdMode> ops::Add<Reference<BM, Mode1>> for Reference<BM, Mode2> {
    type Output = Reference<BM, MultipleIds>;

    fn add(self, rhs: Reference<BM, Mode1>) -> Self::Output {
        let mut vecs = self.id_mode.get_ids_ref().clone();
        vecs.append(&mut rhs.id_mode.get_ids_ref().clone());
        vecs.into()
    }
}

/// Allow removing some ids from a reference
impl<BM: BaseModel, Mode1: IdMode, Mode2: IdMode> ops::Sub<Reference<BM, Mode1>> for Reference<BM, Mode2> {
    type Output = Reference<BM, MultipleIds>;

    fn sub(self, rhs: Reference<BM, Mode1>) -> Self::Output {
        let mut vecs = self.id_mode.get_ids_ref().clone();
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


