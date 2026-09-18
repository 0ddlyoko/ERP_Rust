//! Removing records, and untangling them from their relations on the way out.
use super::*;

impl<'mm> Environment<'mm> {
    /// Delete the given records.
    ///
    /// Relational fields are cleared before the rows go, so nothing keeps pointing at a record
    /// that no longer exists: children of a one2many get their many2one set to NULL, and the
    /// record is pulled out of the one2many lists of the parents it referenced.
    ///
    /// Setting the foreign key to NULL is the only referential policy the model metadata can
    /// express today — there is no `on_delete` attribute — so it is the one applied.
    ///
    /// The deletion is written straight to the database, like creation is; rolling the
    /// environment back therefore undoes it.
    pub fn unlink<Mode: IdMode>(&mut self, model_name: &str, ids: &Mode) -> Result<u32> {
        if ids.is_empty() {
            return Ok(0);
        }
        let model = self.model_manager.try_get_model(model_name)?;

        // Pending changes must reach the database before the relational fields below start
        // reading through the cache, which can itself trigger a flush.
        self.save_model_to_db(model_name)?;

        let relational_fields: Vec<&'mm str> = model
            .fields
            .iter()
            .filter_map(|(field_name, field)| field.inverse.as_ref().map(|_| field_name.as_str()))
            .collect();
        for field_name in relational_fields {
            self.save_field_to_cache(
                model_name,
                field_name,
                ids,
                None,
                &Dirty::UpdateDirty,
                &Update::UpdateIfExists,
            )?;
        }
        // Flush the mirrors that were just detached, before the rows disappear.
        self.save_all_to_db()?;

        let number_of_deletions = self.database.delete(model_name, ids.as_ref())?;
        self.cache.remove_records(model_name, ids);
        Ok(number_of_deletions)
    }
}
