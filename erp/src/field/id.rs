use crate::field::id::sealed::Sealed;

#[derive(Default, Debug)]
pub struct SingleId {
    pub(crate) id: u32,
}

#[derive(Default, Debug)]
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
    /// Check if given id is in the list
    fn contains(&self, id: &u32) -> bool;
}

impl IdMode for SingleId {
    fn get_ids_ref(&self) -> Vec<&u32> {
        vec![&self.id]
    }
    fn get_ids(&self) -> Vec<u32> {
        vec![self.id]
    }
    fn contains(&self, id: &u32) -> bool {
        &self.id == id
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
    fn contains(&self, id: &u32) -> bool {
        self.ids.contains(id)
    }
}

impl Sealed for MultipleIds {}