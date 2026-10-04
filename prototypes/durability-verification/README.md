# Durability verification probes

Run from the dedicated `codex/durability-verification` worktree. The scripts
use the local packaged Server, the existing controlled PostgreSQL fixture,
Node, Python, and Docker. They do not join the default build or test suite.

Build once from a clean worktree:

```sh
make release-package
```

Run one minimal reproducer:

```sh
scripts/dev-postgres.sh run node prototypes/durability-verification/driver.mjs takeover-server target/durability-repro.json --minimal
```

Replace `takeover-server` with `takeover-concurrent`, `concurrent-rename`,
`concurrent-retry`, `concurrent-rename-restart`, or `session-replay` for the
other retained minimum schedules. These six cases exit 1 on the recorded
baseline because they reproduce a defect. Exit 0 means the case oracle passed;
exit 2 means the probe could not complete. Remove `--minimal` to include the
fresh-writer and continued-writing controls. Output contains no nonce or cookie.

Run the complete 16-case matrix with a new evidence label:

```sh
python3 prototypes/durability-verification/run-matrix.py new-round
```

The matrix runner saves JSON under `docs/research/durability-verification/evidence/`,
appends PROGRESS.md, and commits after each case. Use a new label to retain earlier
runs. Raw stdout and verification records stay under ignored `target/`. The
runner continues after a failed or blocked case; inspect every recorded result.
It does not push, create a PR, or change an Issue.

The driver sends HTTP requests through the generated public client. PostgreSQL
locks and `pg_blocking_pids` establish ordering. Timeouts only bound a failed
probe; no sleep releases a barrier. A backend cut uses `pg_terminate_backend`.
A Server cut uses SIGKILL and waits for process exit before restart. Each case
owns a dynamic port and disposable database. `scripts/dev-postgres.sh run`
removes the database; the driver reaps its Server and PostgreSQL client processes.

Read [the report](../../docs/research/durability-verification/REPORT.md) for the
matrix, limitations, root-cause lines, and the exact product baseline.
