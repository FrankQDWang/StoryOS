# Resources for parallel worktrees

Each implementation worktree owns its source, build outputs, and development database. Keep `target/`, `node_modules/`, and release packages separate. Do not share these directories through symlinks. The coordinator bounds concurrent workers to fit host CPU and memory.

## Development PostgreSQL

Run `scripts/dev-postgres.sh run <command> ...` or `scripts/dev-postgres.sh with <command> ...` for tests.
The two forms are the same. The command owns a unique database, exports its environment to the child,
and removes the container and anonymous volume on success, failure, or interruption. `make check` uses the `with` form.

Interactive development requires `up --interactive`, then `reload`, `env`, and `down`. These commands
select the canonical checkout path; `down` also removes the anonymous data volume.
Do not share database environment variables between worktrees.

The old `storyos-dev-postgres` container is retained. New commands do not use or remove it. Migrate any required data before the user explicitly removes that legacy container.
