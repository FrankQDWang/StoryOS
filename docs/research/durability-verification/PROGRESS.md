# Durability verification progress

## Resume here

- Goal: assess D1 through D4 with deterministic concurrent schedules and process cuts.
- Status: step 1 complete. Next: build the local package and map public commands.
- Baseline: `479224809cdaae997cda51cb8853e3fafa242b65` (fetched `origin/main`).
- Branch: `codex/durability-verification`.
- Worktree: `/Users/frankqdwang/.codex/worktrees/durability-verification/StoryOS`.
- Only `prototypes/durability-verification/` and this report directory may change.
- Do not modify product code, migrations, generated files, or existing tests.
- Do not create a PR or Issue. Do not change main or another worktree.
- Start databases only with `scripts/dev-postgres.sh run`. Use dynamic Server ports.
- No complete verification, harness self-tests, or sleep-based coordination.

## Matrix

| Invariant | Concurrent schedule | Process interruption |
| --- | --- | --- |
| D1: acknowledged Author Edit persists | Pending | Pending |
| D2: exact retry returns original acknowledgement once | Pending | Pending |
| D3: concurrent legal commands preserve availability | Pending | Pending |
| D4: takeover fences the old writer | Pending | Pending |

## Decisions and evidence

1. Read repository instructions and verification/resource ownership rules. The
   initial `make verify-status BASE=origin/main` reported stale prior evidence;
   it did not run tests. This research does not reuse that evidence.
2. The user permits retained one-time reproducers and forbids product fixes.
   Use only HTTP, PostgreSQL connections, and process signals to drive cases.
3. ADR 0032 permits `historical_acknowledgement_unavailable` for pre-capture
   commands. Assess new commands separately from that accepted legacy exception.
4. Keep all build outputs and disposable database leases in this worktree.
   Build the repository package once; do not run `make verify-local`.
5. Completion requires a second complete matrix pass with no new findings.

## Step log

- Step 1: created the isolated worktree and branch from current `origin/main`.
  The primary checkout was clean and remains unchanged.
- Step 2: built the package with `make release-package` in 177.06 seconds.
  Package source: `d82ccf6ab8abeceb8ce35695d609b2cc6c5be321`; product code equals
  the baseline. Report: `target/verification/5d8a2fe3bfec4bbe8ef9935b4cbd8559/report.json`.
  Web type checks and package integrity checks passed. No full test run occurred.
  Next: implement and execute the public-interface schedules.
- Use the fixture Project and two local authenticated handles for its one User.
  Each case gets a fresh command-owned database. Disable the in-process Worker
  to keep the schedules focused on author commands. Keep the same allocated
  port across a Server restart to preserve the Client Session Binding origin.
- PostgreSQL `pg_stat_activity`, `pg_blocking_pids`, and explicit row/table locks
  supply barriers. A timeout reports a blocked case; it never releases a barrier.
- Step 3: implemented the one-time driver and ran `durable` successfully.
  An acknowledged edit, its complete acknowledgement, and authority counts
  survived SIGKILL/restart and database-connection termination/restart. A new
  Author Edit then succeeded. PostgreSQL 16.15 reports fsync, synchronous_commit,
  and full_page_writes enabled. Initial probe correction: ignore fresh query
  correlation IDs when comparing independent chapter reads. Command replies
  still use complete-object equality. Initial failed probe is retained under
  `target/durability-verification/durable-initial.json`.
  Write execution evidence to ignored target first, then retain it after the
  command ends; this avoids the verification wrapper's source-change status.
  Next: concurrent commands and Admission/Core interruption schedules.
