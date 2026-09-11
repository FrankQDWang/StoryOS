//! Reuse of idle `storyos_runtime` connections inside one process.
//!
//! One protected request reads the Storage Activation proof and then runs one
//! Application operation. Without reuse each step opens a new PostgreSQL
//! connection, and the idle Worker opens two more every 50 ms. One refused
//! connect then becomes one `project_store_unavailable` refusal. The pool keeps
//! a few idle connections so steady-state traffic opens none.
//!
//! Every runtime statement sets its scope with transaction-local `set_config`, so a
//! connection outside a transaction carries no state into the next checkout.

#[cfg(test)]
#[path = "connection_pool_tests.rs"]
mod tests;

use std::fmt;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tokio_postgres::NoTls;

/// Idle connections kept per pool. A connection returned above this count closes.
const IDLE_CAPACITY: usize = 8;

pub(crate) struct ConnectionPool {
    database_url: String,
    idle: Mutex<Vec<tokio_postgres::Client>>,
}

impl ConnectionPool {
    pub(crate) fn new(database_url: String) -> Self {
        Self {
            database_url,
            idle: Mutex::new(Vec::new()),
        }
    }

    /// Take an open idle connection, or open a new one.
    pub(crate) async fn checkout(self: &Arc<Self>) -> Result<PooledClient, tokio_postgres::Error> {
        let client = loop {
            let idle = self.idle.lock().ok().and_then(|mut idle| idle.pop());
            match idle {
                Some(client) if client.is_closed() => continue,
                Some(client) => break client,
                None => break open(&self.database_url).await?,
            }
        };
        Ok(PooledClient {
            client: Some(client),
            pool: Arc::clone(self),
            in_transaction: AtomicBool::new(false),
        })
    }
}

impl fmt::Debug for ConnectionPool {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let idle = self.idle.lock().map(|idle| idle.len()).unwrap_or_default();
        formatter
            .debug_struct("ConnectionPool")
            .field("idle", &idle)
            .finish_non_exhaustive()
    }
}

/// Open one connection and drive it on its own task.
pub(crate) async fn open(
    database_url: &str,
) -> Result<tokio_postgres::Client, tokio_postgres::Error> {
    let (client, connection) = tokio_postgres::connect(database_url, NoTls).await?;
    tokio::spawn(async move {
        let _connection_result = connection.await;
    });
    Ok(client)
}

/// One checked-out connection.
///
/// Drop returns the connection to the pool only when it is outside a transaction.
/// A connection dropped inside a transaction closes, and PostgreSQL rolls back.
/// PostgreSQL reports the transaction status in each `ReadyForQuery` message, but
/// `tokio_postgres::Client` does not expose it. So `batch_execute` records the
/// transaction-control statements that the adapter runs on the client.
/// `Client::transaction()` rolls back on its own drop and needs no record.
pub(crate) struct PooledClient {
    client: Option<tokio_postgres::Client>,
    pool: Arc<ConnectionPool>,
    // Atomic only so a future that holds `&PooledClient` stays `Send`; one task uses it.
    in_transaction: AtomicBool,
}

impl PooledClient {
    /// Run one simple statement string and track `BEGIN`, `COMMIT`, and `ROLLBACK`.
    ///
    /// This inherent method shadows `Client::batch_execute`. A `BEGIN` marks the
    /// connection before the statement runs. A `COMMIT` or `ROLLBACK` clears the
    /// mark only after PostgreSQL confirms it. A cancelled or failed statement
    /// therefore leaves the mark in place, and the connection closes on drop.
    pub(crate) async fn batch_execute(&self, statement: &str) -> Result<(), tokio_postgres::Error> {
        let control = transaction_control(statement);
        if control == Some(TransactionControl::Begin) {
            self.in_transaction.store(true, Ordering::Relaxed);
        }
        let result = self.deref().batch_execute(statement).await;
        if control == Some(TransactionControl::End) && result.is_ok() {
            self.in_transaction.store(false, Ordering::Relaxed);
        }
        result
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TransactionControl {
    Begin,
    End,
}

fn transaction_control(statement: &str) -> Option<TransactionControl> {
    let mut words = statement
        .split_whitespace()
        .map(|word| word.trim_end_matches(';').to_ascii_uppercase());
    match (words.next().as_deref(), words.next().as_deref()) {
        // `COMMIT AND CHAIN` and `ROLLBACK AND CHAIN` end one transaction and start the next.
        (Some("BEGIN"), _)
        | (Some("START"), Some("TRANSACTION"))
        | (Some("COMMIT" | "END" | "ROLLBACK" | "ABORT"), Some("AND")) => {
            Some(TransactionControl::Begin)
        }
        (Some("ROLLBACK"), Some("TO")) => None,
        (Some("COMMIT" | "END" | "ROLLBACK" | "ABORT"), _) => Some(TransactionControl::End),
        _ => None,
    }
}

impl Deref for PooledClient {
    type Target = tokio_postgres::Client;

    fn deref(&self) -> &Self::Target {
        self.client
            .as_ref()
            .expect("a pooled client is present until drop")
    }
}

impl DerefMut for PooledClient {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.client
            .as_mut()
            .expect("a pooled client is present until drop")
    }
}

impl Drop for PooledClient {
    fn drop(&mut self) {
        let Some(client) = self.client.take() else {
            return;
        };
        if self.in_transaction.load(Ordering::Relaxed) || client.is_closed() {
            return;
        }
        if let Ok(mut idle) = self.pool.idle.lock()
            && idle.len() < IDLE_CAPACITY
        {
            idle.push(client);
        }
    }
}
