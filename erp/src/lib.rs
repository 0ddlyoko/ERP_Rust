pub mod app;
pub mod concurrency;
pub mod config;
pub mod data;
pub mod database;
pub mod environment;
pub mod errors;
pub mod model;
pub mod plugin;
pub mod util;

pub use erp_types as types;
// Re-exported so plugins can name the types that `ModelManager` and `Environment` hand back
// without declaring a direct dependency on each internal crate.
pub use erp_cache as cache;
pub use erp_internal_types as internal_types;
pub use erp_search as search;
// Named by generated RPC wrappers, which live in plugin crates and would otherwise each have to
// declare the dependency and agree on its version.
pub use serde;
pub use serde_json;
