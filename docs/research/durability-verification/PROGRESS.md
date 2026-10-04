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
- Step 4: `concurrent-rename` reached two observed lock waits. The first legal
  rename succeeded; the other returned HTTP 503 `project_store_unavailable`.
  PostgreSQL reported `could not serialize access due to concurrent update`.
  A subsequent Author Edit succeeded. D3/concurrency has a reproducible failure.
  Preliminary evidence: `target/durability-verification/concurrent-rename-v3.json`.
  Probe input correction: GetProject omits revision, so read the fixture's
  current Project revision through PostgreSQL. Wait queries use visible SQL
  prefixes; PostgreSQL truncates long activity query text.
  Root-cause candidates: missing whole-transaction retry, exhausted retry, or
  invalid binding. Next: replay, locate the cause, and run takeover schedules.
- Step 5: `takeover-server` proved D4/interruption false. Admission committed,
  Core waited on the Head, SIGKILL stopped the Server, and the new Server
  acknowledged writer generation 2. GET of the old command outcome then wrote
  `Authoritative AOLD` with one new Authoritative Commit. A fresh stale command
  returned 412, but recovery bypassed that fence. The winner's next edit
  conflicted against its stale base Snapshot. This failed smoke is retained in
  `target/durability-verification/takeover-server-v2.json`.
  Next: verify recovery on a fresh Editor Session and replay all matrix cases.
- Reproducers return 1 for an invariant failure and 2 for a probe error. Public
  query correlation IDs are excluded only from independent query comparisons.
  SQL errors and exact command replies remain in evidence; nonces are omitted.
- Step 6: concurrent exact retry also returned 503 with one Receipt and one
  Admission. A later sequential replay returned the original full reply.
  The adapter starts SERIALIZABLE and propagates errors; no retry loop exists
  in its caller chain. This is the same failure class as Step 4.
- The fixed matrix now includes before-Admission, after-Admission, and
  pre-Core-COMMIT cuts, each by Server SIGKILL and backend termination. A
  second lock on idempotency stops Core after its effect writes but before
  COMMIT. The lost-ack case discards a received HTTP reply then kills Server;
  it does not claim a cut inside Server response serialization.
- Next: run round 1, minimize confirmed findings, then replay the full matrix.
- Matrix round-1, `durable`: completed (exit 0). Evidence: `evidence/round-1/durable.json`.
- Matrix round-1, `lost-ack`: completed (exit 0). Evidence: `evidence/round-1/lost-ack.json`.
- Matrix round-1, `cut-admission-server`: completed (exit 0). Evidence: `evidence/round-1/cut-admission-server.json`.
- Matrix round-1, `cut-admission-database`: completed (exit 0). Evidence: `evidence/round-1/cut-admission-database.json`.
- Matrix round-1, `cut-core-server`: completed (exit 0). Evidence: `evidence/round-1/cut-core-server.json`.
- Matrix round-1, `cut-core-database`: completed (exit 0). Evidence: `evidence/round-1/cut-core-database.json`.
- Matrix round-1, `cut-commit-server`: completed (exit 0). Evidence: `evidence/round-1/cut-commit-server.json`.
- Matrix round-1, `cut-commit-database`: completed (exit 0). Evidence: `evidence/round-1/cut-commit-database.json`.
- Matrix round-1, `concurrent-rename`: invariant failure (exit 1). Evidence: `evidence/round-1/concurrent-rename.json`.
- Matrix round-1, `concurrent-retry`: invariant failure (exit 1). Evidence: `evidence/round-1/concurrent-retry.json`.
- Matrix round-1, `concurrent-rename-restart`: invariant failure (exit 1). Evidence: `evidence/round-1/concurrent-rename-restart.json`.
- Matrix round-1, `concurrent-author`: completed (exit 0). Evidence: `evidence/round-1/concurrent-author.json`.
- Matrix round-1, `session-replay`: invariant failure (exit 1). Evidence: `evidence/round-1/session-replay.json`.
- Matrix round-1, `takeover-server`: blocked (exit 2). Evidence: `evidence/round-1/takeover-server.json`.
  Reason: fetch failed
- Matrix round-1, `takeover-database`: invariant failure (exit 1). Evidence: `evidence/round-1/takeover-database.json`.
- Matrix round-1, `takeover-concurrent`: invariant failure (exit 1). Evidence: `evidence/round-1/takeover-concurrent.json`.
- Step 7: round 1 covered all 16 cases. New findings: F1 (old Admission
  recovery crosses takeover), F2 (serialization failures escape as 503), and
  F3 (CreateEditorSession exact retry reads current state). D1 passed every
  applicable case. The concurrent Author Edit also exposed F2; its prior
  acknowledged edit remained intact. SQL logs include a secondary empty UUID
  scope error on that error path; this does not form a separate public failure.
- `takeover-server` reached F1 but its later optional session-recovery control
  got a fetch error. Other takeover variants completed that control. Retain
  the blocked run. Use fresh HTTP connections (`Connection: close`) and retain
  Server stderr to remove client connection reuse from later schedules. The
  first fetch error's cause is unproved; do not report a Server crash.
- Minimize F3 to takeover plus replay, without a preceding edit. `--minimal`
  omits follow-up controls for the F1/F2 reproducers. No product input changes.
  Next: replay the complete 16-case matrix with these fixed probe inputs.
- Matrix round-2, `durable`: completed (exit 0). Evidence: `evidence/round-2/durable.json`.
- Matrix round-2, `lost-ack`: completed (exit 0). Evidence: `evidence/round-2/lost-ack.json`.
