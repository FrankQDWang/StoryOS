# Contracts directory checkpoint

All 62 test functions in 14 files have manual verdicts. The inventory and verdict (path, name) sets match exactly. This completes source verdicts for all 482 Rust tests in the requested 122 test files. Cross-directory reconciliation and mutation evidence are still pending.

Contracts verdicts: {'KEEP': 16, 'DELETE': 46}. Immediate candidate source lines: 1521. No product, generated artifact or existing test changed.

| Test file | Cases | Verdict counts |
|---|---|---|
| crates/storyos-contracts/src/release1_agent_run_artifacts_tests.rs | 1 | {'KEEP': 1} |
| crates/storyos-contracts/src/release1_artifacts_tests.rs | 11 | {'DELETE': 6, 'KEEP': 5} |
| crates/storyos-contracts/src/release1_create_project_artifacts_tests.rs | 1 | {'KEEP': 1} |
| crates/storyos-contracts/src/release1_fixture_corpus_tests.rs | 1 | {'KEEP': 1} |
| crates/storyos-contracts/src/release1_operation_registry_tests.rs | 1 | {'DELETE': 1} |
| crates/storyos-contracts/src/release1_refused_edit_draft_artifacts_tests.rs | 1 | {'KEEP': 1} |
| crates/storyos-contracts/src/release1_tests.rs | 3 | {'KEEP': 1, 'DELETE': 2} |
| crates/storyos-contracts/src/stage1_bundle_tests.rs | 5 | {'DELETE': 5} |
| crates/storyos-contracts/src/stage1_crosswalk_tests.rs | 11 | {'KEEP': 1, 'DELETE': 10} |
| crates/storyos-contracts/src/stage1_delivery_tests.rs | 7 | {'DELETE': 7} |
| crates/storyos-contracts/src/stage1_handoff_tests.rs | 4 | {'DELETE': 4} |
| crates/storyos-contracts/src/stage1_provenance_tests.rs | 6 | {'DELETE': 5, 'KEEP': 1} |
| crates/storyos-contracts/src/stage2_crosswalk_tests.rs | 6 | {'DELETE': 4, 'KEEP': 2} |
| crates/storyos-contracts/src/stage3_crosswalk_tests.rs | 4 | {'DELETE': 2, 'KEEP': 2} |

## Module links and shared helper disposition

| Source file | Disposition |
|---|---|
| crates/storyos-contracts/src/release1.rs | KEEP product source and retained test links; no module-link savings counted. |
| crates/storyos-contracts/src/release1_artifacts.rs | KEEP product source and retained test links; no module-link savings counted. |
| crates/storyos-contracts/src/release1_operation_registry.rs | KEEP product source. Remove its test-module link only after the all-DELETE file is removed. |
| crates/storyos-contracts/src/stage1_bundle.rs | KEEP product source. Remove its test-module link only after the all-DELETE file is removed. |
| crates/storyos-contracts/src/stage1_crosswalk.rs | KEEP product source and retained test links; no module-link savings counted. |
| crates/storyos-contracts/src/stage1_delivery.rs | KEEP product source. Remove its test-module link only after the all-DELETE file is removed. |
| crates/storyos-contracts/src/stage1_handoff.rs | KEEP product source. Remove its test-module link only after the all-DELETE file is removed. |
| crates/storyos-contracts/src/stage1_provenance.rs | KEEP product source and retained test links; no module-link savings counted. |
| crates/storyos-contracts/src/stage2_crosswalk.rs | KEEP product source and retained test links; no module-link savings counted. |
| crates/storyos-contracts/src/stage3_crosswalk.rs | KEEP product source and retained test links; no module-link savings counted. |

## Decisions for execution

- Keep malformed generated-schema cases: HTTP command handlers do not consume those schema documents. Published response schema validation is its own consumer boundary.
- Keep actual OpenAPI reference resolution and per-operation success-schema placement. String searches for exports and copied enum lists add no equivalent consumer behavior.
- Keep independent fixture-digest reconstruction against emitted bytes. Its local appended-byte loop mutates the test reconstruction, not the product; it is not one of the required mutation checks.
- Historical Stage 1 bundle, handoff and delivery records are privately constructed from fixed constants. D5 rows distinguish impossible post-construction test inputs from real incoming data. Reconcile these judgments against accepted publication contracts before the DELETE population is frozen.
- Keep one complete generated crosswalk comparison per Stage. Stage 2/3 evidence-path existence tests cover current file deletion which historical hashes and nonempty constant paths do not detect.
- Keep missing-journey publication refusal. The source-word PostgreSQL marker does not prove runtime database execution; the actual S1-JRN-001 journey owns that evidence.
- Remove stage1_crosswalk_tests::baseline_reader_never_falls_through_to_the_worktree together with the proposed stage1_delivery_tests.rs removal. The full crosswalk comparison retains coverage of the shared historical reader; the smaller test reads that delivery test file only as an incidental changing probe.
- Three adjacent expected-JSON helpers in stage1_crosswalk_tests.rs have one caller each, all DELETE. Their lines are included with those verdicts. Other unused-import/helper cleanup is conservatively uncounted.
- No scope expansion into scripts tests. Existing command checks are referenced only when they clarify ownership; they are not substituted for a named retained-test mutation result.
