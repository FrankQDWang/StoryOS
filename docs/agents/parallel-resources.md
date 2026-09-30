# Resources for parallel worktrees

Each implementation worktree owns its source, build outputs, verification records, and development database. Keep `target/`, `node_modules/`, and release packages separate. Do not share these directories through symlinks. The coordinator bounds concurrent workers to fit host CPU and memory; verification locks do not impose a host-wide worker budget.

## Development PostgreSQL

Run `scripts/dev-postgres.sh up`, `reload`, `env`, and `down` in the owner checkout. The container name uses the canonical checkout path, so these actions cannot select another checkout's database. A path alias selects the same database. A moved checkout has a new identity; stop its old database from the old checkout before the move.

Run `eval "$(scripts/dev-postgres.sh up)"` separately in each worker shell. Do not share `STORYOS_TEST_DATABASE_URL`, `STORYOS_TEST_ADMIN_DATABASE_URL`, or `STORYOS_TEST_POSTGRES_CONTAINER` between workers. The managed daily database command remains an alternative with its own disposable database. A caller that sets `STORYOS_RECOVERY_COPY_DIR` must provide an exclusive path.

The old `storyos-dev-postgres` container is retained. New commands do not use or remove it. Migrate any required data before the user explicitly removes that legacy container.

## Observation

The dashboard at loopback ports 3749 and 3754 remains one optional service on the local host. It belongs to the checkout recorded in every existing service container's Compose working-directory label. `make observe-start`, `observe-stop`, and `observe-status` refuse another or unknown owner before service mutation. A shared local lock covers the ownership check, Grafana import, and lifecycle action. An implementer does not need a separate dashboard to run checks.

Use the owner checkout to stop the service before another checkout starts it. Stopping the service retains checkout-local data. An unknown legacy owner requires explicit inspection; commands never import, stop, or delete its resources automatically. Use these repository commands rather than raw Compose lifecycle commands. `make observe-rebuild` rebuilds only the current checkout's derived observation database.

`make observe-smoke` uses a unique project for each invocation, disposable directories, and dynamic loopback ports. Two runs, including two runs in one checkout, cannot remove each other's services during cleanup. It does not change the user's observation singleton.

## Validation

`make verify-targeted CHECK=verification-observation-tests` exercises public resource commands with stateful Docker CLI doubles. It checks cross-checkout database preservation, foreign and unknown observation owner refusal, and overlapping smoke cleanup through deterministic socket barriers. `make observe-smoke` checks actual disposable services. Run `make verify-policy` after changes to runner inputs.
