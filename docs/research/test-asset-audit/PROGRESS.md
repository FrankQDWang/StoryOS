# Test asset audit progress

## Resume here

- Status: active. 120 test cases in 25 files have source verdicts. The audit is not complete.
- Worktree: /Users/frankqdwang/.codex/worktrees/test-asset-audit/StoryOS.
- Branch: codex/test-asset-audit. Fixed baseline: 479224809cdaae997cda51cb8853e3fafa242b65.
- Read this file before each resumed session. Do not refresh the baseline or touch the main checkout.
- Complete directories: apps/web/test/node-contract (8 files, 31 cases); crates/storyos-server/tests (1 file, 2 cases).
- Partial directory: crates/storyos-core/src (16 files, 87 tests). core.json and core.md contain its completed files. Do not repeat those reviews.
- Next: apply_author_edit_tests.rs and open_inline_proposal_tests.rs, then the remaining Core files. Compare exact inputs and assertions, not test names. After Core, proceed through Application, Adapter, Server, Contracts, and remaining Web directories.
- No mutation samples selected or executed: 0/30. Select randomly only after the complete DELETE population is stable.
- No active processes or temporary source mutations remain at this checkpoint.
- A clean paired release package and Node dependencies are ready. Startup baseline 7/7 and browser navigation/list-open baseline 2/2 passed. Run managed commands serially; they share one execution budget.

## Contract

Audit all of `apps/web/test`, all crate `*_tests.rs`, inline Rust tests, and crate `tests/` directories. Do not change existing tests or product sources except for temporary mutation checks. Restore each mutation immediately. Write all retained outputs here. Do not change main or other worktrees. No PR, Issue, full test run, or `make verify-local`.

Each test needs KEEP, DELETE, MERGE, or MOVE with a source line, reason code, observable regression, and named coverage evidence. A KEEP needs a distinct regression and a checked comparison. A DELETE must state the covering test, or explicitly record that a static assertion has no observable regression and no covering test exists. Do not invent a covering test. Such uncovered rows are not verified deletion recommendations until reviewed. Count only concrete removed source spans; do not count shared helpers twice.

## Decisions and evidence

- The user request controls the report format and scope. The Brooks test-review skill is diagnostic support only. Do not write its history file outside this directory.
- Use the assertion table shape and reason-code legend from PR 927, `Removed test assertions and reasons`, as the presentation model. Its old line numbers are not current evidence.
- Inventory includes test-module link files for completeness; classify them separately from tests. Parameterized declarations need individual case review. Inventory counts are not runtime counts.
- Do not call a static search proof of execution. Only a baseline PASS, named mutation FAIL, and restored PASS prove a sampled covering test catches that mutation.
- Sample 30 DELETE rows randomly after the complete verdict set is stable. Save the seed, population, and selected rows before injections. Keep failed or blocked samples and correct their class. No replacement to hide failed samples.
- `make verify-status BASE=origin/main` ran on the clean worktree. It reported only missing input-policy evidence and requested `make verify-targeted CHECK=verify-policy`. It later ended SOURCE-CHANGED; see the preserved result below.
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
| crates/storyos-core/src | In progress | core.md; 16 files, 87 tests reviewed |
| crates/storyos-application/src | Pending | inventory.json |
| crates/storyos-adapter-postgres/src | Pending | inventory.json |
| crates/storyos-adapter-postgres/tests | Pending | inventory.json |
| crates/storyos-server/src | Pending | inventory.json |
| crates/storyos-server/tests | Source review complete | server-integration.md; 2 tests; 33 candidate lines |
| crates/storyos-contracts/src | Pending | inventory.json |

## Self-check

0/30 mutation experiments complete. No DELETE sample has been selected yet.

## Delivery

REPORT.md will start with conclusions, directory savings and the top 20 files, then give all per-file tables and the cross-layer duplication table. It does not exist yet; no complete-audit claim is made.

## Latest checkpoint

- Node contract: 31 runtime cases reviewed across 8 files. See node-contract.json for source spans and node-contract.md for decisions.
- Discovery: startup asset cases can fail before asset loading; session-map cases can fail later on missing DATABASE_URL even if mapping validation is bypassed. No runtime mutation has yet been used to validate these findings.
- Discovery: the TypeScript Unicode counter has no product consumer and is imported only by its own test. Do not describe its Rust counterpart as executing that helper.

- Server integration directory complete: 2 duplicate process tests select the packaged Node cases as retained owners. Candidate savings: 33 lines, not yet mutation-proved.

- Core checkpoint: Create/Archive/Update Project files reviewed (11 tests). Current public input guards make three Core tests exercise unreachable internal states; code D5 distinguishes these from actual duplicate executable coverage. Create/Update Volume are now reviewed. Next: Chapter and remaining operations.

- Node/Server citation check corrected test-start line numbers; all cited source spans refer to the fixed baseline.
- `make verify-targeted CHECK=verify-policy` ended with `source-changed` after 389.26 seconds because audit documents were committed during the run. Its printed Python groups passed, but the managed result is NOT PASS. Report: `target/verification/62fab459d82d4bb29a1967e8f6fcbe95/report.json`. Preserve this result. This is a read-only audit; do not run a full verification to replace it.
- Added CROSS_LAYER.md with the comparisons already reviewed. Other layers in those rows remain pending; do not infer their final verdicts.

- Core checkpoint: 49 tests in 9 files reviewed; 82 runtime cases reviewed overall. Continue with set_current_chapter and undo_latest_author_action, then Proposal, text, export, and context modules.
- Exact source comparison found a misleading HTTP deletion test name: no previous-sibling deletion is executed. Keep the Core previous-sibling case. Keep missing-Volume deletion too; foreign-Project refusal is not the same case.
- make release-package PASS in 352.63 seconds, with install, strict TypeScript checks, Vite build, Rust release build, and offline binary checks. No full tests ran. Package source: d1d8393cd32156f0485b5982a2cf37f5515fdcc3; manifest sha256:f4eb3d9d8aeba46dc2bfacfb881c43838a676c2a6a2be782623ab2b775e90bba. Report: target/verification/88f7e20a28e345b380ae15c2133bd0e4/report.json. Product sources equal the audit baseline; later report commits do not change package behavior.
- Docker is available. Use only scripts/dev-postgres.sh run for future database checks.
- Read-ahead (not adjudicated): set_current_chapter_tests.rs, undo_latest_author_action_tests.rs, accept_proposal_tests.rs, append_proposal_generation_batch_tests.rs, open_block_proposal_tests.rs, pause_proposal_generation_tests.rs, compact_active_context_tests.rs, archive_path_tests.rs, readable_export_tests.rs. Re-read the exact body when needed; do not treat this list as completed coverage.

- Latest baseline result: startup 7/7 and two browser cases 2/2 PASS. BASELINES.md and baseline-evidence.json preserve the actual runner outcomes, including the earlier SOURCE-CHANGED result.
- Current partial counts: {'DELETE': 55, 'KEEP': 25, 'MERGE': 1, 'MOVE': 1}; 829 candidate source lines. These are not full-repository totals or mutation-validated savings.

- Current checkpoint: 120 cases in 25 files; {'DELETE': 80, 'KEEP': 38, 'MERGE': 1, 'MOVE': 1}; 1194 candidate source lines. Core now includes Current Chapter, Undo, Acceptance, generation append/pause, Block Proposal and compaction. These are provisional source verdicts, not mutation results.
- Revalidation corrected the Block Proposal comparison: the reserved-block HTTP Author Edit calls the same Core changed-Head classifier when it records conflicted validation. A synthetic present-Block/None-Head tuple is not a unique real input.
- Added render_tables.py to keep manually written rows and Markdown tables consistent. It does not infer verdicts and is not connected to verification commands.
