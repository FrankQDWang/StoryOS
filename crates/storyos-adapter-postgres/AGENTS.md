# storyos-adapter-postgres

This file adds the subtree rules for the `storyos_runtime` store. The root `AGENTS.md` still applies.

## Connection reuse

`PostgresProjectReader` owns one `ConnectionPool` (`connection_pool.rs`). Every request and Worker claim checks out a `PooledClient`, and a settled client returns to the pool on drop. ADR 0031 records the decision.

- Get a runtime connection only through `connect()` or `connect_challenge()`. Do not call `tokio_postgres::connect` outside `connection_pool.rs` and `storage_activation.rs`. The startup gate in `storage_activation_proof.rs` is the one caller of `connection_pool::open`; it runs before a pool exists.
- Run `BEGIN`, `START TRANSACTION`, `COMMIT`, `END`, `ROLLBACK`, and `ABORT` only through `batch_execute` on a `PooledClient`. That method records the transaction state; a `BEGIN` through `execute`, `query`, or `simple_query` is not recorded, so the pool can hand an open transaction to the next request.
- A helper that runs transaction control takes `client: &PooledClient`. A helper that takes `&tokio_postgres::Client` or `impl GenericClient` does not run transaction control. It runs inside a transaction that its caller owns, or it runs one autocommit read such as the Storage Activation proof read.
- Pass `&*client` when a `PooledClient` goes to a helper that takes `impl GenericClient`.
- Set scope only with transaction-local `set_config(..., true)`. Session-level state would survive a settled transaction and reach the next checkout.
- `storage_activation.rs` is the maintenance owner. It runs on its own admin connection that never enters the pool, so these rules do not apply to it.
- `python3 scripts/verify-transaction-control-receivers.py` checks these rules from the source text. It does not read `execute` or `query` calls with a non-literal statement. `make contracts` runs its self-test and `make verify-local` runs the check.

## Run one PostgreSQL test locally

The `#[ignore]` tests in this crate and the `node-postgresql` Vitest project read `STORYOS_TEST_DATABASE_URL`, `STORYOS_TEST_ADMIN_DATABASE_URL`, and `STORYOS_TEST_POSTGRES_CONTAINER`. Complete `make verify` sets them. For one test, prepare the same database yourself:

```sh
make release-package                      # needs a clean worktree; commit first
eval "$(scripts/dev-postgres.sh up)"      # start, activate, set password, load fixture
cargo test -p storyos-adapter-postgres --lib -- --ignored connection_pool
cargo test -p storyos-adapter-postgres --test project_scope -- --ignored
scripts/dev-postgres.sh reload            # empty the domain tables, load the fixture again
STORYOS_VITEST_FILE_ORDER=test/node-postgresql/create-project-http.integration.test.ts: \
  pnpm --dir apps/web exec vitest run --project node-postgresql \
  test/node-postgresql/create-project-http.integration.test.ts
scripts/dev-postgres.sh down
```

The fixture uses fixed identifiers, so run `reload` between two runs of the same Vitest file. Run `verify-project-scope.sh` through `make verify` only; it owns the input oracles and the file order.
