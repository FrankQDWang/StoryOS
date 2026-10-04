# Test asset audit progress

## Resume here

- Status: active. 865 test cases in 256 files have source verdicts. The audit is not complete.
- Worktree: /Users/frankqdwang/.codex/worktrees/test-asset-audit/StoryOS.
- Branch: codex/test-asset-audit. Fixed baseline: 479224809cdaae997cda51cb8853e3fafa242b65.
- Read this file before each resumed session. Do not refresh the baseline or touch the main checkout.
- Complete source-review directories: apps/web/test/node-process-cut (3 files, 5 tests); apps/web/test/browser-source (34 test files, 89 runtime cases, three support files); apps/web/test/node-postgresql (52 test files, 215 runtime cases, one support file); apps/web/test/node-contract (8 files, 31 cases); crates/storyos-server/tests (1 file, 2 cases); crates/storyos-core/src (38 files, 202 tests); crates/storyos-application/src (20 files, 58 tests); crates/storyos-adapter-postgres/tests (2 files, 12 tests); crates/storyos-adapter-postgres/src (36 files, 87 tests); crates/storyos-server/src (11 files, 59 tests); crates/storyos-contracts/src (14 files, 62 tests). Exact-dist is also complete (37 files, 43 cases). All 35 Web support and 78 Rust module/support files have dispositions. Inventory and initial coverage reconciliation are complete; mutation review remains pending.
- Core coverage: CORE_CHECKPOINT.md records all 38 test files and 29 module-link files; core.json and core.md hold every test verdict.
- Next: execute the remaining frozen random sample from mutation-sample.json. All 30 manually designed plans are saved in mutations/. Review each candidate status and exact assertion output; never count setup failure, zero selected cases or a timeout as a kill.
- Mutation self-check: 16/30 complete, ten KILL, five MISS and one timeout-only BLOCKED. Samples 1-15 and 22 are complete. Sample 1 has an excluded/repaired harness failure. D5 semantic-only coverage is now explicit after samples 2/3/22.
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
| apps/web/test/node-postgresql | Source review complete | node-postgresql.md; 215 runtime cases in 52 files, one support disposition; NODE_POSTGRESQL_CHECKPOINT.md |
| apps/web/test/browser-source | Source review complete | browser-source.md; 89 cases in 34 files, three support dispositions; BROWSER_SOURCE_CHECKPOINT.md |
| apps/web/test/browser-exact-dist | Partial | browser-exact-dist.md; 35 cases in 34 files; BROWSER_EXACT_DIST_CHECKPOINT.md |
| apps/web/test/node-process-cut | Source review complete | node-process-cut.md; five tests in three files; NODE_PROCESS_CUT_CHECKPOINT.md |
| apps/web/test/support | Pending | inventory.json |
| crates/storyos-core/src | Source review complete | core.md; 202 tests in 38 files; CORE_CHECKPOINT.md |
| crates/storyos-application/src | Source review complete | application.md; 58 tests in 20 files; APPLICATION_CHECKPOINT.md |
| crates/storyos-adapter-postgres/src | Source review complete | adapter.md; 36 test files, 87 tests; ADAPTER_CHECKPOINT.md |
| crates/storyos-adapter-postgres/tests | Source review complete | adapter.md; 12 tests in 2 files; ADAPTER_INTEGRATION_CHECKPOINT.md |
| crates/storyos-server/src | Source review complete | server.md; 59 tests in 11 files; SERVER_CHECKPOINT.md |
| crates/storyos-server/tests | Source review complete | server-integration.md; 2 tests; 33 candidate lines |
| crates/storyos-contracts/src | Source review complete | contracts.md; 62 tests in 14 files; CONTRACTS_CHECKPOINT.md |

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

- Server source directory complete: 59 tests in 11 files; {'MOVE': 2, 'DELETE': 26, 'KEEP': 27, 'MERGE': 4}; 677 candidate source lines. Nine module-link files have dispositions. Origin, Cookie and payload comparisons now distinguish input differences from shared helper coverage. See SERVER_CHECKPOINT.md.
- Current cumulative source review: 451 cases in 116 files; {'DELETE': 258, 'KEEP': 180, 'MERGE': 10, 'MOVE': 3}; 10047 candidate source lines. Random mutation self-check remains 0/30. No product or test changes and no active processes.

- Contracts directory complete: 62 tests in 14 files; {'KEEP': 16, 'DELETE': 46}; 1521 candidate source lines, including three exclusive adjacent expected-JSON helpers. Ten module-link files have dispositions. No product/test/generated changes.
- All Rust inventory pairs now match verdicts: 482 tests in 122 test files. Remaining Web source review and support-file disposition are pending.
- Current cumulative source review: 513 cases in 130 files; {'DELETE': 304, 'KEEP': 196, 'MERGE': 10, 'MOVE': 3}; 11568 unioned candidate source lines. Random self-check remains 0/30. No samples selected, temporary mutations or active processes.

- Node PostgreSQL checkpoint: 32 tests in 12 files reviewed; {'DELETE': 1, 'KEEP': 27, 'MERGE': 4}. Protocol hosting, Project/Challenge admission, library/tree/search, Snapshot/Activity, rename and Archive are covered. Directory remains incomplete.
- Evidence correction: the two Author Edit count-limit subcases can be masked by invalid nonce rejection; CT007 and NP008 document this. NC022 was corrected to reflect both actual UUID validation sites. No runtime mutation proof is claimed.
- Current cumulative source review: 545 cases in 142 files; {'DELETE': 305, 'KEEP': 223, 'MERGE': 14, 'MOVE': 3}; 11578 unioned candidate source lines. Self-check 0/30. Product and existing tests remain unchanged; no active processes.

- Node PostgreSQL structural/navigation checkpoint: 65 tests in 20 files; {'DELETE': 10, 'KEEP': 51, 'MERGE': 4}. Six structural families, Current Chapter and Undo now have complete per-file verdict tables. See NODE_POSTGRESQL_CHECKPOINT.md for shared replay ownership and exact-input differences.
- Current cumulative source review: 578 cases in 150 files; {'DELETE': 314, 'KEEP': 247, 'MERGE': 14, 'MOVE': 3}; 12832 unioned candidate source lines. These are provisional deletion estimates, not mutation-validated savings. Self-check remains 0/30; no sample selected. No temporary mutations or active processes.

- Proposal/Acceptance checkpoint: Node PostgreSQL now has 79 runtime cases in 24 files (65 KEEP, 10 DELETE, 4 MERGE). Expanded admitted-target test.each into four individual case verdicts; inventory declaration counts differ by three. Current cumulative source review: 592 cases in 154 files; DELETE 314, KEEP 261, MERGE 14, MOVE 3; 12832 unioned candidate source lines. Continue remaining Proposal/Run/export files, then remaining Web directories. Mutation self-check remains 0/30.

- Generation/candidate checkpoint: Node PostgreSQL now has 100 runtime cases in 32 files (80 KEEP, 13 DELETE, 7 MERGE). Completion/Continue fixtures and opening-only checks have explicit merge destinations; three duplicate candidate-discussion parameters have direct retained-input references. Parameter declarations are expanded individually.
- Current cumulative source review: 613 cases in 162 files; DELETE 317, KEEP 276, MERGE 17, MOVE 3; 12832 unioned candidate source lines. Parameter-only deletions sharing retained source lines add zero to this conservative line count. Continue remaining Node PostgreSQL files, then remaining Web directories and cross-directory reconciliation. Self-check 0/30; no samples, temporary mutations or active processes.

- Run/closure checkpoint: Node PostgreSQL now has 122 runtime cases in 39 files (100 KEEP, 13 DELETE, 9 MERGE). Author Withdrawal setup moves into withdrawal Root Undo; successor success checks move into crash-after-fence recovery. Actual Server restart and Worker requeue are distinguished in each verdict.
- Current cumulative source review: 635 cases in 169 files; DELETE 317, KEEP 296, MERGE 19, MOVE 3; 12832 unioned candidate source lines. Remaining Node PostgreSQL admission/recovery, inline/multi-operation and export files are listed in NODE_POSTGRESQL_CHECKPOINT.md. Other Web directories and cross-directory reconciliation remain pending. Self-check 0/30; no samples, mutations or active processes.

- Run admission/Takeover checkpoint: Node PostgreSQL now has 139 runtime cases in 45 files (110 KEEP, 15 DELETE, 13 MERGE, 1 MOVE). Two newly deleted cases duplicate settled replay or test helper quota reset. The Conversation race needs deterministic Adapter scheduling; four assertion transfers consolidate existing fixtures.
- Current cumulative source review: 652 cases in 175 files; DELETE 319, KEEP 306, MERGE 23, MOVE 4; 12956 unioned candidate source lines. Next: remaining inline/multi-operation and export Node PostgreSQL files, then other Web directories and cross-directory reconciliation. Self-check 0/30; no samples, temporary mutations or active processes.

- Export checkpoint: Node PostgreSQL now has 157 runtime cases in 50 files (128 KEEP, 15 DELETE, 13 MERGE, 1 MOVE). Separate embedded Worker dispatch arms and the physical recovery drill's fixture dependencies justify keeping all 18 export tests. See NODE_POSTGRESQL_CHECKPOINT.md and SUPPORT.md.
- Current cumulative source review: 670 cases in 180 files; DELETE 319, KEEP 324, MERGE 23, MOVE 4; 12956 unioned candidate source lines. Only Inline Proposal and multi-operation selection test files remain in Node PostgreSQL. Other Web directories, support and cross-directory reconciliation remain pending. Self-check 0/30; no samples, temporary mutations or active processes.

- Multi-operation checkpoint: Node PostgreSQL now has 197 runtime cases in 51 files (160 KEEP, 21 DELETE, 15 MERGE, 1 MOVE). All 40 cases in the multi-operation file have individual decisions. Six duplicate parameter inputs add zero conservative line savings because their source lines are shared; two assertion transfers remain conditional.
- Current cumulative source review: 710 cases in 181 files; DELETE 325, KEEP 356, MERGE 25, MOVE 4; 12956 unioned candidate source lines. Next: edit-inline-proposal-http.integration.test.ts, then remaining Web directories/support and cross-directory reconciliation. Self-check 0/30; no samples, temporary mutations or active processes.

- Node PostgreSQL directory source review complete: 215 runtime cases in all 52 test files; 178 KEEP, 21 DELETE, 15 MERGE, one MOVE. Its one support file also has a disposition. Inline Proposal adds distinct retry/expansion/Undo and lifecycle fault cases plus physical-recovery fixture dependencies. Every test declaration line is covered and parameter multiplicities are recorded.
- Current cumulative source review: 728 cases in 182 files; DELETE 325, KEEP 374, MERGE 25, MOVE 4; 12956 unioned candidate source lines. Next: browser-source, browser-exact-dist, node-process-cut and remaining support, then cross-directory reconciliation and random self-check. Self-check 0/30; no selected samples, temporary mutations or active processes.

- Browser source checkpoint: 19 runtime cases in 15 files; 16 KEEP, one DELETE, one MERGE, one MOVE. Historical reply cases own separate retry/UI paths. Generic foundation smoke repeats real input commands and has an unused cookie-set helper branch; preserve that D4 limitation. Acceptance pending-projection assertions transfer to public delivery; pure parser negatives move to Node.
- Current cumulative source review: 747 cases in 197 files; DELETE 326, KEEP 390, MERGE 26, MOVE 5; 12996 unioned candidate source lines. Continue browser-source, then exact-dist/process-cut/support and cross-directory reconciliation. Self-check 0/30; no samples, temporary mutations or active processes.

- Browser recovery checkpoint: 56 cases in 22 browser-source files; 47 KEEP, five DELETE, three MERGE, one MOVE. Added Challenge wait/lifetime, SSE ingestion, 16 Takeover inputs and Draft Undo/Discard decisions. Two Takeover rejection inputs and the foreign-locator test have documented masking conditions; the latter requires a stronger oracle before its merge.
- Current cumulative source review: 784 cases in 204 files; DELETE 330, KEEP 421, MERGE 28, MOVE 5; 13144 unioned candidate source lines. Continue remaining browser-source Journal/edit/recovery files, then exact-dist/process-cut/support and cross-directory reconciliation. Self-check 0/30; no selected samples, temporary source changes or active processes.

- Browser Journal/input checkpoint: 77 cases in 30 browser-source files; 65 KEEP, seven DELETE, four MERGE, one MOVE. Eight files add append drift, projection lifetimes, active/fenced collection, working-set retirement, local recovery and Tiptap decisions. Two support files have dispositions. Takeover late-result assertions transfer before collection; two simple adapter behaviors have retained mounted/packaged owners.
- Current cumulative source review: 805 cases in 212 files; DELETE 332, KEEP 439, MERGE 29, MOVE 5; 13188 unioned candidate source lines. Continue the remaining acknowledgement-loss, editor-session, manual-input, reload-recovery and scenario files, then exact-dist/process-cut/support and cross-directory reconciliation. Self-check 0/30; no selected samples, temporary source changes or active processes.

- Browser source directory complete: 89 runtime cases in all 34 test files and dispositions for all three support files. 71 KEEP, eight DELETE, nine MERGE, one MOVE; 752 direct candidate lines. The unused textarea input test adds 520 lines; five real shared-submission scenarios must transfer into acknowledgement-loss before removing their old fixtures. Existing Takeover cases retain storage evidence, not a claim about current Tiptap interaction.
- Current cumulative source review: 817 cases in 216 test files; DELETE 333, KEEP 445, MERGE 34, MOVE 5; 13708 unioned candidate source lines. Continue browser-exact-dist, node-process-cut and remaining support, then covering-test/D5 reconciliation and random self-check. Self-check 0/30; no samples, temporary source changes or active processes.

- Node process cut directory complete: five tests in three files, three KEEP and two MERGE. Move unclaimed export restart assertions into each family's claim/reclaim case before removal. Controlled process stops, claim-only exit, client timeout and weak Receipt-count regex limits are explicit in NODE_PROCESS_CUT_CHECKPOINT.md.
- Current cumulative source review: 822 cases in 219 test files; DELETE 333, KEEP 448, MERGE 36, MOVE 5; 13708 unioned candidate lines. Next: browser-exact-dist and remaining support, then cross-directory coverage/D5 reconciliation and random self-check. Self-check 0/30; no samples, temporary source changes or active processes.

- Browser exact-dist checkpoint: 10 cases in 10 files, four KEEP, one DELETE, five MERGE. Root/empty/library checks transfer into real Chapter/Project/Volume journeys; the standalone Vite/browser-native foundation smoke is DELETE. The support oracle has remaining fixture dependencies to review; two support files received dispositions.
- Current cumulative source review: 832 cases in 229 test files; DELETE 334, KEEP 452, MERGE 41, MOVE 5; 13775 unioned candidate lines. Continue from BROWSER_EXACT_DIST_CHECKPOINT.md, then remaining support and cross-directory reconciliation before selecting random mutation cases. Self-check 0/30; no samples, temporary source changes or active processes.

- Packaged Chapter checkpoint: 16 cases in 16 exact-dist files, eight KEEP, one DELETE, seven MERGE. Navigation/current-Chapter transfers target save-truth; Chapter rename/reorder and delete/Volume refusal UI transitions remain distinct. Save truth proves its first sampled state, not all intermediate frames.
- Current cumulative source review: 838 cases in 235 test files; DELETE 334, KEEP 456, MERGE 43, MOVE 5; 13775 unioned candidate lines. Remaining exact-dist/support and cross-directory reconciliation precede random self-check. Self-check 0/30; no selected samples, source mutations or active processes.

- Packaged input checkpoint: 22 cases in 22 exact-dist files, 11 KEEP, one DELETE, 10 MERGE. Hydration, permissive interruption and single-Undo fixtures have concrete transfers; mounted 2401-input collection, repeated Undo and sustained IME/boundary requests remain distinct. Timing and interruption limits are explicit.
- Current cumulative source review: 844 cases in 241 test files; DELETE 334, KEEP 459, MERGE 46, MOVE 5; 13775 unioned candidate lines. Continue remaining exact-dist/support files, reconcile all cross-directory verdicts and only then freeze/sample DELETE rows. Self-check 0/30; no samples, temporary source changes or active processes.

- Packaged panels checkpoint: 28 cases in 28 exact-dist files, 16 KEEP, one DELETE, 11 MERGE. Derived panel refresh, second export identity, unavailable-assistant collapse and real restored-context writing stay. Stage 2 aggregate transfers remaining assertions to focused cases while preserving release-gate evidence. The optional exact database teardown is now fully reviewed with explicit fixture dependencies.
- Current cumulative source review: 850 cases in 247 test files; DELETE 334, KEEP 464, MERGE 47, MOVE 5; 13775 unioned candidate lines. Continue nine pending exact-dist test files plus their support, then remaining support and cross-directory reconciliation before random sampling. Self-check 0/30; no samples, temporary source changes or active processes.

- Packaged Block/Activity checkpoint: 32 cases in 31 exact-dist files, 20 KEEP, one DELETE, 11 MERGE. Added real Block move/retype, nonempty join after reload, repeated paste identity and real Activity cursor coverage. Misleading copy-refusal/native-drop descriptions are corrected; fixed-key Activity oracle dependencies remain explicit.
- Current cumulative source review: 854 cases in 250 test files; DELETE 334, KEEP 468, MERGE 47, MOVE 5; 13775 unioned candidate lines. Continue the six pending exact-dist test files and Stage 1/support helpers, then remaining support, reconciliation and random self-check. Self-check 0/30; no selected samples, temporary source changes or active processes.

- Stage 1 checkpoint: 33 cases in 32 exact-dist files; 21 KEEP, one DELETE, 11 MERGE. Its full expected/normalization helper and real command routing are reviewed. All remaining wrapper files were read; production-host-command.ts is complete, but the other underlying scenario helpers still need source review. SUPPORT.md records four more dependencies and digest/IME proof limits.
- Current cumulative source review: 855 cases in 251 test files; DELETE 334, KEEP 469, MERGE 47, MOVE 5; 13775 unioned candidate lines. Resume with the five production scenario wrapper files and their helpers as listed in BROWSER_EXACT_DIST_CHECKPOINT.md. Then finish remaining support, reconcile coverage and select the random sample. Self-check 0/30; no samples, temporary mutations or active processes.

- Production evidence checkpoint: exact-dist now 35 cases in 34 files; 21 KEEP, three DELETE, 11 MERGE. Captured Memory and Run evidence are direct API assertions behind browser titles, with retained HTTP owners. Their exclusive helpers add 434 candidate lines in support-removals.json; remove dispatch routes and reconcile exact teardown counts during future implementation. acceptance/composer helpers are also fully reviewed.
- Current cumulative source review: 857 cases in 253 test files; DELETE 336, KEEP 469, MERGE 47, MOVE 5. Candidate total is 14225 unioned lines: 13791 from test-file spans plus 434 exclusive support lines. Future summaries must include support-removals.json. Remaining exact-dist wrappers: production-host (six scenarios), Inline and restored Discard; then remaining support/reconciliation and random self-check. Self-check 0/30; no samples, temporary source changes or active processes.

- Browser exact-dist source review complete: 43 cases in all 37 test files; 29 KEEP, three DELETE, 11 MERGE. Every production scenario helper is fully read. All Web support files have dispositions; the unused statistics helper adds 33 exclusive support lines.
- Current cumulative source review: 865 cases in 256 test files; DELETE 336, KEEP 477, MERGE 47, MOVE 5. Candidate total: 14258 unioned lines (13791 test spans plus 467 exclusive support). All source directories are adjudicated; reconcile inventory, coverage chains and D5 assumptions before freezing the deletion population and random sample. Self-check 0/30; no samples, temporary mutations or active processes.

- Reconciliation checkpoint: AD032 retained for the existing full-u64 persisted representation contract. Deleted coverage chains now point to retained owners; conditional MERGE/MOVE dependencies are explicit in RECONCILIATION.md and JSON. Totals: 865 cases / 256 files; DELETE 335, KEEP 478, MERGE 47, MOVE 5; 14181 unioned candidate lines.
- Frozen all 335 DELETE rows in mutation-population.json and selected 30 without replacement in mutation-sample.json (seed 4205576894231042062; population SHA256 0b4bccaddfe47e67e146c99f776e4fdac66a713f072a42985a4b18324398498b). Do not resample or replace a failure. Next: inspect each selected defect and run clean/mutant/restored targeted tests, recording every result and related verdict correction. Self-check 0/30; no temporary mutations or active processes.

- Mutation checkpoint: 2/30 complete, both KILL (samples 15 and 1); each has clean PASS, intended assertion failure, exact source/package restoration and restored PASS. Sample 1 attempt 1 was a harness manifest-prefix startup failure, retained as BLOCKED and excluded; the same sample passed after runner correction. Eleven further manual plans are saved. No product/test diff remains. Run mutations/run.py with one saved plan at a time; it restores source/package in finally. Candidate-kill statuses require manual log review.

- Mutation checkpoint: 5/30 complete (2 KILL, 3 MISS). The D5 class now explicitly disclaims equivalent isolated-guard coverage. All source/package files are restored; no active processes. Prepared all 30 exact-source mutation plans, including one real-browser runner. Fixed the report renderer's table separator width and regenerated all tables. Resume remaining samples, beginning with 4-14 (skip completed cases).

- Mutation checkpoint: 16/30 complete (10 KILL, 5 MISS, 1 timeout-only BLOCKED). CO124 is now D5; CT010-CT012 and AD046 are now MERGE. Counts: 865 cases / 256 files; DELETE 331, KEEP 478, MERGE 51, MOVE 5; 13825 unioned candidate lines. Frozen population/sample are unchanged. All temporary source/package changes restored and no active process at checkpoint. Next: supplemental unit probes for 4/11, then samples 16-21 and 23-30. Remaining saved Core diagnostic filters were corrected.

- Supplemental checks complete: the exact original unit tests for samples 4 and 11 each pass clean, fail the same mutant, then pass restored. Sample 11 remains a coverage MISS; these probes do not replace its HTTP result. All three release binaries match their original SHA256 after restoration. Before sample 29, relaxed both coupled count ceilings in its manual plan to avoid a unit-only mutation being masked by the unchanged primitive ceiling. No sample was changed or replaced. Next remaining batch: 16-21,23-30.
