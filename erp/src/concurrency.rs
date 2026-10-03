//! How much work runs at once.
//!
//! Distinct from the size of the connection pool, and deliberately so: the pool bounds how many
//! connections exist, this bounds how many requests are being served. A request waiting here has
//! not taken a connection yet, which is what keeps a burst from holding the pool open while it
//! queues.
//!
//! Work that never reaches the database — moving a file, answering from memory — has no reason to
//! queue behind work that does, and simply does not come through here.

use std::sync::{Condvar, Mutex};

/// A bound on how many things happen at the same time.
///
/// Built on the standard library rather than on an async runtime so that any caller can use it,
/// whatever carries its requests.
#[derive(Debug)]
pub struct Gate {
    /// `None` when nothing is bounded, which costs a branch rather than a lock.
    limit: Option<usize>,
    running: Mutex<usize>,
    freed: Condvar,
}

impl Gate {
    /// A gate letting `limit` things through at once.
    ///
    /// A limit of zero would let nothing through at all, so it is read as no limit: a
    /// configuration file saying `0` means "unbounded" everywhere else it appears.
    pub fn new(limit: usize) -> Self {
        Self {
            limit: (limit > 0).then_some(limit),
            running: Mutex::new(0),
            freed: Condvar::new(),
        }
    }

    /// A gate that never holds anything back.
    pub fn unlimited() -> Self {
        Self::new(0)
    }

    /// How many may run at once, or `None` when there is no bound.
    pub fn limit(&self) -> Option<usize> {
        self.limit
    }

    /// How many are running right now.
    pub fn running(&self) -> usize {
        *self
            .running
            .lock()
            .expect("the gate's counter is never poisoned")
    }

    /// Wait until there is room, then take it.
    ///
    /// The room is given back when the returned pass is dropped, including when the work it
    /// guards panics or returns early.
    pub fn enter(&self) -> Pass<'_> {
        if let Some(limit) = self.limit {
            let mut running = self
                .running
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            while *running >= limit {
                running = self.freed.wait(running).expect("not poisoned");
            }
            *running += 1;
        }
        Pass { gate: self }
    }
}

impl Default for Gate {
    fn default() -> Self {
        Self::unlimited()
    }
}

/// Room taken in a [`Gate`], given back when dropped.
#[derive(Debug)]
pub struct Pass<'a> {
    gate: &'a Gate,
}

impl Drop for Pass<'_> {
    fn drop(&mut self) {
        if self.gate.limit.is_none() {
            return;
        }
        let mut running = self
            .gate
            .running
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *running = running.saturating_sub(1);
        // One waiter, because one place was freed.
        self.gate.freed.notify_one();
    }
}
