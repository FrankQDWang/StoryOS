---
status: accepted
---

# Reuse Runtime Connections With Statement-Tracked Transaction State

`PostgresProjectReader` owns one process-local connection pool. A protected request or Worker claim checks out one idle connection to read the Storage Activation proof and returns it. It then checks out one connection for its work and returns that connection on drop. `PooledClient::batch_execute` records `BEGIN`, `COMMIT`, and `ROLLBACK` statements and their synonyms. A connection inside an open transaction, or a closed connection, does not return to the pool. Before this decision every protected request opened two fresh connections, and the Worker opened two more on each 50 ms poll. Under the verify load the Server intermittently answered `project_store_unavailable`.

## Considered options

- Tolerating a transient 503 in the affected test poll was rejected. The refusal came from the product runtime, not from the test.
- Adding a pool crate such as `deadpool-postgres` or `bb8` was rejected. Both need a recycle hook or a typed transaction guard at every call site to know the transaction state, and a new dependency needs its own review. A 160-line local pool fits the current need.
- Rewriting the 77 manual transaction sites to a typed `Transaction` guard was rejected as the first step. The rewrite would spread across every module for one operational fix. It stays available as a later refactor.
- Running `ROLLBACK` on every return to reset the connection was rejected. PostgreSQL logs a warning on each `ROLLBACK` outside a transaction, and the reset hides which site left a transaction open.
- Reading the transaction status from the wire protocol was not possible. PostgreSQL reports it in `ReadyForQuery`, but `tokio_postgres::Client` does not expose it.
- Sharing one pool between the Server and its in-process Worker was deferred. The Server state and the in-process Worker each keep their own `PostgresProjectReader`.

## Consequences

- Transaction control runs only through `batch_execute` on a `PooledClient`. A helper that runs transaction control takes `&PooledClient`. `scripts/verify-transaction-control-receivers.py` guards the rule in `make verify-local`, and `crates/storyos-adapter-postgres/AGENTS.md` states it for the subtree.
- `Client::transaction()` stays valid: its guard rolls back on drop before the pooled client returns.
- ADR 0020 is unchanged. The Server reads the Storage Activation proof on the pooled connection on every protected request; the Worker reads it on every claim.
- The idle set holds at most eight connections per pool and has no idle lifetime and no liveness probe. A connection that the database closed is dropped at checkout.
- A transaction-mode external pooler is not part of the current topology (ADR 0022). If one arrives, session state that survives a settled transaction must be reviewed again.
- `storage_activation.rs` keeps its own admin connection outside the pool. The maintenance owner never shares runtime connections.
