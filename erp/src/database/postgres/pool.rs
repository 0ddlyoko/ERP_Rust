//! Connections shared by every environment.
//!
//! Written here rather than taken from a crate: it is a vector, a mutex and a condition
//! variable, and a dependency runs with the same rights as the rest of the server.
//!
//! What it does beyond handing connections out: it never opens more than it was allowed, it
//! gives up rather than waiting forever, and it throws away a connection the server has closed
//! instead of handing it to the next caller.

use crate::database::{DatabaseConfig, ErrorType};
use crate::request_log;
use postgres::types::ToSql;
use postgres::{Client, NoTls, Row};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

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
    /// How long a connection may sit idle before it is asked whether it is still there.
    ///
    /// Zero by default, which asks every time. `is_closed` is not a substitute: it reports only
    /// what the client already noticed, and a socket the server closed a moment ago has not been
    /// noticed yet — a connection killed just before it is lent out passes that check and then
    /// fails the request. Raising this trades that window for a saved round trip.
    revalidate_after: Duration,
    state: Mutex<State>,
    returned: Condvar,
    /// Connections asked whether they were still alive, for whoever is watching the cost.
    revalidations: AtomicUsize,
    /// Connections found dead and thrown away.
    discarded: AtomicUsize,
    /// The one connection lent to every caller while a test runs, inside its transaction.
    pinned: Mutex<Option<Arc<Mutex<Client>>>>,
    /// Savepoints taken on the pinned connection, each borrower's named after its number.
    savepoints: AtomicUsize,
}

#[derive(Default)]
struct State {
    /// Connections nobody is using, with the moment each came back.
    idle: Vec<Idle>,
    /// Connections that exist at all, idle or lent out. Never above `max_size`.
    open: usize,
}

struct Idle {
    client: Client,
    returned_at: Instant,
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
            #[cfg(unix)]
            settings.host_path(&config.url);
            // Elsewhere there are no unix sockets and `host_path` does not exist. The path is
            // handed over as a host name so the complaint comes from the connection attempt,
            // with the address in it, rather than from a pool that silently talks to localhost.
            #[cfg(not(unix))]
            settings.host(&config.url);
        } else {
            settings.host(&config.url).port(config.port);
        }
        if !config.password.is_empty() {
            settings.password(&config.password);
        }
        // Named so `pg_stat_activity` says which application a backend belongs to, and so a
        // tool — or a test — can act on this pool's connections without touching anyone else's.
        settings.application_name(&format!("erp:{}", config.schema));
        // What the server only notes in passing — a table that already exists — is for debugging.
        settings.notice_callback(|notice| {
            tracing::debug!(target: "postgres", severity = notice.severity(), "{}", notice.message());
        });

        Self {
            inner: Arc::new(Shared {
                settings,
                schema: config.schema.clone(),
                max_size: (config.pool_size as usize).max(1),
                timeout: Duration::from_secs(config.connection_timeout),
                revalidate_after: Duration::from_secs(config.revalidate_after),
                state: Mutex::new(State::default()),
                returned: Condvar::new(),
                revalidations: AtomicUsize::new(0),
                discarded: AtomicUsize::new(0),
                pinned: Mutex::new(None),
                savepoints: AtomicUsize::new(0),
            }),
        }
    }

    /// How many connections exist right now, lent out or not.
    pub fn open(&self) -> usize {
        self.inner.state().open
    }

    /// The most this pool will ever open.
    pub fn max_size(&self) -> usize {
        self.inner.max_size
    }

    /// How many connections were asked whether they were still alive.
    ///
    /// A connection handed straight back out is not asked, so under load this stays near zero.
    pub fn revalidations(&self) -> usize {
        self.inner.revalidations.load(Ordering::Relaxed)
    }

    /// How many connections were found dead and thrown away.
    pub fn discarded(&self) -> usize {
        self.inner.discarded.load(Ordering::Relaxed)
    }

    /// Borrow a connection, waiting for one if they are all out.
    ///
    /// Gives up after the configured timeout rather than blocking for good: a caller told that
    /// the database is saturated can say so, one that never returns cannot.
    pub fn get(&self) -> Result<PooledConnection> {
        if let Some(pinned) = self.inner.pinned().as_ref() {
            let number = self.inner.savepoints.fetch_add(1, Ordering::Relaxed);
            return Ok(PooledConnection {
                pool: Arc::clone(&self.inner),
                client: Some(Lent::Pinned(Arc::clone(pinned), format!("lent_{number}"))),
                broken: false,
                transaction: Transaction::None,
            });
        }
        let client = self.inner.check_out()?;
        Ok(PooledConnection {
            pool: Arc::clone(&self.inner),
            client: Some(Lent::Own(Box::new(client))),
            broken: false,
            transaction: Transaction::None,
        })
    }

    /// Lend one connection to every caller from now on, inside a transaction undone once the
    /// returned guard is let go: a test's work, all of it taken back at its end.
    ///
    /// Each borrower's transaction is a savepoint in it — committing releases the savepoint,
    /// rolling back returns to it — so what one commits, the next sees, as it would.
    pub fn pin(&self) -> Result<PinnedTransaction> {
        let mut client = self.inner.check_out()?;
        client.batch_execute("START TRANSACTION")?;
        *self.inner.pinned() = Some(Arc::new(Mutex::new(client)));
        Ok(PinnedTransaction { pool: self.clone() })
    }
}

/// The transaction a pinned connection holds, rolled back once let go.
pub struct PinnedTransaction {
    pool: ConnectionPool,
}

impl Drop for PinnedTransaction {
    fn drop(&mut self) {
        let Some(pinned) = self.pool.inner.pinned().take() else {
            return;
        };
        let Ok(client) = Arc::try_unwrap(pinned) else {
            return;
        };
        let mut client = client.into_inner().unwrap_or_else(PoisonError::into_inner);
        if client.batch_execute("ROLLBACK").is_ok() {
            self.pool.inner.put_back(client);
        } else {
            self.pool.inner.discard(client);
        }
    }
}

impl Shared {
    /// The pool's state, even after a panic while it was held: what it counts stays right, as
    /// every change to it is made in one step.
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn pinned(&self) -> MutexGuard<'_, Option<Arc<Mutex<Client>>>> {
        self.pinned.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn check_out(&self) -> Result<Client> {
        let deadline = std::time::Instant::now() + self.timeout;
        let mut state = self.state();

        loop {
            // A connection the server hung up on is worse than no connection: discard them
            // rather than hand one over.
            while let Some(idle) = state.idle.pop() {
                if idle.client.is_closed() {
                    state.open = state.open.saturating_sub(1);
                    self.discarded.fetch_add(1, Ordering::Relaxed);
                    continue;
                }
                if idle.returned_at.elapsed() < self.revalidate_after {
                    // Back too recently for anything to have changed, so no round trip.
                    return Ok(idle.client);
                }
                // Asking talks to the server, so it happens without the lock.
                drop(state);
                self.revalidations.fetch_add(1, Ordering::Relaxed);
                let mut client = idle.client;
                let alive = client.is_valid(self.timeout).is_ok();
                state = self.state();
                if alive {
                    return Ok(client);
                }
                state.open = state.open.saturating_sub(1);
                self.discarded.fetch_add(1, Ordering::Relaxed);
                // Room came free, so whoever was waiting for one may now open it.
                self.returned.notify_one();
            }

            if state.open < self.max_size {
                // Counted before connecting, so two callers racing here cannot both decide
                // there is room for the last one.
                state.open += 1;
                drop(state);

                // Opening is slow and talks to the network, so it happens without the lock.
                let client = self.open_one().inspect_err(|_| {
                    {
                        let mut state = self.state();
                        state.open = state.open.saturating_sub(1);
                    }
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
                .unwrap_or_else(PoisonError::into_inner);
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

    /// Close a connection rather than take it back, making room for a new one.
    fn discard(&self, client: Client) {
        {
            let mut state = self.state();
            state.open = state.open.saturating_sub(1);
        }
        self.discarded.fetch_add(1, Ordering::Relaxed);
        self.returned.notify_one();
        // Closing talks to the network, so it happens without the lock, and off any thread
        // that drives the async runtime.
        std::thread::spawn(move || drop(client));
    }

    /// Take a connection back, unless it is no longer usable.
    fn put_back(&self, client: Client) {
        let mut state = self.state();
        if client.is_closed() {
            state.open = state.open.saturating_sub(1);
            self.discarded.fetch_add(1, Ordering::Relaxed);
        } else {
            state.idle.push(Idle {
                client,
                returned_at: Instant::now(),
            });
        }
        // One waiter, because one connection came free.
        self.returned.notify_one();
    }
}

/// A connection borrowed from a [`ConnectionPool`], returned when dropped.
pub struct PooledConnection {
    pool: Arc<Shared>,
    /// Always `Some` until dropped; an `Option` only so the client can be moved out there.
    client: Option<Lent>,
    broken: bool,
    transaction: Transaction,
}

/// The connection a borrower works on: one of its own, or the pinned one, with the name of the
/// savepoint standing for its transaction there.
enum Lent {
    Own(Box<Client>),
    Pinned(Arc<Mutex<Client>>, String),
}

/// Where a connection is in its transaction.
#[derive(Clone, Copy, PartialEq)]
enum Transaction {
    None,
    /// Asked for, and started by the first statement: a unit of work that never reaches the
    /// database costs it nothing.
    Pending,
    Started,
}

impl PooledConnection {
    /// Close the connection once it is let go, rather than lend it again: something failed
    /// that leaves its state uncertain, such as a rollback.
    pub fn mark_broken(&mut self) {
        self.broken = true;
    }

    /// Start a transaction, sent along with the first statement that needs it.
    pub fn begin(&mut self) {
        self.transaction = Transaction::Pending;
    }

    /// Commit the transaction; nothing to send when no statement started it.
    pub fn commit(&mut self) -> std::result::Result<(), postgres::Error> {
        self.end("COMMIT")
    }

    /// Roll the transaction back; nothing to send when no statement started it.
    pub fn rollback(&mut self) -> std::result::Result<(), postgres::Error> {
        self.end("ROLLBACK")
    }

    fn end(&mut self, statement: &str) -> std::result::Result<(), postgres::Error> {
        let started = self.transaction == Transaction::Started;
        self.transaction = Transaction::None;
        if started {
            let statement = match self.savepoint() {
                Some(name) if statement == "COMMIT" => format!("RELEASE SAVEPOINT \"{name}\""),
                Some(name) => {
                    format!("ROLLBACK TO SAVEPOINT \"{name}\"; RELEASE SAVEPOINT \"{name}\"")
                }
                None => statement.to_string(),
            };
            request_log::sql(&statement, || {
                self.with_client(|client| client.batch_execute(&statement))
            })?;
        }
        Ok(())
    }

    fn start_if_pending(&mut self) -> std::result::Result<(), postgres::Error> {
        if self.transaction == Transaction::Pending {
            self.transaction = Transaction::Started;
            let statement = match self.savepoint() {
                Some(name) => format!("SAVEPOINT \"{name}\""),
                None => "START TRANSACTION".to_string(),
            };
            request_log::sql(&statement, || {
                self.with_client(|client| client.batch_execute(&statement))
            })?;
        }
        Ok(())
    }

    /// The savepoint standing for this borrower's transaction on a pinned connection.
    fn savepoint(&self) -> Option<String> {
        match self.client.as_ref() {
            Some(Lent::Pinned(_, name)) => Some(name.clone()),
            _ => None,
        }
    }

    /// Work on the client, the pinned one locked for as long as the work takes.
    fn with_client<T>(&mut self, work: impl FnOnce(&mut Client) -> T) -> T {
        match self.client.as_mut().expect("held until dropped") {
            Lent::Own(client) => work(client),
            Lent::Pinned(client, _) => {
                work(&mut client.lock().unwrap_or_else(PoisonError::into_inner))
            }
        }
    }

    /// [`Client::query`], counted in the request's SQL.
    pub fn query(
        &mut self,
        query: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> std::result::Result<Vec<Row>, postgres::Error> {
        self.start_if_pending()?;
        request_log::sql(query, || {
            self.with_client(|client| client.query(query, params))
        })
    }

    /// [`Client::query_one`], counted in the request's SQL.
    pub fn query_one(
        &mut self,
        query: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> std::result::Result<Row, postgres::Error> {
        self.start_if_pending()?;
        request_log::sql(query, || {
            self.with_client(|client| client.query_one(query, params))
        })
    }

    /// [`Client::execute`], counted in the request's SQL.
    pub fn execute(
        &mut self,
        query: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> std::result::Result<u64, postgres::Error> {
        self.start_if_pending()?;
        request_log::sql(query, || {
            self.with_client(|client| client.execute(query, params))
        })
    }

    /// [`Client::batch_execute`], counted in the request's SQL.
    pub fn batch_execute(&mut self, query: &str) -> std::result::Result<(), postgres::Error> {
        self.start_if_pending()?;
        request_log::sql(query, || {
            self.with_client(|client| client.batch_execute(query))
        })
    }
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        if self.savepoint().is_some() {
            let _ = self.rollback();
            return;
        }
        let Some(Lent::Own(client)) = self.client.take() else {
            return;
        };
        let client = *client;
        if self.broken {
            self.pool.discard(client);
        } else {
            self.pool.put_back(client);
        }
    }
}

impl Drop for Shared {
    /// Close the idle connections somewhere blocking is allowed.
    ///
    /// Closing one blocks, because the driver is synchronous, and blocking on a thread that is
    /// driving an async runtime aborts the process — a panic while unwinding cannot itself
    /// unwind. A pool can be let go from anywhere, including a request handler or a test, so the
    /// rule is kept here rather than asked of every caller.
    fn drop(&mut self) {
        let idle = std::mem::take(
            &mut self
                .state
                .get_mut()
                .unwrap_or_else(PoisonError::into_inner)
                .idle,
        );
        if idle.is_empty() {
            return;
        }
        // Detached on purpose: nothing needs to wait for a socket to close, and at process exit
        // the operating system closes them anyway.
        std::thread::spawn(move || drop(idle));
    }
}
