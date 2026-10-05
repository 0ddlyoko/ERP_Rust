//! Transaction boundaries: commit, savepoints, and recovering the concrete environment.
use super::*;

impl<'mm> Environment<'mm> {
    /// Flush cache to the database, commit, and close the transaction.
    pub fn close(mut self) -> Result<()> {
        self.save_all_to_db()?;
        // Commiting here ensures everything is saved to the database, so we can take back the
        //  database and replace it with a cache one
        self.database.commit_transaction()?;
        self.closed = true;
        self.forget_shared_after_commit();
        crate::plugin::request_installs(std::mem::take(&mut self.requested_installs));
        Ok(())
    }
    /// If an error is returned, rollback the commit and put back the cache as it was
    /// Recover the concrete environment from the type-erased handle given to compute methods.
    ///
    /// Generated model code calls this instead of casting on its own, so the framework's single
    /// `unsafe` lives in one audited place rather than inside every plugin.
    ///
    /// Fully sound erasure would require `erp_types` to be able to name `Environment`, which the
    /// current crate layout forbids; the assertion below is the guard until that cycle is broken.
    ///
    /// # Panics
    /// Panics if `env` is not an [`Environment`].
    pub fn from_erased<'a>(env: &'a mut dyn ErasedEnvironment) -> &'a mut Environment<'a> {
        assert_eq!(
            env.erased_type_name(),
            ENVIRONMENT_TYPE_NAME,
            "ErasedEnvironment must only ever be implemented by Environment"
        );
        // SAFETY: the assertion above establishes that the concrete type behind `env` really is an
        // `Environment`. The returned lifetimes are tied to the borrow of `env`, so the reference
        // cannot outlive the value it points at.
        unsafe { &mut *(env as *mut dyn ErasedEnvironment as *mut Environment<'a>) }
    }

    /// Lock these records until the transaction ends, and read them afresh from then on.
    ///
    /// What waits in the cache for them is written first. Another transaction locking them
    /// waits for this one to end; once it gets them, it reads what this one committed rather
    /// than what it had cached before. What a counter is for — the next number of a series —
    /// is read after locking, or two transactions take the same one.
    ///
    /// Whatever the caller's rights: locking reads and changes nothing.
    pub fn lock_records<Mode: IdMode>(&mut self, model_name: &str, ids: &Mode) -> Result<()> {
        if ids.is_empty() {
            return Ok(());
        }
        self.model_manager.try_get_model(model_name)?;
        self.save_model_to_db(model_name)?;
        self.database.lock(model_name, ids.get_ids_ref())?;
        self.cache.remove_records(model_name, ids);
        Ok(())
    }

    /// Create a new savepoint and commit if the given method doesn't return any error.
    /// If an error is returned, rollback the commit and put back the cache as it was
    pub fn savepoint<F, R>(&mut self, func: F) -> Result<R>
    where
        F: FnOnce(&mut Self) -> Result<R>,
    {
        let cache_copy = self.cache.export_cache();
        let tracked_copy = self.tracked.clone();
        let requested_installs = self.requested_installs.len();
        let uuid = "svp_".to_string() + &Uuid::new_v4().to_string()[..6];
        self.database.savepoint(uuid.as_str())?;

        let result = func(self);
        if result.is_ok() {
            // Commit
            self.database.savepoint_commit(uuid.as_str())?;
        } else {
            // Rollback
            self.database.savepoint_rollback(uuid.as_str())?;
            self.cache.import_cache(cache_copy);
            self.tracked = tracked_copy;
            self.requested_installs.truncate(requested_installs);
            // What was remembered may have been read from rows the rollback just undid.
            self.forget_access();
        }
        result
    }
}
