//! Connections shared by every environment.
//!
//! Written here rather than taken from a crate: it is a vector, a mutex and a condition
//! variable, and a dependency runs with the same rights as the rest of the server.
//!
//! What it does beyond handing connections out: it never opens more than it was allowed, it
//! gives up rather than waiting forever, and it throws away a connection the server has closed
//! instead of handing it to the next caller.

use crate::database::{DatabaseConfig, ErrorType};
use postgres::{Client, NoTls};
use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

type Result<T> = std::result::Result<T, ErrorType>;

/// Connections to one database.
///
/// A handle rather than the pool itself: cloning it costs a reference count, and a borrowed
/// connection keeps the pool alive on its own, which is what lets one outlive the call that
/// asked for it.
#[derive(Clone)]
pub struct ConnectionPool {
    inner: Arc<Shared>,
}

struct Shared {
    settings: postgres::Config,
    schema: String,
    max_size: usize,
    timeout: Duration,
    state: Mutex<State>,
    returned: Condvar,
}

#[derive(Default)]
struct State {
    /// Connections nobody is using.
    idle: Vec<Client>,
    /// Connections that exist at all, idle or lent out. Never above `max_size`.
    open: usize,
}

impl ConnectionPool {
    /// Prepare a pool. No connection is opened until one is asked for.
    pub fn new(config: &DatabaseConfig) -> Self {
        // Built field by field rather than as a URL: a password containing '@', '/', '?' or
        // '#' would otherwise corrupt the connection string.
        let mut settings = postgres::Config::new();
        settings.user(&config.user).dbname(&config.name);
        if config.url.starts_with('/') {
            // A path means a unix socket, which is how most local installs authenticate.
            settings.host_path(&config.url);
        } else {
            settings.host(&config.url).port(config.port);
        }
        if !config.password.is_empty() {
            settings.password(&config.password);
        }

        Self {
            inner: Arc::new(Shared {
                settings,
                schema: config.schema.clone(),
                max_size: (config.pool_size as usize).max(1),
                timeout: Duration::from_secs(config.connection_timeout),
                state: Mutex::new(State::default()),
                returned: Condvar::new(),
            }),
        }
    }

    /// How many connections exist right now, lent out or not.
    pub fn open(&self) -> usize {
        self.inner
            .state
            .lock()
            .expect("the pool is never poisoned")
            .open
    }

    /// The most this pool will ever open.
    pub fn max_size(&self) -> usize {
        self.inner.max_size
    }

    /// Borrow a connection, waiting for one if they are all out.
    ///
    /// Gives up after the configured timeout rather than blocking for good: a caller told that
    /// the database is saturated can say so, one that never returns cannot.
    pub fn get(&self) -> Result<PooledConnection> {
        let client = self.inner.check_out()?;
        Ok(PooledConnection {
            pool: Arc::clone(&self.inner),
            client: Some(client),
        })
    }
}

impl Shared {
    fn check_out(&self) -> Result<Client> {
        let deadline = std::time::Instant::now() + self.timeout;
        let mut state = self.state.lock().expect("not poisoned");

        loop {
            // A connection the server hung up on is worse than no connection: discard them
            // rather than hand one over.
            while let Some(client) = state.idle.pop() {
                if !client.is_closed() {
                    return Ok(client);
                }
                state.open -= 1;
            }

            if state.open < self.max_size {
                // Counted before connecting, so two callers racing here cannot both decide
                // there is room for the last one.
                state.open += 1;
                drop(state);

                // Opening is slow and talks to the network, so it happens without the lock.
                let client = self.open_one().inspect_err(|_| {
                    self.state.lock().expect("not poisoned").open -= 1;
                    self.returned.notify_one();
                })?;
                return Ok(client);
            }

            let Some(remaining) = deadline.checked_duration_since(std::time::Instant::now()) else {
                return Err(ErrorType::Pool(format!(
                    "all {} connections are in use, and none came free within {}s",
                    self.max_size,
                    self.timeout.as_secs(),
                )));
            };
            let (guard, timed_out) = self
                .returned
                .wait_timeout(state, remaining)
                .expect("not poisoned");
            state = guard;
            if timed_out.timed_out() && state.idle.is_empty() && state.open >= self.max_size {
                return Err(ErrorType::Pool(format!(
                    "all {} connections are in use, and none came free within {}s",
                    self.max_size,
                    self.timeout.as_secs(),
                )));
            }
        }
    }

    /// Open one connection and point it at the configured schema.
    ///
    /// The `search_path` is a session setting, so it is set once here rather than on every
    /// borrow: without it every query would look for tables somewhere else.
    fn open_one(&self) -> Result<Client> {
        let mut client = self.settings.connect(NoTls)?;
        client.batch_execute(&format!(
            "SET search_path TO {}",
            super::quote_ident(&self.schema)
        ))?;
        Ok(client)
    }

    /// Take a connection back, unless it is no longer usable.
    fn put_back(&self, client: Client) {
        let mut state = self.state.lock().expect("not poisoned");
        if client.is_closed() {
            state.open -= 1;
        } else {
            state.idle.push(client);
        }
        // One waiter, because one connection came free.
        self.returned.notify_one();
    }
}

/// A connection borrowed from a [`ConnectionPool`], returned when dropped.
pub struct PooledConnection {
    pool: Arc<Shared>,
    /// Always `Some` until dropped; an `Option` only so the client can be moved out there.
    client: Option<Client>,
}

impl Deref for PooledConnection {
    type Target = Client;

    fn deref(&self) -> &Client {
        self.client.as_ref().expect("held until dropped")
    }
}

impl DerefMut for PooledConnection {
    fn deref_mut(&mut self) -> &mut Client {
        self.client.as_mut().expect("held until dropped")
    }
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        if let Some(client) = self.client.take() {
            self.pool.put_back(client);
        }
    }
}
