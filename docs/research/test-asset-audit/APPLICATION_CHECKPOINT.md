# Application directory checkpoint

Source verdicts cover all 58 test functions in 20 files at the fixed baseline. The directory has 54 DELETE and 4 KEEP decisions. Cross-directory reconciliation and mutation review remain pending.

Immediate DELETE spans: 2186 source lines. Product and test files remain unchanged.

## Distinct retained cases

- Statistics watermark above the available Snapshot. Search uses a separate evaluator.
- Multiple matches with astral text before and inside the query.
- Actual UTF-16 scan-work growth on a large dense-match input.
- Readable export joins a live tree Chapter to missing payload facts. Core renderer tests start after this join.

## Internal fixture decisions

Current Server handlers build both client and in-memory challenge fields from the same request/session. Altering only one field in a direct Application fixture is distinct from replacing a real request against a retained challenge. The former uses D5; the latter remains public coverage. Likewise, current Postgres Readers clone request Scope into returned facts. Do not claim a public foreign-scope refusal kills a mutation confined to the redundant inconsistent-facts check.

## Module links and support

| File | Test asset decision |
|---|---|
| crates/storyos-application/src/archive_project.rs | No local test function. Reviewed targets: archive_project_tests.rs. Keep product source; remove a test-module link only after its target is removed in a later implementation. 0 counted lines. |
| crates/storyos-application/src/author_edit.rs | No local test function. Reviewed targets: author_edit_tests.rs. Keep product source; remove a test-module link only after its target is removed in a later implementation. 0 counted lines. |
| crates/storyos-application/src/author_edit_outcome.rs | No local test function. Reviewed targets: author_edit_outcome_tests.rs. Keep product source; remove a test-module link only after its target is removed in a later implementation. 0 counted lines. |
| crates/storyos-application/src/chapter_query.rs | No local test function. Reviewed targets: chapter_query_tests.rs. Keep product source; remove a test-module link only after its target is removed in a later implementation. 0 counted lines. |
| crates/storyos-application/src/create_agent_run.rs | No local test function. Reviewed targets: create_agent_run_tests.rs. Keep product source; remove a test-module link only after its target is removed in a later implementation. 0 counted lines. |
| crates/storyos-application/src/create_project.rs | No local test function. Reviewed targets: create_project_tests.rs. Keep product source; remove a test-module link only after its target is removed in a later implementation. 0 counted lines. |
| crates/storyos-application/src/editor_session.rs | No local test function. Reviewed targets: editor_session_tests.rs. Keep product source; remove a test-module link only after its target is removed in a later implementation. 0 counted lines. |
| crates/storyos-application/src/lib.rs | No local test function. Reviewed targets: author_command_outcome_unknown_tests.rs, project_read_tests.rs. Keep product source; remove a test-module link only after its target is removed in a later implementation. 0 counted lines. |
| crates/storyos-application/src/manuscript_search.rs | No local test function. Reviewed targets: manuscript_search_tests.rs. Keep product source; remove a test-module link only after its target is removed in a later implementation. 0 counted lines. |
| crates/storyos-application/src/manuscript_statistics.rs | No local test function. Reviewed targets: manuscript_statistics_tests.rs. Keep product source; remove a test-module link only after its target is removed in a later implementation. 0 counted lines. |
| crates/storyos-application/src/manuscript_tree.rs | No local test function. Reviewed targets: manuscript_tree_tests.rs. Keep product source; remove a test-module link only after its target is removed in a later implementation. 0 counted lines. |
| crates/storyos-application/src/project_activity.rs | No local test function. Reviewed targets: project_activity_tests.rs. Keep product source; remove a test-module link only after its target is removed in a later implementation. 0 counted lines. |
| crates/storyos-application/src/project_export.rs | No local test function. Reviewed targets: project_export_tests.rs. Keep product source; remove a test-module link only after its target is removed in a later implementation. 0 counted lines. |
| crates/storyos-application/src/readable_export.rs | No local test function. Reviewed targets: readable_export_tests.rs. Keep product source; remove a test-module link only after its target is removed in a later implementation. 0 counted lines. |
| crates/storyos-application/src/set_current_chapter.rs | No local test function. Reviewed targets: set_current_chapter_tests.rs. Keep product source; remove a test-module link only after its target is removed in a later implementation. 0 counted lines. |
| crates/storyos-application/src/takeover.rs | No local test function. Reviewed targets: takeover_tests.rs. Keep product source; remove a test-module link only after its target is removed in a later implementation. 0 counted lines. |
| crates/storyos-application/src/undo_latest_author_action.rs | No local test function. Reviewed targets: undo_latest_author_action_tests.rs. Keep product source; remove a test-module link only after its target is removed in a later implementation. 0 counted lines. |
| crates/storyos-application/src/update_project.rs | No local test function. Reviewed targets: update_project_tests.rs. Keep product source; remove a test-module link only after its target is removed in a later implementation. 0 counted lines. |
| crates/storyos-application/src/update_project_assistance.rs | No local test function. Reviewed targets: update_project_assistance_tests.rs. Keep product source; remove a test-module link only after its target is removed in a later implementation. 0 counted lines. |

The utf16_decode_count module in manuscript_search.rs is instrumentation used by the retained work-count test. It is not an extra test case.
