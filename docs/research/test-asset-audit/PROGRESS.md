# Test asset audit progress

## Resume here

- Status: active. 392 test cases in 105 files have source verdicts. The audit is not complete.
- Worktree: /Users/frankqdwang/.codex/worktrees/test-asset-audit/StoryOS.
- Branch: codex/test-asset-audit. Fixed baseline: 479224809cdaae997cda51cb8853e3fafa242b65.
- Read this file before each resumed session. Do not refresh the baseline or touch the main checkout.
- Complete source-review directories: apps/web/test/node-contract (8 files, 31 cases); crates/storyos-server/tests (1 file, 2 cases); crates/storyos-core/src (38 files, 202 tests); crates/storyos-application/src (20 files, 58 tests); crates/storyos-adapter-postgres/tests (2 files, 12 tests); crates/storyos-adapter-postgres/src (36 files, 87 tests). Cross-directory reconciliation and mutation review are still pending.
- Core coverage: CORE_CHECKPOINT.md records all 38 test files and 29 module-link files; core.json and core.md hold every test verdict.
- Next: crates/storyos-server/src, then crates/storyos-contracts/src and remaining Web directories. ADAPTER_CHECKPOINT.md records all 36 source test files and the shared/module-link disposition. Adapter totals: 99 tests in 38 files.
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
| crates/storyos-core/src | Source review complete | core.md; 202 tests in 38 files; CORE_CHECKPOINT.md |
| crates/storyos-application/src | Source review complete | application.md; 58 tests in 20 files; APPLICATION_CHECKPOINT.md |
| crates/storyos-adapter-postgres/src | Source review complete | adapter.md; 36 test files, 87 tests; ADAPTER_CHECKPOINT.md |
| crates/storyos-adapter-postgres/tests | Source review complete | adapter.md; 12 tests in 2 files; ADAPTER_INTEGRATION_CHECKPOINT.md |
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

- Core directory source review complete: inventory and verdict names match exactly, 202 functions in 38 files. Current overall source verdicts: {'DELETE': 148, 'KEEP': 84, 'MERGE': 2, 'MOVE': 1}; 2585 candidate lines across 47 test files.
- Corrections: canonical Inline boundary input calls the shared owner classifier, so equality-edge assertions are publicly covered; the local crossing/full-selection cases remain distinct. Test citations were corrected to declaration lines.
- Export Write spies do not measure intermediate copies; their DELETE rows use D3. The actual payload-hash hook counts real hash input, so its single-pass performance invariant remains KEEP without a latency claim.
- No source mutation, database operation, full test run, issue change, PR or push occurred during this Core review.

- Application directory source review complete: 58 functions in 20 files, 54 DELETE and 4 KEEP; 2186 candidate lines. Inventory/test-name completeness and citation declaration lines checked.
- Current overall source review: 293 cases / 67 test files, {'DELETE': 202, 'KEEP': 88, 'MERGE': 2, 'MOVE': 1}; 4771 candidate lines. This is still a partial repository total and has zero mutation-validated DELETE samples.
- The Application query fixtures fabricate facts that the current scoped Postgres Readers do not emit. Report D5 explicitly separates these from executable public isolation coverage. No authorization test is claimed redundant only because another test returns the same status.

- Adapter integration directory complete: 12 tests, 10 KEEP / 2 DELETE; 116 candidate lines. Adapter source progress: 25 tests in 9 files. Reviewed owners include Author Edit recovery, outcome locks, observation append, connection pooling, Block payloads, search/tree and challenges.
- Overall source verdicts: 330 cases / 78 files; {'DELETE': 213, 'KEEP': 114, 'MERGE': 2, 'MOVE': 1}; 5295 candidate lines. No mutation experiment has run.

- Adapter source checkpoint: 72 tests in 28 files; with integration, 84 tests in 30 files. Overall: 377 cases / 97 files, DELETE 228, KEEP 143, MERGE 5, MOVE 1; 8105 candidate lines. Mutation checks remain 0/30.
- ADR 0039 explicitly requires Provider preparation refusal and stream sequencing at the Model Gateway seam. Keep the real-Postgres contract tests even though the current FakeDestination does not emit preparation refusal. ADR 0041 explicitly requires in-progress structural replay refusal. These accepted seam/persistence contracts differ from fabricated same-source command bindings (D5). Reconcile that distinction across all D5 rows before freezing the sample.
- Create AgentRun has a vacuous final foreign query: it queries a Run that its own setup already deleted. Retain only its distinct missing-Revision scenario and other proven unique evidence; HTTP owns actual foreign isolation.
- Project rename and archive Activity counts, plus Chapter Head identity preservation on rename, are MERGE recommendations. Do not count their files as immediately removable before those assertions reach the named HTTP owners.
- Positive structural Commit-shape assertions duplicate executable Undo paths. Direct SQL CHECK violations and real rollback faults remain distinct. No temporary mutation or targeted test run occurred in this source-review checkpoint.

- Adapter source directory complete: 87 tests in 36 files. Combined Adapter source/integration: 99 tests, 30 DELETE / 65 KEEP / 4 MERGE. Inventory completeness and all cited Rust/TypeScript declaration lines checked.
- Overall source review: 392 cases / 105 files, DELETE 232, KEEP 153, MERGE 6, MOVE 1; 9370 candidate source lines. Still 0/30 mutation checks, no full-audit claim.
- The hand-written Takeover persistence test never invokes the product Takeover operation; its final invalid insert can fail on missing Commit/Author Action foreign keys even if the intended CHECK is removed. D3 records this masked oracle.
- Direct SQL rank counters measure statement work, not latency. Keep actual five-/seven-Chapter batching and Volume sparse/tombstone cases. Keep independently blocked Takeover races and unequal-counter preservation.
