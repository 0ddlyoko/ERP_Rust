//! What a request did — who asked, and how much SQL it took — gathered on the thread answering
//! it, for the line the server logs once it is answered.

use std::cell::Cell;
use std::time::{Duration, Instant};

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct RequestLog {
    pub uid: Option<u32>,
    pub queries: u32,
    pub sql_time: Duration,
}

thread_local! {
    static CURRENT: Cell<RequestLog> = Cell::new(RequestLog::default());
}

/// Start gathering for a request about to be answered on this thread, forgetting the last one.
///
/// A thread answers one request at a time, so what it gathers until the next start is that
/// request's alone.
pub fn start() {
    CURRENT.set(RequestLog::default());
}

/// What the request answered on this thread did so far.
pub fn current() -> RequestLog {
    CURRENT.get()
}

/// Say who the request is answered for, once their credentials are resolved.
pub fn identified(uid: Option<u32>) {
    CURRENT.set(RequestLog {
        uid,
        ..CURRENT.get()
    });
}

/// Run one SQL statement, counted along with the time it took.
///
/// Each statement is also logged under the `erp::sql` target at debug level, to find out what a
/// request spends its queries on (`RUST_LOG=erp::sql=debug`).
pub fn sql<T>(query: &str, run: impl FnOnce() -> T) -> T {
    let started = Instant::now();
    let result = run();
    let elapsed = started.elapsed();
    tracing::debug!(target: "erp::sql", "{:.2}ms {query}", elapsed.as_secs_f64() * 1000.0);
    let log = CURRENT.get();
    CURRENT.set(RequestLog {
        queries: log.queries + 1,
        sql_time: log.sql_time + elapsed,
        ..log
    });
    result
}
