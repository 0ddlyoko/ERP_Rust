use crate::cache::CacheModels;
use crate::environment::Environment;
use std::collections::HashMap;

pub struct Savepoint {
    cache_copy: HashMap<String, CacheModels>,
}

impl Savepoint {
    pub fn new(environment: &Environment) -> Savepoint {
        let cache_copy = environment.cache.export_cache();
        // TODO Add savepoint
        Savepoint {
            cache_copy,
        }
    }

    pub fn commit(mut self, environment: &mut Environment) {
        // TODO Commit
        self.cache_copy = environment.cache.export_cache();
    }

    pub fn rollback(self, environment: &mut Environment) {
        // TODO Rollback
        environment.cache.import_cache(self.cache_copy.clone());
    }
}
