pub mod access;
pub mod app;
pub mod assets;
pub mod concurrency;
pub mod config;
pub mod data;
pub mod database;
pub mod environment;
pub mod errors;
pub mod http;
pub mod identity;
pub mod inheritance;
pub mod jsonrpc;
pub mod model;
pub mod plugin;
pub mod request_log;
pub mod server_config;
pub mod shared_cache;
pub mod util;
pub mod xml;

/// What a method of a model, a compute or a hook answers: a value, or why it failed — sorted as
/// a business refusal, an input not holding, or an internal failure.
pub type Result<T, E = errors::Error> = std::result::Result<T, E>;
pub use errors::Error;

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
