# Resources for parallel worktrees

Each implementation worktree owns its source, build outputs, verification records, and development database. Keep `target/`, `node_modules/`, and release packages separate. Do not share these directories through symlinks. The coordinator bounds concurrent workers to fit host CPU and memory; verification locks do not impose a host-wide worker budget.

## Development PostgreSQL

Run `scripts/dev-postgres.sh run <command> ...` for tests. The command owns a
unique database, exports its environment to the child, and removes the container
and anonymous volume on success, failure, or interruption. A checkout-local file
lock protects each resource lease. The next run removes abandoned leases only
when no owner holds that lock. Cleanup failures retain the lease for the next run.
The daily database runner uses this same owner.

Interactive development requires `up --interactive`, then `reload`, `env`, and `down`. These commands
select the canonical checkout path; `down` also removes the anonymous data volume.
Do not share database environment variables between worktrees.

The old `storyos-dev-postgres` container is retained. New commands do not use or remove it. Migrate any required data before the user explicitly removes that legacy container.

## Observation

The dashboard at loopback ports 3749 and 3754 remains one optional service on the local host. It belongs to the checkout recorded in every existing service container's Compose working-directory label. `make observe-start`, `observe-stop`, and `observe-status` refuse another or unknown owner before service mutation. A shared local lock covers the ownership check, Grafana import, and lifecycle action. An implementer does not need a separate dashboard to run checks.

Use the owner checkout to stop the service before another checkout starts it. Stopping the service retains checkout-local data. An unknown legacy owner requires explicit inspection; commands never import, stop, or delete its resources automatically. Use these repository commands rather than raw Compose lifecycle commands. `make observe-rebuild` rebuilds the owner's derived observation database from
published records across the repository's worktrees. Each runner atomically
publishes read copies below the common Git directory. Starting observation imports the latest 32 runs and 32 requests per checkout.
Collector health lists imported and older run counts; all older evidence stays
in its original directory and can be read with `--records <directory>`. Newly updated runners publish continuously,
including worktrees created after the service starts. The collector health record
lists the repositories it has collected. Original checkout records remain the
execution evidence; neither the shared read copies nor SQLite authorize execution.

`make observe-smoke` uses a unique project for each invocation, disposable directories, and dynamic loopback ports. Two runs, including two runs in one checkout, cannot remove each other's services during cleanup. It does not change the user's observation singleton.

## Validation

`make verify-targeted CHECK=verification-observation-tests` exercises public resource commands with stateful Docker CLI doubles. It checks cross-checkout database preservation, foreign and unknown observation owner refusal, and overlapping smoke cleanup through deterministic socket barriers. `make observe-smoke` checks actual disposable services. Run `make verify-policy` after changes to runner inputs.

Observation start and rebuild stop the singleton services before a host-side
import. The services start again only after import completes. This prevents two
writers across the host and container filesystem boundary. Run resource-command
checks after an observation lifecycle command completes; both use the same local
singleton lock.
