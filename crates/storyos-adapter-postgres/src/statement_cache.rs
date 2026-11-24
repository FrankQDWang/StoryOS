//! The runtime connection with a cache of its prepared statements (ADR 0031).
//!
//! The types keep the names of the driver types that they replace, so that each call site stays as
//! it is. A second run of one statement text on one connection sends no prepare step: one round
//! trip in place of two. The cache ends with its connection.

#[cfg(test)]
#[path = "statement_cache_tests.rs"]
mod tests;

use std::collections::HashMap;
use std::future::Future;
use std::sync::Mutex;

use tokio_postgres::types::ToSql;
use tokio_postgres::{Error, IsolationLevel, Row, Statement};

/// The limit of the kept statements of one connection. A text that the code builds from data, for
/// example a list of a changing length, can only push the statement of the oldest use out.
pub(crate) const STATEMENT_CAPACITY: usize = 256;

/// The prepared statements of one connection, by text. The code gives no parameter types, so the
/// database infers them from the text: one text has one set of parameter types.
#[derive(Default)]
struct StatementCache {
    kept: Mutex<KeptStatements>,
}

#[derive(Default)]
struct KeptStatements {
    by_text: HashMap<String, KeptStatement>,
    clock: u64,
}

struct KeptStatement {
    statement: Statement,
    last_use: u64,
}

impl StatementCache {
    fn get(&self, text: &str) -> Option<Statement> {
        let mut kept = self.kept.lock().ok()?;
        kept.clock += 1;
        let clock = kept.clock;
        let entry = kept.by_text.get_mut(text)?;
        entry.last_use = clock;
        Some(entry.statement.clone())
    }

    // A dropped `Statement` tells the database to close it, so an evicted statement also leaves
    // the session.
    fn keep(&self, text: &str, statement: &Statement) {
        let Ok(mut kept) = self.kept.lock() else {
            return;
        };
        kept.clock += 1;
        let clock = kept.clock;
        if !kept.by_text.contains_key(text) && kept.by_text.len() >= STATEMENT_CAPACITY {
            let oldest = kept
                .by_text
                .iter()
                .min_by_key(|(_, entry)| entry.last_use)
                .map(|(oldest, _)| oldest.clone());
            if let Some(oldest) = oldest {
                kept.by_text.remove(&oldest);
            }
        }
        kept.by_text.insert(
            text.to_owned(),
            KeptStatement {
                statement: statement.clone(),
                last_use: clock,
            },
        );
    }

    async fn prepared(
        &self,
        connection: &impl tokio_postgres::GenericClient,
        text: &str,
    ) -> Result<Statement, Error> {
        if let Some(statement) = self.get(text) {
            return Ok(statement);
        }
        let statement = connection.prepare(text).await?;
        self.keep(text, &statement);
        Ok(statement)
    }
}

/// One runtime connection and its prepared statements.
pub(crate) struct Client {
    connection: tokio_postgres::Client,
    statements: StatementCache,
}

impl Client {
    pub(crate) fn new(connection: tokio_postgres::Client) -> Self {
        Self {
            connection,
            statements: StatementCache::default(),
        }
    }

    pub(crate) async fn query(
        &self,
        text: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Vec<Row>, Error> {
        let statement = self.statements.prepared(&self.connection, text).await?;
        self.connection.query(&statement, params).await
    }

    pub(crate) async fn query_one(
        &self,
        text: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Row, Error> {
        let statement = self.statements.prepared(&self.connection, text).await?;
        self.connection.query_one(&statement, params).await
    }

    pub(crate) async fn query_opt(
        &self,
        text: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Option<Row>, Error> {
        let statement = self.statements.prepared(&self.connection, text).await?;
        self.connection.query_opt(&statement, params).await
    }

    pub(crate) async fn execute(
        &self,
        text: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<u64, Error> {
        let statement = self.statements.prepared(&self.connection, text).await?;
        self.connection.execute(&statement, params).await
    }

    /// Sends the text with the simple protocol, so it keeps no statement.
    pub(crate) async fn batch_execute(&self, text: &str) -> Result<(), Error> {
        self.connection.batch_execute(text).await
    }

    pub(crate) async fn transaction(&mut self) -> Result<Transaction<'_>, Error> {
        Ok(Transaction {
            transaction: self.connection.transaction().await?,
            statements: &self.statements,
        })
    }

    pub(crate) fn build_transaction(&mut self) -> TransactionBuilder<'_> {
        TransactionBuilder {
            builder: self.connection.build_transaction(),
            statements: &self.statements,
        }
    }

    pub(crate) fn is_closed(&self) -> bool {
        self.connection.is_closed()
    }
}

/// A transaction on a [`Client`]; it rolls back on drop, as the driver transaction does.
pub(crate) struct Transaction<'a> {
    transaction: tokio_postgres::Transaction<'a>,
    statements: &'a StatementCache,
}

impl Transaction<'_> {
    pub(crate) async fn query(
        &self,
        text: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Vec<Row>, Error> {
        let statement = self.statements.prepared(&self.transaction, text).await?;
        self.transaction.query(&statement, params).await
    }

    pub(crate) async fn query_one(
        &self,
        text: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Row, Error> {
        let statement = self.statements.prepared(&self.transaction, text).await?;
        self.transaction.query_one(&statement, params).await
    }

    pub(crate) async fn query_opt(
        &self,
        text: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Option<Row>, Error> {
        let statement = self.statements.prepared(&self.transaction, text).await?;
        self.transaction.query_opt(&statement, params).await
    }

    pub(crate) async fn execute(
        &self,
        text: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<u64, Error> {
        let statement = self.statements.prepared(&self.transaction, text).await?;
        self.transaction.execute(&statement, params).await
    }

    pub(crate) async fn commit(self) -> Result<(), Error> {
        self.transaction.commit().await
    }

    pub(crate) async fn rollback(self) -> Result<(), Error> {
        self.transaction.rollback().await
    }
}

/// The isolation level and the access mode of a [`Transaction`] before it starts.
pub(crate) struct TransactionBuilder<'a> {
    builder: tokio_postgres::TransactionBuilder<'a>,
    statements: &'a StatementCache,
}

impl<'a> TransactionBuilder<'a> {
    pub(crate) fn isolation_level(self, level: IsolationLevel) -> Self {
        Self {
            builder: self.builder.isolation_level(level),
            statements: self.statements,
        }
    }

    pub(crate) fn read_only(self, read_only: bool) -> Self {
        Self {
            builder: self.builder.read_only(read_only),
            statements: self.statements,
        }
    }

    pub(crate) async fn start(self) -> Result<Transaction<'a>, Error> {
        Ok(Transaction {
            transaction: self.builder.start().await?,
            statements: self.statements,
        })
    }
}

/// The statements of a [`Client`] or of a [`Transaction`], for a helper that runs inside the
/// transaction of its caller or as one autocommit read. It runs no transaction control.
pub(crate) trait GenericClient: Sync {
    fn query(
        &self,
        text: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> impl Future<Output = Result<Vec<Row>, Error>> + Send;

    fn query_one(
        &self,
        text: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> impl Future<Output = Result<Row, Error>> + Send;

    fn query_opt(
        &self,
        text: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> impl Future<Output = Result<Option<Row>, Error>> + Send;

    fn execute(
        &self,
        text: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> impl Future<Output = Result<u64, Error>> + Send;
}

impl GenericClient for Client {
    async fn query(&self, text: &str, params: &[&(dyn ToSql + Sync)]) -> Result<Vec<Row>, Error> {
        Client::query(self, text, params).await
    }

    async fn query_one(&self, text: &str, params: &[&(dyn ToSql + Sync)]) -> Result<Row, Error> {
        Client::query_one(self, text, params).await
    }

    async fn query_opt(
        &self,
        text: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Option<Row>, Error> {
        Client::query_opt(self, text, params).await
    }

    async fn execute(&self, text: &str, params: &[&(dyn ToSql + Sync)]) -> Result<u64, Error> {
        Client::execute(self, text, params).await
    }
}

impl GenericClient for Transaction<'_> {
    async fn query(&self, text: &str, params: &[&(dyn ToSql + Sync)]) -> Result<Vec<Row>, Error> {
        Transaction::query(self, text, params).await
    }

    async fn query_one(&self, text: &str, params: &[&(dyn ToSql + Sync)]) -> Result<Row, Error> {
        Transaction::query_one(self, text, params).await
    }

    async fn query_opt(
        &self,
        text: &str,
        params: &[&(dyn ToSql + Sync)],
    ) -> Result<Option<Row>, Error> {
        Transaction::query_opt(self, text, params).await
    }

    async fn execute(&self, text: &str, params: &[&(dyn ToSql + Sync)]) -> Result<u64, Error> {
        Transaction::execute(self, text, params).await
    }
}
