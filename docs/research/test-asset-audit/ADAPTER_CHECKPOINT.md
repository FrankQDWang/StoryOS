# Adapter source directory checkpoint

All 87 test functions in 36 source test files have manual verdicts. The inventory and verdict (path, name) sets match exactly. The two integration files add 12 tests. No product or test changes were made.

Source verdicts: {'DELETE': 28, 'KEEP': 55, 'MERGE': 4}. Immediate candidate source lines: 4483. Mutation review and cross-directory reconciliation remain pending.

| Test file | Cases | Verdict counts |
|---|---|---|
| crates/storyos-adapter-postgres/src/archive_project_tests.rs | 1 | {'MERGE': 1} |
| crates/storyos-adapter-postgres/src/author_command_outcome_unknown_tests.rs | 1 | {'KEEP': 1} |
| crates/storyos-adapter-postgres/src/author_edit_counter_tests.rs | 1 | {'KEEP': 1} |
| crates/storyos-adapter-postgres/src/author_edit_outcome_tests.rs | 4 | {'DELETE': 2, 'KEEP': 2} |
| crates/storyos-adapter-postgres/src/author_edit_tests.rs | 3 | {'KEEP': 2, 'DELETE': 1} |
| crates/storyos-adapter-postgres/src/connection_pool_tests.rs | 1 | {'KEEP': 1} |
| crates/storyos-adapter-postgres/src/create_agent_run_tests.rs | 1 | {'KEEP': 1} |
| crates/storyos-adapter-postgres/src/create_chapter_authority_tests.rs | 2 | {'KEEP': 2} |
| crates/storyos-adapter-postgres/src/create_chapter_tests.rs | 2 | {'DELETE': 2} |
| crates/storyos-adapter-postgres/src/create_project_challenge_tests.rs | 1 | {'KEEP': 1} |
| crates/storyos-adapter-postgres/src/create_project_tests.rs | 1 | {'DELETE': 1} |
| crates/storyos-adapter-postgres/src/create_volume_authority_tests.rs | 2 | {'DELETE': 1, 'KEEP': 1} |
| crates/storyos-adapter-postgres/src/create_volume_tests.rs | 3 | {'DELETE': 3} |
| crates/storyos-adapter-postgres/src/delete_chapter_tests.rs | 2 | {'DELETE': 1, 'KEEP': 1} |
| crates/storyos-adapter-postgres/src/delete_volume_tests.rs | 2 | {'DELETE': 1, 'MERGE': 1} |
| crates/storyos-adapter-postgres/src/export_work_tests.rs | 5 | {'KEEP': 4, 'DELETE': 1} |
| crates/storyos-adapter-postgres/src/manuscript_block_tests.rs | 8 | {'DELETE': 6, 'KEEP': 2} |
| crates/storyos-adapter-postgres/src/manuscript_search_tests.rs | 2 | {'KEEP': 2} |
| crates/storyos-adapter-postgres/src/manuscript_tree_tests.rs | 1 | {'KEEP': 1} |
| crates/storyos-adapter-postgres/src/model_gateway_tests.rs | 6 | {'KEEP': 6} |
| crates/storyos-adapter-postgres/src/project_archive_build_tests.rs | 1 | {'KEEP': 1} |
| crates/storyos-adapter-postgres/src/project_command_challenge_tests.rs | 4 | {'KEEP': 4} |
| crates/storyos-adapter-postgres/src/recovery_visibility_proof_tests.rs | 2 | {'KEEP': 2} |
| crates/storyos-adapter-postgres/src/set_current_chapter_authority_tests.rs | 2 | {'KEEP': 2} |
| crates/storyos-adapter-postgres/src/set_current_chapter_tests.rs | 1 | {'DELETE': 1} |
| crates/storyos-adapter-postgres/src/structural_authority_schema_tests.rs | 7 | {'KEEP': 3, 'DELETE': 4} |
| crates/storyos-adapter-postgres/src/structure_command_tests.rs | 4 | {'KEEP': 3, 'DELETE': 1} |
| crates/storyos-adapter-postgres/src/takeover_admission_tests.rs | 2 | {'KEEP': 2} |
| crates/storyos-adapter-postgres/src/takeover_persistence_tests.rs | 2 | {'DELETE': 1, 'KEEP': 1} |
| crates/storyos-adapter-postgres/src/update_chapter_rank_batch_tests.rs | 3 | {'KEEP': 3} |
| crates/storyos-adapter-postgres/src/update_chapter_tests.rs | 3 | {'MERGE': 1, 'KEEP': 2} |
| crates/storyos-adapter-postgres/src/update_project_assistance_tests.rs | 1 | {'KEEP': 1} |
| crates/storyos-adapter-postgres/src/update_project_tests.rs | 1 | {'MERGE': 1} |
| crates/storyos-adapter-postgres/src/update_volume_rank_batch_tests.rs | 1 | {'KEEP': 1} |
| crates/storyos-adapter-postgres/src/update_volume_storage_order_tests.rs | 1 | {'KEEP': 1} |
| crates/storyos-adapter-postgres/src/update_volume_tests.rs | 3 | {'DELETE': 2, 'KEEP': 1} |

## Module links and support

These files have no test function of their own. Keep product code. Remove a test module declaration only after every test in its linked file is removed and no retained helper imports it. No module-link lines are counted as savings.

| File | Disposition |
|---|---|
| crates/storyos-adapter-postgres/src/agent_run_dispatch.rs | Keep product implementation and links to retained tests; no standalone test verdict. |
| crates/storyos-adapter-postgres/src/author_edit.rs | Keep product implementation and links to retained tests; no standalone test verdict. |
| crates/storyos-adapter-postgres/src/connection_pool.rs | Keep product implementation and links to retained tests; no standalone test verdict. |
| crates/storyos-adapter-postgres/src/export_work.rs | Keep product implementation and links to retained tests; no standalone test verdict. |
| crates/storyos-adapter-postgres/src/lib.rs | Keep product module. After validated removal, remove only the links for create_project_tests, create_volume_tests, create_chapter_tests and set_current_chapter_tests. Other links still have retained tests or helpers. |
| crates/storyos-adapter-postgres/src/manuscript_block.rs | Keep product implementation and links to retained tests; no standalone test verdict. |
| crates/storyos-adapter-postgres/src/manuscript_search.rs | Keep product implementation and links to retained tests; no standalone test verdict. |
| crates/storyos-adapter-postgres/src/manuscript_tree.rs | Keep product implementation and links to retained tests; no standalone test verdict. |
| crates/storyos-adapter-postgres/src/project_archive_build.rs | Keep product implementation and links to retained tests; no standalone test verdict. |
| crates/storyos-adapter-postgres/src/structure_command.rs | Keep product implementation and links to retained tests; no standalone test verdict. |
| crates/storyos-adapter-postgres/tests/fixture.sql | KEEP shared fixture: two Users, two Projects, a non-current Chapter, authoritative Heads/Blocks and replay Snapshots are used by retained RLS and recovery tests. This is setup data, not another assertion. |

## Execution order for the slimming list

1. Keep the real database constraint, race, recovery, Gateway and measured SQL-work owners named in adapter.md.
2. Add the four MERGE scenarios to their named existing owners before removing their source scenarios. Keep delete_volume_tests::apply_delete; rank and Undo tests still import it.
3. Remove duplicate test functions only after their selected self-check evidence and final cross-directory review support removal. Keep shared fixtures in structure_command_tests, author_edit_tests, update_volume_tests, update_chapter_tests and set_current_chapter_authority_tests.
4. Preserve historical failed baseline reports. No mutation sample was selected during this directory review.
