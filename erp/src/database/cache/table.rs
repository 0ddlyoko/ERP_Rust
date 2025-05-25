use std::collections::HashMap;
use crate::database::cache::Row;

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

    pub(crate) fn add_row(&mut self, row: Row) -> u32 {
        self.last_id += 1;
        let id = self.last_id;
        let row = Row {
            id,
            cells: row.cells,
        };
        self.rows.insert(id, row);
        id
    }

    pub(crate) fn delete_row(&mut self, id: &u32) {
        self.rows.remove(id);
    }
}
