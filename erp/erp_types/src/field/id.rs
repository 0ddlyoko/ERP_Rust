use erp_search::RightTuple;
use sealed::Sealed;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::ops::{Add, AddAssign, Sub, SubAssign};
use std::slice::Iter;
use std::sync::Arc;
use std::vec::IntoIter;

/// One record's id, or none: an empty many2one reads as an empty record.
///
/// An empty one holds no id — 0 stands for it, never a record's — so every read of it finds
/// nothing, every write changes nothing, and its relations are empty in turn.
///
/// `prefetch` holds the ids of the recordset the record was taken from, shared rather than
/// copied: reading a field of one of them can then load it for the others in the same query.
/// It never changes which record this is, so equality and hashing ignore it.
#[derive(Default, Debug, Clone, Eq)]
pub struct SingleId {
    id: u32,
    ids: Vec<u32>,
    prefetch: Option<Arc<[u32]>>,
}

impl SingleId {
    /// No record.
    pub fn empty() -> Self {
        SingleId::default()
    }

    /// An id taken from a recordset, remembering the recordset.
    pub fn within(id: u32, prefetch: Arc<[u32]>) -> Self {
        SingleId {
            id,
            ids: vec![id],
            prefetch: Some(prefetch),
        }
    }

    pub fn get_id(&self) -> u32 {
        self.id
    }

    pub fn get_id_ref(&self) -> &u32 {
        &self.id
    }
}

impl Hash for SingleId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

#[derive(Default, Debug, Clone)]
pub struct MultipleIds {
    pub ids: Vec<u32>,
}

pub mod sealed {
    pub trait Sealed {}
}

// TODO Should we transform this into an enum, as we only have 2 structs ?
pub trait IdMode:
    Sealed + Clone + Into<MultipleIds> + Into<RightTuple> + IntoIterator<Item = SingleId> + AsRef<[u32]>
{
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
    /// Ids worth loading along with these ones: the recordset a record was taken from.
    fn prefetch_ids(&self) -> &[u32] {
        self.get_ids_ref()
    }
}

impl IdMode for SingleId {
    fn get_ids_ref(&self) -> &Vec<u32> {
        &self.ids
    }
    fn get_id_at(&self, pos: usize) -> &u32 {
        if pos != 0 || self.ids.is_empty() {
            return &u32::MAX;
        }
        &self.id
    }
    fn contains(&self, id: &u32) -> bool {
        self.ids.contains(id)
    }
    fn remove_dup(&mut self) {
        // Nothing to do here, as it's already a single id
    }
    fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
    fn prefetch_ids(&self) -> &[u32] {
        self.prefetch.as_deref().unwrap_or(&self.ids)
    }
}

impl AsRef<[u32]> for SingleId {
    fn as_ref(&self) -> &[u32] {
        &self.ids
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
        if id == 0 {
            return SingleId::empty();
        }
        SingleId {
            id,
            ids: vec![id],
            prefetch: None,
        }
    }
}

impl From<&u32> for SingleId {
    fn from(id: &u32) -> Self {
        (*id).into()
    }
}

impl From<SingleId> for RightTuple {
    fn from(id: SingleId) -> Self {
        id.id.into()
    }
}

impl From<&SingleId> for RightTuple {
    fn from(id: &SingleId) -> Self {
        id.id.into()
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
        MultipleIds {
            ids: ids.into_iter().copied().collect(),
        }
    }
}

impl From<SingleId> for MultipleIds {
    fn from(id: SingleId) -> Self {
        MultipleIds { ids: id.ids }
    }
}

impl From<&SingleId> for MultipleIds {
    fn from(id: &SingleId) -> Self {
        MultipleIds {
            ids: id.ids.clone(),
        }
    }
}

impl From<Vec<SingleId>> for MultipleIds {
    fn from(ids: Vec<SingleId>) -> Self {
        MultipleIds {
            ids: ids.iter().flat_map(|id| id.ids.iter().copied()).collect(),
        }
    }
}

impl From<&Vec<SingleId>> for MultipleIds {
    fn from(ids: &Vec<SingleId>) -> Self {
        MultipleIds {
            ids: ids.iter().flat_map(|id| id.ids.iter().copied()).collect(),
        }
    }
}

impl From<Vec<&SingleId>> for MultipleIds {
    fn from(ids: Vec<&SingleId>) -> Self {
        MultipleIds {
            ids: ids.iter().flat_map(|id| id.ids.iter().copied()).collect(),
        }
    }
}

impl From<&MultipleIds> for MultipleIds {
    fn from(ids: &MultipleIds) -> Self {
        Self {
            ids: ids.ids.clone(),
        }
    }
}

impl From<MultipleIds> for RightTuple {
    fn from(id: MultipleIds) -> Self {
        id.ids.into()
    }
}

impl From<&MultipleIds> for RightTuple {
    fn from(id: &MultipleIds) -> Self {
        id.ids.to_vec().into()
    }
}

// Iterators
impl IntoIterator for SingleId {
    type Item = SingleId;
    type IntoIter = std::option::IntoIter<SingleId>;

    fn into_iter(self) -> Self::IntoIter {
        (!self.is_empty()).then_some(self).into_iter()
    }
}

impl IntoIterator for MultipleIds {
    type Item = SingleId;
    type IntoIter = MultipleIdsIntoIterator;

    fn into_iter(self) -> Self::IntoIter {
        MultipleIdsIntoIterator::new(self.ids)
    }
}

/// Hands out each id of a recordset, every one remembering the recordset.
pub struct MultipleIdsIntoIterator {
    ids: IntoIter<u32>,
    prefetch: Arc<[u32]>,
}

impl MultipleIdsIntoIterator {
    pub fn new(ids: Vec<u32>) -> Self {
        MultipleIdsIntoIterator {
            prefetch: Arc::from(ids.as_slice()),
            ids: ids.into_iter(),
        }
    }
}

impl Iterator for MultipleIdsIntoIterator {
    type Item = SingleId;

    fn next(&mut self) -> Option<Self::Item> {
        self.ids
            .next()
            .map(|id| SingleId::within(id, self.prefetch.clone()))
    }
}

impl IntoIterator for &SingleId {
    type Item = SingleId;
    type IntoIter = std::option::IntoIter<SingleId>;

    fn into_iter(self) -> Self::IntoIter {
        (!self.is_empty()).then(|| self.clone()).into_iter()
    }
}

impl<'a> IntoIterator for &'a MultipleIds {
    type Item = SingleId;
    type IntoIter = IdsRefIntoIterator<'a>;

    fn into_iter(self) -> Self::IntoIter {
        IdsRefIntoIterator::new(self.get_ids_ref())
    }
}

/// Same as [`MultipleIdsIntoIterator`], borrowing the ids.
pub struct IdsRefIntoIterator<'a> {
    ids: Iter<'a, u32>,
    prefetch: Arc<[u32]>,
}

impl<'a> IdsRefIntoIterator<'a> {
    pub fn new(ids: &'a [u32]) -> Self {
        IdsRefIntoIterator {
            ids: ids.iter(),
            prefetch: Arc::from(ids),
        }
    }
}

impl<'a> Iterator for IdsRefIntoIterator<'a> {
    type Item = SingleId;

    fn next(&mut self) -> Option<Self::Item> {
        self.ids
            .next()
            .map(|id| SingleId::within(*id, self.prefetch.clone()))
    }
}

impl<E> FromIterator<E> for MultipleIds
where
    E: Into<MultipleIds>,
{
    fn from_iter<T: IntoIterator<Item = E>>(iter: T) -> Self {
        let mut result: MultipleIds = Default::default();
        for item in iter {
            result += item.into();
        }
        result.remove_dup();
        result
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
            ids: self
                .ids
                .into_iter()
                .filter(|id| !rhs.contains(id))
                .collect(),
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
        let mut result = Self { ids };
        result.remove_dup();
        result
    }
}

impl Add for SingleId {
    type Output = MultipleIds;

    fn add(self, rhs: Self) -> Self::Output {
        if self.id != rhs.id {
            Self::Output {
                ids: vec![self.id, rhs.id],
            }
        } else {
            Self::Output { ids: vec![self.id] }
        }
    }
}

impl AddAssign for MultipleIds {
    fn add_assign(&mut self, rhs: Self) {
        self.ids.append(rhs.ids.clone().as_mut());
        self.remove_dup();
    }
}
