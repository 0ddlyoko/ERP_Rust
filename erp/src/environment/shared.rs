//! Reading and emptying the caches kept across requests.
use super::*;
use std::sync::Arc;

impl<'mm> Environment<'mm> {
    /// A value of a shared cache, built by `build` when the cache does not hold it.
    ///
    /// Built again without being kept once this transaction changed a model the cache depends on:
    /// the value would hold what no other request can see yet.
    pub fn cached<T, F>(&mut self, cache: &'static str, key: &str, build: F) -> Result<Arc<T>>
    where
        T: Send + Sync + 'static,
        F: FnOnce(&mut Self) -> Result<T>,
    {
        let caches = &self.model_manager.shared_caches;
        if !caches.is_registered(cache) {
            return Err(format!("No plugin declared the shared cache {cache}").into());
        }
        let changed = self.changed_cached_models.iter().map(String::as_str);
        if caches.depends_on(cache, changed) {
            return Ok(Arc::new(build(self)?));
        }
        let (held, generation) = caches.get(cache, key);
        if let Some(value) = held.and_then(|held| held.downcast::<T>().ok()) {
            return Ok(value);
        }
        let value = Arc::new(build(self)?);
        self.model_manager
            .shared_caches
            .store(cache, key, value.clone(), generation);
        Ok(value)
    }

    /// Empty the caches built from a model whose records this transaction changes.
    pub(super) fn forget_shared_of(&mut self, model_name: &str) {
        let caches = &self.model_manager.shared_caches;
        if caches.watches(model_name) {
            caches.forget_model(model_name);
            self.changed_cached_models.insert(model_name.to_string());
        }
    }

    /// Empty them again once the change is committed: a request that read before the commit may
    /// have filled them with what it replaced.
    pub(super) fn forget_shared_after_commit(&mut self) {
        for model_name in std::mem::take(&mut self.changed_cached_models) {
            self.model_manager.shared_caches.forget_model(&model_name);
        }
    }
}
