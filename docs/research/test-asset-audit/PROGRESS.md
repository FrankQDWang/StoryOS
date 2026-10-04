# Test asset audit progress

## Resume here

- Status: active. Inventory captured; detailed review starts with `apps/web/test/node-contract`.
- Worktree: `/Users/frankqdwang/.codex/worktrees/test-asset-audit/StoryOS`.
- Branch: `codex/test-asset-audit`.
- Fixed baseline: `479224809cdaae997cda51cb8853e3fafa242b65` (fetched `origin/main`, 2026-10-05 Asia/Singapore).
- Read this file before each resumed session. Do not refresh the audit baseline.
- Next: finish node-contract review, compare browser and Rust coverage, write its verdict tables, commit this progress file with the tables. Then audit the remaining directories.

## Contract

Audit all of `apps/web/test`, all crate `*_tests.rs`, inline Rust tests, and crate `tests/` directories. Do not change existing tests or product sources except for temporary mutation checks. Restore each mutation immediately. Write all retained outputs here. Do not change main or other worktrees. No PR, Issue, full test run, or `make verify-local`.

Each test needs KEEP, DELETE, MERGE, or MOVE with a source line, reason code, observable regression, and named coverage evidence. A KEEP needs a distinct regression and a checked comparison. A DELETE must state the covering test, or explicitly record that a static assertion has no observable regression and no covering test exists. Do not invent a covering test. Such uncovered rows are not verified deletion recommendations until reviewed. Count only concrete removed source spans; do not count shared helpers twice.

## Decisions and evidence

- The user request controls the report format and scope. The Brooks test-review skill is diagnostic support only. Do not write its history file outside this directory.
- Use the assertion table shape and reason-code legend from PR 927, `Removed test assertions and reasons`, as the presentation model. Its old line numbers are not current evidence.
- Inventory includes test-module link files for completeness; classify them separately from tests. Parameterized declarations need individual case review. Inventory counts are not runtime counts.
- Do not call a static search proof of execution. Only a baseline PASS, named mutation FAIL, and restored PASS prove a sampled covering test catches that mutation.
- Sample 30 DELETE rows randomly after the complete verdict set is stable. Save the seed, population, and selected rows before injections. Keep failed or blocked samples and correct their class. No replacement to hide failed samples.
- `make verify-status BASE=origin/main` ran on the clean worktree. It reported only missing input-policy evidence and requested `make verify-targeted CHECK=verify-policy`. That targeted command is running.
- Reports can exceed the ordinary implementation diff limit: exhaustive tables are explicitly required by this task. Commit each completed directory independently.

## Directory ledger

| Directory | State | Evidence |
|---|---|---|
| apps/web/test/node-contract | In progress | Source read; coverage comparison pending |
| apps/web/test/node-postgresql | Pending | inventory.json |
| apps/web/test/browser-source | Pending | inventory.json |
| apps/web/test/browser-exact-dist | Pending | inventory.json |
| apps/web/test/node-process-cut | Pending | inventory.json |
| apps/web/test/support | Pending | inventory.json |
| crates/storyos-core/src | Pending | inventory.json |
| crates/storyos-application/src | Pending | inventory.json |
| crates/storyos-adapter-postgres/src | Pending | inventory.json |
| crates/storyos-adapter-postgres/tests | Pending | inventory.json |
| crates/storyos-server/src | Pending | inventory.json |
| crates/storyos-server/tests | Pending | inventory.json |
| crates/storyos-contracts/src | Pending | inventory.json |

## Self-check

0/30 mutation experiments complete. No DELETE sample has been selected yet.

## Delivery

REPORT.md will start with conclusions, directory savings and the top 20 files, then give all per-file tables and the cross-layer duplication table. It does not exist yet; no complete-audit claim is made.
