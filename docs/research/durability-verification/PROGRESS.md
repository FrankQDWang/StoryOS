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
