use crate::field::id::{IdsRefIntoIterator, MultipleIdsIntoIterator};
use crate::field::{FieldType, IdMode, MultipleIds, SingleId};
use crate::model::{BaseModel, CommonModel};
use std::marker::PhantomData;
use std::ops;

#[derive(Default, Debug)]
pub struct Reference<BM: BaseModel, Mode: IdMode> {
    pub id_mode: Mode,
    _phantom_data: PhantomData<BM>,
}

impl<BM: BaseModel> Reference<BM, SingleId> {
    /// Retrieves the instance of this ref.
    ///
    /// We don't load the record in cache, nor perform any modification / search to the database.
    pub fn get<M>(&self) -> M
    where
        M: CommonModel<SingleId, BaseModel = BM>,
    {
        M::create_instance(self.id_mode.clone())
    }
}

impl<BM: BaseModel, Mode: IdMode> Reference<BM, Mode> {
    pub fn get_multiple<M>(&self) -> M
    where
        M: CommonModel<MultipleIds, BaseModel = BM>,
    {
        M::create_instance(MultipleIds {
            ids: self.id_mode.get_ids_ref().clone(),
        })
    }

    /// Check if the given id is contained in the current reference
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
    E: Into<SingleId>,
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
    E: Into<MultipleIds>,
{
    fn from(value: E) -> Self {
        Reference {
            id_mode: value.into(),
            _phantom_data: Default::default(),
        }
    }
}

// impl<E: BaseModel> From<&FieldType> for Option<Reference<E, SingleId>> {
//     fn from(t: &FieldType) -> Self {
//         match t {
//             FieldType::Ref(id) => Some(id.into()),
//             _ => None,
//         }
//     }
// }

impl<E: BaseModel> From<&Reference<E, SingleId>> for FieldType {
    fn from(t: &Reference<E, SingleId>) -> Self {
        FieldType::Ref(t.id_mode.get_id())
    }
}

impl<E: BaseModel> From<Reference<E, SingleId>> for FieldType {
    fn from(t: Reference<E, SingleId>) -> Self {
        FieldType::Ref(t.id_mode.get_id())
    }
}

// impl<E: BaseModel> From<&FieldType> for Option<Reference<E, MultipleIds>> {
//     fn from(t: &FieldType) -> Self {
//         match t {
//             FieldType::Ref(id) => Some(vec![*id].into()),
//             FieldType::Refs(ids) => Some(ids.clone().into()),
//             _ => None,
//         }
//     }
// }

impl<E: BaseModel> From<&Reference<E, MultipleIds>> for FieldType {
    fn from(t: &Reference<E, MultipleIds>) -> Self {
        FieldType::Refs(t.id_mode.get_ids_ref().clone())
    }
}

impl<E: BaseModel> From<Reference<E, MultipleIds>> for FieldType {
    fn from(t: Reference<E, MultipleIds>) -> Self {
        FieldType::Refs(t.id_mode.ids)
    }
}

/// Allow merging 2 references together
impl<BM: BaseModel, Mode1: IdMode, Mode2: IdMode> ops::Add<Reference<BM, Mode1>>
    for Reference<BM, Mode2>
{
    type Output = Reference<BM, MultipleIds>;

    fn add(self, rhs: Reference<BM, Mode1>) -> Self::Output {
        let mut vecs = self.id_mode.get_ids_ref().clone();
        vecs.append(&mut rhs.id_mode.get_ids_ref().clone());
        vecs.into()
    }
}

/// Allow removing some ids from a reference
impl<BM: BaseModel, Mode1: IdMode, Mode2: IdMode> ops::Sub<Reference<BM, Mode1>>
    for Reference<BM, Mode2>
{
    type Output = Reference<BM, MultipleIds>;

    fn sub(self, rhs: Reference<BM, Mode1>) -> Self::Output {
        let mut vecs = self.id_mode.get_ids_ref().clone();
        vecs.retain(|id| !rhs.contains(id));
        vecs.into()
    }
}

// Iterators
impl<E: BaseModel> IntoIterator for Reference<E, MultipleIds> {
    type Item = Reference<E, SingleId>;
    type IntoIter = ReferenceIntoIterator<E>;

    fn into_iter(self) -> Self::IntoIter {
        ReferenceIntoIterator {
            ids: self.id_mode.into_iter(),
            _phantom_data: PhantomData,
        }
    }
}

/// Hands out each record of a reference, every one remembering the others so that reading a
/// field of one loads it for all.
pub struct ReferenceIntoIterator<E: BaseModel> {
    ids: MultipleIdsIntoIterator,
    _phantom_data: PhantomData<E>,
}

impl<E: BaseModel> Iterator for ReferenceIntoIterator<E> {
    type Item = Reference<E, SingleId>;

    fn next(&mut self) -> Option<Self::Item> {
        self.ids.next().map(|id_mode| Reference {
            id_mode,
            _phantom_data: PhantomData,
        })
    }
}

impl<'a, E: BaseModel> IntoIterator for &'a Reference<E, MultipleIds> {
    type Item = Reference<E, SingleId>;
    type IntoIter = ReferenceIterator<'a, E>;

    fn into_iter(self) -> Self::IntoIter {
        ReferenceIterator {
            ids: IdsRefIntoIterator::new(&self.id_mode.ids),
            _phantom_data: PhantomData,
        }
    }
}

/// Same as [`ReferenceIntoIterator`], borrowing the reference.
pub struct ReferenceIterator<'a, E: BaseModel> {
    ids: IdsRefIntoIterator<'a>,
    _phantom_data: PhantomData<E>,
}

impl<'a, E: BaseModel> Iterator for ReferenceIterator<'a, E> {
    type Item = Reference<E, SingleId>;

    fn next(&mut self) -> Option<Self::Item> {
        self.ids.next().map(|id_mode| Reference {
            id_mode,
            _phantom_data: PhantomData,
        })
    }
}
