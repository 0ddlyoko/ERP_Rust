use std::collections::HashMap;
use crate::cache::{CacheModel, CacheModels};
use crate::environment::Environment;

pub struct Savepoint<'a> {
    committed: bool,
    environment: &'a mut Environment<'a>,
    cache_copy: HashMap<String, CacheModels>,
}

impl<'a> Savepoint<'a> {
    fn new(environment: &'a mut Environment<'a>) -> Savepoint<'a> {
        let cache_copy = environment.cache.export_cache();
        Savepoint {
            committed: false,
            environment,
            cache_copy,
        }
    }

    pub fn commit(mut self) {
        self.committed = true;
        // TODO Save to env
        self.cache_copy = self.environment.cache.export_cache();
    }

    pub fn rollback(&mut self) {
        // TODO Rollback
        self.committed = true;
        self.environment.cache.import_cache(self.cache_copy.clone());
    }
}

impl<'a> Drop for Savepoint<'a> {
    fn drop(&mut self) {
        if !self.committed {
            self.rollback();
        }
    }
}
