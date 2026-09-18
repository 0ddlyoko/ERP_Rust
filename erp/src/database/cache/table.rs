use crate::database::FieldType;
use crate::database::cache::Row;
use std::collections::HashMap;

#[derive(Default, Clone)]
pub(crate) struct Table {
    last_id: u32,
    pub(crate) rows: HashMap<u32, Row>,
}

impl Table {
    pub(crate) fn get_row(&self, id: &u32) -> Option<&Row> {
        self.rows.get(id)
    }

    pub(crate) fn get_row_mut(&mut self, id: &u32) -> Option<&mut Row> {
        self.rows.get_mut(id)
    }

    /// Insert a row under an id allocated by the shared store.
    ///
    /// Ids are handed out by [`CacheStore`](super::CacheStore) rather than per table, so two
    /// connections creating rows concurrently cannot pick the same one.
    pub(crate) fn insert_row(&mut self, id: u32, mut row: Row) {
        row.cells
            .insert("id".to_string(), Some(FieldType::UInteger(id)));
        self.last_id = self.last_id.max(id);
        self.rows.insert(id, Row { cells: row.cells });
    }
}
