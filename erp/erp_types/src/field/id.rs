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

/// Records of a model, by their ids.
///
/// `prefetch` is the recordset these ids were taken from — what reading a field loads along with
/// them — and `None` while they are a recordset of their own: the ids are then what is loaded.
/// What is taken from them, looping over them or keeping some, gets theirs, so a record taken
/// from a recordset still reads its fields with the others.
#[derive(Default, Debug, Clone)]
pub struct MultipleIds {
    pub ids: Vec<u32>,
    prefetch: Option<Arc<[u32]>>,
}

impl MultipleIds {
    /// These ids, loading their fields with those of `prefetch`: the records their recordset was
    /// reached along with.
    pub fn within(ids: Vec<u32>, prefetch: Arc<[u32]>) -> Self {
        MultipleIds {
            ids,
            prefetch: Some(prefetch),
        }
    }

    /// The one record these ids name — none when empty — with the recordset it was taken from;
    /// how many they are when several.
    pub fn as_single(&self) -> Result<SingleId, usize> {
        match (self.ids.as_slice(), &self.prefetch) {
            ([], _) => Ok(SingleId::empty()),
            ([id], Some(prefetch)) => Ok(SingleId::within(*id, prefetch.clone())),
            ([id], None) => Ok(SingleId::from(*id)),
            (ids, _) => Err(ids.len()),
        }
    }
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
    /// The same, shared, for what is taken from these ids — each record looped over, a part
    /// kept — to load its fields with them.
    fn shared_prefetch(&self) -> Arc<[u32]> {
        Arc::from(self.prefetch_ids())
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
    fn shared_prefetch(&self) -> Arc<[u32]> {
        self.prefetch
            .clone()
            .unwrap_or_else(|| Arc::from(self.ids.as_slice()))
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
    fn prefetch_ids(&self) -> &[u32] {
        self.prefetch.as_deref().unwrap_or(&self.ids)
    }
    fn shared_prefetch(&self) -> Arc<[u32]> {
        self.prefetch
            .clone()
            .unwrap_or_else(|| Arc::from(self.ids.as_slice()))
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
        MultipleIds {
            ids: vec![id],
            prefetch: None,
        }
    }
}

impl From<&u32> for MultipleIds {
    fn from(id: &u32) -> Self {
        MultipleIds {
            ids: vec![*id],
            prefetch: None,
        }
    }
}

impl From<Vec<u32>> for MultipleIds {
    fn from(ids: Vec<u32>) -> Self {
        MultipleIds {
            ids,
            prefetch: None,
        }
    }
}

impl From<&Vec<u32>> for MultipleIds {
    fn from(ids: &Vec<u32>) -> Self {
        MultipleIds {
            ids: ids.clone(),
            prefetch: None,
        }
    }
}

impl From<Vec<&u32>> for MultipleIds {
    fn from(ids: Vec<&u32>) -> Self {
        MultipleIds {
            ids: ids.into_iter().copied().collect(),
            prefetch: None,
        }
    }
}

impl From<SingleId> for MultipleIds {
    fn from(id: SingleId) -> Self {
        MultipleIds {
            ids: id.ids,
            prefetch: id.prefetch,
        }
    }
}

impl From<&SingleId> for MultipleIds {
    fn from(id: &SingleId) -> Self {
        MultipleIds {
            ids: id.ids.clone(),
            prefetch: id.prefetch.clone(),
        }
    }
}

impl From<Vec<SingleId>> for MultipleIds {
    fn from(ids: Vec<SingleId>) -> Self {
        MultipleIds {
            ids: ids.iter().flat_map(|id| id.ids.iter().copied()).collect(),
            prefetch: None,
        }
    }
}

impl From<&Vec<SingleId>> for MultipleIds {
    fn from(ids: &Vec<SingleId>) -> Self {
        MultipleIds {
            ids: ids.iter().flat_map(|id| id.ids.iter().copied()).collect(),
            prefetch: None,
        }
    }
}

impl From<Vec<&SingleId>> for MultipleIds {
    fn from(ids: Vec<&SingleId>) -> Self {
        MultipleIds {
            ids: ids.iter().flat_map(|id| id.ids.iter().copied()).collect(),
            prefetch: None,
        }
    }
}

impl From<&MultipleIds> for MultipleIds {
    fn from(ids: &MultipleIds) -> Self {
        ids.clone()
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
        let prefetch = self.shared_prefetch();
        MultipleIdsIntoIterator {
            ids: self.ids.into_iter(),
            prefetch,
        }
    }
}

/// Hands out each id of a recordset, every one remembering the recordset.
pub struct MultipleIdsIntoIterator {
    ids: IntoIter<u32>,
    prefetch: Arc<[u32]>,
}

impl MultipleIdsIntoIterator {
    /// The ids, each remembering `prefetch`, the recordset they were taken from.
    pub fn within(ids: Vec<u32>, prefetch: Arc<[u32]>) -> Self {
        MultipleIdsIntoIterator {
            ids: ids.into_iter(),
            prefetch,
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
        IdsRefIntoIterator {
            ids: self.ids.iter(),
            prefetch: self.shared_prefetch(),
        }
    }
}

/// Same as [`MultipleIdsIntoIterator`], borrowing the ids.
pub struct IdsRefIntoIterator<'a> {
    ids: Iter<'a, u32>,
    prefetch: Arc<[u32]>,
}

impl<'a> IdsRefIntoIterator<'a> {
    /// The ids, each remembering `prefetch`, the recordset they were taken from.
    pub fn within(ids: &'a [u32], prefetch: Arc<[u32]>) -> Self {
        IdsRefIntoIterator {
            ids: ids.iter(),
            prefetch,
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
        let prefetch = Some(self.shared_prefetch());
        Self {
            ids: self
                .ids
                .into_iter()
                .filter(|id| !rhs.contains(id))
                .collect(),
            prefetch,
        }
    }
}

impl SubAssign for MultipleIds {
    fn sub_assign(&mut self, rhs: Self) {
        self.prefetch = Some(self.shared_prefetch());
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
            prefetch: None,
        };
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
                prefetch: None,
            }
        } else {
            Self::Output {
                ids: vec![self.id],
                prefetch: None,
            }
        }
    }
}

impl AddAssign for MultipleIds {
    /// The union is a recordset of its own: it loads its own ids.
    fn add_assign(&mut self, rhs: Self) {
        self.ids.append(rhs.ids.clone().as_mut());
        self.prefetch = None;
        self.remove_dup();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prefetch_of(id: &SingleId) -> Vec<u32> {
        id.prefetch_ids().to_vec()
    }

    /// A recordset of its own loads its own ids; a record taken from it remembers them.
    #[test]
    fn test_a_record_looped_over_remembers_its_recordset() {
        let ids = MultipleIds::from(vec![1, 2, 3]);
        assert_eq!(ids.prefetch_ids(), &[1, 2, 3]);
        let records: Vec<SingleId> = ids.into_iter().collect();
        assert_eq!(prefetch_of(&records[1]), vec![1, 2, 3]);
    }

    /// A part kept keeps the recordset it was taken from, and so do the records looped over in
    /// it; a union is a recordset of its own.
    #[test]
    fn test_a_part_keeps_the_recordset_a_union_has_its_own() {
        let part = MultipleIds::from(vec![1, 2, 3, 4]) - MultipleIds::from(vec![3, 4]);
        assert_eq!(part.get_ids_ref(), &vec![1, 2]);
        assert_eq!(part.prefetch_ids(), &[1, 2, 3, 4]);
        let first = (&part).into_iter().next().expect("a record");
        assert_eq!(prefetch_of(&first), vec![1, 2, 3, 4]);

        let mut kept = MultipleIds::from(vec![5, 6]);
        kept -= MultipleIds::from(vec![6]);
        assert_eq!(kept.prefetch_ids(), &[5, 6]);

        let union = part + MultipleIds::from(vec![9]);
        assert_eq!(union.prefetch_ids(), &[1, 2, 9]);
    }

    /// One record, through a call and back, still reads with its recordset.
    #[test]
    fn test_one_record_comes_back_with_its_recordset() {
        let record = MultipleIds::from(vec![7, 8])
            .into_iter()
            .nth(1)
            .expect("a record");
        let carried = MultipleIds::from(&record);
        let back = carried.as_single().expect("one record");
        assert_eq!(back.get_id(), 8);
        assert_eq!(prefetch_of(&back), vec![7, 8]);
        assert!(MultipleIds::default().as_single().expect("none").is_empty());
        assert_eq!(MultipleIds::from(vec![1, 2]).as_single().err(), Some(2));
    }
}
