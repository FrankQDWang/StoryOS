//! Idle `storyos_runtime` connections reused inside one process (ADR 0031).

#[cfg(test)]
#[path = "connection_pool_tests.rs"]
mod tests;

use std::fmt;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tokio_postgres::NoTls;
use tokio_postgres::config::{Config, SslMode};

use crate::postgres_tls::VerifiedTls;
use crate::statement_cache::Client;

const IDLE_CAPACITY: usize = 8;

pub(crate) struct ConnectionPool {
    database_url: String,
    idle: Mutex<Vec<Client>>,
}

impl ConnectionPool {
    pub(crate) fn new(database_url: String) -> Self {
        Self {
            database_url,
            idle: Mutex::new(Vec::new()),
        }
    }

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

/// The one connect path of the production connections: verified TLS with `sslmode=require`, and
/// a plain connection as before with each other mode (ADR 0104 section 5).
pub(crate) async fn open(database_url: &str) -> Result<Client, tokio_postgres::Error> {
    let mut config = database_url.parse::<Config>()?;
    if config.get_ssl_mode() == SslMode::Require {
        // With only `hostaddr`, the driver gives the TLS connector no name; the address is the name.
        if config.get_hosts().is_empty() {
            for address in config.get_hostaddrs().to_vec() {
                config.host(address.to_string());
            }
        }
        // A boxed connect future keeps each future that opens a connection small on the stack.
        let (client, connection) = Box::pin(config.connect(VerifiedTls::shared())).await?;
        tokio::spawn(async move {
            let _connection_result = connection.await;
        });
        return Ok(Client::new(client));
    }
    let (client, connection) = Box::pin(config.connect(NoTls)).await?;
    tokio::spawn(async move {
        let _connection_result = connection.await;
    });
    Ok(Client::new(client))
}

/// One checked-out connection; drop returns it and its prepared statements to the pool only
/// outside a transaction.
pub(crate) struct PooledClient {
    client: Option<Client>,
    pool: Arc<ConnectionPool>,
    // Atomic only so a future that holds `&PooledClient` stays `Send`.
    in_transaction: AtomicBool,
}

impl PooledClient {
    /// Shadows `Client::batch_execute`; marks before `BEGIN` and clears only after a confirmed end.
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
    type Target = Client;

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
