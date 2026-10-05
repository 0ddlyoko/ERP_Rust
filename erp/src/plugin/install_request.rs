//! Plugins a unit of work asked to install, handed to whoever serves the application once that
//! work is committed.
//!
//! Installing changes the models every request reads, so it cannot happen inside a request: the
//! request only says what it wants, and the server installs it on a new application once the
//! request is over, then serves that one.

use std::cell::RefCell;

thread_local! {
    static REQUESTED: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

/// Note plugins to install, from a unit of work just committed on this thread.
pub(crate) fn request(names: Vec<String>) {
    REQUESTED.with(|requested| {
        let mut requested = requested.borrow_mut();
        for name in names {
            if !requested.contains(&name) {
                requested.push(name);
            }
        }
    });
}

/// The plugins the work committed on this thread asked to install since the last call, in the
/// order asked, each once.
pub fn take_requested_installs() -> Vec<String> {
    REQUESTED.with(|requested| std::mem::take(&mut *requested.borrow_mut()))
}
