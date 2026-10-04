# Test asset audit progress

## Resume here

- Status: active. Inventory captured; node-contract source review complete.
- Worktree: `/Users/frankqdwang/.codex/worktrees/test-asset-audit/StoryOS`.
- Branch: `codex/test-asset-audit`.
- Fixed baseline: `479224809cdaae997cda51cb8853e3fafa242b65` (fetched `origin/main`, 2026-10-05 Asia/Singapore).
- Read this file before each resumed session. Do not refresh the audit baseline.
- Next: review crates/storyos-core/src next. Revisit cross-layer owners as each directory is read.

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
| apps/web/test/node-contract | Source review complete | node-contract.md; 31 runtime cases, 8 files; mutation review pending |
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

## Latest checkpoint

- Node contract: 31 runtime cases reviewed across 8 files. See node-contract.json for source spans and node-contract.md for decisions.
- Discovery: startup asset cases can fail before asset loading; session-map cases can fail later on missing DATABASE_URL even if mapping validation is bypassed. No runtime mutation has yet been used to validate these findings.
- Discovery: the TypeScript Unicode counter has no product consumer and is imported only by its own test. Do not describe its Rust counterpart as executing that helper.
