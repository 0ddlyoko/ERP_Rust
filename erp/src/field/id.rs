use crate::field::id::sealed::Sealed;
use std::collections::HashSet;
use std::ops::{Add, AddAssign, Sub, SubAssign};
use std::slice::Iter;
use std::vec::IntoIter;

#[derive(Default, Debug, Clone, Hash, Eq)]
pub struct SingleId {
    id: u32,
    ids: Vec<u32>,
}

impl SingleId {
    pub fn get_id(&self) -> u32 {
        self.id
    }

    pub fn get_id_ref(&self) -> &u32 {
        &self.id
    }
}

#[derive(Default, Debug, Clone)]
pub struct MultipleIds {
    pub(crate) ids: Vec<u32>,
}

mod sealed {
    pub trait Sealed {}
}

pub trait IdMode: Sealed + Clone + Into<MultipleIds> + IntoIterator<Item = SingleId> + AsRef<[u32]> {
    /// Returns a vector containing ids saved in this reference
    fn get_ids_ref(&self) -> &Vec<u32>;
    /// Return the id at given pos.
    ///
    /// If pos is < 0 or >= len(ids), return u32::MAX
    fn get_id_at(&self, pos: usize) -> &u32;
    /// Check if given id is in the list
    fn contains(&self, id: &u32) -> bool;
    /// Remove duplicated ids
    fn remove_dup(&mut self);
    /// Check if ids are empty
    fn is_empty(&self) -> bool;
}

impl IdMode for SingleId {
    fn get_ids_ref(&self) -> &Vec<u32> {
        &self.ids
    }
    fn get_id_at(&self, pos: usize) -> &u32 {
        if pos != 0 {
            return &u32::MAX;
        }
        &self.id
    }
    fn contains(&self, id: &u32) -> bool {
        &self.id == id
    }
    fn remove_dup(&mut self) {
        // Nothing to do here, as it's already a single id
    }
    fn is_empty(&self) -> bool {
        false
    }
}
impl AsRef<[u32]> for SingleId {
    fn as_ref(&self) -> &[u32] {
        &self.ids
    }
}
impl From<SingleId> for MultipleIds {
    fn from(id: SingleId) -> Self {
        id.get_id().into()
    }
}
impl Sealed for SingleId {}

impl IdMode for MultipleIds {
    fn get_ids_ref(&self) -> &Vec<u32> {
        &self.ids
    }
    fn get_id_at(&self, pos: usize) -> &u32 {
        if pos >= self.ids.len() {
            return &u32::MAX;
        }
        &self.ids[pos]
    }
    fn contains(&self, id: &u32) -> bool {
        self.ids.contains(id)
    }
    fn remove_dup(&mut self) {
        let mut seen = HashSet::new();
        self.ids.retain(|id| seen.insert(*id));
    }
    fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
}
impl AsRef<[u32]> for MultipleIds {
    fn as_ref(&self) -> &[u32] {
        &self.ids
    }
}
impl Sealed for MultipleIds {}

// From

impl From<u32> for SingleId {
    fn from(id: u32) -> Self {
        SingleId { id, ids: vec![id] }
    }
}

impl From<&u32> for SingleId {
    fn from(id: &u32) -> Self {
        SingleId { id: *id, ids: vec![*id] }
    }
}

impl From<u32> for MultipleIds {
    fn from(id: u32) -> Self {
        MultipleIds { ids: vec![id] }
    }
}

impl From<&u32> for MultipleIds {
    fn from(id: &u32) -> Self {
        MultipleIds { ids: vec![*id] }
    }
}

impl From<Vec<u32>> for MultipleIds {
    fn from(ids: Vec<u32>) -> Self {
        MultipleIds { ids }
    }
}

impl From<&Vec<u32>> for MultipleIds {
    fn from(ids: &Vec<u32>) -> Self {
        MultipleIds { ids: ids.clone() }
    }
}

impl From<Vec<&u32>> for MultipleIds {
    fn from(ids: Vec<&u32>) -> Self {
        MultipleIds { ids: ids.into_iter().copied().collect() }
    }
}

// TODO Find a way to make it work
// impl<E> From<E> for MultipleIds
// where
//     Vec<u32>: From<E>,
// {
//     fn from(value: E) -> Self {
//         todo!()
//     }
// }

impl From<&SingleId> for MultipleIds {
    fn from(id: &SingleId) -> Self {
        id.get_id().into()
    }
}

impl From<Vec<SingleId>> for MultipleIds {
    fn from(ids: Vec<SingleId>) -> Self {
        MultipleIds { ids: ids.iter().map(|id| id.get_id()).collect() }
    }
}

impl From<&Vec<SingleId>> for MultipleIds {
    fn from(ids: &Vec<SingleId>) -> Self {
        MultipleIds { ids: ids.iter().map(|id| id.get_id()).collect() }
    }
}

impl From<Vec<&SingleId>> for MultipleIds {
    fn from(ids: Vec<&SingleId>) -> Self {
        MultipleIds { ids: ids.iter().map(|&id| id.get_id()).collect() }
    }
}

// Iterators

impl IntoIterator for SingleId {
    type Item = SingleId;
    type IntoIter = MultipleIdsIntoIterator;

    fn into_iter(self) -> Self::IntoIter {
        MultipleIdsIntoIterator {
            ids: self.ids.into_iter(),
        }
    }
}

impl IntoIterator for MultipleIds {
    type Item = SingleId;
    type IntoIter = MultipleIdsIntoIterator;

    fn into_iter(self) -> Self::IntoIter {
        MultipleIdsIntoIterator {
            ids: self.ids.into_iter(),
        }
    }
}

pub struct MultipleIdsIntoIterator {
    ids: IntoIter<u32>,
}

impl Iterator for MultipleIdsIntoIterator {
    type Item = SingleId;

    fn next(&mut self) -> Option<Self::Item> {
        self.ids.next().map(|id| id.into())
    }
}

impl<'a> IntoIterator for &'a SingleId {
    type Item = SingleId;
    type IntoIter = IdsRefIntoIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        IdsRefIntoIterator {
            ids: self.get_ids_ref().iter(),
        }
    }
}

impl<'a> IntoIterator for &'a MultipleIds {
    type Item = SingleId;
    type IntoIter = IdsRefIntoIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        IdsRefIntoIterator {
            ids: self.get_ids_ref().iter(),
        }
    }
}

pub struct IdsRefIntoIterator<'a> {
    ids: Iter<'a, u32>,
}

impl<'a> Iterator for IdsRefIntoIterator<'a> {
    type Item = SingleId;

    fn next(&mut self) -> Option<Self::Item> {
        self.ids.next().map(|id| id.into())
    }
}

// Eq

impl PartialEq<u32> for SingleId {
    fn eq(&self, other: &u32) -> bool {
        self.id == *other
    }
}

impl PartialEq<Vec<u32>> for SingleId {
    fn eq(&self, other: &Vec<u32>) -> bool {
        other.len() == 1 && self.id == other[0]
    }
}

impl<Mode: IdMode> PartialEq<Mode> for SingleId {
    fn eq(&self, other: &Mode) -> bool {
        let other_ids = other.get_ids_ref();
        if other_ids.len() != 1 {
            false
        } else {
            other_ids[0] == self.id
        }
    }
}

impl PartialEq<Vec<u32>> for MultipleIds {
    fn eq(&self, other: &Vec<u32>) -> bool {
        &self.ids == other
    }
}

impl PartialEq<u32> for MultipleIds {
    fn eq(&self, other: &u32) -> bool {
        self.ids.len() == 1 && self.ids[0] == *other
    }
}

impl<Mode: IdMode> PartialEq<Mode> for MultipleIds {
    fn eq(&self, other: &Mode) -> bool {
        &self.ids == other.get_ids_ref()
    }
}

// +, -

impl Sub for MultipleIds {
    type Output = MultipleIds;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            ids: self.ids.into_iter().filter(|id| !rhs.contains(id)).collect(),
        }
    }
}

impl SubAssign for MultipleIds {
    fn sub_assign(&mut self, rhs: Self) {
        self.ids.retain(|id| !rhs.contains(id));
    }
}

impl Add for MultipleIds {
    type Output = MultipleIds;

    fn add(self, rhs: Self) -> Self::Output {
        let mut ids = self.ids.clone();
        ids.append(rhs.ids.clone().as_mut());
        let mut result = Self {
            ids,
        };
        result.remove_dup();
        result
    }
}

impl AddAssign for MultipleIds {
    fn add_assign(&mut self, rhs: Self) {
        self.ids.append(rhs.ids.clone().as_mut());
        self.remove_dup();
    }
}
