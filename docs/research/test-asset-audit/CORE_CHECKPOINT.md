# Core directory checkpoint

Source verdicts cover all 202 test functions in 38 files at the fixed baseline. Mutation review and comparison against all remaining directories are pending. No product or test files changed.

| Verdict | Test functions |
|---|---|
| DELETE | 140 |
| KEEP | 61 |
| MERGE | 1 |

Immediate DELETE source spans: 2401 lines. MERGE, MOVE, shared helper cleanup, module-link cleanup and removable assertions inside KEEP tests are not included.

## Module links and support

These inventory entries contain no test function. Preserve their product code. Remove a test-module link only when the corresponding test file is actually removed in a later implementation; this audit does not make that edit.

| File | Test asset decision |
|---|---|
| crates/storyos-core/src/accept_proposal.rs | No local test function. Reviewed targets: accept_proposal_tests.rs. Product source retained; 0 removal lines. |
| crates/storyos-core/src/append_proposal_generation_batch.rs | No local test function. Reviewed targets: append_proposal_generation_batch_tests.rs. Product source retained; 0 removal lines. |
| crates/storyos-core/src/archive_path.rs | No local test function. Reviewed targets: archive_path_tests.rs. Product source retained; 0 removal lines. |
| crates/storyos-core/src/assemble_context.rs | No local test function. Reviewed targets: assemble_context_tests.rs. Product source retained; 0 removal lines. |
| crates/storyos-core/src/compact_active_context.rs | No local test function. Reviewed targets: compact_active_context_tests.rs. Product source retained; 0 removal lines. |
| crates/storyos-core/src/continuation_input.rs | No local test function. Reviewed targets: continuation_input_tests.rs. Product source retained; 0 removal lines. |
| crates/storyos-core/src/create_agent_run.rs | No local test function. Reviewed targets: create_agent_run_tests.rs. Product source retained; 0 removal lines. |
| crates/storyos-core/src/delete_chapter.rs | No local test function. Reviewed targets: delete_chapter_tests.rs. Product source retained; 0 removal lines. |
| crates/storyos-core/src/delete_volume.rs | No local test function. Reviewed targets: delete_volume_tests.rs. Product source retained; 0 removal lines. |
| crates/storyos-core/src/lib.rs | No local test function. Reviewed targets: apply_author_edit_tests.rs, create_project_tests.rs, update_project_tests.rs, archive_project_tests.rs, create_volume_tests.rs, create_chapter_tests.rs, manuscript_payload_tests.rs, update_volume_tests.rs, set_current_chapter_tests.rs, undo_latest_author_action_tests.rs. Product source retained; 0 removal lines. |
| crates/storyos-core/src/open_block_proposal.rs | No local test function. Reviewed targets: open_block_proposal_tests.rs. Product source retained; 0 removal lines. |
| crates/storyos-core/src/open_inline_proposal.rs | No local test function. Reviewed targets: open_inline_proposal_tests.rs. Product source retained; 0 removal lines. |
| crates/storyos-core/src/pause_proposal_generation.rs | No local test function. Reviewed targets: pause_proposal_generation_tests.rs. Product source retained; 0 removal lines. |
| crates/storyos-core/src/project_archive.rs | No local test function. Reviewed targets: project_archive_tests.rs. Product source retained; 0 removal lines. |
| crates/storyos-core/src/project_export.rs | No local test function. Reviewed targets: project_export_tests.rs. Product source retained; 0 removal lines. |
| crates/storyos-core/src/readable_export.rs | No local test function. Reviewed targets: readable_export_tests.rs. Product source retained; 0 removal lines. |
| crates/storyos-core/src/readable_export_command.rs | No local test function. Reviewed targets: readable_export_command_tests.rs. Product source retained; 0 removal lines. |
| crates/storyos-core/src/rebuild_expired_reference.rs | No local test function. Reviewed targets: rebuild_expired_reference_tests.rs. Product source retained; 0 removal lines. |
| crates/storyos-core/src/reject_proposal_operations.rs | No local test function. Reviewed targets: reject_proposal_operations_tests.rs. Product source retained; 0 removal lines. |
| crates/storyos-core/src/reopen_rejected_operations.rs | No local test function. Reviewed targets: reopen_rejected_operations_tests.rs. Product source retained; 0 removal lines. |
| crates/storyos-core/src/reopen_withdrawn_proposal.rs | No local test function. Reviewed targets: reopen_withdrawn_proposal_tests.rs. Product source retained; 0 removal lines. |
| crates/storyos-core/src/replan_proposal.rs | No local test function. Reviewed targets: replan_proposal_tests.rs. Product source retained; 0 removal lines. |
| crates/storyos-core/src/retrieve_original_result.rs | No local test function. Reviewed targets: retrieve_original_result_tests.rs. Product source retained; 0 removal lines. |
| crates/storyos-core/src/revision_comparison.rs | No local test function. Reviewed targets: revision_comparison_tests.rs. Product source retained; 0 removal lines. |
| crates/storyos-core/src/statistics_profile.rs | No local test function. Reviewed targets: statistics_profile_tests.rs. Product source retained; 0 removal lines. |
| crates/storyos-core/src/unknown_create_successor.rs | No local test function. Reviewed targets: unknown_create_successor_tests.rs. Product source retained; 0 removal lines. |
| crates/storyos-core/src/update_chapter.rs | No local test function. Reviewed targets: update_chapter_tests.rs. Product source retained; 0 removal lines. |
| crates/storyos-core/src/update_project_assistance.rs | No local test function. Reviewed targets: update_project_assistance_tests.rs. Product source retained; 0 removal lines. |
| crates/storyos-core/src/withdraw_proposal.rs | No local test function. Reviewed targets: withdraw_proposal_tests.rs. Product source retained; 0 removal lines. |

## Remaining comparison work

- Reconcile each named public owner when its directory is reviewed. A citation is not a KEEP verdict for the other file.
- Resolve any deletion chain to a retained final owner before freezing the mutation population.
- Review removable fixtures and imports only after final test verdicts; current mixed-file savings intentionally exclude them.
- Preserve failed, missed or blocked mutation samples. Source equivalence is not a runtime kill.
